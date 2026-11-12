//! The media card: one browser tile with its thumbnail, badges, check
//! circle and selection ring.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::types::format_duration_badge;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::grid::{
    media_card_tooltip, ALBUM_TILE_RADIUS, CAPTION_HEIGHT, CARD_PADDING, COMPACT_TILE,
    UNSELECTED_CARD_OPACITY,
};
use crate::views::marks::{flag_badge, REJECTED_OPACITY};
use crate::views::sidebar::AlbumSelection;

/// Inset of the badges from the tile's edges.
const BADGE_INSET: f32 = 8.0;
/// Height of the pill badges (duration, RAW, meta).
const BADGE_H: f32 = 18.0;
/// How far the selection ring reaches outside the tile: a 1px gap plus the
/// 2px border. Half the 6px tile gap, so adjacent selected rings never meet.
const RING_OUTSET: f32 = 3.0;
/// Corner radius of a captioned Videos thumb (frame `DoXZy`: 5; the theme
/// has 4 and 6 only).
const VIDEO_THUMB_RADIUS: f32 = 5.0;

impl LuminaApp {
    /// Renders the card for `item` at view position `index`. `size` is the
    /// photo well: its height sets the card's height and its smaller side
    /// picks the compact badge set. The card's width comes from its parent.
    /// Where [`Self::captioned_tiles`] holds, a caption follows the well and
    /// the selection outline sits on the well itself.
    pub(crate) fn render_media_card(
        &mut self,
        item: &MediaItem,
        index: usize,
        size: Size<f32>,
        tex: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item_id_num = item.id.0 as usize;
        let is_selected = self.is_index_selected(index)
            || self
                .selected_item
                .as_ref()
                .map(|i| i.id == item.id)
                .unwrap_or(false);
        let is_fav = item.is_favorite;
        let item_rating = self.get_item_rating(item);
        let item_color = self.get_item_color_label(item);
        let is_video = item.media_type == MediaType::Video;
        let is_missing = self.missing_items.contains(&item.id);
        let is_edited = self.edits.edited_ids.contains(&item.id);
        let flag = self.item_flag(item.id);
        let is_raw = item.media_type == MediaType::Raw;
        // Selection mode: two or more selected. A single plain selection is browsing and dims
        // nothing.
        let selection_mode = self.selected_indices.len() > 1;
        let card_opacity = if selection_mode && !is_selected {
            UNSELECTED_CARD_OPACITY
        } else {
            1.0
        };

        let item_for_click = item.clone();
        let item_for_context = item.clone();
        let dur_text = format_duration_badge(item.duration_secs());
        let has_duration = is_video && !dur_text.is_empty();
        let has_type_badge = has_duration || is_raw;
        let captioned = self.captioned_tiles();
        let in_pip = self.video.in_pip && self.video.item.as_ref().is_some_and(|v| v.id == item.id);
        // COMPACT_TILE guards the top badge rows' width. A captioned 16:9 thumb
        // is wide but short; its ~120px still clears the top and bottom rows.
        let compact = if captioned {
            size.width
        } else {
            size.width.min(size.height)
        } < COMPACT_TILE;
        let show_fav = is_fav && !compact;
        let show_edited = is_edited && !compact;
        // The open album page's cover, from its cached stats.
        let is_key_photo = !compact && self.albums.page.shown_cover == Some(item.id);
        // An open album's tiles always fill (Design 07 `w4zpqh`); the Fit
        // toggle is Library's and is not offered there.
        let album_tiles = self.album_tiles();
        let fit_mode = self.card_fit_mode && !album_tiles;
        let (well_radius, ring_outset) = if captioned {
            (px(VIDEO_THUMB_RADIUS), 0.0)
        } else if album_tiles {
            (px(ALBUM_TILE_RADIUS), RING_OUTSET)
        } else {
            (theme::RADIUS_SM, RING_OUTSET)
        };
        let in_trash = self.selection == AlbumSelection::Trash;
        let has_texture = tex.is_some();
        let thumb_failed = !has_texture
            && self
                .grid_state
                .as_ref()
                .is_some_and(|s| s.is_failed(item.id));
        let fresh_thumb = has_texture
            && self
                .grid_state
                .as_ref()
                .is_some_and(|s| s.thumb_is_fresh(item.id));
        let photo_background: Background = if has_texture {
            theme::card_surface().into()
        } else {
            linear_gradient(
                135.0,
                linear_color_stop(theme::bg_2(), 0.0),
                linear_color_stop(theme::bg_0(), 1.0),
            )
        };

        div()
            .id(item_id_num)
            // Stable automation contract for card state:
            // media-card-<id>, media-card-photo-<id>,
            // media-card-skeleton-<id>, media-card-thumb-<id>,
            // media-card-failed-<id>,
            // media-card-selected-<id>.
            .debug_selector(move || format!("media-card-{}", item_id_num))
            .flex_1()
            // An img's min-content width is its height times its aspect; without this a landscape
            // thumbnail widens its column.
            .min_w_0()
            .h(px(size.height
                + 2.0 * CARD_PADDING
                + if captioned { CAPTION_HEIGHT } else { 0.0 }))
            .p(px(CARD_PADDING))
            .relative()
            // No card surface and no border: the thumbnail sits directly on the library
            // background (FIXES.md). Badges sit on the photo; selection adds an accent ring and,
            // in selection mode, check circles.
            .bg(theme::card_surface())
            .map(|card| {
                // The hook records what each branch applies.
                #[cfg(test)]
                let record = |o: f32| {
                    crate::views::grid::CARD_OPACITY.with(|m| {
                        m.borrow_mut().insert(item_id_num, o);
                    })
                };
                if card_opacity < 1.0 {
                    #[cfg(test)]
                    record(card_opacity);
                    card.opacity(card_opacity)
                } else {
                    #[cfg(test)]
                    record(1.0);
                    card
                }
            })
            .cursor_pointer()
            // The full filename lives on the tooltip now that the card no longer prints it below
            // the photo.
            .tooltip(media_card_tooltip(item.file_name.clone()))
            .flex()
            .flex_col()
            .justify_between()
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                // Album cover pick mode takes the click (and its double-click).
                if this.cover_pick_click(item_for_click.id, event.click_count(), cx) {
                    return;
                }
                // A sweep that ends over this card must not collapse the selection it just built.
                if this.consume_grid_drag_click() {
                    return;
                }
                let mods = window.modifiers();
                let is_shift = mods.shift;
                let is_cmd = mods.secondary(cx) || mods.control;
                if !is_shift && !is_cmd && event.click_count() >= 2 {
                    this.open_viewer(item_for_click.clone(), cx);
                } else {
                    this.handle_card_click(item_for_click.clone(), index, is_shift, is_cmd, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    // Picking a cover starts no sweep.
                    if this.cover_picking() {
                        cx.stop_propagation();
                        return;
                    }
                    let mods = window.modifiers();
                    let additive = mods.shift
                        || mods.secondary(cx)
                        || mods.control
                        || (this.selected_indices.len() > 1
                            && this.selected_indices.contains(&index));
                    this.begin_grid_drag(event.position, Some(index), additive);
                    // Keep the press away from the grid background's own drag start.
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_move(
                cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.drag_over_card(index, event.position, cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                    this.open_card_context_menu(item_for_context.clone(), index, e.position, cx);
                }),
            )
            // Inner Photo Container. The tinted gradient is always present so a decoded thumbnail
            // replaces the skeleton without a black flash or a layout jump.
            .child(
                div()
                    .debug_selector(move || format!("media-card-photo-{}", item_id_num))
                    .w_full()
                    // Photo well (r4): the height comes from `size` (square in Grid).
                    .h(px(size.height))
                    .flex_shrink_0()
                    // The loading gradient is only needed while the thumbnail is missing; a
                    // decoded image has no background around it.
                    .bg(photo_background)
                    // Recently Deleted reads as set aside.
                    .when(in_trash, |well| well.opacity(0.6))
                    .when(flag == lumina_core::Flag::Reject, |well| {
                        well.opacity(REJECTED_OPACITY)
                    })
                    // Tiles are r4 (design 01 Library); Videos thumbs r5.
                    .rounded(well_radius)
                    .overflow_hidden()
                    .relative()
                    .child(if let Some(render_img) = tex {
                        #[cfg(test)]
                        crate::views::grid::CARD_CONTAIN.with(|m| {
                            m.borrow_mut().insert(item_id_num, fit_mode);
                        });
                        let thumb = div()
                            .debug_selector(move || format!("media-card-thumb-{}", item_id_num))
                            .size_full()
                            .child(img(render_img).size_full().object_fit(if fit_mode {
                                ObjectFit::Contain
                            } else {
                                ObjectFit::Cover
                            }));
                        // A just-decoded image fades in over the skeleton.
                        if fresh_thumb {
                            crate::motion::fade_in(
                                thumb,
                                ElementId::NamedInteger("thumb-in".into(), item_id_num as u64),
                                crate::motion::THUMB_IN,
                            )
                            .into_any_element()
                        } else {
                            thumb.into_any_element()
                        }
                    } else if thumb_failed {
                        // The decode failed for good: say so in text (not only in a tooltip) and
                        // look unlike the skeleton.
                        div()
                            .id(("media-card-failed", item_id_num as u64))
                            .debug_selector(move || format!("media-card-failed-{}", item_id_num))
                            .size_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(6.0))
                            .tooltip(theme::tooltip_label(crate::i18n::t("grid.preview_failed")))
                            .child(
                                div()
                                    .size(px(34.0))
                                    .rounded_full()
                                    .bg(theme::bg_2())
                                    .border_1()
                                    .border_color(theme::line())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(render_icon(Icon::TriangleAlert, px(17.0), theme::t2())),
                            )
                            .child(
                                div()
                                    .debug_selector(move || {
                                        format!("media-card-failed-caption-{}", item_id_num)
                                    })
                                    .text_xs()
                                    .text_color(theme::t2())
                                    .child(crate::i18n::t("grid.no_preview")),
                            )
                            .into_any_element()
                    } else {
                        // Static skeleton placeholder. The seed fixtures decode to no texture, so
                        // this is also the automation signal for "not yet decoded"
                        // (media-card-skeleton-<id>).
                        div()
                            .debug_selector(move || format!("media-card-skeleton-{}", item_id_num))
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .size(px(34.0))
                                    .rounded_full()
                                    .bg(theme::bg_2())
                                    .border_1()
                                    .border_color(theme::line())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(render_icon(
                                        if is_video { Icon::Film } else { Icon::Image },
                                        px(17.0),
                                        theme::t2(),
                                    )),
                            )
                            .into_any_element()
                    })
                    // Meta pill (bottom-left): stars and colour label. It sits above the duration
                    // or RAW badge so the two never collide.
                    .child(if !compact && (item_rating > 0 || item_color.is_some()) {
                        div()
                            .debug_selector(move || format!("media-card-meta-{}", item_id_num))
                            .absolute()
                            .left(px(BADGE_INSET))
                            .bottom(px(if has_type_badge {
                                BADGE_INSET + BADGE_H + 4.0
                            } else {
                                BADGE_INSET
                            }))
                            .h(px(BADGE_H))
                            .px(px(5.0))
                            .rounded(theme::RADIUS_SM)
                            .bg(theme::media_scrim())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(5.0))
                            .when(item_rating > 0, |d| {
                                d.child(div().flex().flex_row().gap(px(1.0)).children(
                                    (0..item_rating).map(|_| {
                                        render_icon(Icon::StarFilled, px(9.0), theme::on_media())
                                    }),
                                ))
                            })
                            .when_some(item_color, |d, c| d.child(theme::label_dot(c)))
                            .into_any_element()
                    } else {
                        Empty.into_any_element()
                    })
                    // Top-left row: the check circle in selection mode, the key photo badge, then
                    // the edited badge.
                    .when(selection_mode || show_edited || is_key_photo, |well| {
                        well.child(
                            badge_row()
                                .left(px(BADGE_INSET))
                                .when(selection_mode, |row| {
                                    let item = item.clone();
                                    row.child(
                                        check_circle(item_id_num, is_selected)
                                            // Keep the press from starting the card's marquee
                                            // drag.
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation()
                                            })
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _w, cx| {
                                                    cx.stop_propagation();
                                                    // The ⌘-click path: toggle this item.
                                                    this.handle_card_click(
                                                        item.clone(),
                                                        index,
                                                        false,
                                                        true,
                                                        cx,
                                                    );
                                                },
                                            )),
                                    )
                                })
                                .when(is_key_photo, |row| {
                                    row.child(crate::views::album_page::key_photo::badge(
                                        item_id_num as u64,
                                    ))
                                })
                                .when(show_edited, |row| row.child(edited_badge(item_id_num))),
                        )
                    })
                    // Top-right row, left to right: flag, favourite heart, missing file. One flex
                    // row, so no mark can cover another.
                    .when(
                        show_fav || is_missing || flag != lumina_core::Flag::Unflagged,
                        |well| {
                            well.child(
                                badge_row()
                                    .right(px(BADGE_INSET))
                                    .child(flag_badge(item_id_num, flag))
                                    .when(show_fav, |row| row.child(fav_badge(item_id_num)))
                                    .when(is_missing, |row| row.child(missing_badge(item_id_num))),
                            )
                        },
                    )
                    // Video duration (bottom-left); the clip playing in PiP
                    // shows audio lines and accent text instead.
                    .when(has_duration, |well| {
                        well.child(duration_badge(item_id_num, dur_text, in_pip))
                    })
                    // RAW (bottom-right).
                    .when(is_raw, |well| well.child(raw_badge(item_id_num))),
            )
            .when(captioned, |card| {
                card.child(super::video_caption::video_caption(item, is_selected))
            })
            // Selection: a 2px amber ring with a 1px gap outside the r4 well; its r7 corners (4 +
            // RING_OUTSET) stay concentric. A captioned thumb instead takes the 2px outline on
            // itself (design 04). It sits on the card because the well clips.
            .when(is_selected, |card| {
                card.child(
                    div()
                        .debug_selector(move || format!("media-card-selected-{}", item_id_num))
                        .absolute()
                        .top(px(CARD_PADDING - ring_outset))
                        // left/right trace the well's real width, which the parent sets.
                        .left(px(CARD_PADDING - ring_outset))
                        .right(px(CARD_PADDING - ring_outset))
                        .h(px(size.height + 2.0 * ring_outset))
                        .rounded(well_radius + px(ring_outset))
                        .border_2()
                        .border_color(theme::accent()),
                )
            })
            .into_any_element()
    }
}

