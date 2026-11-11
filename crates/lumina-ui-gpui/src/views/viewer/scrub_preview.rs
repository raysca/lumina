//! Scrub feedback above the transport track: the time badge, plus the
//! nearest strip frame once the open video has a preview strip.
//!
//! Visual only: no `occlude()` and no mouse listeners, so it can never take
//! the hover or the drag away from the track underneath.

use gpui::*;

use crate::app::types::format_time_fractional;
use crate::app::LuminaApp;
use crate::theme;

/// Preview image box; strip frames are fitted inside it.
const PREVIEW_WIDTH: f32 = 160.0;
const PREVIEW_HEIGHT: f32 = 90.0;

fn time_badge(time: f64) -> Stateful<Div> {
    div()
        .id("video-time-badge")
        .debug_selector(|| "video-time-badge".to_string())
        .px_2()
        .py_0p5()
        .bg(theme::bg_float())
        .border_1()
        .border_color(theme::line_2())
        .rounded(theme::RADIUS_SM)
        .shadow_lg()
        .text_xs()
        .font_family(theme::FONT_MONO)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::t1())
        .child(format_time_fractional(time))
}

/// Feedback for the hover or scrub destination at `pct` (0..=1) of the track.
pub(super) fn render(this: &LuminaApp, pct: f32, time: f64) -> AnyElement {
    let Some((frame_time, image)) = this.scrub_preview_at(time) else {
        // No strip (yet, or this video has none): time only.
        return time_badge(time)
            .absolute()
            .top(px(-28.0))
            .left(relative((pct - 0.06).clamp(0.01, 0.85)))
            .into_any_element();
    };
    // A zero-size anchor at the destination; the card hangs centred above it.
    div()
        .absolute()
        .top(px(-6.0))
        // Kept off the track ends so the card stays inside the transport.
        .left(relative(pct.clamp(0.12, 0.88)))
        .size_0()
        .child(
            div()
                .debug_selector(|| "video-scrub-preview".to_string())
                .absolute()
                .bottom_0()
                .left(px(-PREVIEW_WIDTH / 2.0 - 4.0))
                .p_1()
                .bg(theme::bg_float())
                .border_1()
                .border_color(theme::line_2())
                .rounded(theme::RADIUS_MD)
                .shadow_lg()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    img(image)
                        .debug_selector(move || {
                            format!("video-scrub-preview-{}", (frame_time * 1000.0).round())
                        })
                        .w(px(PREVIEW_WIDTH))
                        .h(px(PREVIEW_HEIGHT))
                        .rounded(theme::RADIUS_SM)
                        .bg(theme::bg_media())
                        .object_fit(ObjectFit::Contain),
                )
                .child(time_badge(time)),
        )
        .into_any_element()
}
