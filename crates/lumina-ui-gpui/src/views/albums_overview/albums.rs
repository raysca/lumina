//! The Albums tab: a page list of the smart section
//! (`smart.rs`), a head row with "Sort ▾", then rows of album cards, a
//! stack of two plates under the cover and a caption. Rows are virtualised;
//! covers come from the thumbnail texture cache. State and keys live in
//! `app/albums_overview.rs` and `app/smart_cards.rs`.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;

use super::smart;
use crate::app::albums_overview::OverviewFocus;
use crate::app::smart_cards::{FIRST_ALBUM_ROW, SMART_ROW_LIMIT};
use crate::config::AlbumSort;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::{popover_under, text_marker};
use crate::LuminaApp;

/// The body's padding (frame: pad 22 28 0 28).
pub(crate) const BODY_PAD_X: f32 = 28.0;
pub(crate) const BODY_PAD_TOP: f32 = 22.0;
/// A one-line empty state, and a section head's line.
pub(crate) const EMPTY_HEIGHT: f32 = 20.0;
/// Under a section head ("Smart Albums", "Albums").
pub(crate) const HEAD_GAP: f32 = 14.0;
/// A section head: its line and the gap under it.
pub(crate) const HEAD_HEIGHT: f32 = EMPTY_HEIGHT + HEAD_GAP;
const CARD_GAP: f32 = 16.0;
const ROW_GAP: f32 = 20.0;
// ponytail: a 200 minimum gives 6 cards of 201 at a 1600 window;
// the frame shows 5 of 245 there, which needs a minimum over 202 and at
// most 244.8 (the 1288 wide body). 230 keeps the frame's 5.
const CARD_MIN_WIDTH: f32 = 230.0;
const STACK_HEIGHT: f32 = 194.0;
const COVER_HEIGHT: f32 = 183.0;
const PLATE_HEIGHT: f32 = 12.0;
const CAPTION_GAP: f32 = 10.0;
/// Title (13) and meta (10) lines with the frame's 4 between them.
const CAPTION_HEIGHT: f32 = 34.0;
const ROW_HEIGHT: f32 = STACK_HEIGHT + CAPTION_GAP + CAPTION_HEIGHT + ROW_GAP;

/// Cards per row in a main column `pane` wide: as many as fit at the
/// minimum width, at least one; the cards stretch to fill the row.
pub(crate) fn card_columns(pane: f32) -> usize {
    let width = pane - 2.0 * BODY_PAD_X;
    (((width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor() as usize).max(1)
}

fn sort_name(sort: AlbumSort) -> String {
    t(match sort {
        AlbumSort::Name => "albums.sort_name",
        AlbumSort::Created => "albums.sort_created",
        AlbumSort::NewestPhoto => "albums.sort_newest",
        AlbumSort::ItemCount => "albums.sort_count",
    })
}

fn sort_selector(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::Name => "albums-sort-name",
        AlbumSort::Created => "albums-sort-created",
        AlbumSort::NewestPhoto => "albums-sort-newest",
        AlbumSort::ItemCount => "albums-sort-count",
    }
}

/// The Albums tab body: one variable-height list, so the smart section,
/// the albums head and the card rows scroll as one page. Rows outside the
/// viewport (and its overdraw) are not built.
pub(crate) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let app = cx.entity().downgrade();
    list(
        this.albums.overview.list.state.clone(),
        move |ix, _window, cx| {
            app.update(cx, |this, cx| list_item(this, ix, cx))
                .unwrap_or_else(|_| Empty.into_any_element())
        },
    )
    .size_full()
    .into_any_element()
}

/// The list's item `ix`, `list_item_height` tall.
fn list_item(this: &LuminaApp, ix: usize, cx: &mut Context<LuminaApp>) -> AnyElement {
    let empty = this.albums.list.is_empty();
    match ix {
        0 => smart::section(this, cx),
        1 => head_row(this, empty, cx).into_any_element(),
        _ if empty => empty_state(cx).into_any_element(),
        _ => album_row(this, ix - FIRST_ALBUM_ROW, cx),
    }
}

/// Item heights are fixed, so the page can scroll to a row it has not
/// measured yet (see `reveal_overview_item`).
pub(crate) fn list_item_height(this: &LuminaApp, ix: usize) -> f32 {
    match ix {
        0 => smart::section_height(
            this.user_smart_albums.len().min(SMART_ROW_LIMIT),
            this.albums.overview.smart_columns,
        ),
        1 => HEAD_HEIGHT,
        _ if this.albums.list.is_empty() => EMPTY_HEIGHT,
        _ => ROW_HEIGHT,
    }
}