/// An absolute row of badges along a tile's top edge; callers pin it
/// left or right. A flex row keeps its badges from covering each other.
fn badge_row() -> Div {
    div()
        .absolute()
        .top(px(BADGE_INSET))
        .h(px(20.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
}

/// A 20 px square badge on the media scrim with a tooltip and an image role.
fn square_badge(id: ElementId, label: &'static str, icon: Icon, color: Rgba) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Image)
        .aria_label(label)
        .size(px(20.0))
        .rounded(theme::RADIUS_SM)
        .bg(theme::media_scrim())
        .flex()
        .items_center()
        .justify_center()
        .tooltip(theme::tooltip_label(label))
        .child(render_icon(icon, px(12.0), color))
}

/// Badge for an item with Lumina edits or a rotation; reads the cached
/// `edited_ids`, never the catalog.
fn edited_badge(item_id_num: usize) -> impl IntoElement {
    square_badge(
        ("grid-edited-badge", item_id_num as u64).into(),
        "Edited",
        Icon::Sliders,
        theme::on_media(),
    )
    .debug_selector(move || format!("grid-edited-badge-{item_id_num}"))
}

/// Badge for an item whose file is missing.
fn missing_badge(item_id_num: usize) -> impl IntoElement {
    square_badge(
        ("grid-missing-badge", item_id_num as u64).into(),
        "File is missing",
        Icon::TriangleAlert,
        theme::danger(),
    )
    .debug_selector(move || format!("grid-missing-badge-{item_id_num}"))
}

/// Favourite: an outline heart on the photo with no backing (design 01).
fn fav_badge(item_id_num: usize) -> impl IntoElement {
    div()
        .id(("media-card-fav", item_id_num as u64))
        .debug_selector(move || format!("media-card-fav-{item_id_num}"))
        .role(Role::Image)
        .aria_label("Favorite")
        .child(render_icon(Icon::Heart, px(14.0), theme::on_media()))
}

/// Selection-mode check circle: accent with a check when selected, a
/// faint circle otherwise. The caller wires the click (a ⌘-click toggle).
fn check_circle(item_id_num: usize, selected: bool) -> Stateful<Div> {
    div()
        .id(("media-card-check", item_id_num as u64))
        .debug_selector(move || format!("media-card-check-{item_id_num}"))
        .role(Role::CheckBox)
        .aria_label("Select")
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .size(px(22.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |d| {
            d.bg(theme::accent()).child(
                div()
                    .debug_selector(move || format!("media-card-check-mark-{item_id_num}"))
                    .child(render_icon(Icon::Check, px(13.0), theme::on_accent())),
            )
        })
        .when(!selected, |d| d.bg(theme::check_idle()))
}

/// A mono pill badge pinned to the bottom of the tile (duration, RAW).
fn pill_badge(id: ElementId) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Image)
        .absolute()
        .bottom(px(BADGE_INSET))
        .h(px(BADGE_H))
        .px(px(6.0))
        .rounded(theme::RADIUS_SM)
        .bg(theme::media_scrim())
        .font_family(theme::FONT_MONO)
        .text_size(px(9.5))
        .flex()
        .flex_row()
        .items_center()
}

/// Video duration, bottom-left: play icon and the mono time. The clip
/// playing in PiP (`in_pip`) shows audio lines and the time in accent
/// (design 04); its icon carries `media-card-playing-<id>` (an element has
/// one debug selector, and the pill keeps `media-card-duration-<id>`).
fn duration_badge(item_id_num: usize, text: String, in_pip: bool) -> impl IntoElement {
    let (icon, color, label) = if in_pip {
        (Icon::AudioLines, theme::accent(), "grid.badge_playing")
    } else {
        (Icon::Play, theme::on_media(), "grid.badge_video")
    };
    pill_badge(("media-card-duration", item_id_num as u64).into())
        .debug_selector(move || format!("media-card-duration-{item_id_num}"))
        .aria_label(crate::i18n::t_with(label, &[&text]))
        .left(px(BADGE_INSET))
        .gap(px(4.0))
        .text_color(color)
        .child(
            div()
                .when(in_pip, |icon| {
                    icon.debug_selector(move || format!("media-card-playing-{item_id_num}"))
                })
                .child(render_icon(icon, px(9.0), color)),
        )
        .child(text)
}

/// RAW file, bottom-right: "RAW" in mono accent.
fn raw_badge(item_id_num: usize) -> impl IntoElement {
    pill_badge(("media-card-raw", item_id_num as u64).into())
        .debug_selector(move || format!("media-card-raw-{item_id_num}"))
        .aria_label("RAW")
        .right(px(BADGE_INSET))
        .text_color(theme::accent())
        .child("RAW")
}
