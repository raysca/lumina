use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;
use gpui::*;

pub const PRESET_TAGS: &[&str] = &[
    "nature",
    "travel",
    "portraits",
    "family",
    "vacation",
    "street",
    "architecture",
];

pub fn render_tags_tab(
    weak_app: WeakEntity<LuminaApp>,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
) -> AnyElement {
    render_tags_section(
        weak_app,
        tags,
        library_tag_counts,
        tag_input_active,
        tag_input_buffer,
    )
}

pub fn render_tags_section(
    weak_app: WeakEntity<LuminaApp>,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
) -> AnyElement {
    let mut suggestions: Vec<&str> = Vec::with_capacity(10);
    for (t, _) in library_tag_counts {
        if suggestions.len() >= 10 {
            break;
        }
        if !suggestions.iter().any(|&s| s.eq_ignore_ascii_case(t)) {
            suggestions.push(t.as_str());
        }
    }
    for &preset in PRESET_TAGS {
        if suggestions.len() >= 10 {
            break;
        }
        if !suggestions.iter().any(|&s| s.eq_ignore_ascii_case(preset)) {
            suggestions.push(preset);
        }
    }

    let app_start_tag = weak_app.clone();
    let app_commit_tag = weak_app.clone();
    let app_cancel_tag = weak_app.clone();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_2p5()
        .rounded_lg()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(theme::section_label("KEYWORDS & TAGS"))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(format!("{} tags", tags.len())),
                ),
        )
        // Active Tags List
        .child(if tags.is_empty() {
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("No tags added yet. Type or click below to add.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(tags.iter().map(|tag| {
                    let tag_name = tag.clone();
                    let filter_query = if tag_name.contains(' ') {
                        format!("tag:\"{}\"", tag_name)
                    } else {
                        format!("tag:{}", tag_name)
                    };
                    let app_filter = weak_app.clone();
                    let app_remove = weak_app.clone();
                    let tag_for_remove = tag_name.clone();

                    div()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(theme::selection_bg())
                        .border_1()
                        .border_color(theme::selection_border())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .id(ElementId::Name(format!("filter-tag-{}", tag_name).into()))
                                .cursor_pointer()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::selection_fg())
                                .hover(|s| s.text_color(theme::text()))
                                .child(format!("#{}", tag_name))
                                .on_click(move |_e, _window, cx| {
                                    if let Some(app) = app_filter.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.set_search_query(&filter_query, cx);
                                        });
                                    }
                                }),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(format!("remove-tag-{}", tag_name).into()))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(theme::selection_fg())
                                .hover(|s| s.text_color(theme::text()))
                                .child(render_icon(Icon::X, px(10.0), theme::selection_fg()))
                                .on_click(move |_e, _window, cx| {
                                    if let Some(app) = app_remove.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.batch_remove_tag(tag_for_remove.clone(), cx);
                                        });
                                    }
                                }),
                        )
                }))
                .into_any_element()
        })
        // Custom Tag Input Row
        .child(if tag_input_active {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(theme::bg_viewer())
                        .border_1()
                        .border_color(theme::accent())
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(if tag_input_buffer.is_empty() {
                            div()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child("Type tag... (Enter to save)")
                                .into_any_element()
                        } else {
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(format!("{}|", tag_input_buffer))
                                .into_any_element()
                        }),
                )
                .child(
                    div()
                        .id("commit-tag-btn")
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(theme::accent())
                        .hover(|s| s.bg(theme::accent_hover()).text_color(theme::accent_fg()))
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent_fg())
                        .child("Add")
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_commit_tag.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.commit_tag_input(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("cancel-tag-btn")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(theme::bg_control())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(render_icon(Icon::X, px(11.0), theme::text_muted()))
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_cancel_tag.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.cancel_tag_input(cx);
                                });
                            }
                        }),
                )
                .into_any_element()
        } else {
            div()
                .id("start-add-tag-btn")
                .px_2p5()
                .py_1()
                .rounded_md()
                .bg(theme::bg_control())
                .hover(|s| s.bg(theme::bg_control_hover()))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::accent())
                        .child("+ Add Tag…"),
                )
                .on_click(move |_e, _window, cx| {
                    if let Some(app) = app_start_tag.upgrade() {
                        app.update(cx, |this, cx| {
                            this.start_tag_input(cx);
                        });
                    }
                })
                .into_any_element()
        })
        // Quick Suggested Tags
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_disabled())
                        .child("Suggestions:"),
                )
                .child(div().flex().flex_row().flex_wrap().gap_1().children(
                    suggestions.into_iter().map(|suggested| {
                        let is_present = tags.iter().any(|t| t.eq_ignore_ascii_case(suggested));
                        let tag_str = suggested.to_string();
                        let app_suggest = weak_app.clone();
                        div()
                            .id(ElementId::Name(format!("suggest-tag-{}", suggested).into()))
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(if is_present {
                                theme::selection_bg()
                            } else {
                                theme::bg_control()
                            })
                            .hover(|s| s.bg(theme::accent()).text_color(theme::accent_fg()))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if is_present {
                                theme::selection_fg()
                            } else {
                                theme::text_muted()
                            })
                            .child(if is_present {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .child(render_icon(Icon::Check, px(9.0), theme::selection_fg()))
                                    .child(suggested.to_string())
                                    .into_any_element()
                            } else {
                                div().child(format!("+ {}", suggested)).into_any_element()
                            })
                            .on_click(move |_e, _window, cx| {
                                if let Some(app) = app_suggest.upgrade() {
                                    app.update(cx, |this, cx| {
                                        if is_present {
                                            this.batch_remove_tag(tag_str.clone(), cx);
                                        } else {
                                            this.batch_add_tag(tag_str.clone(), cx);
                                        }
                                    });
                                }
                            })
                    }),
                )),
        )
        .into_any_element()
}
