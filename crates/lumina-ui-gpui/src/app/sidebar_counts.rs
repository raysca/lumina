//! The counts and per-item maps the sidebar, the timeline rail and the
//! inspector read. Fetching them is a couple of dozen catalog reads (four of
//! them over every item), so the fetch is a plain value built off the UI
//! thread when it can be, and applied on it in one step.

use gpui::*;
use lumina_core::{
    Album, DuplicateGroup, Flag, LuminaEngine, MediaId, SmartAlbum, SystemAlbum, TimelineBucket,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

#[cfg(test)]
thread_local! {
    /// Count fetches run on this test thread.
    pub(crate) static COUNT_FETCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One read of everything [`LuminaApp::apply_sidebar_counts`] sets.
pub(crate) struct SidebarCounts {
    total: usize,
    system: HashMap<SystemAlbum, usize>,
    smart_albums: Vec<SmartAlbum>,
    smart_counts: HashMap<i64, usize>,
    albums: Vec<(Album, usize)>,
    tag_counts: Vec<(String, usize)>,
    untagged: usize,
    duplicates: usize,
    ratings: HashMap<MediaId, u8>,
    color_labels: HashMap<MediaId, u8>,
    flags: HashMap<MediaId, Flag>,
    captions: HashMap<MediaId, String>,
    missing: HashSet<MediaId>,
    months: Vec<TimelineBucket>,
    years: Vec<TimelineBucket>,
    aliases: anyhow::Result<Vec<(String, String)>>,
    /// The cached duplicate groups, read only while Duplicates is shown.
    duplicate_groups: Option<Vec<DuplicateGroup>>,
}

impl SidebarCounts {
    /// Reads every count and map. Blocking: run it off the UI thread
    /// unless the caller must see the result at once.
    pub(crate) fn fetch(engine: &LuminaEngine, duplicates_shown: bool) -> Self {
        #[cfg(test)]
        COUNT_FETCHES.with(|n| n.set(n.get() + 1));
        let system = SystemAlbum::all()
            .iter()
            .map(|album| (*album, engine.query_system_album_count(*album)))
            .collect();
        let smart_albums = engine.list_smart_albums();
        let smart_counts = smart_albums
            .iter()
            .map(|album| (album.id, engine.query_smart_album_count(album)))
            .collect();
        Self {
            total: engine.total_count(),
            system,
            smart_albums,
            smart_counts,
            albums: engine.list_albums_with_counts(),
            tag_counts: engine.query_tag_counts(),
            untagged: engine.query_untagged_count(),
            duplicates: engine.query_duplicate_count(),
            ratings: engine.get_ratings_map(),
            color_labels: engine.get_color_labels_map(),
            flags: engine.get_flags_map(),
            captions: engine.get_captions_map(),
            missing: engine.get_missing_ids(),
            months: engine.get_timeline_buckets("month"),
            years: engine.get_timeline_buckets("year"),
            aliases: engine.aliases(),
            // Never the hashing scan here: that runs on its own thread.
            duplicate_groups: duplicates_shown.then(|| engine.get_cached_duplicate_groups()),
        }
    }
}

/// The background count refresh: at most one fetch runs, and requests made
/// while it runs fold into one more fetch after it.
#[derive(Default)]
pub(crate) struct CountRefresh {
    in_flight: bool,
    again: bool,
    /// The catalog revision when the running fetch started.
    started_at: u64,
}

impl LuminaApp {
    /// Refreshes every count and map now, on the calling thread. User
    /// actions use this so the sidebar matches the action at once.
    pub fn refresh_album_counts(&mut self) {
        if let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) {
            let counts = SidebarCounts::fetch(&engine, self.duplicates_shown());
            self.apply_sidebar_counts(counts);
        } else {
            self.after_count_refresh();
        }
    }

    /// Refreshes the counts off the UI thread, folding requests that arrive
    /// meanwhile into one more fetch. Engine events use this: a burst of
    /// them (an import, a big Empty Trash) must not run a fetch each on the
    /// UI thread. Tests fetch inline so they never depend on timing.
    pub(crate) fn refresh_album_counts_soon(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        {
            let _ = cx;
            self.refresh_album_counts();
        }
        #[cfg(not(test))]
        self.refresh_album_counts_in_background(cx);
    }

    /// The background half of [`Self::refresh_album_counts_soon`]: at most
    /// one fetch in flight, later requests folded into one more.
    pub(crate) fn refresh_album_counts_in_background(&mut self, cx: &mut Context<Self>) {
        if self.count_refresh.in_flight {
            self.count_refresh.again = true;
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            return;
        };
        self.start_count_fetch(engine, cx);
    }

    fn start_count_fetch(&mut self, engine: Arc<LuminaEngine>, cx: &mut Context<Self>) {
        self.count_refresh.in_flight = true;
        self.count_refresh.again = false;
        self.count_refresh.started_at = engine.data_revision();
        let library = self.library_generation;
        let duplicates_shown = self.duplicates_shown();
        let task = cx
            .background_executor()
            .spawn(async move { SidebarCounts::fetch(&engine, duplicates_shown) });
        cx.spawn(async move |this, cx| {
            let counts = task.await;
            let _ = this.update(cx, |app, cx| {
                app.count_refresh.in_flight = false;
                let Some(engine) = app.grid_state.as_ref().map(|s| s.engine.clone()) else {
                    app.count_refresh.again = false;
                    return;
                };
                // Another library's counts (a switch resets its own), or
                // read before a write this thread made meanwhile (a flag,
                // a caption): applying them would undo what the user sees.
                // Fetch again instead.
                let stale = app.library_generation != library
                    || engine.revision_written_here() > app.count_refresh.started_at;
                if !stale {
                    app.apply_sidebar_counts(counts);
                }
                // Requests made while this ran, including the new
                // library's, get one more fetch.
                if stale || std::mem::take(&mut app.count_refresh.again) {
                    app.start_count_fetch(engine, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Installs a fetch on the UI thread.
    pub(crate) fn apply_sidebar_counts(&mut self, counts: SidebarCounts) {
        let SidebarCounts {
            total,
            system,
            smart_albums,
            smart_counts,
            albums,
            tag_counts,
            untagged,
            duplicates,
            ratings,
            color_labels,
            flags,
            captions,
            missing,
            months,
            years,
            aliases,
            duplicate_groups,
        } = counts;
        self.total_library_count = total;
        self.system_album_counts = system;
        self.user_album_counts = smart_counts;
        self.user_smart_albums = smart_albums;
        self.albums.counts = albums.iter().map(|(album, n)| (album.id, *n)).collect();
        self.albums.list = albums.into_iter().map(|(album, _)| album).collect();
        self.library_tag_counts = tag_counts;
        self.untagged_count = untagged;
        self.duplicate_items_count = duplicates;
        // The cached groups now, the hashing scan on its own thread: never
        // hash files on the UI thread.
        if let Some(groups) = duplicate_groups.filter(|_| self.duplicates_shown()) {
            self.set_duplicate_groups(groups);
            self.start_duplicate_scan();
        }
        // Aliases change with tag renames, so they reload with the counts.
        match aliases {
            Ok(aliases) => self.keywords.aliases = aliases,
            Err(err) => log::warn!("could not load keyword aliases: {err:#}"),
        }
        self.item_ratings = ratings;
        self.item_color_labels = color_labels;
        self.item_flags = flags;
        self.item_captions = captions;
        self.missing_items = missing;
        self.sync_missing_card();
        self.month_header.counts = months
            .iter()
            .filter_map(|b| {
                let (year, month) = b.bucket_key.split_once('-')?;
                Some(((year.parse().ok()?, month.parse().ok()?), b.item_count))
            })
            .collect();
        self.rail.set_counts(&self.month_header.counts);
        self.levels.set_covers(&years, &months);
        self.timeline_year_buckets = years
            .iter()
            .filter_map(|b| Some((b.bucket_key.parse::<i32>().ok()?, b.item_count)))
            .collect();
        self.after_count_refresh();
    }

    fn duplicates_shown(&self) -> bool {
        self.selection == AlbumSelection::Duplicates
    }

    fn after_count_refresh(&mut self) {
        // Tag mutations, scan removals, and undo/redo all refresh counts here.
        // If the active tag view lost its last item, drop the stale
        // position-based selection instead of leaving it pointing nowhere.
        if matches!(
            self.selection,
            AlbumSelection::Tag(_) | AlbumSelection::Untagged
        ) && self.get_active_count() == 0
            && (self.selected_item.is_some() || !self.selected_indices.is_empty())
        {
            self.clear_selection_fields();
        }
    }
}
