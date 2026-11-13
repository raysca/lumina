//! The compare pane: the selected group's head, its copies side by side,
//! and the footer that merges it.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{DuplicateGroup, MediaId};

use super::card::card;
use super::{group_summary, group_title};
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

pub(super) fn render(
    this: &LuminaApp,
    visible: &[&DuplicateGroup],
    index: usize,
    cx: &mut Context<LuminaApp>,
) -> Div {
    let group = visible[index];
    let keeper = this.stacks.review.keeper(group);
    let trash: Vec<MediaId> = group
        .items
        .iter()
        .map(|i| i.id)
        .filter(|id| *id != keeper)
        .collect();
    let freed: u64 = group
        .items
        .iter()
        .filter(|i| i.id != keeper)
        .map(|i| i.file_size)
        .sum();
    let summary: SharedString = format!(
        "{} · {}",
        t_with("duplicates.keeping", &[&group.items.len().to_string()]),
        if trash.len() == 1 {
            t("duplicates.moves_one")
        } else {
            t_with("duplicates.moves_other", &[&trash.len().to_string()])
        }
    )
    .into();
    let frees: SharedString = t_with("duplicates.frees", &[&format_bytes(freed)]).into();
    div()
        .debug_selector(|| "duplicates-compare".to_string())
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .child(head(visible, index, cx))
        .child(
            div()
                .id("duplicates-candidates")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    div()
                        .id("duplicates-candidates-row")
                        .overflow_x_scroll()
                        .px(px(24.0))
                        .py(px(20.0))
                        .flex()
                        .flex_row()
                        .gap(px(14.0))
                        .children(group.items.iter().enumerate().map(|(i, item)| {
                            card(this, &group.hash, i, item, item.id == keeper, cx)
                        })),
                )
                .child(super::differences::render(this, group)),
        )
        .child(
            div()
                .h(px(63.5))
                .flex_shrink_0()
                .px(px(24.0))
                .bg(theme::bg_2())
                .border_t_1()
                .border_color(theme::line())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .children(text_marker("duplicates-summary", &summary))
                        .child(summary),
                )
                .child(
                    div()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.5))
                        .text_color(theme::t3())
                        .children(text_marker("duplicates-frees", &frees))
                        .child(frees),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .id("duplicates-skip-group")
                        .debug_selector(|| "duplicates-skip-group".to_string())
                        .role(Role::Button)
                        .aria_label(t("duplicates.skip_group"))
                        .aria_keyshortcuts("S")
                        .tooltip(theme::tooltip_label(t_with(
                            "duplicates.with_key",
                            &[&t("duplicates.skip_group_hint"), "S"],
                        )))
                        .h(px(36.0))
                        .px(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .rounded(px(9.0))
                        .bg(theme::bg_3())
                        .border_1()
                        .border_color(theme::line_2())
                        .hover(|s| s.bg(theme::bg_2()))
                        .cursor_pointer()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .child(t("duplicates.skip_group"))
                        .child(theme::kbd("S"))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.duplicates_skip_shown(cx);
                        })),
                )
                .child(
                    div()
                        .id("duplicates-merge-group")
                        .debug_selector(|| "duplicates-merge-group".to_string())
                        .role(Role::Button)
                        .aria_label(t("duplicates.merge_group"))
                        .aria_keyshortcuts("Enter")
                        .tooltip(theme::tooltip_label(t_with(
                            "duplicates.with_key",
                            &[&t("duplicates.merge_group_hint"), "⏎"],
                        )))
                        .h(px(36.0))
                        .px(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .rounded(px(9.0))
                        .bg(theme::accent())
                        .hover(|s| s.bg(theme::accent_hover()))
                        .cursor_pointer()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::on_accent())
                        .child(t("duplicates.merge_group"))
                        // The plan's kbd on accent: on-accent at 20%.
                        .child(
                            div()
                                .px(px(5.0))
                                .py(px(1.0))
                                .rounded(theme::RADIUS_SM)
                                .bg(theme::on_accent_kbd())
                                .font_family(theme::FONT_MONO)
                                .text_size(px(11.0))
                                .child("⏎"),
                        )
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.duplicates_merge_shown(cx);
                        })),
                ),
        )
}

fn head(visible: &[&DuplicateGroup], index: usize, cx: &mut Context<LuminaApp>) -> Div {
    let group = visible[index];
    let len = visible.len();
    let step = |i: usize| (i < len).then_some(i);
    let title = group_title(group);
    div()
        .h(px(55.5))
        .flex_shrink_0()
        .px(px(24.0))
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .id("duplicates-compare-title")
                .min_w_0()
                .truncate()
                .tooltip(theme::tooltip_label(title.clone()))
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(title),
        )
        .child(
            div()
                .flex_shrink_0()
                .h(px(20.0))
                .px(px(7.0))
                .flex()
                .items_center()
                .rounded(px(5.0))
                .bg(theme::accent_soft())
                .font_family(theme::FONT_MONO)
                .text_size(px(10.0))
                .text_color(theme::accent())
                .child(t("duplicates.kind_exact_match")),
        )
        .child(
            div()
                .flex_shrink_0()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.5))
                .text_color(theme::t3())
                .child(format!(
                    "{} · {}",
                    group_summary(group),
                    t_with(
                        "duplicates.group_position",
                        &[&crate::i18n::count(index + 1), &crate::i18n::count(len)]
                    )
                )),
        )
        .child(div().flex_1())
        .child(step_button(
            "duplicates-prev",
            Icon::ChevronLeft,
            t("duplicates.prev"),
            "↑",
            index.checked_sub(1).and_then(step),
            cx,
        ))
        .child(step_button(
            "duplicates-next",
            Icon::ChevronRight,
            t("duplicates.next"),
            "↓",
            step(index + 1),
            cx,
        ))
}

/// A 30×30 prev/next button; dimmed and inert when `target` is None.
fn step_button(
    selector: &'static str,
    icon: Icon,
    label: String,
    key: &'static str,
    target: Option<usize>,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_keyshortcuts(if key == "↑" { "ArrowUp" } else { "ArrowDown" })
        .tooltip(theme::tooltip_label(t_with(
            "duplicates.with_key",
            &[&label, key],
        )))
        .flex_shrink_0()
        .size(px(30.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_LG)
        .bg(theme::bg_2())
        .border_1()
        .border_color(theme::line())
        .child(render_icon(
            icon,
            px(14.0),
            if target.is_some() {
                theme::t2()
            } else {
                theme::t3()
            },
        ))
        .when_some(target, |button, target| {
            button
                .hover(|s| s.bg(theme::bg_3()))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.duplicates_select_group(target, cx);
                }))
        })
}
