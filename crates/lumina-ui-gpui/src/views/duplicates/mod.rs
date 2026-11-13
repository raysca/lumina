//! Review Duplicates (frame `PwE8h`): the exact-copy groups in a list on
//! the left, the selected group's copies side by side on the right.

mod compare;
mod list;

use std::sync::Arc;

use gpui::*;
use lumina_core::{DuplicateGroup, MediaItem};

use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::Icon;
use crate::views::page_header::header_primary_button;
use crate::LuminaApp;

/// Width of the group list pane.
const GROUP_LIST_WIDTH: f32 = 372.0;

/// The Exact copies body: the empty state, or the list and compare panes.
pub fn render(this: &LuminaApp, total_count: usize, cx: &mut Context<LuminaApp>) -> AnyElement {
    if this.duplicate_groups.is_empty() && total_count == 0 {
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
        .child(list::render(this, cx))
        .children(
            this.stacks
                .review
                .selected(this.duplicate_groups.len())
                .map(|index| compare::render(this, index, cx)),
        )
        .into_any_element()
}

/// The page header's meta line: "128 groups · 412 items · 18.4 GB recoverable".
pub(crate) fn meta(this: &LuminaApp) -> String {
    let groups = &this.duplicate_groups;
    let items: usize = groups.iter().map(|g| g.items.len()).sum();
    let bytes: u64 = groups.iter().map(|g| g.potential_reclaimable_bytes).sum();
    format!(
        "{} · {} · {}",
        crate::i18n::duplicate_groups(groups.len()),
        crate::i18n::items(items),
        t_with("duplicates.recoverable", &[&format_bytes(bytes)])
    )
}

/// The header's "Merge {n} groups": merges every group, keeper first.
pub(crate) fn merge_all_button(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let n = this.duplicate_groups.len();
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

/// A group's name: its keeper's title, else the keeper's file stem.
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
