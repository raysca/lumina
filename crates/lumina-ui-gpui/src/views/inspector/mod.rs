pub mod batch;
pub mod body;
mod partial_tags;
pub mod summary;
pub mod tabs;

pub use body::{format_bytes, render_inspector_body, InspectorTab};

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::*;

pub struct InspectorView;

impl InspectorView {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
        let weak_app = cx.entity().downgrade();
        let selected_item = this.selected_item.clone();
        let selected_count = this.selected_indices.len();
        let batch_summary = this.batch_summary.clone();
        let viewer_image = this.viewer_image.clone();
        let tags = this.selected_item_tags.clone();
        let tag_counts = this.selected_tag_counts.clone();
        let library_tag_counts = this.library_tag_counts.clone();
        let tag_input_active = this.tag_input_active;
        let tag_input_buffer = this.tag_input_buffer.clone();
        let is_trash = this.selection == AlbumSelection::Trash;
        let tab = this.inspector_tab;
        let rating = selected_item
            .as_ref()
            .map(|it| this.get_item_rating(it))
            .unwrap_or(0);
        let color_label = selected_item
            .as_ref()
            .and_then(|it| this.item_color_labels.get(&it.id).copied());
        let caption = selected_item
            .as_ref()
            .and_then(|it| this.item_captions.get(&it.id).cloned());
        let caption_input_active = this.caption.input_active;
        let caption_input_buffer = this.caption.input_buffer.clone();
        let title_input_active = this.caption.title_input_active;
        let title_input_buffer = this.caption.title_input_buffer.clone();
        let date_input_active = this.caption.date_input_active;
        let date_input_buffer = this.caption.date_input_buffer.clone();
        let location_input_active = this.caption.location_input_active;
        let location_input_buffer = this.caption.location_input_buffer.clone();

        // v3: a single item gets the one-column summary with no header strip;
        // Info / ⌘I closes it and Get Info… opens the full tabbed window.
        if selected_count <= 1 {
            if let Some(item) = selected_item.as_ref() {
                return div()
                    .w(px(this.inspector_width))
                    .h_full()
                    .border_l_1()
                    .border_color(theme::border_subtle())
                    .child(summary::render_summary_body(
                        weak_app,
                        item,
                        &tags,
                        &library_tag_counts,
                        tag_input_active,
                        &tag_input_buffer,
                        is_trash,
                        rating,
                        color_label,
                    ))
                    .into_any_element();
            }
        }

        if selected_item.is_none() {
            return div()
                .debug_selector(|| "inspector-empty".to_string())
                .w(px(this.inspector_width))
                .h_full()
                .flex_shrink_0()
                .bg(theme::bg_pane())
                .border_l_1()
                .border_color(theme::border_subtle())
                .p_4()
                .text_sm()
                .text_color(theme::text_subtle())
                .child("Nothing selected.")
                .into_any_element();
        }

        div()
            .w(px(this.inspector_width))
            .h_full()
            .bg(theme::bg_app())
            .border_l_1()
            .border_color(theme::border())
            .flex()
            .flex_col()
            .text_color(theme::text())
            // Docked Header Strip
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme::border())
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(div().text_xs().text_color(theme::accent()).child("ℹ"))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text_muted())
                                    .child(if selected_count > 1 {
                                        format!("BATCH SELECTION ({})", selected_count)
                                    } else {
                                        "INFO & METADATA".to_string()
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .id(if selected_count > 1 {
                                        "undock-batch-inspector-btn"
                                    } else {
                                        "undock-inspector-btn"
                                    })
                                    .tooltip(theme::tooltip_label("Open inspector in a window"))
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded_md()
                                    .hover(|s| s.bg(theme::bg_control()).text_color(theme::text()))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child(render_icon(Icon::Pip, px(13.0), theme::text_muted()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.undock_inspector(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id(if selected_count > 1 {
                                        "close-batch-inspector-btn"
                                    } else {
                                        "close-inspector-btn"
                                    })
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded_md()
                                    .hover(|s| s.bg(theme::bg_control()))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text_subtle())
                                    .child(render_icon(Icon::X, px(13.0), theme::text_subtle()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.inspector_open = false;
                                        this.cancel_tag_input(cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Inspector Body
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(render_inspector_body(
                        weak_app,
                        selected_item,
                        selected_count,
                        &batch_summary,
                        viewer_image,
                        &tags,
                        &tag_counts,
                        &library_tag_counts,
                        tag_input_active,
                        &tag_input_buffer,
                        is_trash,
                        tab,
                        rating,
                        color_label,
                        caption.as_deref(),
                        caption_input_active,
                        &caption_input_buffer,
                        title_input_active,
                        &title_input_buffer,
                        date_input_active,
                        &date_input_buffer,
                        location_input_active,
                        &location_input_buffer,
                    )),
            )
            .into_any_element()
    }
}
