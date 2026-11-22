//! Smart album cards: the Albums tab's "Smart Albums"
//! section (up to four cards) and the Smart Albums tab (every card). A card
//! is a 2×2 mosaic of the album's first matches, the title, "N items ·
//! live" and a chip per rule. State and keys live in `app/smart_cards.rs`.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::SmartAlbum;

use super::albums::{thumbnail, BODY_PAD_TOP, BODY_PAD_X, EMPTY_HEIGHT, HEAD_GAP, HEAD_HEIGHT};
use crate::app::albums_overview::{AlbumsTab, OverviewFocus};
use crate::app::rule_label::{rule_icon, rule_label};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

const CARD_HEIGHT: f32 = 138.0;
/// Between cards, across and down.
const CARD_GAP: f32 = 14.0;
const MOSAIC_WIDTH: f32 = 110.0;
const TILE_HEIGHT: f32 = 55.0;
const TILE_GAP: f32 = 4.0;
/// Between the smart section and the Albums head (frame: body gap 26).
const SECTION_GAP: f32 = 26.0;
/// Chips on a card before a "+N" chip.
const CHIP_LIMIT: usize = 2;

/// Smart cards per row: four, two in a main column under ~900 wide.
pub(crate) fn smart_columns(pane: f32) -> usize {
    if pane < 900.0 {
        2
    } else {
        4
    }
}

/// `cards` in rows of `columns`, or the one-line empty state.
fn grid_height(cards: usize, columns: usize) -> f32 {
    let rows = cards.div_ceil(columns.max(1)) as f32;
    if cards == 0 {
        EMPTY_HEIGHT
    } else {
        rows * CARD_HEIGHT + (rows - 1.0) * CARD_GAP
    }
}

/// The Albums tab's smart section, the page list's first item.
pub(crate) fn section_height(cards: usize, columns: usize) -> f32 {
    BODY_PAD_TOP + HEAD_HEIGHT + grid_height(cards, columns) + SECTION_GAP
}

/// "Smart Albums" with "Manage rules", then up to four cards.
pub(crate) fn section(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let ids = this.smart_card_order();
    let columns = this.albums.overview.smart_columns;
    div()
        .w_full()
        .h(px(section_height(ids.len(), columns)))
        .pt(px(BODY_PAD_TOP))
        .px(px(BODY_PAD_X))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(HEAD_HEIGHT))
                .pb(px(HEAD_GAP))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id("smart-albums-head")
                        .role(Role::Heading)
                        .font_family(theme::FONT_DISPLAY)
                        .text_size(theme::TEXT_LARGE)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t1())
                        .child(t("albums.tab_smart")),
                )
                .child(
                    div()
                        .id("manage-rules")
                        .debug_selector(|| "manage-rules".to_string())
                        .role(Role::Button)
                        .aria_label(t("albums.manage_rules"))
                        .cursor_pointer()
                        .text_size(theme::TEXT_SMALL)
                        .text_color(theme::accent())
                        .hover(|s| s.text_color(theme::accent_hover()))
                        .child(t("albums.manage_rules"))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.set_albums_tab(AlbumsTab::SmartAlbums, cx);
                        })),
                ),
        )
        .children(rows(this, &ids, cx))
        .into_any_element()
}

/// The Smart Albums tab: every card, wrapping, scrolled as one column.
/// ponytail: not virtualised (a library has a handful of smart albums);
/// make it a list like the Albums tab if hundreds show up.
pub(crate) fn tab(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let ids = this.smart_card_order();
    div()
        .id("smart-albums-grid")
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&this.albums.overview.smart_scroll)
        .pt(px(BODY_PAD_TOP))
        .px(px(BODY_PAD_X))
        .pb(px(BODY_PAD_TOP))
        .flex()
        .flex_col()
        .gap(px(CARD_GAP))
        .children(rows(this, &ids, cx))
        .into_any_element()
}

/// Rows of cards (stretched to fill, a short last row padded), or the
/// empty state with New Smart Album.
fn rows(this: &LuminaApp, ids: &[i64], cx: &mut Context<LuminaApp>) -> Vec<AnyElement> {
    if ids.is_empty() {
        return vec![empty_state(cx)];
    }
    let columns = this.albums.overview.smart_columns.max(1);
    ids.chunks(columns)
        .map(|row| {
            div()
                .w_full()
                .flex_none()
                .flex()
                .flex_row()
                .gap(px(CARD_GAP))
                .children(
                    row.iter()
                        .filter_map(|&id| this.smart_album(id))
                        .map(|album| card(this, album, cx)),
                )
                .children((row.len()..columns).map(|_| div().flex_1().min_w_0()))
                .into_any_element()
        })
        .collect()
}

fn empty_state(cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .debug_selector(|| "smart-empty".to_string())
        .h(px(EMPTY_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .text_size(theme::TEXT_BODY)
        .text_color(theme::t2())
        .child(t("albums.smart_empty"))
        .child(
            div()
                .id("smart-empty-new")
                .debug_selector(|| "smart-empty-new".to_string())
                .role(Role::Button)
                .aria_label(t("command.new_smart_album"))
                .cursor_pointer()
                .text_color(theme::accent())
                .hover(|s| s.text_color(theme::accent_hover()))
                .child(t("command.new_smart_album"))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.open_album_builder(cx);
                })),
        )
        .into_any_element()
}

