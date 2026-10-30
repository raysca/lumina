//! Smart album builder sheet, modelled on Photos' New Smart Album:
//! name, "Match all/any of the following conditions", one row per condition,
//! then a live count and preview of the first matches.

mod rows;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::smart_draft::{match_label, DraftFocus};
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
            .bg(rgba(0x09090b99))
            .flex()
            .justify_center()
            .items_center()
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
                    .w(px(640.0))
                    .max_h(px(620.0))
                    .bg(theme::bg_app())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_xl()
                    .shadow_lg()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .text_color(theme::text())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        rows::text_field(
                            "album-builder-title".to_string(),
                            &draft.album_title,
                            "Untitled Smart Album",
                            focus == DraftFocus::Title,
                            DraftFocus::Title,
                            cx,
                        )
                        .h(px(32.0))
                        .text_sm(),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .text_color(theme::text_muted())
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
                    .child(
                        div()
                            .id("album-builder-scroll-content")
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_2()
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

/// The first matches as thumbnails; tiles decode like grid cards.
fn render_preview(app: &LuminaApp) -> impl IntoElement {
    let textures = app.grid_state.as_ref().map(|state| &state.texture_cache);
    div()
        .h(px(56.0))
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
                .size(px(56.0))
                .rounded(px(4.0))
                .overflow_hidden()
                .bg(theme::bg_raised())
                .children(texture.map(|t| img(t).size_full().object_fit(ObjectFit::Cover)))
        }))
}

fn render_footer(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let draft = &app.album_draft;
    let valid = draft.match_count.is_some();
    let (status, color) = match draft.match_count {
        None if draft.problem.is_some() => (
            format!(
                "These conditions can't run: {}",
                draft.problem.as_deref().unwrap_or_default()
            ),
            theme::danger_text(),
        ),
        None => (
            "Fix the highlighted conditions".to_string(),
            theme::danger_text(),
        ),
        Some(0) => ("No items match".to_string(), theme::text_muted()),
        Some(1) => ("1 item matches".to_string(), theme::text_muted()),
        Some(n) => (format!("{n} items match"), theme::text_muted()),
    };
    let ring = |focused: bool| {
        if focused {
            theme::text()
        } else {
            theme::border()
        }
    };
    let ok = if valid {
        theme::control_primary("album-builder-save")
    } else {
        theme::control("album-builder-save")
            .text_color(theme::text_disabled())
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
                        .child("Cancel")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.close_album_builder(cx);
                        })),
                )
                .child(
                    ok.debug_selector(|| "album-builder-save".to_string())
                        .when(draft.focus == DraftFocus::Save, |b| {
                            b.border_color(theme::text())
                        })
                        .child("OK")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.save_draft_smart_album(cx);
                        })),
                ),
        )
}
