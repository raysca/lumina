//! The Albums overview page: how it is reached, its tabs and its keyboard
//! focus. The view is `views/albums_overview`.

use gpui::*;

use crate::app::LuminaApp;
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
}

/// The overview's tab (kept between visits) and keyboard focus.
#[derive(Debug, Default)]
pub struct OverviewState {
    pub tab: AlbumsTab,
    pub focus: Option<OverviewFocus>,
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
            self.albums.overview.focus = None;
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

    pub(crate) fn set_albums_tab(&mut self, tab: AlbumsTab, cx: &mut Context<Self>) {
        self.albums.overview.tab = tab;
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

    /// Overview keys: Tab / ⇧Tab move the focus between the tab bar and
    /// New Album, ←/→ switch tabs while the bar has it, ↵ runs New Album,
    /// Esc clears the focus. Every key that reaches it stops here: there is
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
        let focus = self.albums.overview.focus;
        match keystroke.key.as_str() {
            "tab" if plain => {
                self.albums.overview.focus = Some(match focus {
                    Some(OverviewFocus::Tabs) => OverviewFocus::NewAlbum,
                    Some(OverviewFocus::NewAlbum) => OverviewFocus::Tabs,
                    None if m.shift => OverviewFocus::NewAlbum,
                    None => OverviewFocus::Tabs,
                });
                cx.notify();
            }
            "left" | "right" if plain && !m.shift && focus == Some(OverviewFocus::Tabs) => {
                let at = AlbumsTab::ALL
                    .iter()
                    .position(|tab| *tab == self.albums.overview.tab)
                    .unwrap_or(0);
                let next = if keystroke.key == "left" {
                    at.saturating_sub(1)
                } else {
                    (at + 1).min(AlbumsTab::ALL.len() - 1)
                };
                self.set_albums_tab(AlbumsTab::ALL[next], cx);
            }
            "enter" | "return" if plain && focus == Some(OverviewFocus::NewAlbum) => {
                self.new_album(cx);
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
