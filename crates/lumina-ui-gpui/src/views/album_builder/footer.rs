use std::sync::Arc;

use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_live_preview(
    match_count: usize,
    thumb1: Arc<RenderImage>,
    thumb2: Arc<RenderImage>,
    thumb3: Arc<RenderImage>,
) -> impl IntoElement {
    div()
        .w_full()
        .pt_2()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap_3()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("LIVE PREVIEW"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent())
                        .child(format!("{} items match", match_count)),
                )
                .child(div().text_xs().text_color(theme::text_subtle()).child(
                    "updates as you edit · the album stays in sync as your library changes",
                )),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .w(px(58.0))
                        .h(px(58.0))
                        .rounded_lg()
                        .overflow_hidden()
                        .child(img(thumb1).size_full().object_fit(ObjectFit::Cover)),
                )
                .child(
                    div()
                        .w(px(58.0))
                        .h(px(58.0))
                        .rounded_lg()
                        .overflow_hidden()
                        .child(img(thumb2).size_full().object_fit(ObjectFit::Cover)),
                )
                .child(
                    div()
                        .w(px(58.0))
                        .h(px(58.0))
                        .rounded_lg()
                        .overflow_hidden()
                        .child(img(thumb3).size_full().object_fit(ObjectFit::Cover)),
                ),
        )
}

pub(super) fn render_footer_actions(
    app: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    div()
        .w_full()
        .pt_2()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        // Left: Checkbox "Show in sidebar"
        .child(
            div()
                .id("toggle-sidebar-draft-cb")
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .child(
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded_sm()
                        .bg(if app.album_draft.show_in_sidebar {
                            theme::accent()
                        } else {
                            theme::bg_raised()
                        })
                        .border_1()
                        .border_color(if app.album_draft.show_in_sidebar {
                            theme::accent()
                        } else {
                            theme::border_strong()
                        })
                        .flex()
                        .justify_center()
                        .items_center()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent_fg())
                        .child(if app.album_draft.show_in_sidebar {
                            render_icon(Icon::Check, px(10.0), theme::accent_fg())
                                .into_any_element()
                        } else {
                            Empty.into_any_element()
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Show in sidebar"),
                )
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_draft_show_in_sidebar(cx);
                })),
        )
        // Right: Cancel Esc + Create Smart Album
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .id("cancel-album-btn")
                        .px_3p5()
                        .py_1p5()
                        .bg(theme::bg_raised())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .border_1()
                        .border_color(theme::border())
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .child(div().text_xs().text_color(theme::text()).child("Cancel"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child("Esc"),
                        )
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.close_album_builder(cx);
                        })),
                )
                .child(
                    div()
                        .id("save-album-btn")
                        .px_4()
                        .py_2()
                        .bg(theme::accent())
                        .hover(|s| s.bg(theme::accent_hover()))
                        .rounded_lg()
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent_fg())
                        .child("Create Smart Album")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.save_draft_smart_album(cx);
                        })),
                ),
        )
}
