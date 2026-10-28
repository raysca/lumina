use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::{LuminaApp, TagSortOrder};

pub struct TagManagerHeader;

impl TagManagerHeader {
    pub fn render_title(
        app: &LuminaApp,
        total_count: usize,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let _ = app;
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .size(px(32.0))
                            .rounded_lg()
                            .bg(theme::selection_bg())
                            .border_1()
                            .border_color(theme::accent())
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_base()
                            .text_color(theme::accent())
                            .child(render_icon(Icon::Tag, px(16.0), theme::accent())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text())
                                    .child("Tag Manager"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .child("Organize, rename, merge, and clean up your tags"),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .bg(theme::bg_raised())
                            .border_1()
                            .border_color(theme::border())
                            .rounded_full()
                            .text_xs()
                            .font_family(theme::FONT_MONO)
                            .text_color(theme::text())
                            .child(format!("{} total", total_count)),
                    ),
            )
            .child(
                div()
                    .id("close-tag-manager-btn")
                    .debug_selector(|| "tag-manager-close".to_string())
                    .tooltip(theme::tooltip_label("Close tag manager"))
                    .size(px(28.0))
                    .rounded_md()
                    .bg(theme::bg_raised())
                    .hover(|s| s.bg(theme::bg_control_hover()))
                    .cursor_pointer()
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_xs()
                    .text_color(theme::text())
                    .child(render_icon(Icon::X, px(13.0), theme::text()))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.close_tag_manager(cx);
                    })),
            )
    }

    pub fn render_presets(
        app: &LuminaApp,
        selected_count: usize,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .flex_wrap()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::text_subtle())
                    .child("FILTER BY"),
            )
            .child(
                div()
                    .id("filter-all-tags-btn")
                    .px_3()
                    .py_1()
                    .bg(if app.tag_manager.search.is_empty() {
                        theme::bg_control()
                    } else {
                        theme::bg_raised()
                    })
                    .hover(|s| s.bg(theme::bg_control_hover()))
                    .border_1()
                    .border_color(theme::border())
                    .rounded_full()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(theme::text())
                    .child("All Tags")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.tag_manager.search.clear();
                        cx.notify();
                    })),
            )
            .children((selected_count > 0).then(|| {
                div()
                    .px_3()
                    .py_1()
                    .bg(theme::selection_bg())
                    .border_1()
                    .border_color(theme::accent())
                    .rounded_full()
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::accent())
                    .child(format!("Selected ({})", selected_count))
            }))
            .child(
                div()
                    .id("filter-most-popular-btn")
                    .px_3()
                    .py_1()
                    .bg(theme::bg_raised())
                    .hover(|s| s.bg(theme::bg_control_hover()))
                    .border_1()
                    .border_color(theme::border())
                    .rounded_full()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(theme::text())
                    .child("Most Popular")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.set_tag_sort_order(TagSortOrder::CountDesc, cx);
                    })),
            )
            .child(
                div()
                    .id("filter-alpha-tags-btn")
                    .px_3()
                    .py_1()
                    .bg(theme::bg_raised())
                    .hover(|s| s.bg(theme::bg_control_hover()))
                    .border_1()
                    .border_color(theme::border())
                    .rounded_full()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(theme::text())
                    .child("Alphabetical")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.set_tag_sort_order(TagSortOrder::Alphabetical, cx);
                    })),
            )
    }

    pub fn render_controls(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            // Search box
            .child(
                div()
                    .flex_1()
                    .h(px(34.0))
                    .px_3()
                    .bg(theme::bg_app())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_lg()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(theme::text_subtle()).child(render_icon(
                        Icon::Search,
                        px(12.0),
                        theme::text_subtle(),
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if app.tag_manager.search.is_empty() {
                                theme::text_subtle()
                            } else {
                                theme::text()
                            })
                            .child(if app.tag_manager.search.is_empty() {
                                "Filter tags by name...".to_string()
                            } else {
                                format!("{}|", app.tag_manager.search)
                            }),
                    ),
            )
            // Segmented Sort Toggle: [ A-Z | Count ▼ ]
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text_subtle())
                            .child("SORT:"),
                    )
                    .child(
                        div()
                            .id("tag-sort-az-btn")
                            .px_2p5()
                            .py_1()
                            .bg(if app.tag_manager.sort == TagSortOrder::Alphabetical {
                                theme::bg_control_hover()
                            } else {
                                theme::bg_raised()
                            })
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .border_1()
                            .border_color(theme::border())
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if app.tag_manager.sort == TagSortOrder::Alphabetical {
                                theme::text()
                            } else {
                                theme::text_muted()
                            })
                            .child("Name A-Z")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.set_tag_sort_order(TagSortOrder::Alphabetical, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("tag-sort-count-btn")
                            .px_2p5()
                            .py_1()
                            .bg(if app.tag_manager.sort == TagSortOrder::CountDesc {
                                theme::bg_control_hover()
                            } else {
                                theme::bg_raised()
                            })
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .border_1()
                            .border_color(theme::border())
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if app.tag_manager.sort == TagSortOrder::CountDesc {
                                theme::text()
                            } else {
                                theme::text_muted()
                            })
                            .child("Count ▼")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.set_tag_sort_order(TagSortOrder::CountDesc, cx);
                            })),
                    ),
            )
    }
}
