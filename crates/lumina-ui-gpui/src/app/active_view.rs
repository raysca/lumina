//! Count and rows of the active view, fetched off the UI thread.
//!
//! Rendering reads only this cache, so a scan or import holding the catalog
//! lock never stalls a frame. A frame that finds the cache missing or behind
//! the catalog's revision asks for a fetch and keeps drawing what it has; the
//! fetch lands, notifies, and the next frame shows it.

use std::ops::Range;

use gpui::Context;
use std::collections::HashSet;

use lumina_core::{
    DateSpan, LuminaEngine, MediaId, MediaItem, SmartAlbum, SortSpec, SystemAlbum, ViewScope,
    ViewTotals,
};

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;
use crate::views::video_filters::is_videos_scope;

#[cfg(test)]
thread_local! {
    /// Rows asked of [`LuminaApp::navigation_rows`] on this test thread.
    pub(crate) static ROWS_READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Which items the grid shows, in which order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ViewKey {
    /// Which opened library: ids and revisions restart with each one.
    library: u64,
    query: String,
    year: Option<i32>,
    selection: AlbumSelection,
    sort: SortSpec,
}

/// A [`ViewKey`] plus what its queries need, so it can run on any thread.
#[derive(Clone)]
pub(crate) struct ViewQuery {
    key: ViewKey,
    album: Option<SmartAlbum>,
}

fn tag_query(tag: &str) -> String {
    if tag.contains(' ') {
        format!("tag:\"{tag}\"")
    } else {
        format!("tag:{tag}")
    }
}

impl ViewQuery {
    /// Number of items in the view.
    pub(crate) fn count(&self, engine: &LuminaEngine) -> usize {
        let key = &self.key;
        if !key.query.is_empty() {
            return engine.search_query_count(&key.query);
        }
        if let Some(year) = key.year {
            return engine.search_query_count(&format!("year:{year}"));
        }
        match &key.selection {
            AlbumSelection::AllLibrary => engine.total_count(),
            AlbumSelection::System(album) => engine.query_system_album_count(*album),
            AlbumSelection::User(_) => self
                .album
                .as_ref()
                .map_or(0, |album| engine.query_smart_album_count(album)),
            AlbumSelection::Album(id) => engine.query_album_count(*id),
            AlbumSelection::Trash => engine.query_system_album_count(SystemAlbum::RecentlyDeleted),
            AlbumSelection::Tag(tag) => engine.search_query_count(&tag_query(tag)),
            AlbumSelection::Untagged => engine.query_untagged_count(),
            AlbumSelection::Duplicates => engine.query_duplicate_count(),
            // Cards, keywords or downloads, not media: no grid rows to count.
            AlbumSelection::AlbumsOverview | AlbumSelection::Tags | AlbumSelection::Downloads => 0,
        }
    }

    /// The view as Delete Rejected… scopes it, the same precedence as
    /// [`Self::count`]; `None` for the Trash and Duplicates, which it does
    /// not cover.
    pub(crate) fn reject_scope(&self) -> Option<ViewScope> {
        let key = &self.key;
        if !key.query.is_empty() {
            return Some(ViewScope::Search(key.query.clone()));
        }
        if let Some(year) = key.year {
            return Some(ViewScope::Search(format!("year:{year}")));
        }
        Some(match &key.selection {
            AlbumSelection::AllLibrary => ViewScope::Library,
            AlbumSelection::System(album) => ViewScope::System(*album),
            AlbumSelection::User(_) => ViewScope::Smart(self.album.clone()?),
            AlbumSelection::Album(id) => ViewScope::Album(*id),
            AlbumSelection::Tag(tag) => ViewScope::Search(tag_query(tag)),
            AlbumSelection::Untagged => ViewScope::Untagged,
            AlbumSelection::Trash
            | AlbumSelection::Duplicates
            | AlbumSelection::AlbumsOverview
            | AlbumSelection::Tags
            | AlbumSelection::Downloads => return None,
        })
    }

    /// The capture months of the view's dated items, over the rows of
    /// [`Self::count`]; `None` for Duplicates, which is not one WHERE.
    /// ponytail: one engine query for All Media too (two index seeks), not
    /// the cached month buckets, so every view reads undated the same way.
    pub(crate) fn date_span(&self, engine: &LuminaEngine) -> Option<DateSpan> {
        engine.view_date_span(&self.layout_scope()?)
    }

    /// The view as one WHERE with the rows and order of [`Self::slice`]:
    /// [`Self::reject_scope`] plus the Trash. `None` for Duplicates (not one
    /// WHERE) and a smart album that is gone; the Mosaic draws the Grid body
    /// there, and the header shows no date span.
    pub(crate) fn layout_scope(&self) -> Option<ViewScope> {
        match self.reject_scope() {
            Some(scope) => Some(scope),
            None if self.key.selection == AlbumSelection::Trash => {
                Some(ViewScope::System(SystemAlbum::RecentlyDeleted))
            }
            None => None,
        }
    }

    /// Which view this is.
    pub(crate) fn key(&self) -> &ViewKey {
        &self.key
    }

    /// How many of a search's results are videos: one more count, run only
    /// for a query with free text (only then does the results head show)
    /// that does not name a kind (`is:video` would override its
    /// `is:photo`, and the kind is then known anyway). `is:video` joins the
    /// closed tokens, so an unclosed quote does not swallow it.
    pub(crate) fn video_count(&self, engine: &LuminaEngine) -> Option<usize> {
        let query = &self.key.query;
        if !crate::app::search::query_has_text(query)
            || lumina_core::StructuredQuery::parse(query)
                .media_type
                .is_some()
        {
            return None;
        }
        let mut tokens = crate::app::search::query_tokens_closed(query);
        tokens.push("is:video".to_string());
        Some(engine.search_query_count(&tokens.join(" ")))
    }

    /// Count, duration, size and month counts of the Videos view (with or
    /// without its 4K chip), for its header and month band; `None` for every
    /// other view, which never shows them. Read over the view's own WHERE
    /// (`layout_scope`), so they total exactly the rows shown.
    pub(crate) fn totals(&self, engine: &LuminaEngine) -> Option<ViewTotals> {
        let key = &self.key;
        if !is_videos_scope(&key.selection, &key.query, key.year) {
            return None;
        }
        engine.view_totals(&self.layout_scope()?)
    }

    /// The view's items at `range`, in view order.
    pub(crate) fn slice(&self, engine: &LuminaEngine, range: Range<usize>) -> Vec<MediaItem> {
        let (offset, limit) = (range.start, range.len());
        if limit == 0 {
            return Vec::new();
        }
        let key = &self.key;
        let sort = key.sort;
        if !key.query.is_empty() {
            return engine.search_query_sorted(&key.query, limit, offset, sort);
        }
        if let Some(year) = key.year {
            return engine.search_query_sorted(&format!("year:{year}"), limit, offset, sort);
        }
        match &key.selection {
            AlbumSelection::AllLibrary => engine.get_item_slice_sorted(offset, limit, sort),
            AlbumSelection::System(album) => {
                engine.query_system_album_sorted(*album, limit, offset, sort)
            }
            AlbumSelection::User(_) => self.album.as_ref().map_or_else(Vec::new, |album| {
                engine.query_smart_album_sorted(album, limit, offset, sort)
            }),
            AlbumSelection::Album(id) => engine.query_album_sorted(*id, limit, offset, sort),
            AlbumSelection::Trash => {
                engine.query_system_album_sorted(SystemAlbum::RecentlyDeleted, limit, offset, sort)
            }
            AlbumSelection::Tag(tag) => {
                engine.search_query_sorted(&tag_query(tag), limit, offset, sort)
            }
            AlbumSelection::Untagged => engine.query_untagged_sorted(limit, offset, sort),
            AlbumSelection::Duplicates => engine.query_duplicate_items_slice(offset, limit),
            AlbumSelection::AlbumsOverview | AlbumSelection::Tags | AlbumSelection::Downloads => {
                Vec::new()
            }
        }
    }
}

/// Same size and mtime: the bytes did not change (a rename or a move).
pub(crate) fn same_file_signature(a: &MediaItem, b: &MediaItem) -> bool {
    (a.file_size, a.mtime_epoch_ms) == (b.file_size, b.mtime_epoch_ms)
}

/// What a fetch read, and at which catalog revision.
struct Fetched {
    key: ViewKey,
    revision: u64,
    count: usize,
    /// Read with `count`, so it is stale exactly when the count is.
    span: Option<DateSpan>,
    /// Videos among a search's results, for the results head's title;
    /// None outside a search or when the query names a kind itself.
    videos: Option<usize>,
    /// Videos-view totals, read with `count`; None in other views.
    totals: Option<ViewTotals>,
    start: usize,
    items: Vec<MediaItem>,
}

/// The cached count and a window of rows around what the grid last showed.
#[derive(Default)]
pub(crate) struct ActiveView {
    data: Option<Fetched>,
    in_flight: bool,
    /// Item range the grid (or the viewer) last needed.
    wanted: Range<usize>,
    /// Tab focus on this view's empty-state buttons.
    pub(crate) empty_focus: Option<crate::app::render::empty_state::EmptyFocus>,
}

/// Items fetched either side of what is wanted, so scrolling and viewer
/// steps usually hit the cache.
const WINDOW_PAD: usize = 64;

impl ActiveView {
    /// Holds off refetches, as a slow fetch would: the cached rows go stale
    /// while the app keeps working.
    #[cfg(test)]
    pub(crate) fn hold_fetch(&mut self) {
        self.in_flight = true;
    }

    /// The cached rows for `key`, if they are for that view and current.
    fn fresh(&self, key: &ViewKey, revision: u64) -> Option<&Fetched> {
        self.data
            .as_ref()
            .filter(|data| &data.key == key && data.revision == revision)
    }

    /// Gives cached rows the file (path, size, mtime, dimensions) of the
    /// same items in a watcher batch, so a decode asked for before the
    /// refetch lands reads the file that is there now and keys the disk
    /// cache on it. Other columns wait for the refetch: the batch has the
    /// file's date, not an edited one. Returns the ids whose row only moved
    /// (new path, same size and mtime): a rename, which changes no pixels.
    /// ponytail: rows × batch scan; a watcher batch is a few files.
    pub(crate) fn refresh_row_files(&mut self, changed: &[MediaItem]) -> HashSet<MediaId> {
        let mut renamed = HashSet::new();
        let Some(data) = self.data.as_mut() else {
            return renamed;
        };
        for row in &mut data.items {
            if let Some(new) = changed.iter().find(|item| item.id == row.id) {
                if row.path != new.path && same_file_signature(row, new) {
                    renamed.insert(row.id);
                }
                row.path = new.path.clone();
                row.parent_dir = new.parent_dir.clone();
                row.file_name = new.file_name.clone();
                row.file_size = new.file_size;
                row.mtime_epoch_ms = new.mtime_epoch_ms;
                row.width = new.width;
                row.height = new.height;
            }
        }
        renamed
    }

    fn covers(data: &Fetched, range: &Range<usize>) -> bool {
        let end = (data.start + data.items.len()).max(data.start);
        // Past the end of the view counts as covered: there is nothing to fetch.
        let range_end = range.end.min(data.count);
        range.start >= data.start && range_end <= end
    }
}

impl LuminaApp {
    /// The active view's name: the page title and the viewer's Back label.
    pub(crate) fn view_label(&self) -> String {
        // The 4K chip's search still reads as Videos.
        if self.videos_4k_active() {
            return SystemAlbum::Videos.title().into();
        }
        match &self.selection {
            AlbumSelection::AllLibrary => t("view.all_media"),
            AlbumSelection::System(album) => album.title().into(),
            AlbumSelection::User(id) => self
                .user_smart_albums
                .iter()
                .find(|a| a.id == *id)
                .map(|a| a.title.clone())
                .unwrap_or_else(|| t("album.fallback_title")),
            AlbumSelection::Album(id) => self
                .album_title(*id)
                .map_or_else(|| t("album.fallback_title"), str::to_string),
            AlbumSelection::Tag(tag) => format!("#{tag}"),
            AlbumSelection::Untagged => t("view.untagged"),
            AlbumSelection::Duplicates => t("duplicates.title"),
            AlbumSelection::Trash => t("view.recently_deleted"),
            AlbumSelection::AlbumsOverview => t("albums.title"),
            AlbumSelection::Tags => t("tags.title"),
            AlbumSelection::Downloads => t("downloads.title"),
        }
    }

    /// The Videos view proper: Videos selected with no search or year over
    /// it (those show the whole library's matches), or its 4K chip's
    /// search. Its counts read "clips".
    pub(crate) fn videos_view(&self) -> bool {
        is_videos_scope(&self.selection, &self.search_query, self.active_year)
    }

    /// A count as this view names its items: "N clips" in Videos, else
    /// "N items".
    pub(crate) fn count_label(&self, n: usize) -> String {
        if self.videos_view() {
            crate::i18n::plural("header.clips", n)
        } else {
            crate::i18n::plural("header.items", n)
        }
    }

    pub(crate) fn view_key(&self) -> ViewKey {
        ViewKey {
            library: self.library_generation,
            query: self.search_query.clone(),
            year: self.active_year,
            selection: self.selection.clone(),
            sort: self.sort,
        }
    }

    pub(crate) fn view_query(&self) -> ViewQuery {
        let key = self.view_key();
        let album = match key.selection {
            AlbumSelection::User(id) => self.user_smart_albums.iter().find(|a| a.id == id).cloned(),
            _ => None,
        };
        ViewQuery { key, album }
    }

    fn catalog_revision(&self) -> Option<u64> {
        self.grid_state
            .as_ref()
            .map(|state| state.engine.data_revision())
    }

    /// Count from the cache when it is for this view and current; never
    /// queries. Used by the synchronous [`Self::get_active_count`].
    pub(crate) fn fresh_view_count(&self) -> Option<usize> {
        let revision = self.catalog_revision()?;
        self.active_view
            .fresh(&self.view_key(), revision)
            .map(|data| data.count)
    }

    /// Rows from the cache when it is for this view, current and covers
    /// `range`; never queries. Used by the synchronous
    /// [`Self::get_active_slice`].
    pub(crate) fn fresh_view_rows(&self, range: &Range<usize>) -> Option<Vec<MediaItem>> {
        let revision = self.catalog_revision()?;
        let data = self.active_view.fresh(&self.view_key(), revision)?;
        ActiveView::covers(data, range).then(|| rows_in(data, range))
    }

    /// The latest fetched count, this view's or (while its fetch runs) the
    /// previous view's, so a count can stay put instead of blinking out.
    /// Never queries.
    pub(crate) fn last_fetched_count(&self) -> Option<usize> {
        self.active_view.data.as_ref().map(|data| data.count)
    }

    /// Count to draw: the cache's for this view even if a newer revision is
    /// on its way, else the sidebar's cached count. Never queries.
    pub(crate) fn displayed_count(&self) -> usize {
        let key = self.view_key();
        match &self.active_view.data {
            Some(data) if data.key == key => data.count,
            _ => self.cached_active_count().unwrap_or(0),
        }
    }

    /// Date span to draw, from the same fetch as [`Self::displayed_count`];
    /// `None` until this view's first fetch lands. Never queries.
    pub(crate) fn displayed_span(&self) -> Option<DateSpan> {
        let key = self.view_key();
        match &self.active_view.data {
            Some(data) if data.key == key => data.span,
            _ => None,
        }
    }

    /// Videos among the results, from the same fetch as
    /// [`Self::displayed_count`]. Never queries.
    pub(crate) fn displayed_videos(&self) -> Option<usize> {
        let key = self.view_key();
        match &self.active_view.data {
            Some(data) if data.key == key => data.videos,
            _ => None,
        }
    }

    /// Videos-view totals, from the same fetch as
    /// [`Self::displayed_count`]. Never queries.
    pub(crate) fn displayed_totals(&self) -> Option<&ViewTotals> {
        let key = self.view_key();
        match &self.active_view.data {
            Some(data) if data.key == key => data.totals.as_ref(),
            _ => None,
        }
    }

    /// True until the first fetch for the current view lands, so an empty
    /// view is not announced before it is known to be empty.
    pub(crate) fn view_loading(&self) -> bool {
        let key = self.view_key();
        !matches!(&self.active_view.data, Some(data) if data.key == key)
    }

    /// Rows to draw at `range`: whatever the cache holds for this view from
    /// `range.start` on (possibly fewer, possibly a revision behind). Never
    /// queries; missing rows draw empty until their fetch lands.
    pub(crate) fn displayed_rows(&self, range: Range<usize>) -> Vec<MediaItem> {
        let key = self.view_key();
        match &self.active_view.data {
            Some(data) if data.key == key => rows_in(data, &range),
            _ => Vec::new(),
        }
    }

    /// The cache for navigation: this view's rows even a revision behind,
    /// as long as the app itself has not written since they were read. A
    /// scan's writes (other threads) leave them usable, so arrows never wait
    /// on its lock; the app's own trash or edit forces a fresh read so the
    /// selection is remapped (AGENTS 5.3).
    fn navigable(&self) -> Option<&Fetched> {
        let engine = &self.grid_state.as_ref()?.engine;
        let key = self.view_key();
        self.active_view.data.as_ref().filter(|data| {
            data.key == key
                && (data.revision == engine.data_revision()
                    || data.revision >= engine.revision_written_here())
        })
    }

    /// Count for keyboard navigation; see [`Self::navigable`].
    pub(crate) fn navigation_count(&self) -> usize {
        self.navigable()
            .map(|data| data.count)
            .unwrap_or_else(|| self.get_active_count())
    }

    /// Items at `range` for keyboard navigation and selection; see
    /// [`Self::navigable`].
    pub(crate) fn navigation_rows(&self, range: Range<usize>) -> Vec<MediaItem> {
        #[cfg(test)]
        ROWS_READ.with(|n| n.set(n.get() + range.len()));
        match self.navigable() {
            Some(data) if ActiveView::covers(data, &range) => rows_in(data, &range),
            _ => self.get_active_slice(range),
        }
    }

    pub(crate) fn navigation_item(&self, index: usize) -> Option<MediaItem> {
        self.navigation_rows(index..index + 1).into_iter().next()
    }

    /// Notes that `range` is needed next (the grid's visible rows, or the
    /// viewer's neighbours) and fetches it if the cache does not cover it.
    pub(crate) fn want_view_rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        self.active_view.wanted = range;
        self.refresh_active_view(cx);
    }

    /// Starts a background fetch when the cache is missing, for another
    /// view, behind the catalog or short of the wanted rows. One fetch runs
    /// at a time; when it lands the next frame asks again.
    /// ponytail: no throttle, so a long scan refetches back to back; add one
    /// if that measurably slows scans.
    pub(crate) fn refresh_active_view(&mut self, cx: &mut Context<Self>) {
        if self.active_view.in_flight {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let query = self.view_query();
        // A new view starts at the top, not at the last view's scroll.
        if self.view_loading() {
            let len = self.active_view.wanted.len();
            self.active_view.wanted = 0..len;
        }
        let wanted = self.active_view.wanted.clone();
        let revision = engine.data_revision();
        let fresh = self.active_view.fresh(&query.key, revision);
        if fresh.is_some_and(|data| ActiveView::covers(data, &wanted)) {
            return;
        }
        // A scroll past the cached rows keeps the span, as long as nothing
        // is written before the fetch reads its revision (checked there).
        let known_span = fresh.map(|data| (data.span, data.videos, data.totals.clone()));
        let known_at = revision;
        // ponytail: a panicking fetch would leave this set and freeze the
        // cache; the engine's reads log and return defaults instead of failing.
        self.active_view.in_flight = true;
        let task = cx.background_executor().spawn(async move {
            // Read before querying: a write that lands meanwhile leaves this
            // revision behind, and the next frame fetches again.
            let revision = engine.data_revision();
            let count = query.count(&engine);
            let start = wanted.start.saturating_sub(WINDOW_PAD).min(count);
            let end = (wanted.end + WINDOW_PAD).min(count).max(start);
            let items = query.slice(&engine, start..end);
            // After the rows, so a view switch never waits on it for them.
            let (span, videos, totals) = known_span
                .filter(|_| revision == known_at)
                .unwrap_or_else(|| {
                    (
                        query.date_span(&engine),
                        query.video_count(&engine),
                        query.totals(&engine),
                    )
                });
            Fetched {
                key: query.key,
                revision,
                count,
                span,
                videos,
                totals,
                start,
                items,
            }
        });
        cx.spawn(async move |this, cx| {
            let fetched = task.await;
            let _ = this.update(cx, |app, cx| {
                app.active_view.in_flight = false;
                if fetched.key == app.view_key() {
                    app.active_view.data = Some(fetched);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn rows_in(data: &Fetched, range: &Range<usize>) -> Vec<MediaItem> {
    let Some(from) = range.start.checked_sub(data.start) else {
        return Vec::new();
    };
    let to = (range.end - data.start).min(data.items.len());
    data.items
        .get(from..to)
        .map(<[_]>::to_vec)
        .unwrap_or_default()
}
