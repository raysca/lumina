//! Tag manager (v3): a floating sheet with the tag list on the left and the
//! focused tag's details on the right. Same shell as the smart album builder.

pub mod detail;
pub mod list;

use gpui::*;

use crate::icons::{render_icon, Icon};
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
            // while the dim blocks the grid/viewer underneath.
            .occlude()
            .bg(theme::modal_dim())
            // A sheet hanging from under the toolbar, not a centred dialog.
            .flex()
            .justify_center()
            .items_start()
            .pt(px(crate::views::page_header::PAGE_HEADER_HEIGHT))
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
                    .h(px((window_h - 76.0).clamp(420.0, 720.0)))
                    .bg(theme::bg_2())
                    .border_1()
                    .border_t_0()
                    .border_color(theme::line_2())
                    .rounded_b(px(10.0))
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
        .border_color(theme::line())
        .child(render_icon(Icon::Tag, px(16.0), theme::accent()))
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child("Tag Manager"),
        )
        // ponytail: no "· M unused" or "+ New Tag": a tag exists only while an
        // item carries it, so neither can occur until tags get their own table.
        .child(
            div()
                .id("tag-manager-total")
                .debug_selector(|| "tag-manager-total".to_string())
                .text_xs()
                .text_color(theme::t2())
                .child(format!("{total} tags")),
        )
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(theme::t2())
                .child("↑↓ browse · Space check · ↵ rename · Esc close"),
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
