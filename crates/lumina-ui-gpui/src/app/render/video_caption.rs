//! The caption under a Videos tile: the display
//! title over a mono "14 Jun 2025 · 4K · 1:18" line, one line each.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaItem;

use crate::theme;
use crate::views::grid::CAPTION_HEIGHT;

/// The caption block, a fixed [`CAPTION_HEIGHT`] tall so every Grid row is
/// the same height. Both lines truncate; the full filename stays on the
/// card's tooltip. A selected tile's title is accent.
pub(crate) fn video_caption(item: &MediaItem, selected: bool) -> Div {
    let id = item.id.0;
    div()
        .debug_selector(move || format!("media-card-caption-{id}"))
        .h(px(CAPTION_HEIGHT))
        .flex_shrink_0()
        .min_w_0()
        .pt(px(9.0))
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .debug_selector(move || format!("media-card-caption-title-{id}"))
                .h(px(16.0))
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_SANS)
                .text_size(px(12.5))
                .text_color(if selected {
                    theme::accent()
                } else {
                    theme::t1()
                })
                .child(crate::views::inspector::body::display_title(item))
                // An element has one debug selector: the accent marker is a
                // zero-size child, so the title keeps its own.
                .when(selected, |title| {
                    title.child(
                        div().debug_selector(move || {
                            format!("media-card-caption-title-selected-{id}")
                        }),
                    )
                }),
        )
        .child(
            div()
                .debug_selector(move || format!("media-card-caption-meta-{id}"))
                .h(px(13.0))
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.0))
                .text_color(theme::t3())
                .child(caption_meta(item)),
        )
}

/// "14 Jun 2025 · 4K · 1:18": capture date, resolution label and duration,
/// each left out when unknown (undated, unprobed size, no duration).
pub(crate) fn caption_meta(item: &MediaItem) -> String {
    let date = (item.timestamp_epoch_ms != 0)
        .then(|| chrono::NaiveDate::from_ymd_opt(item.year, item.month, item.day))
        .flatten()
        .map(|date| {
            date.format(&crate::i18n::t("grid.caption_date"))
                .to_string()
        });
    let duration = crate::app::types::format_duration_badge(item.duration_secs());
    [
        date,
        resolution_label(item.width, item.height),
        Some(duration),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" · ")
}

/// "4K" from a 3840 long edge, else "1080p" / "720p" by the short edge;
/// `None` below 720p or when the size is unknown.
fn resolution_label(width: u32, height: u32) -> Option<String> {
    let (long, short) = (width.max(height), width.min(height));
    Some(if long >= 3840 {
        "4K".into()
    } else if short >= 1080 {
        "1080p".into()
    } else if short >= 720 {
        "720p".into()
    } else {
        return None;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn caption_meta_reads_date_resolution_and_duration() {
        let mut item = crate::app::types::get_sample_items()[0].clone();
        item.timestamp_epoch_ms = 1_749_900_000_000;
        (item.year, item.month, item.day) = (2025, 6, 14);
        (item.width, item.height) = (3840, 2160);
        item.duration_ms = Some(78_000);
        assert_eq!(caption_meta(&item), "14 Jun 2025 · 4K · 1:18");
        (item.width, item.height) = (1080, 1920);
        assert_eq!(
            caption_meta(&item),
            "14 Jun 2025 · 1080p · 1:18",
            "a portrait clip reads by its short edge"
        );
        item.timestamp_epoch_ms = 0;
        (item.width, item.height) = (0, 0);
        item.duration_ms = None;
        assert_eq!(caption_meta(&item), "", "nothing known, nothing shown");
    }
}
