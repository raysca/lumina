//! The builder's lower half: the Pin option, the live preview strip and the
//! footer band with the match count, Cancel and Create.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::smart_draft::DraftFocus;
use crate::app::state::AlbumDraftState;
use crate::i18n::{count, t};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

const TILE_H: f32 = 96.0;
const TILE_MIN_W: f32 = 68.0;
const TILE_MAX_W: f32 = 92.0;

/// The footer's status and its colour: the count, or why OK is disabled.
pub(crate) fn footer_status(draft: &AlbumDraftState) -> (String, Rgba) {
    match draft.match_count {
        None if draft.problem.is_some() => (
            format!(
                "These conditions can't run: {}",
                draft.problem.as_deref().unwrap_or_default()
            ),
            theme::danger(),
        ),
        None => (
            "Fix the highlighted conditions".to_string(),
            theme::danger(),
        ),
        Some(0) => (t("smart_album.no_matches"), theme::t1()),
        Some(1) => (t("smart_album.matches_one"), theme::t1()),
        Some(n) => (
            t("smart_album.matches_other").replace("{n}", &count(n)),
            theme::t1(),
        ),
    }
}

/// "+N" for the matches beyond the shown tiles; `None` when all are shown.
pub(crate) fn more_label(matches: Option<usize>, shown: usize) -> Option<String> {
    let rest = matches?.checked_sub(shown).filter(|n| *n > 0)?;
    Some(format!("+{}", count(rest)))
}

/// The primary button: "Create Smart Album", or "Save Smart Album" when editing.
pub(crate) fn save_label(editing: bool) -> String {
    t(if editing {
        "smart_album.save"
    } else {
        "smart_album.create"
    })
}

/// "Pin to sidebar": a 16 px checkbox and its label; Space toggles it when
/// focused (Tab order: after the rows, before Cancel).
pub(super) fn render_options(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let draft = &app.album_draft;
    let on = draft.pin;
    let label = t("smart_album.pin");
    let box_ = div()
        .flex_none()
        .size(px(16.0))
        .rounded(px(5.0))
        .flex()
        .items_center()
        .justify_center()
        .map(|b| {
            if on {
                b.bg(theme::accent())
                    .child(render_icon(Icon::Check, px(11.0), theme::on_accent()))
            } else {
                b.bg(theme::bg_0()).border_1().border_color(theme::line_2())
            }
        });
    div().flex().flex_row().gap(px(22.0)).child(
        div()
            .id("album-builder-pin")
            .debug_selector(|| "album-builder-pin".to_string())
            .role(Role::CheckBox)
            .aria_toggled(if on { Toggled::True } else { Toggled::False })
            .aria_label(label.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(4.0))
            .py(px(2.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(if draft.focus == DraftFocus::Pin {
                theme::focus_ring()
            } else {
                theme::card_surface()
            })
            .cursor_pointer()
            .children(text_marker(
                "album-builder-pin",
                &(if on { "on" } else { "off" }).into(),
            ))
            .child(box_)
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(if on { theme::t1() } else { theme::t2() })
                    .child(label),
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.toggle_draft_pin(cx);
            })),
    )
}

