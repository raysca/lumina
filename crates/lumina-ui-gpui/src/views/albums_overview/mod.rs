//! The Albums overview page: the tab bar and body under
//! the page header while `AlbumSelection::AlbumsOverview` shows. State and
//! keys live in `app/albums_overview.rs`.

pub(crate) mod albums;
pub(crate) mod smart;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::albums_overview::{AlbumsTab, OverviewFocus};
use crate::i18n::t;
use crate::icons::Icon;
use crate::theme;
use crate::views::page_header::{header_primary_button, page_tabs, PageTab};
use crate::LuminaApp;

/// The page under the header: tabs, then the body: the smart row and the
/// album cards on the Albums tab, every smart card on the Smart Albums
/// tab; and the open card menu with its dismiss backdrop.
pub(crate) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let overview = &this.albums.overview;
    let menu_open = overview.sort_menu.is_some() || overview.card_menu.is_some();
    let tabs: Vec<PageTab> = AlbumsTab::ALL
        .iter()
        .map(|&tab| {
            let (label, count) = match tab {
                AlbumsTab::Albums => (t("albums.title"), this.albums.list.len()),
                AlbumsTab::SmartAlbums => (t("albums.tab_smart"), this.user_smart_albums.len()),
            };
            PageTab {
                id: tab.id(),
                label: label.into(),
                count,
                active: overview.tab == tab,
            }
        })
        .collect();
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme::bg_0())
        .child(page_tabs(
            t("albums.title"),
            &tabs,
            overview.focus == Some(OverviewFocus::Tabs),
            cx.listener(|this, id: &&'static str, _w, cx| {
                if let Some(tab) = AlbumsTab::ALL.into_iter().find(|tab| tab.id() == *id) {
                    this.set_albums_tab(tab, cx);
                }
            }),
        ))
        .child(
            div()
                .id("albums-overview")
                .debug_selector(|| "albums-overview".to_string())
                .relative()
                .flex_1()
                .min_h_0()
                .child(match overview.tab {
                    AlbumsTab::Albums => albums::render(this, cx),
                    AlbumsTab::SmartAlbums => smart::tab(this, cx),
                })
                .when(menu_open, |body| {
                    body.child(div().absolute().inset_0().occlude().on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                            this.close_overview_menus(cx);
                        }),
                    ))
                })
                .children(overview.card_menu.map(|menu| match menu.smart {
                    true => smart::card_menu(menu.album, menu.position, cx),
                    false => albums::card_menu(menu.album, menu.position, cx),
                })),
        )
        .into_any_element()
}

/// The header's primary New Album, the same flow as ⌘N; ↵ runs it while
/// Tab has focused it.
pub(crate) fn new_album_button(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let focused = this.albums.overview.focus == Some(OverviewFocus::NewAlbum);
    header_primary_button(
        "new-album-page-btn",
        Icon::FolderPlus,
        t("command.new_album"),
        t("album.new_tooltip"),
    )
    .border_1()
    .border_color(if focused {
        theme::focus_ring()
    } else {
        theme::accent()
    })
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.new_album(cx);
    }))
}
