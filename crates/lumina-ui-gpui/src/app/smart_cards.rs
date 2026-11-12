//! Smart album cards on the Albums overview (order, focus, menu, opening)
//! and the Albums tab's page list, which scrolls the smart row and the
//! album grid as one. Views: `views/albums_overview/{smart,albums}.rs`.

use gpui::*;
use lumina_core::SmartAlbum;

use crate::app::albums_overview::{AlbumsTab, CardMenu, OverviewFocus};
use crate::app::LuminaApp;
use crate::views::albums_overview::albums::list_item_height;
use crate::views::sidebar::AlbumSelection;

/// Smart cards on the Albums tab; the Smart Albums tab shows them all.
pub(crate) const SMART_ROW_LIMIT: usize = 4;
/// The Albums tab list: item 0 is the smart section, 1 the albums head,
/// then the album card rows (or the albums empty state).
pub(crate) const FIRST_ALBUM_ROW: usize = 2;

/// The Albums tab's variable-height list, and the shape its first items
/// were measured at.
pub struct OverviewList {
    pub state: ListState,
    shape: Option<(usize, usize, bool)>,
}

impl Default for OverviewList {
    fn default() -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, px(600.0)),
            shape: None,
        }
    }
}

/// The card after `gone` in `old` that is still in `new`, else the one
/// before it.
fn neighbour(old: &[i64], new: &[i64], gone: i64) -> Option<i64> {
    let at = old.iter().position(|id| *id == gone)?;
    let after = old[at + 1..].iter();
    let before = old[..at].iter().rev();
    after.chain(before).find(|id| new.contains(id)).copied()
}

impl LuminaApp {
    pub(crate) fn smart_album(&self, id: i64) -> Option<&SmartAlbum> {
        self.user_smart_albums.iter().find(|album| album.id == id)
    }

    /// Smart albums with a card on the current tab, in sidebar order: up to
    /// four on Albums, every one on Smart Albums.
    pub(crate) fn smart_card_order(&self) -> Vec<i64> {
        let limit = match self.albums.overview.tab {
            AlbumsTab::Albums => SMART_ROW_LIMIT,
            AlbumsTab::SmartAlbums => usize::MAX,
        };
        let albums = self.user_smart_albums.iter().take(limit);
        albums.map(|album| album.id).collect()
    }

    /// The keyboard-focused smart card, while it is on screen.
    pub(crate) fn focused_smart_card(&self) -> Option<i64> {
        match self.albums.overview.focus {
            Some(OverviewFocus::SmartCard(id))
                if self.albums_overview_shown() && self.smart_card_order().contains(&id) =>
            {
                Some(id)
            }
            _ => None,
        }
    }

    /// The smart album ⌘K Edit / Delete Smart Album… act on: the open one,
    /// or the focused card on the overview.
    pub(crate) fn palette_smart_album(&self) -> Option<i64> {
        match self.selection {
            AlbumSelection::User(id) => Some(id),
            _ => self.focused_smart_card(),
        }
    }

    /// A smart card's click or ↵: the smart album, one Back entry.
    pub(crate) fn open_smart_card(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.smart_album(id).is_some() {
            self.select_album(AlbumSelection::User(id), cx);
        }
    }

    pub(crate) fn open_smart_card_menu(
        &mut self,
        album: i64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.albums.overview.sort_menu = None;
        self.albums.overview.card_menu = Some(CardMenu {
            album,
            smart: true,
            position,
        });
        cx.notify();
    }

    /// Arrows between smart cards, within the row(s) on screen.
    pub(crate) fn move_smart_focus(&mut self, id: i64, key: &str, cx: &mut Context<Self>) {
        let order = self.smart_card_order();
        let at = order.iter().position(|o| *o == id).unwrap_or(0);
        let columns = self.albums.overview.smart_columns.max(1);
        let next = match key {
            "left" => at.saturating_sub(1),
            "right" => (at + 1).min(order.len().saturating_sub(1)),
            "up" => at.checked_sub(columns).unwrap_or(at),
            _ if at + columns < order.len() => at + columns,
            _ => at,
        };
        if let Some(&id) = order.get(next) {
            self.albums.overview.focus = Some(OverviewFocus::SmartCard(id));
            self.scroll_focused_smart_card();
            cx.notify();
        }
    }

