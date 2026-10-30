//! Left column of the tag manager: filter, sort, checkable rows, bulk bar.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::{LuminaApp, TagSortOrder};

pub fn render_list(
    app: &LuminaApp,
    rows: &[(String, usize)],
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let selected = &app.tag_manager.selected;
    let all_checked = !rows.is_empty() && rows.iter().all(|(tag, _)| selected.contains(tag));
    let merge_source = app.tag_manager.merge_source.clone();

    div()
        .w(px(440.0))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(theme::border_subtle())
        // Filter field and sort.
        .child(
            div()
                .p(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(render_search(app, cx))
                .child(
                    theme::segmented("tag-sort")
                        .child(sort_item(
                            "tag-sort-az-btn",
                            "Name",
                            TagSortOrder::Alphabetical,
                            app,
                            cx,
                        ))
                        .child(sort_item(
                            "tag-sort-count-btn",
                            "Count",
                            TagSortOrder::CountDesc,
                            app,
                            cx,
                        )),
                ),
        )
        .children(merge_source.map(|source| render_merge_banner(&source, cx)))
        // Column header with select-all.
        .child(
            div()
                .h(px(28.0))
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .border_t_1()
                .border_b_1()
                .border_color(theme::border_subtle())
                .bg(theme::bg_pane())
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text_subtle())
                .child(
                    checkbox("tag-manager-select-all", all_checked)
                        .debug_selector(|| "tag-manager-select-all".to_string())
                        .tooltip(theme::tooltip_label(if all_checked {
                            "Deselect all"
                        } else {
                            "Select all shown"
                        }))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.toggle_all_tag_manager_rows(cx);
                        })),
                )
                .child(div().flex_1().child("TAG"))
                .child("ITEMS"),
        )
        .child(
            div()
                .id("modal-tags-scroll-table")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .track_scroll(&app.tag_manager.list_scroll)
                .p(px(4.0))
                .when(rows.is_empty(), |list| {
                    list.child(
                        div()
                            .p_4()
                            .text_xs()
                            .text_color(theme::text_subtle())
                            .child(if app.library_tag_counts.is_empty() {
                                "No tags yet. Add tags from the inspector."
                            } else {
                                "No tags match the filter."
                            }),
                    )
                })
                .children(
                    rows.iter()
                        .map(|(tag, count)| render_row(app, tag, *count, cx)),
                ),
        )
        .when(!selected.is_empty(), |col| {
            col.child(render_bulk_bar(app, cx))
        })
}

fn render_search(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let search = &app.tag_manager.search;
    theme::field("tag-manager-search")
        .debug_selector(|| "tag-manager-search".to_string())
        .flex_1()
        .gap_2()
        // Typing filters whenever the sheet is open; the accent border shows
        // that the field is the keyboard target.
        .when(app.tag_manager.search_focused, |f| {
            f.border_color(theme::accent())
        })
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.tag_manager.search_focused = true;
            cx.notify();
        }))
        .child(render_icon(Icon::Search, px(12.0), theme::text_subtle()))
        .child(
            div()
                .text_color(if search.is_empty() {
                    theme::text_subtle()
                } else {
                    theme::text()
                })
                .child(if search.is_empty() {
                    "Filter tags by name…".to_string()
                } else {
                    format!("{search}|")
                }),
        )
}

fn sort_item(
    id: &'static str,
    label: &'static str,
    order: TagSortOrder,
    app: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    theme::segmented_item(id, app.tag_manager.sort == order)
        .debug_selector(move || id.to_string())
        .child(label)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.set_tag_sort_order(order, cx);
        }))
}

fn render_row(app: &LuminaApp, tag: &str, count: usize, cx: &mut Context<LuminaApp>) -> AnyElement {
    let focused = app.tag_manager.focused.as_deref() == Some(tag);
    let checked = app.tag_manager.selected.contains(tag);
    let merge_source = app.tag_manager.merge_source.as_deref();
    let tag_owned = tag.to_string();
    let (row_tag, check_tag, merge_tag) = (tag_owned.clone(), tag_owned.clone(), tag_owned.clone());
    let row_selector = format!("tag-manager-row-{tag}");
    let check_selector = format!("tag-manager-checkbox-{tag}");

    theme::row(
        ElementId::Name(format!("modal-tag-row-{tag}").into()),
        focused,
    )
    .debug_selector(move || row_selector.clone())
    .h(px(32.0))
    .px(px(8.0))
    .gap(px(10.0))
    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
        this.focus_tag_manager_tag(row_tag.clone(), cx);
    }))
    .child(
        checkbox(
            ElementId::Name(format!("modal-tag-cb-{tag}").into()),
            checked,
        )
        .debug_selector(move || check_selector.clone())
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            // The row would otherwise also take the click and move focus.
            cx.stop_propagation();
            this.toggle_tag_manager_selection(check_tag.clone(), cx);
        })),
    )
    .child(
        div()
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .text_sm()
            .child(tag_owned),
    )
    .child(match merge_source {
        Some(source) if source == tag => div()
            .text_xs()
            .text_color(theme::accent())
            .child("merging…")
            .into_any_element(),
        Some(_) => theme::control(ElementId::Name(format!("modal-merge-target-{tag}").into()))
            .h(px(22.0))
            .child("Merge here")
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.commit_tag_merge(merge_tag.clone(), cx);
            }))
            .into_any_element(),
        None => div()
            .font_family(theme::FONT_MONO)
            .text_xs()
            .text_color(if focused {
                theme::selection_fg()
            } else {
                theme::text_subtle()
            })
            .child(count.to_string())
            .into_any_element(),
    })
    .into_any_element()
}

fn checkbox(id: impl Into<ElementId>, checked: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(16.0))
        .flex_shrink_0()
        .rounded(px(3.0))
        .border_1()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .map(|b| {
            if checked {
                b.bg(theme::accent())
                    .border_color(theme::accent())
                    .child(render_icon(Icon::Check, px(11.0), theme::accent_fg()))
            } else {
                b.border_color(theme::border_strong())
            }
        })
}

fn render_merge_banner(source: &str, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .mx(px(12.0))
        .mb(px(10.0))
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(theme::selection_bg())
        .border_1()
        .border_color(theme::selection_border())
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .text_xs()
        .text_color(theme::selection_fg())
        .child(
            div()
                .flex_1()
                .child(format!("Merging “{source}” — pick the tag to merge into")),
        )
        .child(
            theme::control("cancel-merge-btn")
                .h(px(22.0))
                .child("Cancel")
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.cancel_tag_merge(cx);
                })),
        )
}

fn render_bulk_bar(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let selected: Vec<String> = app.tag_manager.selected.iter().cloned().collect();
    let n = selected.len();
    div()
        .h(px(48.0))
        .flex_shrink_0()
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .border_t_1()
        .border_color(theme::border_subtle())
        .bg(theme::bg_pane())
        .child(
            div()
                .flex_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text())
                .child(format!("{n} selected")),
        )
        .child(
            theme::control("bulk-delete-tags-btn")
                .debug_selector(|| "bulk-delete-tags-btn".to_string())
                .text_color(theme::danger_text())
                .child(format!("Delete {n}"))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.bulk_delete_tags(selected.clone(), cx);
                })),
        )
        .child(
            theme::control("clear-tag-selection-btn")
                .child("Deselect All")
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.clear_tag_manager_selection(cx);
                })),
        )
}