fn card(this: &LuminaApp, album: &SmartAlbum, cx: &mut Context<LuminaApp>) -> AnyElement {
    let id = album.id;
    let title: SharedString = album.title.clone().into();
    let count = this.user_album_counts.get(&id).copied().unwrap_or(0);
    let meta: SharedString = t("albums.smart_meta")
        .replace("{items}", &crate::i18n::plural("header.items", count))
        .into();
    let focused = this.albums.overview.focus == Some(OverviewFocus::SmartCard(id));
    div()
        .id(("smart-card", id as u64))
        .debug_selector(move || format!("smart-card-{id}"))
        .role(Role::Button)
        .aria_label(format!("{title}, {meta}"))
        .flex_1()
        .min_w_0()
        .h(px(CARD_HEIGHT))
        .p(px(12.0))
        .flex()
        .flex_row()
        .gap(px(13.0))
        .rounded(px(10.0))
        .bg(theme::bg_1())
        // Always a border, so the ring never moves the content.
        .border_1()
        .border_color(if focused {
            Hsla::from(theme::focus_ring())
        } else {
            transparent_black()
        })
        .when(!focused, |card| {
            card.hover(|s| s.border_color(theme::line_2()))
        })
        .cursor_pointer()
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.open_smart_card(id, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                this.open_smart_card_menu(id, e.position, cx);
            }),
        )
        .child(mosaic(this, id))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .id(("smart-card-title", id as u64))
                                .tooltip(theme::tooltip_label(title.clone()))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .child(render_icon(Icon::Sparkles, theme::ICON_SM, theme::accent()))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(theme::TEXT_BODY)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::t1())
                                        .children(text_marker("smart-card-title", &title))
                                        .child(title),
                                ),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_family(theme::FONT_SANS)
                                .font_features(theme::tabular_figures())
                                .text_size(theme::TEXT_CAPTION)
                                .text_color(theme::t3())
                                .children(text_marker("smart-card-meta", &meta))
                                .child(meta),
                        ),
                )
                .child(chips(album)),
        )
        .into_any_element()
}

/// 2×2 tiles of the first four matches; a tile without one stays bg-2.
fn mosaic(this: &LuminaApp, id: i64) -> Div {
    let tiles = this.albums.overview.smart_tiles.get(&id);
    div()
        .w(px(MOSAIC_WIDTH))
        .flex_none()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(TILE_GAP))
        .children((0..4).map(|n| {
            let texture = tiles
                .and_then(|tiles| tiles.get(n))
                .and_then(|&media| thumbnail(this, media));
            div()
                .debug_selector(move || format!("smart-card-tile-{id}-{n}"))
                .w(px((MOSAIC_WIDTH - TILE_GAP) / 2.0))
                .h(px(TILE_HEIGHT))
                .rounded(px(3.0))
                .overflow_hidden()
                .bg(theme::bg_2())
                .children(
                    texture.map(|texture| img(texture).size_full().object_fit(ObjectFit::Cover)),
                )
        }))
}

/// A chip per rule, at most two and a "+N" for the rest.
fn chips(album: &SmartAlbum) -> Div {
    let id = album.id;
    let extra = album.rules.len().saturating_sub(CHIP_LIMIT);
    let chip = |n: usize, icon: Option<Icon>, label: SharedString| {
        div()
            .debug_selector(move || format!("smart-chip-{id}-{n}"))
            .h(px(21.0))
            .px(px(7.0))
            .min_w_0()
            .max_w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .rounded(px(5.0))
            .bg(theme::bg_0())
            .font_family(theme::FONT_SANS)
            .font_features(theme::tabular_figures())
            .text_size(theme::TEXT_CAPTION)
            .text_color(theme::t2())
            .children(icon.map(|icon| render_icon(icon, theme::ICON_XS, theme::t2())))
            .children(text_marker("smart-chip", &label))
            .child(div().min_w_0().truncate().child(label))
    };
    let mut chips: Vec<Div> = album
        .rules
        .iter()
        .take(CHIP_LIMIT)
        .enumerate()
        .map(|(n, rule)| chip(n, Some(rule_icon(&rule.field)), rule_label(rule).into()))
        .collect();
    let more = (extra > 0).then(|| {
        let more = t("albums.more_rules").replace("{n}", &extra.to_string());
        chip(CHIP_LIMIT, None, more.into()).flex_none()
    });
    let last = chips.pop();
    // One chip per line, as the design stacks them; "+N" shares the last line.
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(7.0))
        .children(chips)
        .children(last.map(|last| {
            div()
                .max_w_full()
                .flex()
                .flex_row()
                .gap(px(5.0))
                .child(last)
                .children(more)
        }))
}

/// Edit and Delete for the right-clicked smart card, at the pointer.
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
                theme::menu("smart-card-menu")
                    .debug_selector(|| "smart-card-menu".to_string())
                    .role(Role::Menu)
                    .aria_label(t("albums.smart_card_menu"))
                    .w(px(160.0))
                    .occlude()
                    .child(
                        row("smart-card-menu-edit", t("albums.smart_edit")).on_click(cx.listener(
                            move |this, _e: &ClickEvent, _w, cx| {
                                this.close_overview_menus(cx);
                                if let Some(album) = this.smart_album(album).cloned() {
                                    this.open_album_builder_for_edit(album, cx);
                                }
                            },
                        )),
                    )
                    .child(
                        row("smart-card-menu-delete", t("albums.card_delete")).on_click(
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_overview_menus(cx);
                                this.prompt_delete_smart_album(album, cx);
                            }),
                        ),
                    ),
            ),
    )
    .with_priority(1)
    .into_any_element()
}
