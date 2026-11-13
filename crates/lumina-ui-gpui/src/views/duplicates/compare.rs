//! The compare pane: the selected group's head, its copies side by side,
//! and the footer that merges it.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{DuplicateGroup, MediaId, MediaItem};

use super::{group_summary, group_title, texture};
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// Cards share the width but never get narrower than this; past that the
/// row scrolls sideways.
const CARD_MIN_WIDTH: f32 = 180.0;

pub(super) fn render(this: &LuminaApp, index: usize, cx: &mut Context<LuminaApp>) -> Div {
    let group = &this.duplicate_groups[index];
    let keeper = group.items.first().map_or(MediaId(0), |i| i.id);
    let trash: Vec<MediaId> = group.items.iter().skip(1).map(|i| i.id).collect();
    div()
        .debug_selector(|| "duplicates-compare".to_string())
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .child(head(this, index, group, cx))
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
                        .children(
                            group
                                .items
                                .iter()
                                .enumerate()
                                .map(|(i, item)| card(this, i, item, cx)),
                        ),
                ),
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
                .justify_end()
                .child(
                    div()
                        .id("duplicates-merge-group")
                        .debug_selector(|| "duplicates-merge-group".to_string())
                        .role(Role::Button)
                        .aria_label(t("duplicates.merge_group"))
                        .tooltip(theme::tooltip_label(t("duplicates.merge_group_hint")))
                        .h(px(36.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .rounded(px(9.0))
                        .bg(theme::accent())
                        .hover(|s| s.bg(theme::accent_hover()))
                        .cursor_pointer()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::on_accent())
                        .child(t("duplicates.merge_group"))
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.merge_duplicate_group(keeper, &trash, cx);
                        })),
                ),
        )
}

fn head(
    this: &LuminaApp,
    index: usize,
    group: &DuplicateGroup,
    cx: &mut Context<LuminaApp>,
) -> Div {
    let len = this.duplicate_groups.len();
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
            index.checked_sub(1),
            cx,
        ))
        .child(step_button(
            "duplicates-next",
            Icon::ChevronRight,
            t("duplicates.next"),
            (index + 1 < len).then_some(index + 1),
            cx,
        ))
}

/// A 30×30 prev/next button; dimmed and inert when `target` is None.
fn step_button(
    selector: &'static str,
    icon: Icon,
    label: String,
    target: Option<usize>,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(label))
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
                    this.stacks.review.select(target);
                    cx.notify();
                }))
        })
}

/// One copy: the image with its tag and resolution, then its facts. The
/// keeper (the first copy) has the accent outline; the selected card is
/// lifted to bg-3 with the focus ring instead.
fn card(
    this: &LuminaApp,
    position: usize,
    item: &MediaItem,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let id = item.id.0;
    let keeper = position == 0;
    let selected = this.selected_item.as_ref().is_some_and(|i| i.id == item.id);
    let item_clone = item.clone();
    let tag = if keeper {
        t("duplicates.original")
    } else {
        t_with("duplicates.copy_n", &[&position.to_string()])
    };
    let format = std::path::Path::new(&item.file_name)
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    let source = match (item.make.as_deref(), item.model.as_deref()) {
        (Some(make), Some(model)) if model.starts_with(make) => model.to_string(),
        (Some(make), Some(model)) => format!("{make} {model}"),
        (Some(one), None) | (None, Some(one)) => one.to_string(),
        (None, None) => item.parent_dir.clone(),
    };
    let edits = if this.edits.edited_ids.contains(&item.id) {
        t("duplicates.edited")
    } else {
        t("duplicates.not_edited")
    };
    let resolution =
        (item.width > 0 && item.height > 0).then(|| format!("{} × {}", item.width, item.height));
    div()
        .id(("duplicate-candidate", id))
        .debug_selector(move || format!("duplicate-candidate-{id}"))
        .role(Role::Group)
        .aria_label(item.file_name.clone())
        .flex_1()
        .min_w(px(CARD_MIN_WIDTH))
        .rounded(px(12.0))
        .bg(if selected {
            theme::bg_3()
        } else {
            theme::bg_1()
        })
        .map(|card| {
            if keeper {
                card.border(px(1.5))
            } else {
                card.border_1()
            }
        })
        // The selection (what ⌫ and ⌘E act on) wins over the keeper's accent.
        .border_color(if selected {
            theme::focus_ring()
        } else if keeper {
            theme::accent()
        } else {
            theme::line()
        })
        .children(
            selected
                .then(|| text_marker("duplicate-selected", &id.to_string().into()))
                .flatten(),
        )
        .overflow_hidden()
        .flex()
        .flex_col()
        .cursor_pointer()
        .on_click(cx.listener(move |this, e: &ClickEvent, _w, cx| {
            if e.click_count() >= 2 {
                this.open_viewer(item_clone.clone(), cx);
            } else {
                this.selected_item = Some(item_clone.clone());
                this.selected_index = None;
                this.selected_indices.clear();
                this.refresh_selected_tags();
                cx.notify();
            }
        }))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(400.0))
                .bg(theme::bg_media())
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(match texture(this, item) {
                    Some(tex) => img(tex)
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .into_any_element(),
                    None => render_icon(Icon::Image, px(24.0), theme::t3()).into_any_element(),
                })
                .child(media_tag(tag).top(px(10.0)).left(px(10.0)))
                .children(resolution.map(|r| media_tag(r).bottom(px(10.0)).left(px(10.0)))),
        )
        .child(
            div()
                .p(px(14.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(fact(
                    "duplicates.fact_size",
                    format_bytes(item.file_size),
                    false,
                ))
                .child(fact("duplicates.fact_format", format, false))
                .child(fact("duplicates.fact_edits", edits, false))
                .child(fact("duplicates.fact_source", source, false))
                .child(fact(
                    "duplicates.fact_captured",
                    item.formatted_timestamp(),
                    true,
                ))
                .child(fact(
                    "duplicates.fact_folder",
                    item.parent_dir.clone(),
                    false,
                )),
        )
}

/// A mono label over the image, on the media scrim.
fn media_tag(text: impl Into<SharedString>) -> Div {
    div()
        .absolute()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(theme::RADIUS_SM)
        .bg(theme::media_scrim())
        .font_family(theme::FONT_MONO)
        .text_size(px(10.0))
        .text_color(theme::on_media())
        .child(text.into())
}

/// A key/value row; `dim` draws the value in t3 (Captured).
fn fact(key: &str, value: String, dim: bool) -> Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(11.5))
                .text_color(theme::t3())
                .child(t(key)),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_MONO)
                .text_size(px(11.5))
                .text_color(if dim { theme::t3() } else { theme::t1() })
                .child(value),
        )
}
