use gpui::*;
use lumina_core::SmartAlbum;

use super::nav::{self, nav_entries, NavEntry};
use super::AlbumSelection;
use crate::icons::Icon;
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_library_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px(px(10.0))
                // 5px under the label: this plus the rows gap of 3.
                .pb(px(2.0))
                .child(theme::section_label("LIBRARY"))
                .child(
                    div()
                        .id("change-folder-btn")
                        .debug_selector(|| "switch-library".to_string())
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(theme::RADIUS_SM)
                        .hover(|s| s.bg(theme::bg_2()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .tooltip(theme::tooltip_label("Open Library… (⌘O)"))
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::Folder,
                            theme::ICON_XS,
                            theme::t2(),
                        ))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.pick_folder_dialog(cx);
                        })),
                ),
        )
        .children(
            nav_entries(this)
                .into_iter()
                // Untagged lists under Smart Albums, Places under Tags.
                .filter(|entry| !matches!(entry.key, "untagged" | "places" | "keywords"))
                .map(|entry| render_entry(this, entry, cx)),
        )
        .children(render_downloads_row(this, cx))
}

/// Downloads: shown while the download list has rows (or its screen is
/// selected), with the count of unfinished ones as attention. Opens the
/// Downloads screen; ⌥⌘L keeps the dropdown.
fn render_downloads_row(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<Stateful<Div>> {
    if this.downloads.entries.is_empty() && this.selection != AlbumSelection::Downloads {
        return None;
    }
    let unfinished = this.download_badge_count();
    Some(render_nav_item(
        "downloads",
        "Downloads",
        Icon::ArrowDownToLine,
        unfinished,
        unfinished > 0,
        this.downloads_screen_shown(),
        cx.listener(|this, _e: &ClickEvent, _w, cx| this.show_downloads_screen(cx)),
    ))
}

pub(super) fn render_user_smart_album_row(
    _this: &LuminaApp,
    album: &SmartAlbum,
    count: usize,
    is_active: bool,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let album_id = album.id;
    let edit_album = album.clone();
    theme::nav_item(
        format!("smart-album-row-{album_id}"),
        Icon::Sparkles,
        &album.title,
        count,
        false,
        is_active,
    )
    .group("smart-album-row")
    .child(
        div()
            .id(("smart-album-edit", album_id as u64))
            .debug_selector(move || format!("smart-album-edit-{album_id}"))
            // Row actions appear on hover only (v3 sidebar).
            .opacity(0.0)
            .group_hover("smart-album-row", |s| s.opacity(1.0))
            .px_1()
            .rounded(theme::RADIUS_SM)
            .cursor_pointer()
            .tooltip(theme::tooltip_label("Edit rules"))
            .child(crate::icons::render_icon(
                crate::icons::Icon::Pencil,
                theme::ICON_XS,
                theme::t2(),
            ))
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.open_album_builder_for_edit(edit_album.clone(), cx);
            })),
    )
    .child(
        div()
            .id(("smart-album-delete", album_id as u64))
            .debug_selector(move || format!("smart-album-delete-{album_id}"))
            // Row actions appear on hover only (v3 sidebar).
            .opacity(0.0)
            .group_hover("smart-album-row", |s| s.opacity(1.0))
            .px_1()
            .rounded(theme::RADIUS_SM)
            .cursor_pointer()
            .tooltip(theme::tooltip_label("Delete smart album"))
            .child(crate::icons::render_icon(
                crate::icons::Icon::X,
                theme::ICON_XS,
                theme::t2(),
            ))
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.prompt_delete_smart_album(album_id, cx);
            })),
    )
    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
        this.select_album(AlbumSelection::User(album_id), cx);
    }))
}

pub(super) fn render_smart_albums_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let user_albums = &this.user_smart_albums;

    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px(px(10.0))
                // 5px under the label: this plus the rows gap of 3.
                .pb(px(2.0))
                .child(theme::section_label("SMART ALBUMS"))
                .child(
                    div()
                        .id("new-smart-album-btn")
                        .debug_selector(|| "new-smart-album".to_string())
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(theme::RADIUS_SM)
                        .hover(|s| s.bg(theme::bg_2()).text_color(theme::t1()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .tooltip(theme::tooltip_label("New smart album (⌥⌘N)"))
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::Plus,
                            theme::ICON_XS,
                            theme::t2(),
                        ))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.open_album_builder(cx);
                        })),
                ),
        )
        .children(if user_albums.is_empty() {
            if this.grid_state.is_none() {
                vec![render_nav_item(
                    "sample-smart-album",
                    "Fast Glass",
                    Icon::Sparkles,
                    0,
                    false,
                    false,
                    cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_album_builder(cx);
                    }),
                )
                .into_any_element()]
            } else {
                Vec::new()
            }
        } else {
            user_albums
                .iter()
                // Pinned ones are drawn in the Pinned group instead.
                .filter(|album| !album.pinned)
                .map(|album| {
                    let is_active = this.sidebar_highlights(&AlbumSelection::User(album.id));
                    let count = this.user_album_counts.get(&album.id).copied().unwrap_or(0);
                    render_user_smart_album_row(this, album, count, is_active, cx)
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        // Untagged is a built-in smart album (v3 sidebar).
        .children(
            nav_entries(this)
                .into_iter()
                .filter(|entry| entry.selection == AlbumSelection::Untagged)
                .map(|entry| render_entry(this, entry, cx)),
        )
}

/// TAGS: Keywords (the Tags page) and Places, as in the design. Keywords
/// themselves are listed, renamed and deleted on the Tags page.
pub(super) fn render_tags_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .debug_selector(|| "sidebar-tags-header".to_string())
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px(px(10.0))
                // 5px under the label: this plus the rows gap of 3.
                .pb(px(2.0))
                .child(theme::section_label("TAGS")),
        )
        .children(
            nav_entries(this)
                .into_iter()
                .filter(|entry| matches!(entry.key, "keywords" | "places"))
                .map(|entry| render_entry(this, entry, cx)),
        )
}

fn render_entry(this: &LuminaApp, entry: NavEntry, cx: &mut Context<LuminaApp>) -> AnyElement {
    let is_active = this.sidebar_highlights(&entry.selection);
    let selection = entry.selection;
    render_nav_item(
        entry.key,
        entry.label,
        entry.icon,
        entry.count,
        entry.attention,
        is_active,
        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            nav::open_entry(this, &selection, cx);
        }),
    )
    .into_any_element()
}

/// A NavItem named `sidebar-nav-{key}` (stable whatever the label says).
fn render_nav_item(
    key: &str,
    label: &str,
    icon: Icon,
    count: usize,
    attention: bool,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    theme::nav_item(
        format!("sidebar-nav-{key}"),
        icon,
        label,
        count,
        attention,
        is_active,
    )
    .on_click(on_click)
}
