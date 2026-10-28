use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub struct TagManagerTable;

impl TagManagerTable {
    pub fn render_table(
        app: &LuminaApp,
        filtered_tags: &[(String, usize)],
        query: &str,
        is_merge_active: bool,
        merge_source: &str,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        div()
            .id("modal-tags-scroll-table")
            .w_full()
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1p5()
            .children(if filtered_tags.is_empty() {
                vec![div()
                    .py_12()
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_xs()
                    .text_color(theme::text_subtle())
                    .child(if query.is_empty() {
                        "No tags found in catalog."
                    } else {
                        "No tags matching filter."
                    })
                    .into_any_element()]
            } else {
                filtered_tags
                    .iter()
                    .map(|(tag_name, count)| {
                        let is_editing = app.tag_manager.editing.as_ref() == Some(tag_name);
                        let is_selected = app.tag_manager.selected.contains(tag_name);
                        let is_this_source = is_merge_active && merge_source == tag_name;

                        let tag_for_select = tag_name.clone();
                        let tag_for_rename = tag_name.clone();
                        let tag_for_merge = tag_name.clone();
                        let tag_for_delete = tag_name.clone();
                        let tag_for_filter = tag_name.clone();

                        div()
                            .id(ElementId::Name(
                                format!("modal-tag-row-{}", tag_name).into(),
                            ))
                            .w_full()
                            .px_3()
                            .py_2()
                            .bg(if is_this_source || is_selected {
                                theme::selection_bg()
                            } else {
                                theme::bg_app()
                            })
                            .border_1()
                            .border_color(if is_this_source || is_selected {
                                theme::accent()
                            } else {
                                theme::border_subtle()
                            })
                            .rounded_lg()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            // Left: Checkbox + Tag Pill
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2p5()
                                    .child(
                                        div()
                                            .id(ElementId::Name(
                                                format!("modal-tag-cb-{}", tag_name).into(),
                                            ))
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(if is_selected {
                                                theme::accent()
                                            } else {
                                                theme::border()
                                            })
                                            .bg(if is_selected {
                                                theme::accent()
                                            } else {
                                                theme::bg_pane()
                                            })
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::accent_fg())
                                            .child(if is_selected { "✓" } else { "" })
                                            .on_click(cx.listener(
                                                move |this, _e: &ClickEvent, _w, cx| {
                                                    this.toggle_tag_manager_selection(
                                                        tag_for_select.clone(),
                                                        cx,
                                                    );
                                                },
                                            )),
                                    )
                                    .child(if is_editing {
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::text())
                                            .child(format!("#{}|", app.tag_manager.editing_buffer))
                                            .into_any_element()
                                    } else {
                                        div()
                                            .id(ElementId::Name(
                                                format!("modal-tag-name-filter-{}", tag_name)
                                                    .into(),
                                            ))
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(theme::accent()))
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::text())
                                            .child(format!("#{}", tag_name))
                                            .on_click(cx.listener(
                                                move |this, _e: &ClickEvent, _w, cx| {
                                                    let query = format!("tag:{}", tag_for_filter);
                                                    this.set_search_query(&query, cx);
                                                    this.close_tag_manager(cx);
                                                },
                                            ))
                                            .into_any_element()
                                    }),
                            )
                            // Right: Count Badge + Action Buttons
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(theme::bg_raised())
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child(format!("{} items", count)),
                                    )
                                    .child(if is_editing {
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("modal-save-rename-{}", tag_name)
                                                            .into(),
                                                    ))
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme::accent())
                                                    .hover(|s| s.bg(theme::accent_hover()))
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme::accent_fg())
                                                    .child("✓ Save")
                                                    .on_click(cx.listener(
                                                        |this, _e: &ClickEvent, _w, cx| {
                                                            this.commit_modal_tag_rename(cx);
                                                        },
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("modal-cancel-rename-{}", tag_name)
                                                            .into(),
                                                    ))
                                                    .px_2()
                                                    .py_0p5()
                                                    .rounded_md()
                                                    .bg(theme::bg_raised())
                                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(theme::text_subtle())
                                                    .child(render_icon(
                                                        Icon::X,
                                                        px(11.0),
                                                        theme::text_subtle(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        |this, _e: &ClickEvent, _w, cx| {
                                                            this.cancel_modal_tag_rename(cx);
                                                        },
                                                    )),
                                            )
                                            .into_any_element()
                                    } else if is_merge_active {
                                        if !is_this_source {
                                            div()
                                                .id(ElementId::Name(
                                                    format!("modal-merge-target-{}", tag_name)
                                                        .into(),
                                                ))
                                                .px_2p5()
                                                .py_1()
                                                .rounded_md()
                                                .bg(theme::accent())
                                                .hover(|s| s.bg(theme::accent_hover()))
                                                .cursor_pointer()
                                                .text_xs()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme::accent_fg())
                                                .child("Merge Here")
                                                .on_click(cx.listener(
                                                    move |this, _e: &ClickEvent, _w, cx| {
                                                        this.commit_tag_merge(
                                                            tag_for_merge.clone(),
                                                            cx,
                                                        );
                                                    },
                                                ))
                                                .into_any_element()
                                        } else {
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(theme::accent())
                                                .child("(Source)")
                                                .into_any_element()
                                        }
                                    } else {
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("modal-rename-btn-{}", tag_name)
                                                            .into(),
                                                    ))
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(theme::bg_raised())
                                                    .hover(|s| {
                                                        s.bg(theme::bg_control_hover())
                                                            .text_color(theme::text())
                                                    })
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(theme::text_muted())
                                                    .child("✎ Rename")
                                                    .on_click(cx.listener(
                                                        move |this, _e: &ClickEvent, _w, cx| {
                                                            this.start_modal_tag_rename(
                                                                tag_for_rename.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("modal-merge-btn-{}", tag_name)
                                                            .into(),
                                                    ))
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(theme::bg_raised())
                                                    .hover(|s| {
                                                        s.bg(theme::bg_control_hover())
                                                            .text_color(theme::text())
                                                    })
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(theme::text_muted())
                                                    .child("⇄ Merge")
                                                    .on_click(cx.listener(
                                                        move |this, _e: &ClickEvent, _w, cx| {
                                                            this.start_tag_merge(
                                                                tag_for_merge.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("modal-del-btn-{}", tag_name)
                                                            .into(),
                                                    ))
                                                    .px_2()
                                                    .py_1()
                                                    .rounded_md()
                                                    .bg(theme::bg_raised())
                                                    .hover(|s| {
                                                        s.bg(theme::danger_bg())
                                                            .text_color(theme::text())
                                                    })
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(theme::text_muted())
                                                    .child(render_icon(
                                                        Icon::Trash,
                                                        px(12.0),
                                                        theme::text_muted(),
                                                    ))
                                                    .on_click(cx.listener(
                                                        move |this, _e: &ClickEvent, _w, cx| {
                                                            this.delete_tag_globally(
                                                                tag_for_delete.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .into_any_element()
                                    }),
                            )
                            .into_any_element()
                    })
                    .collect()
            })
    }
}
