//! The Albums overview page: how it is reached, its tabs, its album cards
//! (covers, order, menus) and its keyboard focus. The view is
//! `views/albums_overview`.

use std::cmp::Reverse;
use std::collections::HashMap;

use gpui::*;
use lumina_core::AlbumCard;

use crate::app::LuminaApp;
use crate::config::AlbumSort;
use crate::views::sidebar::AlbumSelection;

/// The overview's tabs, in bar order.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AlbumsTab {
    #[default]
    Albums,
    SmartAlbums,
}

impl AlbumsTab {
    pub const ALL: [AlbumsTab; 2] = [AlbumsTab::Albums, AlbumsTab::SmartAlbums];

    /// The `page-tab-{id}` selector's id.
    pub fn id(self) -> &'static str {
        match self {
            AlbumsTab::Albums => "albums",
            AlbumsTab::SmartAlbums => "smart-albums",
        }
    }
}

/// What Tab has focused on the overview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewFocus {
    /// The tab bar: ←/→ switch tabs.
    Tabs,
    /// The header's New Album button: ↵ runs it.
    NewAlbum,
    /// The Albums tab's "Sort ▾" control: ↵ opens its menu.
    Sort,
    /// An album card: arrows move, ↵ opens, ⌘⌫ asks to delete.
    Card(i64),
}

/// The card whose right-click menu is open, and where.
#[derive(Debug, Clone, Copy)]
pub struct CardMenu {
    pub album: i64,
    pub position: Point<Pixels>,
}

/// The overview's tab (kept between visits), keyboard focus, open menus
/// and the album card covers.
#[derive(Default)]
pub struct OverviewState {
    pub tab: AlbumsTab,
    pub focus: Option<OverviewFocus>,
    /// The sort menu, while open: its highlighted row in `AlbumSort::ALL`.
    pub sort_menu: Option<usize>,
    pub card_menu: Option<CardMenu>,
    /// Cover and newest capture time per album id, read off the UI thread.
    pub cards: HashMap<i64, AlbumCard>,
    /// (library generation, catalog revision) `cards` was read at.
    cards_loaded: Option<(u64, u64)>,
    cards_in_flight: bool,
    /// Cards per row at the current window width (set before each frame).
    pub columns: usize,
    /// The card grid's rows; keeps the focused card in view.
    pub scroll: UniformListScrollHandle,
}

impl OverviewState {
    /// Leaving the page: its focus and menus never outlive the visit (↵
    /// would fire them).
    pub(crate) fn leave(&mut self) {
        self.focus = None;
        self.sort_menu = None;
        self.card_menu = None;
    }
}

impl LuminaApp {
    /// The Albums overview is what the main column shows: selected, with
    /// no search or year over it (those show their matches instead).
    pub(crate) fn albums_overview_shown(&self) -> bool {
        self.selection == AlbumSelection::AlbumsOverview
            && self.search_query.is_empty()
            && self.active_year.is_none()
    }

    /// The sidebar ALBUMS label, ⌘K Show Albums and Go > Albums: the
    /// overview page, one Back entry. Not while compare holds the view.
    pub(crate) fn show_albums_overview(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() || self.compare_holds_view() {
            return;
        }
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        if !self.albums_overview_shown() {
            self.select_album(AlbumSelection::AlbumsOverview, cx);
        }
        cx.notify();
    }

    /// ⌘K Show Albums Tab / Show Smart Albums Tab: the overview on `tab`.
    pub(crate) fn show_albums_tab(&mut self, tab: AlbumsTab, cx: &mut Context<Self>) {
        self.show_albums_overview(cx);
        if self.albums_overview_shown() {
            self.set_albums_tab(tab, cx);
        }
    }

    /// Every tab change (click, ←/→, palette) closes the menus and drops a
    /// Sort or card focus: they belong to the Albums tab, and keys must not
    /// act on hidden cards.
    pub(crate) fn set_albums_tab(&mut self, tab: AlbumsTab, cx: &mut Context<Self>) {
        let overview = &mut self.albums.overview;
        overview.tab = tab;
        overview.sort_menu = None;
        overview.card_menu = None;
        if matches!(
            overview.focus,
            Some(OverviewFocus::Sort | OverviewFocus::Card(_))
        ) {
            overview.focus = None;
        }
        cx.notify();
    }

    /// The header meta: "24 albums · 12 smart albums".
    pub(crate) fn albums_overview_meta(&self) -> String {
        format!(
            "{} · {}",
            crate::i18n::albums(self.albums.list.len()),
            crate::i18n::smart_albums(self.user_smart_albums.len())
        )
    }

    /// Sets the card columns for this frame's width and, when the catalog
    /// moved since the last read, reads the covers on the background
    /// executor. Called per frame while the page shows; it only compares
    /// numbers then (as the map does).
    pub(crate) fn prepare_albums_overview(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.albums_overview_shown() {
            return;
        }
        let pane = crate::views::grid::center_pane_width(
            window.viewport_size().width.into(),
            self.sidebar_width,
            self.sidebar_collapsed,
            false,
            None, // the inspector is hidden on the page
        );
        self.albums.overview.columns = crate::views::albums_overview::albums::card_columns(pane);
        self.refresh_album_cards(cx);
    }

