//! Smart album builder sheet, modelled on Photos' New Smart Album:
//! name, "Match all/any of the following conditions", one row per condition,
//! then a live count and preview of the first matches.

use crate::app::text_input::TextTarget;
mod rows;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::smart_draft::{match_label, DraftFocus};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub struct AlbumBuilderModal;

impl AlbumBuilderModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        if !app.album_draft.modal_open {
            return Empty.into_any_element();
        }
        let draft = &app.album_draft;
        let focus = draft.focus;
        let popup_on_match = draft.popup.map(|p| p.target) == Some(DraftFocus::Match);

        div()
            .id("album-builder-backdrop")
            .debug_selector(|| "album-builder-backdrop".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::modal_dim())
            // A sheet hanging from under the toolbar, not a centred dialog.
            .flex()
            .justify_center()
            .items_start()
            .pt(px(crate::app::render::HEADER_HEIGHT))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_album_builder(cx);
                }),
            )
            .child(
                div()
                    .id("album-builder-modal-card")
                    .debug_selector(|| "album-builder-card".to_string())
                    .w(px(760.0))
                    .max_h(px(620.0))
                    .bg(theme::bg_1())
                    .border_1()
                    .border_t_0()
                    .border_color(theme::line_2())
                    .rounded_b(px(10.0))
                    .shadow_lg()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_color(theme::t1())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    // Amber star and a large borderless title (ring only on focus).
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(render_icon(Icon::Star, px(18.0), theme::accent()))
                            .child(
                                rows::text_field(
                                    "album-builder-title".to_string(),
                                    &draft.album_title,
                                    "Untitled Smart Album",
                                    focus == DraftFocus::Title,
                                    app.field_input_for(TextTarget::AlbumDraft)
                                        .filter(|_| focus == DraftFocus::Title),
                                    DraftFocus::Title,
                                    cx,
                                )
                                .flex_1()
                                .h(px(34.0))
                                .bg(transparent_black())
                                .when(focus != DraftFocus::Title, |f| {
                                    f.border_color(transparent_black())
                                })
                                .text_size(px(17.0))
                                .font_weight(FontWeight::SEMIBOLD),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .text_color(theme::t2())
                            .child("Match")
                            .child(
                                rows::picker(
                                    "album-builder-match".to_string(),
                                    match_label(draft.match_mode),
                                    focus == DraftFocus::Match,
                                    DraftFocus::Match,
                                    cx,
                                )
                                .w(px(70.0)),
                            )
                            .child("of the following conditions:"),
                    )
                    .when(popup_on_match, |el| el.child(rows::render_popup(app, cx)))
                    // One bordered dark container; rows draw their dividers.
                    .child(
                        div()
                            .id("album-builder-scroll-content")
                            .debug_selector(|| "album-builder-conditions".to_string())
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()
                            .rounded(theme::RADIUS_LG)
                            .border_1()
                            .border_color(theme::line())
                            .bg(theme::bg_0())
                            .flex()
                            .flex_col()
                            .when(draft.rows.is_empty(), |el| {
                                el.child(render_empty_conditions(focus, cx))
                            })
                            .children(
                                draft
                                    .rows
                                    .iter()
                                    .enumerate()
                                    .map(|(i, row)| rows::render_row(app, i, row, cx)),
                            ),
                    )
                    .child(render_preview(app))
                    .child(render_footer(app, cx)),
            )
            .into_any_element()
    }
}

/// "No conditions — every item matches." plus the way back to a condition.
fn render_empty_conditions(focus: DraftFocus, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .debug_selector(|| "album-builder-empty".to_string())
        .px(px(12.0))
        .py(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .text_sm()
        .text_color(theme::t2())
        .child("No conditions — every item matches.")
        .child(rows::square_button(
            "album-rule-add-empty".to_string(),
            Icon::Plus,
            "Add condition",
            focus == DraftFocus::Add(0),
            |this, cx| this.add_draft_row_after(0, cx),
            cx,
        ))
}

/// The first matches as thumbnails; tiles decode like grid cards.
fn render_preview(app: &LuminaApp) -> impl IntoElement {
    let textures = app.grid_state.as_ref().map(|state| &state.texture_cache);
    let tiles = div()
        .h(px(PREVIEW_TILE))
        .flex()
        .flex_row()
        .gap_1p5()
        .children(app.album_draft.preview.iter().map(|item| {
            let id = item.id;
            let texture = textures.and_then(|cache| cache.lock().get(&id).cloned());
            if texture.is_none() {
                app.queue_thumbnail_request(id);
            }
            div()
                .debug_selector(move || format!("album-builder-preview-{}", id.0))
                .size(px(PREVIEW_TILE))
                .overflow_hidden()
                .bg(theme::bg_2())
                .children(texture.map(|t| img(t).size_full().object_fit(ObjectFit::Cover)))
        }));
    div()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap_2()
                .child(theme::section_label("LIVE PREVIEW"))
                .child(
                    div()
                        .debug_selector(|| "album-builder-preview-help".to_string())
                        .text_xs()
                        .text_color(theme::t2())
                        .child("The first matches, updated as you edit."),
                ),
        )
        .child(tiles)
}

const PREVIEW_TILE: f32 = 72.0;

fn render_footer(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let draft = &app.album_draft;
    let valid = draft.match_count.is_some();
    let (status, color) = match draft.match_count {
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
        Some(0) => ("No items match".to_string(), theme::t2()),
        Some(1) => ("1 item matches".to_string(), theme::t2()),
        Some(n) => (format!("{n} items match"), theme::t2()),
    };
    let ring = |focused: bool| {
        if focused {
            theme::t1()
        } else {
            theme::line()
        }
    };
    let ok = if valid {
        theme::control_primary("album-builder-save")
    } else {
        theme::control("album-builder-save")
            .text_color(theme::t3())
            .cursor_default()
    };

    div()
        .w_full()
        .pt_1()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .child(
            div()
                .debug_selector(|| "album-builder-count".to_string())
                .text_xs()
                .text_color(color)
                .child(status),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    theme::control("album-builder-cancel")
                        .debug_selector(|| "album-builder-cancel".to_string())
                        .border_color(ring(draft.focus == DraftFocus::Cancel))
                        .gap_2()
                        .child("Cancel")
                        .child(theme::kbd("Esc"))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.close_album_builder(cx);
                        })),
                )
                .child(
                    ok.debug_selector(|| "album-builder-save".to_string())
                        .when(draft.focus == DraftFocus::Save, |b| {
                            b.border_color(theme::t1())
                        })
                        .child("OK")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.save_draft_smart_album(cx);
                        })),
                ),
        )
}
