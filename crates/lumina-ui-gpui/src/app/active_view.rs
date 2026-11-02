//! Count and rows of the active view, fetched off the UI thread.
//!
//! Rendering reads only this cache, so a scan or import holding the catalog
//! lock never stalls a frame. A frame that finds the cache missing or behind
//! the catalog's revision asks for a fetch and keeps drawing what it has; the
//! fetch lands, notifies, and the next frame shows it.

use std::ops::Range;

use gpui::Context;
use lumina_core::{LuminaEngine, MediaItem, SmartAlbum, SortSpec, SystemAlbum};

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

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
            AlbumSelection::Trash => engine.query_system_album_count(SystemAlbum::RecentlyDeleted),
            AlbumSelection::Tag(tag) => engine.search_query_count(&tag_query(tag)),
            AlbumSelection::Untagged => engine.query_untagged_count(),
            AlbumSelection::Duplicates => engine.query_duplicate_count(),
        }
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
            AlbumSelection::Trash => {
                engine.query_system_album_sorted(SystemAlbum::RecentlyDeleted, limit, offset, sort)
            }
            AlbumSelection::Tag(tag) => {
                engine.search_query_sorted(&tag_query(tag), limit, offset, sort)
            }
            AlbumSelection::Untagged => engine.query_untagged_sorted(limit, offset, sort),
            AlbumSelection::Duplicates => engine.query_duplicate_items_slice(offset, limit),
        }
    }
}

/// What a fetch read, and at which catalog revision.
struct Fetched {
    key: ViewKey,
    revision: u64,
    count: usize,
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
}

/// Items fetched either side of what is wanted, so scrolling and viewer
/// steps usually hit the cache.
const WINDOW_PAD: usize = 64;

impl ActiveView {
    /// The cached rows for `key`, if they are for that view and current.
    fn fresh(&self, key: &ViewKey, revision: u64) -> Option<&Fetched> {
        self.data
            .as_ref()
            .filter(|data| &data.key == key && data.revision == revision)
    }

    fn covers(data: &Fetched, range: &Range<usize>) -> bool {
        let end = (data.start + data.items.len()).max(data.start);
        // Past the end of the view counts as covered: there is nothing to fetch.
        let range_end = range.end.min(data.count);
        range.start >= data.start && range_end <= end
    }
}

impl LuminaApp {
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

    /// Count to draw: the cache's for this view even if a newer revision is
    /// on its way, else the sidebar's cached count. Never queries.
    pub(crate) fn displayed_count(&self) -> usize {
        let key = self.view_key();
        match &self.active_view.data {
            Some(data) if data.key == key => data.count,
            _ => self.cached_active_count().unwrap_or(0),
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
        if self
            .active_view
            .fresh(&query.key, revision)
            .is_some_and(|data| ActiveView::covers(data, &wanted))
        {
            return;
        }
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
            Fetched {
                key: query.key,
                revision,
                count,
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
