//! The chapter label row above the scrub strip: "Name m:ss" at
//! each chapter's start, the current chapter in `accent`. Labels that would
//! overlap are dropped; the current chapter's label is always kept.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::types::format_clock;
use crate::app::video_tracks::{chapter_name, current_chapter};
use crate::theme;

/// Height of the label row.
pub(super) const LABELS_HEIGHT: f32 = 15.0;
/// Advance of one JetBrains Mono glyph at [`theme::TEXT_MICRO`] (0.6 em).
const GLYPH: f32 = 5.4;
/// Minimum space between two labels.
const LABEL_GAP: f32 = 8.0;

/// One label to draw: its chapter index, left edge and width in pixels on
/// a track `track_width` wide, text, and whether it is the current chapter.
#[derive(Debug)]
pub(crate) struct ChapterLabel {
    pub index: usize,
    pub x: f32,
    pub width: f32,
    pub text: String,
    pub current: bool,
}

/// The labels that fit on a `track_width` track, in time order.
// ponytail: widths are estimated from the mono advance, not shaped text;
// GPUI only measures text during layout.
pub(crate) fn chapter_labels(
    chapters: &[(f64, String)],
    duration: f64,
    time: f64,
    track_width: f32,
) -> Vec<ChapterLabel> {
    if duration <= 0.0 || track_width <= 0.0 {
        return Vec::new();
    }
    let current = current_chapter(chapters, time);
    let all: Vec<ChapterLabel> = chapters
        .iter()
        .enumerate()
        .map(|(index, (start, _))| {
            let text = format!(
                "{} {}",
                chapter_name(chapters, index),
                format_clock(*start, false)
            );
            let width = text.chars().count() as f32 * GLYPH;
            // Kept inside the track at the far end.
            let x = ((start / duration) as f32 * track_width).min(track_width - width);
            ChapterLabel {
                index,
                x: x.max(0.0),
                width,
                text,
                current: Some(index) == current,
            }
        })
        .collect();
    let overlaps = |a: &ChapterLabel, b: &ChapterLabel| {
        a.x < b.x + b.width + LABEL_GAP && b.x < a.x + a.width + LABEL_GAP
    };
    // The current label first, then the others in order where they fit.
    let mut kept: Vec<usize> = current.into_iter().collect();
    for (i, label) in all.iter().enumerate() {
        if !label.current && kept.iter().all(|k| !overlaps(&all[*k], label)) {
            kept.push(i);
        }
    }
    kept.sort_unstable();
    all.into_iter()
        .enumerate()
        .filter(|(i, _)| kept.binary_search(i).is_ok())
        .map(|(_, label)| label)
        .collect()
}

/// The label row, or `None` without chapters (the transport then shrinks).
pub(super) fn render(
    chapters: &[(f64, String)],
    duration: f64,
    time: f64,
    track_width: f32,
) -> Option<Div> {
    if chapters.is_empty() || duration <= 0.0 {
        return None;
    }
    let labels = chapter_labels(chapters, duration, time, track_width);
    Some(
        div()
            .debug_selector(|| "video-chapter-labels".to_string())
            .relative()
            .w_full()
            .h(px(LABELS_HEIGHT))
            // Mono, so the widths estimated from GLYPH hold.
            .font_family(theme::FONT_MONO)
            .text_size(theme::TEXT_MICRO)
            .children(labels.into_iter().map(|label| {
                let n = label.index + 1;
                div()
                    .debug_selector(move || format!("video-chapter-label-{n}"))
                    .absolute()
                    .top_0()
                    .left(px(label.x))
                    .whitespace_nowrap()
                    .text_color(if label.current {
                        theme::accent()
                    } else {
                        theme::t3()
                    })
                    // The text carries the current flag for automation.
                    .child(
                        div()
                            .when(label.current, |text| {
                                text.debug_selector(|| "video-chapter-label-current".to_string())
                            })
                            .child(label.text),
                    )
            })),
    )
}