    fn refresh_album_cards(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let generation = self.library_generation;
        let overview = &mut self.albums.overview;
        if overview.cards_in_flight
            || overview.cards_loaded == Some((generation, engine.data_revision()))
        {
            return;
        }
        overview.cards_in_flight = true;
        let task = cx.background_executor().spawn(async move {
            // Read before querying: a write meanwhile leaves this revision
            // behind, and the next frame reads again. A failed read is
            // logged and bannered by the engine and gives no covers.
            let revision = engine.data_revision();
            (revision, engine.album_cards())
        });
        cx.spawn(async move |this, cx| {
            let (revision, cards) = task.await;
            this.update(cx, |app, cx| {
                // Another library opened meanwhile: opening it reset the
                // album state, so this read touches nothing there.
                if app.library_generation == generation {
                    let overview = &mut app.albums.overview;
                    overview.cards_in_flight = false;
                    overview.cards = cards.into_iter().map(|c| (c.album_id, c)).collect();
                    overview.cards_loaded = Some((generation, revision));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The album cards are on screen: the Albums tab with an album.
    pub(crate) fn album_cards_shown(&self) -> bool {
        self.albums.overview.tab == AlbumsTab::Albums && !self.albums.list.is_empty()
    }

    /// Album ids in card order, by the "Sort ▾" choice. In memory: the
    /// list, counts and covers are cached.
    pub(crate) fn album_card_order(&self) -> Vec<i64> {
        let mut albums: Vec<&lumina_core::Album> = self.albums.list.iter().collect();
        let count = |id: i64| self.albums.counts.get(&id).copied().unwrap_or(0);
        let newest = |id: i64| {
            let card = self.albums.overview.cards.get(&id);
            card.and_then(|card| card.newest_epoch_ms)
        };
        // Name breaks every tie. `Reverse(None)` sorts after any date.
        match self.config.album_sort {
            AlbumSort::Name => albums.sort_by_cached_key(|a| (a.title.to_lowercase(), a.id)),
            AlbumSort::Created => albums
                .sort_by_cached_key(|a| (Reverse(a.created_at_epoch_ms), a.title.to_lowercase())),
            AlbumSort::NewestPhoto => {
                albums.sort_by_cached_key(|a| (Reverse(newest(a.id)), a.title.to_lowercase()))
            }
            AlbumSort::ItemCount => {
                albums.sort_by_cached_key(|a| (Reverse(count(a.id)), a.title.to_lowercase()))
            }
        }
        albums.into_iter().map(|a| a.id).collect()
    }

    /// The keyboard-focused album card, while it is on screen. ⌘K Rename
    /// Album… and Delete Album… act on it.
    pub(crate) fn focused_album_card(&self) -> Option<i64> {
        match self.albums.overview.focus {
            Some(OverviewFocus::Card(id))
                if self.albums_overview_shown()
                    && self.album_cards_shown()
                    && self.album_title(id).is_some() =>
            {
                Some(id)
            }
            _ => None,
        }
    }

    /// The card the focus moves to when the focused album `id` is deleted:
    /// the next one, else the previous; `None` when it is the only one.
    pub(crate) fn album_card_neighbour(&self, id: i64) -> Option<i64> {
        let order = self.album_card_order();
        let at = order.iter().position(|o| *o == id)?;
        order
            .get(at + 1)
            .or(at.checked_sub(1).and_then(|i| order.get(i)))
            .copied()
    }

    /// Scrolls the focused card's row into view (its place can change after
    /// a rename or a delete).
    pub(crate) fn scroll_focused_album_card(&mut self) {
        let Some(OverviewFocus::Card(id)) = self.albums.overview.focus else {
            return;
        };
        if let Some(at) = self.album_card_order().iter().position(|o| *o == id) {
            let columns = self.albums.overview.columns.max(1);
            self.albums
                .overview
                .scroll
                .scroll_to_item(at / columns, ScrollStrategy::Nearest);
        }
    }

    /// A card's click or ↵: the album, one Back entry.
    pub(crate) fn open_album_card(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.album_title(id).is_some() {
            self.select_album(AlbumSelection::Album(id), cx);
        }
    }

    pub(crate) fn open_album_card_menu(
        &mut self,
        album: i64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.albums.overview.sort_menu = None;
        self.albums.overview.card_menu = Some(CardMenu { album, position });
        cx.notify();
    }

    pub(crate) fn close_overview_menus(&mut self, cx: &mut Context<Self>) {
        self.albums.overview.sort_menu = None;
        self.albums.overview.card_menu = None;
        cx.notify();
    }

    pub(crate) fn toggle_album_sort_menu(&mut self, cx: &mut Context<Self>) {
        let overview = &mut self.albums.overview;
        overview.card_menu = None;
        overview.sort_menu = match overview.sort_menu {
            Some(_) => None,
            None => AlbumSort::ALL
                .iter()
                .position(|sort| *sort == self.config.album_sort),
        };
        cx.notify();
    }

    /// Picks the card order. Saved in the config; a failed save puts the
    /// order back and is in the error banner.
    pub(crate) fn set_album_sort(&mut self, sort: AlbumSort, cx: &mut Context<Self>) {
        self.albums.overview.sort_menu = None;
        let previous = self.config.album_sort;
        if previous != sort {
            self.config.album_sort = sort;
            if !self.save_config() {
                self.config.album_sort = previous;
            }
        }
        cx.notify();
    }

    /// Focuses the card at `index` in card order and scrolls its row in.
    fn focus_album_card(&mut self, order: &[i64], index: usize, cx: &mut Context<Self>) {
        if let Some(&id) = order.get(index) {
            self.albums.overview.focus = Some(OverviewFocus::Card(id));
            self.scroll_focused_album_card();
            cx.notify();
        }
    }

    /// The stops Tab moves through, in order.
    fn overview_stops(&self) -> Vec<OverviewFocus> {
        let mut stops = vec![OverviewFocus::Tabs, OverviewFocus::NewAlbum];
        if let (true, Some(&first)) = (self.album_cards_shown(), self.album_card_order().first()) {
            stops.extend([OverviewFocus::Sort, OverviewFocus::Card(first)]);
        }
        stops
    }

    /// Overview keys: Tab / ⇧Tab move the focus between the tab bar, New
    /// Album, Sort and the cards; ←/→ switch tabs while the bar has it; ↵
    /// runs New Album, opens the sort menu or the focused album; arrows move
    /// between cards; ⌘⌫ asks to delete the focused album; Esc closes a menu
    /// or clears the focus. Every key that reaches it stops here: there is
    /// no grid under the page to act on.
    pub(crate) fn handle_albums_overview_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.albums_overview_shown() || self.is_fullscreen_viewer {
            return false;
        }
        let m = &keystroke.modifiers;
        let plain = !(m.platform || m.control || m.alt || m.function);
        let key = keystroke.key.as_str();
        if self.albums.overview.card_menu.is_some() {
            if key == "escape" {
                self.close_overview_menus(cx);
            }
            return true;
        }
        if let Some(at) = self.albums.overview.sort_menu {
            let last = AlbumSort::ALL.len() - 1;
            match key {
                "up" => self.albums.overview.sort_menu = Some(at.saturating_sub(1)),
                "down" => self.albums.overview.sort_menu = Some((at + 1).min(last)),
                "enter" | "return" | "space" => self.set_album_sort(AlbumSort::ALL[at], cx),
                "escape" => self.albums.overview.sort_menu = None,
                _ => {}
            }
            cx.notify();
            return true;
        }
        let focus = self.albums.overview.focus;
        match key {
            "tab" if plain => {
                let stops = self.overview_stops();
                let at = focus.and_then(|focus| {
                    let kind = std::mem::discriminant(&focus);
                    stops.iter().position(|s| std::mem::discriminant(s) == kind)
                });
                let n = stops.len();
                self.albums.overview.focus = Some(match at {
                    None if m.shift => OverviewFocus::NewAlbum,
                    None => stops[0],
                    Some(i) if m.shift => stops[(i + n - 1) % n],
                    Some(i) => stops[(i + 1) % n],
                });
                if let Some(OverviewFocus::Card(_)) = self.albums.overview.focus {
                    self.albums
                        .overview
                        .scroll
                        .scroll_to_item(0, ScrollStrategy::Nearest);
                }
                cx.notify();
            }
            "left" | "right" if plain && !m.shift && focus == Some(OverviewFocus::Tabs) => {
                let at = AlbumsTab::ALL
                    .iter()
                    .position(|tab| *tab == self.albums.overview.tab)
                    .unwrap_or(0);
                let next = if key == "left" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(AlbumsTab::ALL.len() - 1)
                };
                self.set_albums_tab(AlbumsTab::ALL[next], cx);
            }
            "left" | "right" | "up" | "down" if plain && !m.shift => {
                if let Some(OverviewFocus::Card(id)) = focus {
                    let order = self.album_card_order();
                    let at = order.iter().position(|o| *o == id).unwrap_or(0);
                    let columns = self.albums.overview.columns.max(1);
                    let next = match key {
                        "left" => at.saturating_sub(1),
                        "right" => (at + 1).min(order.len().saturating_sub(1)),
                        "up" => at.checked_sub(columns).unwrap_or(at),
                        _ if at + columns < order.len() => at + columns,
                        _ => at,
                    };
                    self.focus_album_card(&order, next, cx);
                }
            }
            "enter" | "return" if plain => match focus {
                Some(OverviewFocus::NewAlbum) => self.new_album(cx),
                Some(OverviewFocus::Sort) => self.toggle_album_sort_menu(cx),
                Some(OverviewFocus::Card(id)) => self.open_album_card(id, cx),
                _ => {}
            },
            "backspace" if m.platform && !(m.shift || m.control || m.alt) => {
                if let Some(OverviewFocus::Card(id)) = focus {
                    if self.album_title(id).is_some() {
                        self.prompt_delete_album(id, cx);
                    }
                }
            }
            "escape" if focus.is_some() => {
                self.albums.overview.focus = None;
                cx.notify();
            }
            _ => {}
        }
        true
    }
}
