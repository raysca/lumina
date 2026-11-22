//! The filter row over the Duplicates destination: kind chips with their
//! counts, and how the groups are sorted.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::stacks::{DuplicatesTab, StacksList};
use crate::i18n::t;
use crate::theme::{self, ChipKind};
use crate::LuminaApp;

/// "All", "Exact match" and "Similar shots". Only byte-identical groups
/// exist, so All and Exact match are the same review: both are lit on it,
/// but only All reports selected, and only Exact match carries the count
/// (All is not a sum). The Similar count is known once that review is read.
pub(crate) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let exact = this.stacks.tab == DuplicatesTab::Exact;
    let groups = this.stacks.review.visible(this.duplicate_groups()).len();
    let similar = match &this.stacks.list {
        StacksList::Loaded(stacks) => Some(stacks.len()),
        _ => None,
    };
    div()
        .id("duplicates-kinds")
        .role(Role::TabList)
        .aria_label(t("duplicates.title"))
        .w_full()
        .h(px(51.5))
        .flex_shrink_0()
        .px(px(28.0))
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(chip(
            "duplicates-kind-all",
            t("duplicates.kind_all"),
            "duplicates.tab_exact_hint",
            None,
            (exact, exact),
            DuplicatesTab::Exact,
            cx,
        ))
        .child(chip(
            "duplicates-kind-exact",
            t("duplicates.kind_exact_match"),
            "duplicates.tab_exact_hint",
            Some(groups),
            (exact, false),
            DuplicatesTab::Exact,
            cx,
        ))
        .child(chip(
            "duplicates-kind-similar",
            t("duplicates.tab_similar"),
            "duplicates.tab_similar_hint",
            similar,
            (!exact, !exact),
            DuplicatesTab::Similar,
            cx,
        ))
        .child(div().flex_1())
        // ponytail: one sort exists, so a label and no sort button.
        .when(exact, |row| {
            row.child(
                div()
                    .debug_selector(|| "duplicates-sorted".to_string())
                    .flex_shrink_0()
                    .font_family(theme::FONT_SANS)
                    .font_features(theme::tabular_figures())
                    .text_size(theme::TEXT_CAPTION)
                    .text_color(theme::t3())
                    .child(t("duplicates.sorted")),
            )
        })
}

/// A kind chip: accent-soft with an accent outline while lit, else bg-2
/// with a `line` outline and t2 text; the count in mono after the label.
/// `(lit, selected)`: what it looks like, and what it reports as a tab.
fn chip(
    selector: &'static str,
    label: String,
    hint: &str,
    count: Option<usize>,
    (on, selected): (bool, bool),
    tab: DuplicatesTab,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    theme::chip(selector, ChipKind::Normal)
        .debug_selector(move || selector.to_string())
        .role(Role::Tab)
        .aria_label(label.clone())
        .aria_selected(selected)
        .tooltip(theme::tooltip_label(t(hint)))
        .flex_shrink_0()
        .cursor_pointer()
        .map(|chip| {
            if on {
                chip.bg(theme::accent_soft())
                    .border_color(theme::accent())
                    .text_color(theme::t1())
            } else {
                chip.text_color(theme::t2()).hover(|s| s.bg(theme::bg_3()))
            }
        })
        .child(label)
        .children(count.map(|n| {
            div()
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_CAPTION)
                .text_color(if on { theme::accent() } else { theme::t3() })
                .child(crate::i18n::count(n))
        }))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            // All and Exact match share a review: switching between them
            // must not clear the selection.
            if this.stacks.tab != tab {
                this.set_duplicates_tab(tab, cx);
            }
        }))
}
