use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub struct TagManagerToolbars;

impl TagManagerToolbars {
    pub fn render_bulk_actions(
        app: &LuminaApp,
        selected_count: usize,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        if selected_count > 0 {
            let selected_vec: Vec<String> = app.tag_manager.selected.iter().cloned().collect();
            div()
                .w_full()
                .px_3p5()
                .py_2()
                .bg(theme::bg_raised())
                .border_1()
                .border_color(theme::accent())
                .rounded_lg()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent())
                        .child(format!("{} tags selected", selected_count)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("bulk-delete-tags-btn")
                                .px_2p5()
                                .py_1()
                                .rounded_md()
                                .bg(theme::danger_bg())
                                .hover(|s| s.bg(theme::danger_bg_hover()))
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(render_icon(Icon::Trash, px(12.0), theme::text()))
                                .child("Delete Selected")
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.bulk_delete_tags(selected_vec.clone(), cx);
                                })),
                        )
                        .child(
                            div()
                                .id("clear-tag-selection-btn")
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(theme::bg_control())
                                .hover(|s| s.bg(theme::bg_control_hover()))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Deselect All")
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.clear_tag_manager_selection(cx);
                                })),
                        ),
                )
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }

    pub fn render_merge_banner(
        is_merge_active: bool,
        merge_source: &str,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        if is_merge_active {
            let src_copy = merge_source.to_string();
            div()
                .w_full()
                .px_3p5()
                .py_2()
                .bg(theme::selection_bg())
                .border_1()
                .border_color(theme::accent())
                .rounded_lg()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().text_sm().text_color(theme::accent()).child("⇄"))
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::accent())
                                .child(format!(
                                    "Merging '#{}' · click 'Merge Here' on target tag below:",
                                    src_copy
                                )),
                        ),
                )
                .child(
                    div()
                        .id("cancel-merge-btn")
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(theme::bg_raised())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(theme::text())
                        .child("Cancel")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.cancel_tag_merge(cx);
                        })),
                )
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }

    pub fn render_footer(filtered_count: usize, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        div()
            .w_full()
            .pt_2()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            // Left: Live stats matching the mockup
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
                            .child("LIVE STATS"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::accent())
                            .child(format!("{} tags matching", filtered_count)),
                    ),
            )
            // Right: Cancel Esc + Done
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("cancel-tag-manager-footer-btn")
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
                                this.close_tag_manager(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("done-tag-manager-btn")
                            .px_4()
                            .py_2()
                            .bg(theme::accent())
                            .hover(|s| s.bg(theme::accent_hover()))
                            .rounded_lg()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::accent_fg())
                            .child("Done")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.close_tag_manager(cx);
                            })),
                    ),
            )
    }
}
