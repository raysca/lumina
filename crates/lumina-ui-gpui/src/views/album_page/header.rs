//! The album header band: the cover under `header_fade`,
//! the "Albums › title" breadcrumb, Slideshow and More, the title and the
//! counts line, all white over the photo.

use gpui::prelude::FluentBuilder;
use gpui::*;

use super::AlbumMenuItem;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::{popover_under, text_marker};

/// Height of the band; it replaces the page header for an open album.
pub(crate) const ALBUM_HEADER_HEIGHT: f32 = 180.0;

/// A transparent 32-high button over the cover with a 1px `line` outline:
/// radius 8, 13px icon, Inter 12 `t1`. `selector` doubles as the element id.
fn ghost_button(selector: &'static str, label: SharedString) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label)
        .h(px(32.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(7.0))
        .rounded(theme::RADIUS_LG)
        .border_1()
        .border_color(theme::line())
        .hover(|s| s.bg(theme::media_scrim()))
        .cursor_pointer()
        .text_size(theme::TEXT_SMALL)
        .text_color(theme::t1())
}

/// A 32×32 icon-only [`ghost_button`]: `label` is its accessible name and
/// tooltip (with `shortcut`); a scrim fill while `on`.
fn ghost_icon_button(
    selector: &'static str,
    icon: Icon,
    label: impl Into<SharedString>,
    shortcut: Option<&str>,
    on: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let tooltip = match shortcut {
        Some(keys) => format!("{label} ({keys})"),
        None => label.to_string(),
    };
    ghost_button(selector, label)
        .w(px(32.0))
        .relative()
        .when(on, |button| button.bg(theme::media_scrim()))
        .tooltip(theme::tooltip_label(tooltip))
        .child(render_icon(icon, theme::ICON_SM, theme::t1()))
}

/// The page header's Downloads, Add filter, Sort and Info, as ghost
/// buttons over the cover: the album page keeps them, and their popovers
/// hang from here. Selectors match the page header's.
fn view_buttons(this: &LuminaApp, window: &Window, cx: &mut Context<LuminaApp>) -> Vec<AnyElement> {
    let downloads = ghost_icon_button(
        "downloads-toolbar-btn",
        Icon::ArrowDownToLine,
        t("header.downloads"),
        Some("⌥⌘L"),
        this.downloads.open,
    );
    let filter_open = this.filter_menu_under_header();
    let filter = ghost_icon_button(
        "header-filter",
        Icon::SlidersHorizontal,
        t("header.filter"),
        None,
        filter_open,
    )
    .when(filter_open, |button| {
        button.child(popover_under(
            Anchor::TopRight,
            crate::views::filter_bar::filter_menu_card(this, cx),
        ))
    })
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.toggle_filter_menu(false, cx);
    }));
    let sort = ghost_icon_button(
        "sort-menu-button",
        Icon::ArrowUpNarrowWide,
        crate::i18n::t_with("header.sort", &[&this.sort_label()]),
        None,
        this.sort_menu_open,
    )
    .when(this.sort_menu_open, |button| {
        button.child(popover_under(
            Anchor::TopRight,
            crate::views::sort_menu::sort_menu_card(this, cx),
        ))
    })
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.toggle_sort_menu(cx);
    }));
    let info = ghost_icon_button(
        "inspector-toggle",
        Icon::Info,
        t("header.info"),
        Some("I"),
        this.inspector_open,
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.toggle_inspector(cx);
    }));
    this.downloads_button(downloads, window, cx)
        .map(IntoElement::into_any_element)
        .into_iter()
        .chain([
            filter.into_any_element(),
            sort.into_any_element(),
            info.into_any_element(),
        ])
        .collect()
}

