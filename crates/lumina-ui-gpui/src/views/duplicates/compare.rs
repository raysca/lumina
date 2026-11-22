//! The compare pane: the selected group's head, its copies side by side,
//! and the footer that merges it.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{DuplicateGroup, MediaId};

use super::card::{card, CARD_MAX_HEIGHT};
use super::{group_summary, group_title};
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// The footer text's narrowest beside the buttons: "frees …" and a summary
/// of at most three lines. At 1280 wide it gets about 313 (two lines).
const TEXT_MIN_WIDTH: f32 = 280.0;

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
    let footer_height = this.stacks.review.footer_height.clone();
    // The keeper is KEEP; the others are COPY 1, 2… in group order.
    let mut copies = 0;
    let mut cards = Vec::with_capacity(group.items.len());
    for (i, item) in group.items.iter().enumerate() {
        let copy = (item.id != keeper).then(|| {
            copies += 1;
            copies
        });
        cards.push(card(this, &group.hash, i, item, copy, cx));
    }
    div()
        .debug_selector(|| "duplicates-compare".to_string())
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .child(head(visible, index, keeper, cx))
        .child(
            div()
                .id("duplicates-candidates")
                .flex_1()
                .min_h_0()
                // The cards take what the differences line leaves and
                // their photos shrink to fit; past the photos' minimum
                // the pane scrolls.
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("duplicates-candidates-row")
                        .debug_selector(|| "duplicates-candidates-row".to_string())
                        // Grows from the cards' height at the photos'
                        // minimum up to the cards' cap (plus the 20 px
                        // padding), and never shrinks below the minimum.
                        .flex_grow(1.0)
                        .flex_shrink_0()
                        .max_h(px(CARD_MAX_HEIGHT + 40.0))
                        .overflow_x_scroll()
                        .px(px(24.0))
                        .py(px(20.0))
                        .flex()
                        .flex_row()
                        .gap(px(14.0))
                        .children(cards),
                )
                .child(super::differences::render(this, group).flex_shrink_0()),
        )
        .child(
            div()
                .debug_selector(|| "duplicates-footer".to_string())
                .relative()
                .min_h(px(super::FOOTER_MIN_HEIGHT))
                .flex_shrink_0()
                .px(px(24.0))
                .py(px(12.0))
                .bg(theme::bg_2())
                .border_t_1()
                .border_color(theme::line())
                .flex()
                .flex_row()
                .flex_wrap()
                .justify_end()
                .items_center()
                .gap(px(8.0))
                // Records the laid-out height for the toast. This runs in
                // prepaint, where `refresh` is ignored, so a change asks for
                // the next frame instead. The canvas sits inside the 1 px
                // top border: add it back.
                .child(
                    canvas(
                        move |bounds, window, _| {
                            let height = f32::from(bounds.size.height) + 1.0;
                            if footer_height.replace(height) != height {
                                window.request_animation_frame();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                // The summary wraps rather than truncating, so it never
                // shows a fragment, and "frees …" and the buttons stay
                // whole: the text sits beside the buttons while it gets
                // `TEXT_MIN_WIDTH`, else it takes its own line above them.
                .child(
                    div()
                        .debug_selector(|| "duplicates-footer-text".to_string())
                        .flex_1()
                        .min_w(px(TEXT_MIN_WIDTH))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .debug_selector(|| "duplicates-footer-summary".to_string())
                                .min_w_0()
                                .text_size(theme::TEXT_BODY)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::t1())
                                .children(text_marker("duplicates-summary", &summary))
                                .child(summary),
                        )
                        .child(
                            div()
                                .debug_selector(|| "duplicates-footer-frees".to_string())
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .font_family(theme::FONT_SANS)
                                .font_features(theme::tabular_figures())
                                .text_size(theme::TEXT_SMALL)
                                .text_color(theme::t3())
                                .children(text_marker("duplicates-frees", &frees))
                                .child(frees),
                        ),
                )
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
                        .flex_shrink_0()
                        .h(px(36.0))
                        .px(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .rounded(theme::RADIUS_LG)
                        .bg(theme::bg_3())
                        .border_1()
                        .border_color(theme::line_2())
                        .hover(|s| s.bg(theme::bg_2()))
                        .cursor_pointer()
                        .text_size(theme::TEXT_SMALL)
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
                        .flex_shrink_0()
                        .h(px(36.0))
                        .px(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .rounded(theme::RADIUS_LG)
                        .bg(theme::accent())
                        .hover(|s| s.bg(theme::accent_hover()))
                        .cursor_pointer()
                        .text_size(theme::TEXT_SMALL)
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
                                .font_family(theme::FONT_SANS)
                                .font_features(theme::tabular_figures())
                                .text_size(theme::TEXT_CAPTION)
                                .child("⏎"),
                        )
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.duplicates_merge_shown(cx);
                        })),
                ),
        )
}

fn head(
    visible: &[&DuplicateGroup],
    index: usize,
    keeper: MediaId,
    cx: &mut Context<LuminaApp>,
) -> Div {
    let group = visible[index];
    let len = visible.len();
    let step = |i: usize| (i < len).then_some(i);
    let title: SharedString = group_title(group, keeper).into();
    div()
        .debug_selector(|| "duplicates-compare-head".to_string())
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
                .debug_selector(|| "duplicates-compare-title".to_string())
                .min_w_0()
                .truncate()
                .tooltip(theme::tooltip_label(title.clone()))
                .font_family(theme::FONT_DISPLAY)
                .text_size(theme::TEXT_LARGE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .children(text_marker("duplicates-compare-title", &title))
                .child(title),
        )
        .child(
            div()
                .debug_selector(|| "duplicates-compare-badge".to_string())
                .flex_shrink_0()
                .h(px(20.0))
                .px(px(7.0))
                .flex()
                .items_center()
                .rounded(theme::RADIUS_SM)
                .bg(theme::accent_soft())
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_CAPTION)
                .text_color(theme::accent())
                .child(t("duplicates.kind_exact_match")),
        )
        .child(
            div()
                .debug_selector(|| "duplicates-compare-summary".to_string())
                // Gives way well before the group name does: a narrow window
                // shortens the summary and, at the 960 px minimum, hides it,
                // so the name stays readable.
                .flex_shrink(8.0)
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_CAPTION)
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
            theme::ICON_SM,
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
