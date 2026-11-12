//! The Tags page bulk bar: Merge / Delete / Deselect for two or more
//! checked keywords.

use gpui::*;

use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_bulk_bar(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
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
        .border_color(theme::line_2())
        .bg(theme::bg_float())
        .child(
            div()
                .flex_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
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
                .tooltip(theme::tooltip_label(
                    t("tags.bulk_delete_tooltip").replace("{n}", &n.to_string()),
                ))
                .size(px(28.0))
                .rounded(theme::RADIUS_MD)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(theme::danger_soft()))
                .child(render_icon(Icon::Trash, px(14.0), theme::danger()))
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
                .hover(|s| s.bg(theme::bg_3()))
                .child(render_icon(Icon::X, px(14.0), theme::t2()))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.clear_tag_manager_selection(cx);
                })),
        )
}
