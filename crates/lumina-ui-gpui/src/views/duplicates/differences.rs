//! The differences bar under the copies: which facts are not the same on
//! every copy of the group.

use gpui::*;
use lumina_core::{DuplicateGroup, MediaItem};

use super::card::{camera, format_of};
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// Whether `key` gives some copy a different value than the others.
fn differs<T: PartialEq>(items: &[MediaItem], key: impl Fn(&MediaItem) -> T) -> bool {
    items.windows(2).any(|pair| key(&pair[0]) != key(&pair[1]))
}

/// The facts that differ, as (selector suffix, label key). Source compares
/// the camera only: the folder it falls back to has its own chip. Name,
/// captured date and title are compared too: copies in one folder differ
/// by name at least.
fn differences(this: &LuminaApp, items: &[MediaItem]) -> Vec<(&'static str, &'static str)> {
    let edited = |i: &MediaItem| this.edits.edited_ids.contains(&i.id);
    [
        (
            "dimensions",
            "duplicates.fact_dimensions",
            differs(items, |i| (i.width, i.height)),
        ),
        (
            "size",
            "duplicates.fact_size",
            differs(items, |i| i.file_size),
        ),
        (
            "format",
            "duplicates.fact_format",
            differs(items, format_of),
        ),
        ("edits", "duplicates.fact_edits", differs(items, edited)),
        ("source", "duplicates.fact_source", differs(items, camera)),
        (
            "folder",
            "duplicates.fact_folder",
            differs(items, |i| i.parent_dir.clone()),
        ),
        (
            "name",
            "duplicates.fact_name",
            differs(items, |i| i.file_name.clone()),
        ),
        (
            "captured",
            "duplicates.fact_captured",
            differs(items, |i| i.timestamp_epoch_ms),
        ),
        (
            "title",
            "duplicates.fact_title",
            differs(items, |i| i.title.clone()),
        ),
    ]
    .into_iter()
    .filter(|(_, _, differ)| *differ)
    .map(|(field, label, _)| (field, label))
    .collect()
}

/// "1 difference · Folder": bg-2, r10, 12×14, one chip per field.
pub(super) fn render(this: &LuminaApp, group: &DuplicateGroup) -> Div {
    let fields = differences(this, &group.items);
    let headline: SharedString = match fields.len() {
        0 => t("duplicates.no_differences"),
        1 => t("duplicates.one_difference"),
        n => t_with("duplicates.differences", &[&crate::i18n::count(n)]),
    }
    .into();
    div()
        .debug_selector(|| "duplicates-differences".to_string())
        .mx(px(24.0))
        .mb(px(20.0))
        .px(px(14.0))
        .py(px(12.0))
        .rounded(px(10.0))
        .bg(theme::bg_2())
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(render_icon(Icon::GitCompareArrows, px(14.0), theme::t2()))
        .child(
            div()
                .mr(px(4.0))
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::t1())
                .children(text_marker("duplicates-differences", &headline))
                .child(headline),
        )
        .children(fields.into_iter().map(|(field, label)| {
            div()
                .debug_selector(move || format!("duplicates-difference-{field}"))
                .h(px(22.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .rounded(theme::RADIUS_MD)
                .bg(theme::bg_3())
                .font_family(theme::FONT_MONO)
                .text_size(px(10.5))
                .text_color(theme::t2())
                .child(t(label))
        }))
}
