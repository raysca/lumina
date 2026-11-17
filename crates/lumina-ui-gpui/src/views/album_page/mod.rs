//! An open album's page: the header band over
//! the album grid. This module holds the page's state (the More menu and
//! the header's stats, read off the UI thread) and its keys; `header`
//! draws the band, `selection_bar` the band under it, `panel` the
//! settings column on the right and `key_photo` its KEY PHOTO section,
//! pick mode and the cover card's badge.

pub(crate) mod header;
pub(crate) mod key_photo;
pub(crate) mod panel;
pub(crate) mod selection_bar;

use gpui::*;
use lumina_core::AlbumStats;

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

/// A row of the header's More menu, in menu order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlbumMenuItem {
    Rename,
    Pin,
    /// The macOS share sheet for the selection.
    Share,
    Delete,
}

impl AlbumMenuItem {
    /// The rows this platform offers: Share… needs the macOS share sheet.
    pub(crate) fn all() -> Vec<AlbumMenuItem> {
        let mut items = vec![AlbumMenuItem::Rename, AlbumMenuItem::Pin];
        if cfg!(target_os = "macos") {
            items.push(AlbumMenuItem::Share);
        }
        items.push(AlbumMenuItem::Delete);
        items
    }

    pub(crate) fn selector(self) -> &'static str {
        match self {
            AlbumMenuItem::Rename => "album-menu-rename",
            AlbumMenuItem::Pin => "album-menu-pin",
            AlbumMenuItem::Share => "album-menu-share",
            AlbumMenuItem::Delete => "album-menu-delete",
        }
    }
}

/// The album page's More menu and the header stats.
#[derive(Default)]
pub struct AlbumPageState {
    /// The More menu, while open: its highlighted row in [`AlbumMenuItem::all`].
    pub menu: Option<usize>,
    /// The last stats read, and for which album. Shown (even a revision
    /// behind) while the album is open; another album's are not.
    stats: Option<(i64, AlbumStats)>,
    /// (library generation, album, catalog revision) `stats` was read at.
    stats_loaded: Option<(u64, i64, u64)>,
    stats_in_flight: bool,
    /// The album's stored key photo when the last read started.
    stats_stored_cover: Option<lumina_core::MediaId>,
    /// The file name of `stats`' cover, read with it.
    cover_name: Option<SharedString>,
    /// The album whose cover the next clicked or ↵-ed item becomes, while
    /// Change is picking; it ends on leaving that album.
    pub(crate) picking_cover: Option<i64>,
    /// The item whose click picked the cover: the second click of that
    /// double-click must not open the viewer.
    pub(crate) picked_by_click: Option<lumina_core::MediaId>,
    /// [`LuminaApp::album_page_cover`] for this frame, set once before the
    /// grid paints so cards compare one id.
    pub(crate) shown_cover: Option<lumina_core::MediaId>,
    /// The album grid's zoom, once ⌘+ / ⌘− changed it on an album page
    /// (`None`: [`crate::views::grid::ALBUM_COLUMNS`]). Shared by every
    /// album and kept for the session only; Library's `grid_columns` is
    /// the persisted zoom.
    pub(crate) columns: Option<usize>,
}

impl LuminaApp {
    /// The open album, while its page shows: no search or year over it.
    pub(crate) fn album_page_album(&self) -> Option<i64> {
        let AlbumSelection::Album(id) = self.selection else {
            return None;
        };
        (self.search_query.is_empty()
            && self.active_year.is_none()
            && !self.search_screen_shown()
            && self.album_title(id).is_some())
        .then_some(id)
    }

    /// The header's stats for album `id`, once read.
    pub(crate) fn album_page_stats(&self, id: i64) -> Option<AlbumStats> {
        match self.albums.page.stats {
            Some((album, stats)) if album == id => Some(stats),
            _ => None,
        }
    }

    /// The cover's file name for album `id`, once its stats are read.
    pub(crate) fn album_page_cover_name(&self, id: i64) -> Option<SharedString> {
        self.album_page_stats(id)
            .and(self.albums.page.cover_name.clone())
    }

