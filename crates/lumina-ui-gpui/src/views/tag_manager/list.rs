//! Left column of the tag manager: filter, sort, checkable rows, bulk bar.

use crate::app::caret::caret;
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
    let shown_checked = rows
        .iter()
        .filter(|(tag, _)| selected.contains(tag))
        .count();
    let all_state = match shown_checked {
        0 => Check::Off,
        n if n == rows.len() => Check::On,
        _ => Check::Mixed,
    };
    let max_count = rows.iter().map(|(_, n)| *n).max().unwrap_or(0);

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
                            "A–Z",
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
                    checkbox("tag-manager-select-all", all_state)
                        .debug_selector(|| "tag-manager-select-all".to_string())
                        .tooltip(theme::tooltip_label(if all_state == Check::On {
                            "Deselect all shown (⌘A)"
                        } else {
                            "Select all shown (⌘A)"
                        }))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.toggle_all_tag_manager_rows(cx);
                        })),
                )
                .child(div().flex_1().child("NAME"))
                .child(div().w(px(USAGE_WIDTH)).child("USAGE")),
        )
        .child(
            div()
                .id("modal-tags-scroll-table")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .track_scroll(&app.tag_manager.list_scroll)
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
                        .map(|(tag, count)| render_row(app, tag, *count, max_count, cx)),
                ),
        )
        // The bar is for acting on several tags; one tag uses the detail.
        .when(selected.len() >= 2, |col| {
            col.child(render_bulk_bar(app, cx))
        })
}

fn render_search(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let search = &app.tag_manager.search;
    theme::field("tag-manager-search")
        .debug_selector(|| "tag-manager-search".to_string())
        .flex_1()
        .gap_2()
        // Typing filters whenever the sheet is open; the focus border shows
        // that the field is the keyboard target.
        .when(app.tag_manager.search_focused, |f| {
            f.border_color(theme::focus_ring())
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
                    format!("{search}{}", caret())
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

fn render_row(
    app: &LuminaApp,
    tag: &str,
    count: usize,
    max_count: usize,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let focused = app.tag_manager.focused.as_deref() == Some(tag);
    let checked = app.tag_manager.selected.contains(tag);
    let tag_owned = tag.to_string();
    let (row_tag, check_tag) = (tag_owned.clone(), tag_owned.clone());
    let row_selector = format!("tag-manager-row-{tag}");
    let check_selector = format!("tag-manager-checkbox-{tag}");
    let usage_selector = format!("tag-manager-usage-{tag}");
    let fill_selector = format!("tag-manager-usage-fill-{tag}");
    let fill = if max_count == 0 {
        0.0
    } else {
        USAGE_BAR * count as f32 / max_count as f32
    };

    theme::row(
        ElementId::Name(format!("modal-tag-row-{tag}").into()),
        focused,
    )
    .debug_selector(move || row_selector.clone())
    .h(px(36.0))
    .px(px(12.0))
    .gap(px(10.0))
    .rounded(px(0.0))
    .border_b_1()
    .border_color(theme::border_subtle())
    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
        this.focus_tag_manager_tag(row_tag.clone(), cx);
    }))
    .child(
        checkbox(
            ElementId::Name(format!("modal-tag-cb-{tag}").into()),
            if checked { Check::On } else { Check::Off },
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
    .child(
        div()
            .w(px(USAGE_WIDTH))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id(ElementId::Name(format!("tag-usage-{tag}").into()))
                    .debug_selector(move || usage_selector.clone())
                    .w(px(USAGE_BAR))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(theme::bg_control())
                    .child(
                        div()
                            .id(ElementId::Name(format!("tag-usage-fill-{tag}").into()))
                            .debug_selector(move || fill_selector.clone())
                            .w(px(fill))
                            .h_full()
                            .rounded_full()
                            .bg(theme::text_muted()),
                    ),
            )
            .child(
                div()
                    .font_family(theme::FONT_MONO)
                    .text_xs()
                    .text_color(if focused {
                        theme::selection_fg()
                    } else {
                        theme::text_subtle()
                    })
                    .child(count.to_string()),
            ),
    )
    .into_any_element()
}

/// Usage column: a 110px bar scaled to the most-used shown tag, then the count.
const USAGE_BAR: f32 = 110.0;
const USAGE_WIDTH: f32 = 150.0;

#[derive(Clone, Copy, PartialEq)]
enum Check {
    Off,
    On,
    /// Some of the shown rows are checked ("–").
    Mixed,
}

fn checkbox(id: impl Into<ElementId>, state: Check) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(16.0))
        .flex_shrink_0()
        .rounded(theme::RADIUS_SM)
        .border_1()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .map(|b| match state {
            Check::On => b
                .bg(theme::accent())
                .border_color(theme::accent())
                .child(render_icon(Icon::Check, px(11.0), theme::accent_fg())),
            Check::Mixed => b
                .bg(theme::accent())
                .border_color(theme::accent())
                .child(div().w(px(8.0)).h(px(2.0)).bg(theme::accent_fg())),
            Check::Off => b.border_color(theme::border_strong()),
        })
}

fn render_bulk_bar(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let selected: Vec<String> = app.tag_manager.selected.iter().cloned().collect();
    let n = selected.len();
    let target = app.tag_merge_target().unwrap_or_default();
    div()
        .id("tag-selection-bar")
        .debug_selector(|| "tag-selection-bar".to_string())
        .h(px(48.0))
        .flex_shrink_0()
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .border_t_1()
        .border_color(theme::border_strong())
        .bg(theme::bg_hud())
        .child(
            div()
                .flex_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text())
                .child(format!("{n} selected")),
        )
        .child(
            theme::control_primary("merge-tags-btn")
                .debug_selector(|| "merge-tags-btn".to_string())
                .tooltip(theme::tooltip_label(format!("Merge into “{target}” (⌘M)")))
                .child("Merge")
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.merge_checked_tags(cx);
                })),
        )
        .child(
            div()
                .id("bulk-delete-tags-btn")
                .debug_selector(|| "bulk-delete-tags-btn".to_string())
                .tooltip(theme::tooltip_label(format!("Delete {n} tags (⌘⌫)")))
                .size(px(28.0))
                .rounded(theme::RADIUS_MD)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(theme::danger_bg()))
                .child(render_icon(Icon::Trash, px(14.0), theme::danger_text()))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.bulk_delete_tags(selected.clone(), cx);
                })),
        )
        .child(
            div()
                .id("clear-tag-selection-btn")
                .debug_selector(|| "clear-tag-selection-btn".to_string())
                .tooltip(theme::tooltip_label("Deselect all (Esc)"))
                .size(px(28.0))
                .rounded(theme::RADIUS_MD)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(theme::bg_control_hover()))
                .child(render_icon(Icon::X, px(14.0), theme::text_muted()))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.clear_tag_manager_selection(cx);
                })),
        )
}
