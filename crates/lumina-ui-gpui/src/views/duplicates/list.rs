//! The group list pane: a head with the position, then one virtualized row
//! per exact-copy group.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::DuplicateGroup;

use super::{group_summary, group_title, texture, GROUP_LIST_WIDTH};
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

const ROW_HEIGHT: f32 = 75.5;
const THUMB: f32 = 40.0;

pub(super) fn render(
    this: &LuminaApp,
    visible: &[&DuplicateGroup],
    cx: &mut Context<LuminaApp>,
) -> Div {
    let len = visible.len();
    let position = this.stacks.review.selected(visible).map_or(0, |i| i + 1);
    let position: SharedString = t_with(
        "duplicates.position",
        &[&crate::i18n::count(position), &crate::i18n::count(len)],
    )
    .into();
    div()
        .debug_selector(|| "duplicates-group-list".to_string())
        .w(px(GROUP_LIST_WIDTH))
        .flex_shrink_0()
        .h_full()
        .bg(theme::bg_1())
        .border_r_1()
        .border_color(theme::line())
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(43.5))
                .flex_shrink_0()
                .px(px(18.0))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .border_b_1()
                .border_color(theme::line())
                // ponytail: the design's +0.9 tracking is skipped; GPUI
                // 0.3.11 has no letter-spacing style.
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t3())
                        .child(t("duplicates.groups_head")),
                )
                .child(
                    div()
                        .debug_selector(|| "duplicates-group-position".to_string())
                        .font_family(theme::FONT_MONO)
                        .text_size(px(10.5))
                        .text_color(theme::t3())
                        .children(text_marker("duplicates-position", &position))
                        .child(position),
                ),
        )
        .child(
            div()
                .id("duplicates-groups")
                .role(Role::List)
                .aria_label(t("duplicates.groups_list"))
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    // Virtualized: only the visible rows are built and ask
                    // for thumbnails.
                    uniform_list(
                        "duplicates-group-rows",
                        len,
                        cx.processor(|this, range: std::ops::Range<usize>, _window, cx| {
                            let visible = this.stacks.review.visible(&this.duplicate_groups);
                            let selected = this.stacks.review.selected(&visible);
                            range
                                .filter_map(|index| {
                                    let group = visible.get(index)?;
                                    Some(row(this, index, group, selected == Some(index), cx))
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(&this.stacks.review.scroll)
                    .flex_1()
                    .min_h_0(),
                ),
        )
}

fn row(
    this: &LuminaApp,
    index: usize,
    group: &DuplicateGroup,
    selected: bool,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let title = group_title(group);
    let hash = group.hash.clone();
    let thumbs: Vec<_> = group
        .items
        .iter()
        .take(2)
        .map(|i| texture(this, i))
        .collect();
    div()
        .id(("duplicates-group-row", index))
        .debug_selector(move || format!("duplicates-group-row-{index}"))
        .role(Role::ListItem)
        .aria_label(title.clone())
        .aria_selected(selected)
        .tooltip(theme::tooltip_label(title.clone()))
        .relative()
        .h(px(ROW_HEIGHT))
        .px(px(18.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .border_b_1()
        .border_color(theme::line())
        .when(selected, |row| row.bg(theme::bg_2()))
        .hover(|s| s.bg(theme::bg_2()))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.stacks.review.select(index, &hash);
            cx.notify();
        }))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.0))
                .when(selected, |marker| marker.bg(theme::accent())),
        )
        // Two copies stacked: the front one sits lower right with a bg-1 rim.
        .child(
            div()
                .relative()
                .flex_shrink_0()
                .size(px(THUMB + 8.0))
                .children(thumbs.into_iter().enumerate().rev().map(|(i, tex)| {
                    let offset = if i == 0 { 8.0 } else { 0.0 };
                    let thumb = div()
                        .absolute()
                        .left(px(offset))
                        .top(px(offset))
                        .size(px(THUMB))
                        .rounded(theme::RADIUS_MD)
                        .overflow_hidden()
                        .bg(theme::bg_3())
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(i == 0, |front| front.border_2().border_color(theme::bg_1()));
                    match tex {
                        Some(tex) => thumb.child(img(tex).size_full()),
                        None => thumb.child(render_icon(Icon::Image, px(14.0), theme::t3())),
                    }
                })),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if selected { theme::t1() } else { theme::t2() })
                        .child(title),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(10.5))
                        .text_color(theme::t3())
                        .child(group_summary(group)),
                ),
        )
        .child(kind_badge())
        .into_any_element()
}

/// The row's kind badge. Only exact copies exist so far.
fn kind_badge() -> Div {
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
        .child(t("duplicates.kind_exact"))
}
