//! Review Duplicates: the exact-copy groups in a list on
//! the left, the selected group's copies side by side on the right.

mod card;
mod compare;
mod differences;
pub(crate) mod kinds;
mod list;

use std::sync::Arc;

use gpui::*;
use lumina_core::{DuplicateGroup, MediaItem};

use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::header_primary_button;
use crate::LuminaApp;

/// Width of the group list pane.
const GROUP_LIST_WIDTH: f32 = 372.0;

/// The compare footer's height on one row; it wraps taller when narrow.
pub(crate) const FOOTER_MIN_HEIGHT: f32 = 63.5;

/// The Exact copies body: the empty state, or the list and compare panes.
pub fn render(this: &LuminaApp, total_count: usize, cx: &mut Context<LuminaApp>) -> AnyElement {
    let visible = this.stacks.review.visible(&this.duplicate_groups);
    // Nothing found, or every group skipped. Groups still loading (none
    // yet, a count already) draw the empty panes instead.
    if visible.is_empty() && (total_count == 0 || !this.duplicate_groups.is_empty()) {
        return this.render_empty_state(cx);
    }
    div()
        .id("duplicates-view")
        .debug_selector(|| "duplicates-view".to_string())
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_row()
        .child(list::render(this, &visible, cx))
        .children(
            this.stacks
                .review
                .selected(&visible)
                .map(|index| compare::render(this, &visible, index, cx)),
        )
        .into_any_element()
}

/// The page header's meta line: "128 groups · 412 items · 18.4 GB recoverable".
/// Skipped groups are left out.
pub(crate) fn meta(this: &LuminaApp) -> String {
    let groups = this.stacks.review.visible(&this.duplicate_groups);
    let items: usize = groups.iter().map(|g| g.items.len()).sum();
    let bytes: u64 = groups.iter().map(|g| g.potential_reclaimable_bytes).sum();
    format!(
        "{} · {} · {}",
        crate::i18n::plural("duplicates.groups", groups.len()),
        crate::i18n::plural("header.items", items),
        t_with("duplicates.recoverable", &[&format_bytes(bytes)])
    )
}

/// The header's "Merge {n} groups": merges every group into its keeper.
pub(crate) fn merge_all_button(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let n = this.stacks.review.visible(&this.duplicate_groups).len();
    let label = if n == 1 {
        t("duplicates.merge_groups_one")
    } else {
        t("duplicates.merge_groups_other").replace("{n}", &crate::i18n::count(n))
    };
    header_primary_button(
        "merge-all-duplicates",
        Icon::Combine,
        label,
        t("duplicates.merge_groups_hint"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.merge_all_duplicates(cx);
    }))
}

/// The header's "Auto-select best": every group back to the heuristic pick.
pub(crate) fn auto_best_button(cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    header_button(
        "duplicates-auto-best",
        Icon::WandSparkles,
        t("duplicates.auto_best"),
        t("duplicates.auto_best_hint"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.duplicates_auto_best(cx);
    }))
}

/// The header's "Skip all": hides every group until Duplicates is opened
/// again. The catalog is not touched.
pub(crate) fn skip_all_button(cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    header_button(
        "duplicates-skip-all",
        Icon::StepForward,
        t("duplicates.skip_all"),
        t("duplicates.skip_all_hint"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.duplicates_skip_all(cx);
    }))
}

/// A header button: 32 high, bg-2 with a `line` outline, icon and 12.5 label.
fn header_button(selector: &'static str, icon: Icon, label: String, hint: String) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(hint))
        .h(px(32.0))
        .px(px(12.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(7.0))
        .rounded(theme::RADIUS_LG)
        .bg(theme::bg_2())
        .border_1()
        .border_color(theme::line())
        .hover(|s| s.bg(theme::bg_3()))
        .cursor_pointer()
        .text_size(px(12.5))
        .text_color(theme::t1())
        .child(render_icon(icon, px(14.0), theme::t2()))
        .child(label)
}

/// A group's name: the heuristic pick's title (the first item, whichever
/// copy is kept), else that copy's file stem.
fn group_title(group: &DuplicateGroup) -> String {
    let Some(keeper) = group.items.first() else {
        return String::new();
    };
    keeper
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| {
            std::path::Path::new(&keeper.file_name)
                .file_stem()
                .map_or_else(|| keeper.file_name.clone(), |s| s.to_string_lossy().into())
        })
}

/// "3 copies · 62.1 MB": the copy count and the bytes they take together.
fn group_summary(group: &DuplicateGroup) -> String {
    format!(
        "{} · {}",
        t_with("duplicates.copies", &[&group.items.len().to_string()]),
        format_bytes(group.file_size * group.items.len() as u64)
    )
}

/// `item`'s thumbnail from the cache, queueing a decode when it is missing.
/// Never decodes on the UI thread.
fn texture(this: &LuminaApp, item: &MediaItem) -> Option<Arc<RenderImage>> {
    let texture = this
        .grid_state
        .as_ref()
        .and_then(|state| state.texture_cache.lock().get(&item.id).cloned());
    if texture.is_none() {
        this.queue_thumbnail_request_item(item);
    }
    texture
}
