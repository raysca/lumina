//! Tag manager (v3): a floating sheet with the tag list on the left and the
//! focused tag's details on the right. Same shell as the smart album builder.

pub mod detail;
pub mod list;

use gpui::*;

use crate::theme;
use crate::LuminaApp;

pub struct TagManagerModal;

impl TagManagerModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let rows = app.tag_manager_rows();
        let window_w = app.window_bounds.map_or(1440.0, |(_, _, w, _)| w);
        let window_h = app.window_bounds.map_or(900.0, |(_, _, _, h)| h);

        div()
            .id("tag-manager-backdrop")
            .debug_selector(|| "tag-manager-backdrop".to_string())
            .absolute()
            .inset_0()
            // The card is painted after this backdrop, so it stays clickable
            // while the scrim blocks the grid/viewer underneath.
            .occlude()
            .bg(theme::scrim())
            .flex()
            .justify_center()
            .items_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_tag_manager(cx);
                }),
            )
            .child(
                div()
                    .id("tag-manager-dialog")
                    .debug_selector(|| "tag-manager-dialog".to_string())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .w(px((window_w - 48.0).clamp(640.0, 1000.0)))
                    .h(px((window_h - 96.0).clamp(420.0, 720.0)))
                    .bg(theme::bg_raised())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(px(10.0))
                    .shadow_lg()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(render_header(app.library_tag_counts.len(), cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.0))
                            .flex()
                            .flex_row()
                            .child(list::render_list(app, &rows, cx))
                            .child(detail::render_detail(app, cx)),
                    ),
            )
            .into_any_element()
    }
}

fn render_header(total: usize, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .h(px(56.0))
        .flex_shrink_0()
        .pl(px(24.0))
        .pr(px(16.0))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text())
                .child("Tag Manager"),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child(format!("{total} tags")),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("↑↓ browse · ↵ rename · Esc close"),
        )
        .child(
            theme::control_primary("done-tag-manager-btn")
                .debug_selector(|| "tag-manager-close".to_string())
                .child("Done")
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.close_tag_manager(cx);
                })),
        )
}
