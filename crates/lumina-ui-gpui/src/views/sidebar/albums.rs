//! The PINNED group and the ALBUMS section: hand-picked albums, above the
//! smart albums. Pinned albums and smart albums leave their own sections
//! for the Pinned group, which comes first. The collapsed rail shows only
//! the default collections, not these.

use gpui::*;
use lumina_core::Album;

use super::nav;
use super::sections::{self, text_color};
use super::AlbumSelection;
use crate::app::text_input::TextTarget;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_albums_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    // Albums live in a library's catalog; the welcome screen has none.
    if this.grid_state.is_none() {
        return Empty.into_any_element();
    }
    div()
        .debug_selector(|| "sidebar-albums-section".to_string())
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .debug_selector(|| "sidebar-albums-header".to_string())
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_2()
                .pt(px(16.0))
                .pb_1()
                .child(theme::section_label(t("sidebar.albums")))
                .child(
                    div()
                        .id("new-album-btn")
                        .debug_selector(|| "new-album".to_string())
                        .role(Role::Button)
                        .aria_label(t("command.new_album"))
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(theme::RADIUS_SM)
                        .hover(|s| s.bg(theme::bg_control()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .tooltip(theme::tooltip_label(t("album.new_tooltip")))
                        .child(render_icon(Icon::Plus, px(12.0), theme::text_subtle()))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.new_album(cx);
                        })),
                ),
        )
        .children(
            this.albums
                .list
                .iter()
                .filter(|album| !album.pinned)
                .map(|album| render_album_or_rename(this, album, cx)),
        )
        .into_any_element()
}

/// Pinned albums, then pinned smart albums; nothing when none is pinned.
pub(super) fn render_pinned_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let albums: Vec<&Album> = this.albums.list.iter().filter(|a| a.pinned).collect();
    let smart: Vec<_> = this.user_smart_albums.iter().filter(|a| a.pinned).collect();
    if this.grid_state.is_none() || (albums.is_empty() && smart.is_empty()) {
        return Empty.into_any_element();
    }
    div()
        .debug_selector(|| "sidebar-pinned-section".to_string())
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .debug_selector(|| "sidebar-pinned-header".to_string())
                .px_2()
                .pt(px(16.0))
                .pb_1()
                .child(theme::section_label(t("sidebar.pinned"))),
        )
        .children(
            albums
                .into_iter()
                .map(|album| render_album_or_rename(this, album, cx)),
        )
        .children(smart.into_iter().map(|album| {
            let is_active = this.selection == AlbumSelection::User(album.id);
            let count = this.user_album_counts.get(&album.id).copied().unwrap_or(0);
            sections::render_user_smart_album_row(this, album, count, is_active, cx)
                .into_any_element()
        }))
        .into_any_element()
}

fn render_album_or_rename(
    this: &LuminaApp,
    album: &Album,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    if this.albums.renaming == Some(album.id) {
        render_rename_row(this, cx)
    } else {
        render_album_row(this, album, cx)
    }
}

fn render_album_row(this: &LuminaApp, album: &Album, cx: &mut Context<LuminaApp>) -> AnyElement {
    let id = album.id;
    let count = this.albums.counts.get(&id).copied().unwrap_or(0);
    let is_active = this.selection == AlbumSelection::Album(id);
    let action = |name: &'static str, tooltip: &'static str, icon: Icon| {
        div()
            .id((name, id as u64))
            .debug_selector(move || format!("{name}-{id}"))
            .role(Role::Button)
            .aria_label(t(tooltip))
            // Row actions appear on hover only, like the smart albums'.
            .opacity(0.0)
            .group_hover("album-row", |s| s.opacity(1.0))
            .px_1()
            .rounded(theme::RADIUS_SM)
            .cursor_pointer()
            .tooltip(theme::tooltip_label(t(tooltip)))
            .child(render_icon(icon, px(11.0), theme::text_subtle()))
    };

    div()
        .id(("album-row", id as u64))
        .group("album-row")
        .debug_selector(move || format!("album-row-{id}"))
        .role(Role::ListItem)
        .aria_label(nav::aria_label(&album.title, count))
        .tooltip(theme::tooltip_label(t("album.row_tooltip")))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .h(px(26.0))
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .bg(if is_active {
            theme::selection_bg()
        } else {
            theme::bg_app()
        })
        .hover(move |s| {
            if is_active {
                s.bg(theme::selection_bg_hover())
            } else {
                s.bg(theme::bg_raised())
            }
        })
        .child(render_icon(Icon::Image, px(16.0), text_color(is_active)).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme::TEXT_BODY)
                .text_color(text_color(is_active))
                .child(album.title.clone()),
        )
        .child(
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(if is_active {
                    theme::selection_count()
                } else {
                    theme::text_subtle()
                })
                .child(count.to_string()),
        )
        .child(
            action("album-rename", "album.rename_tooltip", Icon::Pencil).on_click(cx.listener(
                move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.start_album_rename(id, cx);
                },
            )),
        )
        .child(
            action("album-delete", "album.delete_tooltip", Icon::X).on_click(cx.listener(
                move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.prompt_delete_album(id, cx);
                },
            )),
        )
        .on_click(cx.listener(move |this, e: &ClickEvent, _w, cx| {
            if e.click_count() >= 2 {
                this.start_album_rename(id, cx);
            } else {
                this.select_album(AlbumSelection::Album(id), cx);
            }
        }))
        .into_any_element()
}

/// The inline name field, as the tag rename draws it.
fn render_rename_row(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let button = |name: &'static str, icon: Icon, bg: Rgba, fg: Rgba| {
        div()
            .id(name)
            .debug_selector(move || name.to_string())
            .role(Role::Button)
            .flex_none()
            .px_1p5()
            .py_0p5()
            .rounded(theme::RADIUS_SM)
            .bg(bg)
            .cursor_pointer()
            .child(render_icon(icon, px(10.0), fg))
    };
    div()
        .debug_selector(|| "album-rename-field".to_string())
        .px_2()
        .py_1()
        .rounded(theme::RADIUS_MD)
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::focus_ring())
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(theme::TEXT_BODY)
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text())
                .children(this.field_input_for(TextTarget::AlbumRename)),
        )
        .child(
            button(
                "album-rename-save",
                Icon::Check,
                theme::accent(),
                theme::accent_fg(),
            )
            .aria_label(t("album.rename_tooltip"))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.commit_album_rename(cx);
            })),
        )
        .child(
            button(
                "album-rename-cancel",
                Icon::X,
                theme::bg_control(),
                theme::text_muted(),
            )
            .aria_label(t("album.rename_cancel"))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.cancel_album_rename(cx);
            })),
        )
        .into_any_element()
}