    /// Run from render: while the page shows, reads the stats (and cover)
    /// when the album or the catalog changed. One read at a time, so reads
    /// never stack: the next frame after one lands reads the latest
    /// revision. During a scan the stats are reread only when the open
    /// album's member count or stored key photo moved, not on every batch.
    /// Per frame it only
    /// compares numbers. Off the page the menu closes.
    pub(crate) fn prepare_album_page(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.album_page_album() else {
            self.albums.page.menu = None;
            self.albums.page.picking_cover = None;
            self.albums.page.shown_cover = None;
            return;
        };
        // Pick mode lives in the panel: it ends with the album or when the
        // panel hides (Info on, a narrow window, Hide Album Settings).
        if !self.album_panel_shown()
            || self
                .albums
                .page
                .picking_cover
                .is_some_and(|album| album != id)
        {
            self.albums.page.picking_cover = None;
        }
        self.albums.page.shown_cover = self.album_page_cover();
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let (engine, scanning) = (state.engine.clone(), state.is_scanning);
        let generation = self.library_generation;
        // Current enough to wait for the scan: the stats count what the
        // grid shows and were read for the key photo stored now (the list
        // reloads right after Set as Cover and its undo).
        let live = self.displayed_count();
        let stored = self
            .albums
            .list
            .iter()
            .find(|album| album.id == id)
            .and_then(|album| album.cover_media_id);
        let current = self
            .album_page_stats(id)
            .is_some_and(|s| s.photos + s.videos == live)
            && self.albums.page.stats_stored_cover == stored;
        let page = &mut self.albums.page;
        if page.stats_in_flight
            || (scanning && current)
            || page.stats_loaded == Some((generation, id, engine.data_revision()))
        {
            return;
        }
        page.stats_in_flight = true;
        page.stats_stored_cover = stored;
        let task = cx.background_executor().spawn(async move {
            // Read before querying: a write meanwhile leaves this revision
            // behind, and the next frame reads again. A failed read is
            // logged by the engine and reads as empty.
            let revision = engine.data_revision();
            let stats = engine.album_stats(id);
            let name = stats
                .cover
                .and_then(|cover| engine.get_item_by_id(cover))
                .map(|item| SharedString::from(item.file_name));
            (revision, stats, name)
        });
        cx.spawn(async move |this, cx| {
            let (revision, stats, name) = task.await;
            this.update(cx, |app, cx| {
                // Another library opened meanwhile: opening it reset the
                // page state, so this read touches nothing there.
                if app.library_generation == generation {
                    let page = &mut app.albums.page;
                    page.stats_in_flight = false;
                    page.stats = Some((id, stats));
                    page.cover_name = name;
                    page.stats_loaded = Some((generation, id, revision));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The More button: opens the menu on its first row, or closes it.
    pub(crate) fn toggle_album_menu(&mut self, cx: &mut Context<Self>) {
        self.albums.page.menu = match self.albums.page.menu {
            Some(_) => None,
            None => Some(0),
        };
        cx.notify();
    }

    pub(crate) fn close_album_menu(&mut self, cx: &mut Context<Self>) {
        self.albums.page.menu = None;
        cx.notify();
    }

    /// Whether a row acts now: Share… needs a selection.
    pub(crate) fn album_menu_item_enabled(&self, item: AlbumMenuItem) -> bool {
        item != AlbumMenuItem::Share || self.has_selection()
    }

    /// Runs a More menu row on the open album. The menu closes first: the
    /// actions refuse to start under an open menu.
    pub(crate) fn run_album_menu_item(&mut self, item: AlbumMenuItem, cx: &mut Context<Self>) {
        let Some(id) = self.album_page_album() else {
            return;
        };
        if !self.album_menu_item_enabled(item) {
            return;
        }
        self.albums.page.menu = None;
        match item {
            AlbumMenuItem::Rename => self.start_album_rename(id, cx),
            AlbumMenuItem::Pin => {
                let pinned = self.open_album_pinned().unwrap_or(false);
                self.set_open_album_pinned(!pinned, cx);
            }
            #[cfg(target_os = "macos")]
            AlbumMenuItem::Share => self.share_selection(None, cx),
            // No share picker off macOS; the menu hides Share there.
            #[cfg(not(target_os = "macos"))]
            AlbumMenuItem::Share => {}
            AlbumMenuItem::Delete => self.prompt_delete_album(id, cx),
        }
        cx.notify();
    }

    /// More menu keys: ↑/↓ move over the enabled rows, ↵ or Space runs the row, Esc closes. The
    /// caller absorbs every other key, so ⌫ never trashes the selection
    /// behind the menu.
    pub(crate) fn handle_album_menu_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(at) = self.albums.page.menu else {
            return;
        };
        let items = AlbumMenuItem::all();
        // The nearest enabled row that way; the highlight stays without one.
        let enabled = |i: &usize| self.album_menu_item_enabled(items[*i]);
        let up = (0..at).rev().find(enabled).unwrap_or(at);
        let down = (at + 1..items.len()).find(enabled).unwrap_or(at);
        match key {
            "up" => self.albums.page.menu = Some(up),
            "down" => self.albums.page.menu = Some(down),
            "enter" | "return" | "space" => self.run_album_menu_item(items[at], cx),
            "escape" => self.albums.page.menu = None,
            _ => {}
        }
        cx.notify();
    }
}

/// The dismiss backdrop behind the More menu; the card hangs from the
/// More button, painted above this.
pub(crate) fn render_menu_backdrop(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if app.albums.page.menu.is_none() {
        return Empty.into_any_element();
    }
    div()
        .absolute()
        .inset_0()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.close_album_menu(cx);
            }),
        )
        .into_any_element()
}