/// The first matches as thumbnails (width by aspect ratio), then "+N more".
/// Tiles decode like grid cards; nothing here queries the catalog.
pub(super) fn render_preview(app: &LuminaApp) -> impl IntoElement {
    let draft = &app.album_draft;
    let textures = app.grid_state.as_ref().map(|state| &state.texture_cache);
    let tiles = draft.preview.iter().map(|item| {
        let id = item.id;
        let texture = textures.and_then(|cache| cache.lock().get(&id).cloned());
        if texture.is_none() {
            app.queue_thumbnail_request(id);
        }
        let aspect = if item.height > 0 {
            item.width as f32 / item.height as f32
        } else {
            1.0
        };
        div()
            .debug_selector(move || format!("album-builder-preview-{}", id.0))
            .flex_none()
            .h(px(TILE_H))
            .w(px((TILE_H * aspect).clamp(TILE_MIN_W, TILE_MAX_W)))
            .rounded(px(7.0))
            .overflow_hidden()
            .bg(theme::bg_2())
            .children(texture.map(|t| img(t).size_full().object_fit(ObjectFit::Cover)))
    });
    let more = more_label(draft.match_count, draft.preview.len()).map(|more| {
        let marker = text_marker("album-builder-more", &more.clone().into());
        div()
            .debug_selector(|| "album-builder-more".to_string())
            .flex_none()
            .h(px(TILE_H))
            .w(px(TILE_MIN_W))
            .rounded(px(7.0))
            .bg(theme::bg_2())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .children(marker)
            .child(
                div()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(14.0))
                    .text_color(theme::t1())
                    .child(more),
            )
            .child(
                div()
                    .text_size(px(10.5))
                    .text_color(theme::t3())
                    .child(t("smart_album.more")),
            )
    });
    let strip = if draft.match_count == Some(0) {
        div()
            .debug_selector(|| "album-builder-preview-empty".to_string())
            .text_size(px(12.0))
            .text_color(theme::t3())
            .child(t("smart_album.preview_empty"))
    } else {
        div()
            .flex()
            .flex_row()
            .gap(px(7.0))
            .overflow_hidden()
            .children(tiles)
            .children(more)
    };
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap_2()
                .child(theme::section_label(t("smart_album.live_preview")))
                .child(
                    div()
                        .debug_selector(|| "album-builder-preview-help".to_string())
                        .text_size(px(12.0))
                        .text_color(theme::t2())
                        .child(t("smart_album.preview_help")),
                ),
        )
        .child(strip)
}

/// A footer key hint: 20 tall, r5, mono 11.
fn key_hint(label: &'static str, bg: Rgba, color: Rgba) -> Div {
    div()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(5.0))
        .bg(bg)
        .flex()
        .items_center()
        .font_family(theme::FONT_MONO)
        .text_size(px(11.0))
        .text_color(color)
        .child(label)
}

/// The band under the dialog: count, then Cancel (Esc) and Create (↵).
pub(super) fn render_footer(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let draft = &app.album_draft;
    let valid = draft.match_count.is_some() && draft.problem.is_none();
    let (status, color) = footer_status(draft);
    let ring = |focused: bool| {
        if focused {
            theme::t1()
        } else {
            theme::card_surface()
        }
    };
    let button = |id: &'static str| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .h(px(36.0))
            .px(px(12.0))
            .rounded(px(9.0))
            .border_1()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .text_size(px(13.0))
            .cursor_pointer()
    };
    let save_text: SharedString = save_label(draft.editing_album_id.is_some()).into();
    let save = button("album-builder-save")
        .children(text_marker("album-builder-save", &save_text))
        .border_color(ring(draft.focus == DraftFocus::Save))
        .font_weight(FontWeight::SEMIBOLD)
        .map(|b| {
            if valid {
                b.bg(theme::accent())
                    .text_color(theme::on_accent())
                    .hover(|s| s.bg(theme::accent_hover()))
                    .child(save_text.clone())
                    .child(key_hint("⏎", theme::on_accent_kbd(), theme::on_accent()))
            } else {
                b.bg(theme::bg_3())
                    .text_color(theme::t3())
                    .cursor_default()
                    .child(save_text.clone())
            }
        })
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.save_draft_smart_album(cx);
        }));
    let cancel = button("album-builder-cancel")
        .border_color(ring(draft.focus == DraftFocus::Cancel))
        .bg(theme::bg_3())
        .hover(|s| s.bg(theme::plate()))
        .text_color(theme::t1())
        .font_weight(FontWeight::MEDIUM)
        .child(t("smart_album.cancel"))
        .child(key_hint("Esc", theme::bg_0(), theme::t3()))
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.close_album_builder(cx);
        }));

    div()
        .flex_none()
        .w_full()
        .h(px(63.5))
        .px(px(20.0))
        .bg(theme::bg_2())
        .border_t_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .when(valid, |el| {
                    el.child(render_icon(Icon::Sparkles, px(14.0), theme::accent()))
                })
                .child(
                    div()
                        .debug_selector(|| "album-builder-count".to_string())
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(color)
                        .truncate()
                        .child(status),
                )
                .when(valid, |el| {
                    el.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::t3())
                            .child(t("smart_album.stays_current")),
                    )
                }),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .gap_2()
                .child(cancel)
                .child(save),
        )
}