    /// Scrolls the focused smart card into view on either tab.
    pub(crate) fn scroll_focused_smart_card(&mut self) {
        let Some(OverviewFocus::SmartCard(id)) = self.albums.overview.focus else {
            return;
        };
        match self.albums.overview.tab {
            AlbumsTab::Albums => self.reveal_overview_item(0),
            AlbumsTab::SmartAlbums => {
                let at = self.smart_card_order().iter().position(|o| *o == id);
                let columns = self.albums.overview.smart_columns.max(1);
                if let Some(at) = at {
                    self.albums
                        .overview
                        .smart_scroll
                        .scroll_to_item(at / columns);
                }
            }
        }
    }

    /// Per frame (and right after a delete): caches the card orders, and
    /// moves a focus whose album or smart album is gone (deleted from a
    /// card, ⌘K, the sidebar or elsewhere) to its neighbour in the previous
    /// order: the next card still there, else the previous; else none.
    pub(crate) fn sync_overview_cards(&mut self) {
        let cards = self.album_card_order();
        let smart = self.smart_card_order();
        let overview = &mut self.albums.overview;
        let moved = match overview.focus {
            Some(OverviewFocus::Card(id)) if !cards.contains(&id) => {
                Some(neighbour(&overview.card_order, &cards, id).map(OverviewFocus::Card))
            }
            Some(OverviewFocus::SmartCard(id)) if !smart.contains(&id) => {
                Some(neighbour(&overview.smart_order, &smart, id).map(OverviewFocus::SmartCard))
            }
            _ => None,
        };
        overview.card_order = cards;
        overview.smart_order = smart;
        if let Some(focus) = moved {
            overview.focus = focus;
            self.scroll_focused_album_card();
            self.scroll_focused_smart_card();
        }
    }

    /// Keeps the Albums tab list's item count in step with the albums, and
    /// re-measures its first items when the smart section or the empty
    /// state changes height. Per frame; it only compares numbers.
    pub(crate) fn sync_overview_list(&mut self) {
        let overview = &self.albums.overview;
        let rows = self.albums.list.len().div_ceil(overview.columns.max(1));
        let count = FIRST_ALBUM_ROW + rows.max(1);
        let smart = self.user_smart_albums.len().min(SMART_ROW_LIMIT);
        let shape = (smart, overview.smart_columns, rows == 0);
        let list = &mut self.albums.overview.list;
        let old = list.state.item_count();
        // A splice at the end keeps the scroll position; a reset would not.
        if old < count {
            list.state.splice(old..old, count - old);
        } else if old > count {
            list.state.splice(count..old, 0);
        }
        if list.shape != Some(shape) {
            list.shape = Some(shape);
            list.state
                .remeasure_items(0..(FIRST_ALBUM_ROW + 1).min(count));
        }
    }

    /// Scrolls the Albums tab list the least that shows item `ix` whole.
    /// Item heights are fixed by the layout, so off-screen items the list
    /// has not measured yet are placed exactly.
    pub(crate) fn reveal_overview_item(&self, ix: usize) {
        let list = &self.albums.overview.list.state;
        let count = list.item_count();
        if ix >= count {
            return;
        }
        let viewport = f32::from(list.viewport_bounds().size.height);
        if viewport <= 0.0 {
            // Not laid out yet: put the item at the top.
            list.scroll_to(ListOffset {
                item_ix: ix,
                offset_in_item: px(0.0),
            });
            return;
        }
        // ponytail: O(items) sums per call, a few hundred additions on a
        // keypress; keep prefix sums if albums reach the thousands.
        let height = |i: usize| list_item_height(self, i);
        let top_of = |i: usize| (0..i).map(height).sum::<f32>();
        let scroll = list.logical_scroll_top();
        let y = top_of(scroll.item_ix) + f32::from(scroll.offset_in_item);
        let (top, bottom) = (top_of(ix), top_of(ix) + height(ix));
        let target = if top < y {
            top
        } else if bottom > y + viewport {
            bottom - viewport
        } else {
            return;
        };
        let (mut item, mut start) = (0, 0.0);
        while item + 1 < count && start + height(item) <= target {
            start += height(item);
            item += 1;
        }
        list.scroll_to(ListOffset {
            item_ix: item,
            offset_in_item: px(target - start),
        });
    }
}