fn album_row(this: &LuminaApp, row: usize, cx: &mut Context<LuminaApp>) -> AnyElement {
    let order = &this.albums.overview.card_order;
    let columns = this.albums.overview.columns.max(1);
    let start = (row * columns).min(order.len());
    let ids = &order[start..(start + columns).min(order.len())];
    div()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px(px(BODY_PAD_X))
        .pb(px(ROW_GAP))
        .flex()
        .flex_row()
        .gap(px(CARD_GAP))
        .children(ids.iter().map(|&id| album_card(this, id, cx)))
        // Empty cells keep a short last row's cards as wide as the rest.
        .children((ids.len()..columns).map(|_| div().flex_1().min_w_0()))
        .into_any_element()
}

/// "Albums" on the left; "Sort: Name ▾" on the right while there are cards.
fn head_row(this: &LuminaApp, empty: bool, cx: &mut Context<LuminaApp>) -> Div {
    let overview = &this.albums.overview;
    let focused = overview.focus == Some(OverviewFocus::Sort);
    let label = t("albums.sort").replace("{field}", &sort_name(this.config.album_sort));
    div()
        .w_full()
        .h(px(HEAD_HEIGHT))
        .px(px(BODY_PAD_X))
        .pb(px(HEAD_GAP))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .id("albums-head")
                .role(Role::Heading)
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(t("albums.title")),
        )
        .when(!empty, |row| {
            row.child(
                div()
                    .id("albums-sort")
                    .debug_selector(|| "albums-sort".to_string())
                    .role(Role::Button)
                    .aria_label(label.clone())
                    .relative()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(theme::RADIUS_SM)
                    .border_1()
                    .border_color(if focused {
                        Hsla::from(theme::focus_ring())
                    } else {
                        transparent_black()
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::bg_2()))
                    .text_size(px(11.5))
                    .text_color(theme::accent())
                    .child(format!("{label} ▾"))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_album_sort_menu(cx);
                    }))
                    .children(overview.sort_menu.map(|highlight| {
                        popover_under(Anchor::TopRight, sort_menu(this, highlight, cx))
                    })),
            )
        })
}

fn sort_menu(this: &LuminaApp, highlight: usize, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    theme::menu("albums-sort-menu")
        .debug_selector(|| "albums-sort-menu".to_string())
        .role(Role::Menu)
        .aria_label(t("albums.sort_menu"))
        .w(px(180.0))
        .occlude()
        .children(AlbumSort::ALL.iter().enumerate().map(|(i, &sort)| {
            let selector = sort_selector(sort);
            let name = sort_name(sort);
            theme::menu_item(selector, i == highlight)
                .debug_selector(move || selector.to_string())
                .role(Role::MenuItemRadio)
                .aria_label(name.clone())
                .aria_selected(this.config.album_sort == sort)
                .child(name)
                .child(if this.config.album_sort == sort {
                    "✓"
                } else {
                    ""
                })
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_album_sort(sort, cx);
                }))
        }))
}

/// The one-line state for a library without albums, with New Album.
fn empty_state(cx: &mut Context<LuminaApp>) -> Div {
    div()
        .debug_selector(|| "albums-empty".to_string())
        .h(px(EMPTY_HEIGHT))
        .px(px(BODY_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .text_size(theme::TEXT_BODY)
        .text_color(theme::t2())
        .child(t("albums.empty"))
        .child(
            div()
                .id("albums-empty-new")
                .debug_selector(|| "albums-empty-new".to_string())
                .role(Role::Button)
                .aria_label(t("command.new_album"))
                .cursor_pointer()
                .text_color(theme::accent())
                .hover(|s| s.text_color(theme::accent_hover()))
                .child(t("command.new_album"))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.new_album(cx);
                })),
        )
}

/// "284 items · Jun 2025", the count alone without a dated cover.
fn card_meta(count: usize, newest_epoch_ms: Option<i64>) -> String {
    use chrono::Datelike;
    let items = crate::i18n::items(count);
    match newest_epoch_ms.and_then(chrono::DateTime::from_timestamp_millis) {
        Some(date) => t("header.meta").replace("{items}", &items).replace(
            "{range}",
            &crate::i18n::month_year(date.year(), date.month()),
        ),
        None => items,
    }
}

