//! The video InfoPanel sections: Chapters (only when the file
//! has chapters; click seeks) and Playback (duration and pixel size; frame
//! rate, codec, bitrate and audio wait for a player-side info read).

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaItem, MediaType};
use std::sync::Arc;

use crate::app::types::format_clock;
use crate::app::video_tracks::{chapter_name, current_chapter};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::capture::{render_capture_row, CaptureRow};
use crate::LuminaApp;

/// One chapter row: its 0-based index, name, "0:22 — 0:48" range, the
/// strip frame nearest its start (if the strip is built), and whether it
/// is playing.
pub(crate) struct ChapterRow {
    index: usize,
    start: f64,
    name: String,
    range: String,
    thumb: Option<Arc<RenderImage>>,
    current: bool,
}

/// What the panel needs from the player for the item it shows.
pub(crate) struct VideoPanel {
    chapters: Vec<ChapterRow>,
    duration: f64,
}

/// The video panel data for `item`: the player's chapters and duration when
/// the player holds this item, else the catalog duration and no chapters.
/// `None` for photos.
pub(crate) fn video_panel(this: &LuminaApp, item: &MediaItem) -> Option<VideoPanel> {
    if item.media_type != MediaType::Video {
        return None;
    }
    let video = &this.video;
    let loaded = video.item.as_ref().is_some_and(|v| v.id == item.id);
    if !loaded || video.duration <= 0.0 {
        return Some(VideoPanel {
            chapters: Vec::new(),
            duration: item.duration_secs(),
        });
    }
    let chapters = &video.tracks.chapters;
    let playing = current_chapter(chapters, video.playback_time);
    let rows = chapters
        .iter()
        .enumerate()
        .map(|(index, (start, _))| {
            let end = chapters.get(index + 1).map_or(video.duration, |c| c.0);
            ChapterRow {
                index,
                start: *start,
                name: chapter_name(chapters, index),
                range: format!(
                    "{} — {}",
                    format_clock(*start, false),
                    format_clock(end, false)
                ),
                thumb: this.scrub_preview_at(*start).map(|(_, image)| image),
                current: Some(index) == playing,
            }
        })
        .collect();
    Some(VideoPanel {
        chapters: rows,
        duration: video.duration,
    })
}

/// "1:18.420", or "2:02:02.500" from an hour up.
pub(crate) fn format_duration_ms(secs: f64) -> String {
    let ms = if secs.is_finite() && secs > 0.0 {
        (secs * 1000.0).round() as u64
    } else {
        0
    };
    format!(
        "{}.{:03}",
        format_clock((ms / 1000) as f64, false),
        ms % 1000
    )
}

/// Playback rows: the facts Lumina knows (no "—" rows).
pub(crate) fn playback_rows(item: &MediaItem, duration: f64) -> Vec<CaptureRow> {
    let mut rows = Vec::new();
    if duration > 0.0 && duration.is_finite() {
        rows.push(CaptureRow {
            key: t("inspector.duration"),
            selector: "inspector-playback-duration",
            value: format_duration_ms(duration),
        });
    }
    if item.width > 0 && item.height > 0 {
        rows.push(CaptureRow {
            key: t("inspector.resolution"),
            selector: "inspector-playback-resolution",
            value: format!("{} × {}", item.width, item.height),
        });
    }
    rows
}

/// A chapter row: 44px, thumb 50x28, name and range; the playing one is
/// outlined in accent with the audio-lines mark. Click seeks to its start.
fn chapter_row(weak_app: WeakEntity<LuminaApp>, row: ChapterRow) -> Stateful<Div> {
    let n = row.index + 1;
    let start = row.start;
    let selector = format!("inspector-chapter-{n}");
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(row.name.clone())
        .h(px(44.0))
        .px(px(8.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(if row.current {
            theme::accent()
        } else {
            theme::line()
        })
        .bg(if row.current {
            theme::bg_3()
        } else {
            theme::bg_0()
        })
        .hover(|s| s.bg(theme::bg_3()))
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(match row.thumb {
            Some(image) => img(image)
                .flex_none()
                .w(px(50.0))
                .h(px(28.0))
                .rounded(px(3.0))
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            // No strip (yet): a neutral box.
            None => div()
                .flex_none()
                .w(px(50.0))
                .h(px(28.0))
                .rounded(px(3.0))
                .bg(theme::bg_2())
                .into_any_element(),
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .truncate()
                        .text_size(theme::TEXT_SMALL)
                        .text_color(if row.current {
                            theme::accent()
                        } else {
                            theme::t1()
                        })
                        .when(row.current, |name| name.font_weight(FontWeight::MEDIUM))
                        .child(row.name),
                )
                .child(
                    div()
                        .font_family(theme::FONT_SANS)
                        .font_features(theme::tabular_figures())
                        .text_size(theme::TEXT_CAPTION)
                        .text_color(theme::t3())
                        .child(row.range),
                ),
        )
        .when(row.current, |r| {
            r.child(
                div()
                    .debug_selector(|| "inspector-chapter-current".to_string())
                    .flex_none()
                    .child(render_icon(
                        Icon::AudioLines,
                        theme::ICON_SM,
                        theme::accent(),
                    )),
            )
        })
        .on_click(move |_e, _w, cx| {
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| this.seek_to_chapter_start(start, cx));
            }
        })
}

/// The Chapters section (when there are chapters) then Playback.
pub(super) fn render_video_sections(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
    panel: VideoPanel,
) -> Vec<Div> {
    let mut sections = Vec::new();
    if !panel.chapters.is_empty() {
        sections.push(
            super::summary::section(
                "inspector.chapters",
                div().flex().flex_col().gap(px(6.0)).children(
                    panel
                        .chapters
                        .into_iter()
                        .map(|row| chapter_row(weak_app.clone(), row)),
                ),
            )
            .debug_selector(|| "inspector-chapters".to_string()),
        );
    }
    let rows = playback_rows(item, panel.duration);
    if !rows.is_empty() {
        sections.push(
            super::summary::section(
                "inspector.playback",
                div()
                    .flex()
                    .flex_col()
                    .children(rows.into_iter().map(render_capture_row)),
            )
            .debug_selector(|| "inspector-playback".to_string()),
        );
    }
    sections
}