/// The band for album `id`.
pub(crate) fn render(
    this: &LuminaApp,
    id: i64,
    window: &Window,
    cx: &mut Context<LuminaApp>,
) -> Div {
    let title: SharedString = this.album_title(id).unwrap_or_default().to_string().into();
    let meta: SharedString =
        crate::i18n::album_meta(this.displayed_count(), this.album_page_stats(id)).into();
    let cover = this
        .album_page_stats(id)
        .and_then(|stats| stats.cover)
        .and_then(|cover| crate::views::albums_overview::albums::thumbnail(this, cover));
    let menu = this.albums.page.menu;
    div()
        .debug_selector(|| "album-header".to_string())
        .relative()
        .w_full()
        .h(px(ALBUM_HEADER_HEIGHT))
        .flex_shrink_0()
        .overflow_hidden()
        // Under the cover while it decodes, and for an empty album.
        .bg(theme::bg_2())
        .children(cover.map(|texture| {
            img(texture)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
        }))
        .child(div().absolute().inset_0().bg(theme::header_fade()))
        .child(
            div()
                .relative()
                .size_full()
                .px(px(28.0))
                // The top row is the title bar band; the title block keeps 20 below.
                .pb(px(20.0))
                // The traffic lights reach past a collapsed sidebar, as in
                // the page header.
                .when(
                    cfg!(target_os = "macos") && this.sidebar_collapsed,
                    |band| band.pl(theme::collapsed_header_inset()),
                )
                .flex()
                .flex_col()
                .justify_between()
                .child(
                    div()
                        .w_full()
                        .h(px(theme::TITLEBAR_HEIGHT))
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        // The frame has no sidebar button; with the sidebar
                        // collapsed, mouse users still need a way back.
                        .when(this.sidebar_collapsed, |row| {
                            row.child(this.render_sidebar_toggle(cx))
                        })
                        .child(breadcrumb(title.clone(), cx))
                        .children(view_buttons(this, window, cx))
                        .child(
                            ghost_button("album-slideshow-btn", t("command.slideshow").into())
                                .px(px(11.0))
                                .tooltip(theme::tooltip_label(t("album_page.slideshow_tooltip")))
                                .child(render_icon(Icon::Play, theme::ICON_SM, theme::t1()))
                                .child(t("command.slideshow"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
                                    this.start_slideshow(window, cx);
                                })),
                        )
                        .child(
                            ghost_icon_button(
                                "album-more-btn",
                                Icon::Ellipsis,
                                t("album_page.more"),
                                None,
                                menu.is_some(),
                            )
                            .when_some(menu, |button, highlight| {
                                button.child(popover_under(
                                    Anchor::TopRight,
                                    menu_card(this, highlight, cx),
                                ))
                            })
                            .on_click(cx.listener(
                                |this, _e: &ClickEvent, _w, cx| {
                                    this.toggle_album_menu(cx);
                                },
                            )),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            // ponytail: the design's -0.6 tracking is skipped;
                            // GPUI 0.3.11 has no letter-spacing style.
                            div()
                                .debug_selector(|| "album-header-title".to_string())
                                .min_w_0()
                                .truncate()
                                .font_family(theme::FONT_DISPLAY)
                                .text_size(theme::TEXT_DISPLAY)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::on_media())
                                .children(text_marker("album-header-title", &title))
                                .child(title),
                        )
                        .child(
                            div()
                                .debug_selector(|| "album-header-meta".to_string())
                                .min_w_0()
                                .truncate()
                                .font_family(theme::FONT_SANS)
                                .font_features(theme::tabular_figures())
                                .text_size(theme::TEXT_CAPTION)
                                .text_color(theme::on_media_meta())
                                .children(text_marker("album-header-meta", &meta))
                                .child(meta),
                        ),
                ),
        )
}

/// "Albums › title": the root opens the Albums overview; the current title
/// truncates before anything else in the row moves.
fn breadcrumb(title: SharedString, cx: &mut Context<LuminaApp>) -> Div {
    let root: SharedString = t("albums.title").into();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(theme::TEXT_SMALL)
        .child(
            div()
                .id("album-crumb-root")
                .debug_selector(|| "album-crumb-root".to_string())
                .role(Role::Link)
                .aria_label(root.clone())
                .tooltip(theme::tooltip_label(t("album_page.crumb_tooltip")))
                .flex_shrink_0()
                .cursor_pointer()
                .text_color(theme::on_media_muted())
                .hover(|s| s.text_color(theme::on_media()))
                .child(root)
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.show_albums_overview(cx);
                })),
        )
        .child(
            render_icon(Icon::ChevronRight, theme::ICON_XS, theme::on_media_muted())
                .flex_shrink_0(),
        )
        .child(
            div()
                .debug_selector(|| "album-crumb-current".to_string())
                .min_w_0()
                .truncate()
                .text_color(theme::on_media())
                .child(title),
        )
}

/// The More menu: Rename, Pin/Unpin, Share… (macOS) and Delete Album….
/// `highlight` is the keyboard row.
fn menu_card(this: &LuminaApp, highlight: usize, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let pinned = this.open_album_pinned().unwrap_or(false);
    theme::menu("album-menu")
        .debug_selector(|| "album-menu".to_string())
        .role(Role::Menu)
        .aria_label(t("album_page.more"))
        .w(px(180.0))
        .occlude()
        .children(
            AlbumMenuItem::all()
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    let selector = item.selector();
                    let enabled = this.album_menu_item_enabled(item);
                    let label = t(match item {
                        AlbumMenuItem::Rename => "command.rename_album",
                        AlbumMenuItem::Pin if pinned => "command.unpin_album",
                        AlbumMenuItem::Pin => "command.pin_album",
                        AlbumMenuItem::Share => "command.share",
                        AlbumMenuItem::Delete => "command.delete_album",
                    });
                    theme::menu_item(selector, i == highlight && enabled)
                        .debug_selector(move || selector.to_string())
                        .role(Role::MenuItem)
                        .aria_label(label.clone())
                        .when(!enabled, |row| {
                            let why = t("album_page.share_tooltip");
                            row.aria_description(why.clone())
                                .text_color(theme::t3())
                                .cursor_default()
                                .tooltip(theme::tooltip_label(why))
                        })
                        .when(item == AlbumMenuItem::Delete && i != highlight, |row| {
                            row.text_color(theme::danger())
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.run_album_menu_item(item, cx);
                        }))
                }),
        )
}