fn album_card(this: &LuminaApp, id: i64, cx: &mut Context<LuminaApp>) -> AnyElement {
    let title: SharedString = this.album_title(id).unwrap_or_default().to_string().into();
    let card = this.albums.overview.cards.get(&id);
    let cover = card.and_then(|card| card.cover);
    let meta: SharedString = card_meta(
        this.albums.counts.get(&id).copied().unwrap_or(0),
        card.and_then(|card| card.newest_epoch_ms),
    )
    .into();
    let focused = this.albums.overview.focus == Some(OverviewFocus::Card(id));
    let texture = cover.and_then(|cover| thumbnail(this, cover));
    let plate = |inset: f32, top: f32, color: Rgba| {
        div()
            .absolute()
            .top(px(top))
            .left(px(inset))
            .right(px(inset))
            .h(px(PLATE_HEIGHT))
            .rounded(theme::RADIUS_SM)
            .bg(color)
    };
    div()
        .id(("album-card", id as u64))
        .debug_selector(move || format!("album-card-{id}"))
        .role(Role::Button)
        .aria_label(format!("{title}, {meta}"))
        .group("album-card")
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(CAPTION_GAP))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.open_album_card(id, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                this.open_album_card_menu(id, e.position, cx);
            }),
        )
        .child(
            // The stack: Back2 (217 of 245) and Back1 (231) peek above the
            // cover, which sits 11 down.
            div()
                .relative()
                .w_full()
                .h(px(STACK_HEIGHT))
                .child(plate(14.0, 0.0, theme::plate()))
                .child(plate(7.0, 5.0, theme::plate_2()))
                .child(
                    div()
                        .debug_selector(move || format!("album-card-cover-{id}"))
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(COVER_HEIGHT))
                        .rounded(theme::RADIUS_MD)
                        .overflow_hidden()
                        .bg(theme::bg_2())
                        // Always a border, so the ring never moves the cover.
                        .border_2()
                        .border_color(if focused {
                            Hsla::from(theme::focus_ring())
                        } else {
                            transparent_black()
                        })
                        .when(!focused, |cover| {
                            cover.group_hover("album-card", |s| s.border_color(theme::line_2()))
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(match (cover, texture) {
                            (_, Some(texture)) => img(texture)
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .into_any_element(),
                            (None, None) => div()
                                .debug_selector(move || format!("album-card-placeholder-{id}"))
                                .child(render_icon(Icon::Folder, px(28.0), theme::t3()))
                                .into_any_element(),
                            // Still decoding: the bg-2 well alone.
                            (Some(_), None) => Empty.into_any_element(),
                        }),
                ),
        )
        .child(
            div()
                .h(px(CAPTION_HEIGHT))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .id(("album-card-title", id as u64))
                        .tooltip(theme::tooltip_label(title.clone()))
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .children(text_marker("album-card-title", &title))
                        .child(title),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(10.0))
                        .text_color(theme::t3())
                        .children(text_marker("album-card-meta", &meta))
                        .child(meta),
                ),
        )
        .into_any_element()
}

/// A thumbnail from the texture cache, queued for decoding while missing.
pub(crate) fn thumbnail(this: &LuminaApp, id: lumina_core::MediaId) -> Option<Arc<RenderImage>> {
    let texture = this
        .grid_state
        .as_ref()
        .and_then(|state| state.texture_cache.lock().get(&id).cloned());
    if texture.is_none() {
        this.queue_thumbnail_request(id);
    }
    texture
}

/// Rename and Delete for the right-clicked card, at the pointer.
pub(crate) fn card_menu(
    album: i64,
    position: Point<Pixels>,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let row = |selector: &'static str, label: String| {
        theme::menu_item(selector, false)
            .debug_selector(move || selector.to_string())
            .role(Role::MenuItem)
            .aria_label(label.clone())
            .child(label)
    };
    deferred(
        anchored()
            .position(position)
            .snap_to_window_with_margin(px(8.0))
            .child(
                theme::menu("album-card-menu")
                    .debug_selector(|| "album-card-menu".to_string())
                    .role(Role::Menu)
                    .aria_label(t("albums.card_menu"))
                    .w(px(160.0))
                    .occlude()
                    .child(
                        row("album-card-menu-rename", t("albums.card_rename")).on_click(
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_overview_menus(cx);
                                this.start_album_rename(album, cx);
                            }),
                        ),
                    )
                    .child(
                        row("album-card-menu-delete", t("albums.card_delete")).on_click(
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_overview_menus(cx);
                                this.prompt_delete_album(album, cx);
                            }),
                        ),
                    ),
            ),
    )
    .with_priority(1)
    .into_any_element()
}
