//! The Downloads screen's status bar: shown while something downloads,
//! with the combined bytes, speed and time left, the last 20 seconds of
//! speed as bars, and the overall percentage.

use gpui::*;
use lumina_core::DownloadState;

use crate::app::downloads::SPEED_SAMPLES;
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// Tallest throughput bar.
const BAR_MAX: f32 = 34.0;

/// "about 35 s remaining" style durations; under a second reads "1 s".
fn duration(secs: u64) -> String {
    match secs.max(1) {
        secs @ 0..=59 => format!("{secs} s"),
        60..=3599 => format!("{} min {} s", secs / 60, secs % 60),
        _ => format!("{} h {} min", secs / 3600, (secs % 3600) / 60),
    }
}

/// Bytes done across running rows, and their total: `None` unless every
/// running row knows its size. The detail line and the overall % both read
/// this one pair, so they never disagree.
fn totals(app: &LuminaApp) -> (u64, Option<u64>) {
    let progress: Vec<_> = app
        .downloads
        .entries
        .iter()
        .filter(|entry| entry.row.state == DownloadState::Running)
        .map(|entry| entry.progress.as_ref())
        .collect();
    let done = progress.iter().flatten().map(|p| p.bytes_downloaded).sum();
    let total = progress
        .iter()
        .map(|p| p.and_then(|p| p.total_bytes).filter(|t| *t > 0))
        .sum();
    (done, total)
}

/// "150.0 MB of 500.0 MB · 10.0 MB/s combined · about 35 s remaining".
/// The "of" part (and so the time left) shows only with a known total; the
/// speed only once one is reported.
fn detail(app: &LuminaApp, done: u64, total: Option<u64>) -> String {
    let speed = app.download_speed();
    let mut parts = vec![match total {
        Some(total) => t_with(
            "downloads.status_of",
            &[&format_bytes(done), &format_bytes(total)],
        ),
        None => format_bytes(done),
    }];
    if speed > 0 {
        parts.push(t_with("downloads.status_speed", &[&format_bytes(speed)]));
        if let Some(total) = total.filter(|total| *total > done) {
            let left = duration((total - done).div_ceil(speed));
            parts.push(t_with("downloads.status_remaining", &[&left]));
        }
    }
    parts.join(" · ")
}

/// 70 high, 0/18 padding, bg-1, 12 radius, `line` outline. `None` while
/// nothing runs.
pub(crate) fn render(app: &LuminaApp) -> Option<Div> {
    let count = |states: &[DownloadState]| {
        app.downloads
            .entries
            .iter()
            .filter(|entry| states.contains(&entry.row.state))
            .count()
    };
    let running = count(&[DownloadState::Running]);
    if running == 0 {
        return None;
    }
    // M is every unfinished row: running, waiting and paused.
    let unfinished = count(&[
        DownloadState::Running,
        DownloadState::Waiting,
        DownloadState::Paused,
    ]);
    let headline: SharedString = t_with(
        "downloads.status_headline",
        &[&running.to_string(), &unfinished.to_string()],
    )
    .into();
    let (done, total) = totals(app);
    let detail: SharedString = detail(app, done, total).into();
    let samples: Vec<u64> = app.downloads.speed.samples().collect();
    let peak = samples.iter().copied().max().unwrap_or(0).max(1);
    // Empty slots on the left until 20 seconds have been sampled.
    let pad = SPEED_SAMPLES - samples.len();
    let bars = (0..SPEED_SAMPLES).map(|slot| {
        let sample = slot.checked_sub(pad).map(|at| samples[at]);
        let height = sample.map_or(2.0, |s| (BAR_MAX * s as f32 / peak as f32).max(2.0));
        let newest = slot == SPEED_SAMPLES - 1 && sample.is_some();
        div()
            .w(px(3.0))
            .h(px(height))
            .rounded(px(1.0))
            .bg(theme::accent())
            .opacity(if newest { 1.0 } else { 0.4 })
    });
    // With no total for every row there is no percentage the detail backs.
    let overall = total.map(|total| {
        let fraction = (done as f32 / total as f32).min(1.0);
        let percent: SharedString = format!("{}%", (fraction * 100.0).round() as u32).into();
        div()
            .debug_selector(|| "downloads-overall".to_string())
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_end()
            .children(text_marker("downloads-overall", &percent))
            .child(
                div()
                    .font_family(theme::FONT_DISPLAY)
                    .text_size(px(21.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::t1())
                    .child(percent),
            )
            .child(
                div()
                    .text_size(px(9.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::t3())
                    .child(t("downloads.overall")),
            )
    });
    Some(
        div()
            .debug_selector(|| "downloads-status-bar".to_string())
            .mx(px(28.0))
            .mt(px(18.0))
            .h(px(70.0))
            .flex_shrink_0()
            .px(px(18.0))
            .bg(theme::bg_1())
            .rounded(px(12.0))
            .border_1()
            .border_color(theme::line())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
            .child(
                div()
                    .size(px(7.0))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(theme::accent()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .children(text_marker("downloads-status-headline", &headline))
                            .child(headline),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_family(theme::FONT_MONO)
                            .text_size(px(11.0))
                            .text_color(theme::t2())
                            .children(text_marker("downloads-status-detail", &detail))
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "downloads-throughput".to_string())
                    .flex_shrink_0()
                    .h(px(BAR_MAX))
                    .flex()
                    .flex_row()
                    .items_end()
                    .gap(px(3.0))
                    .children(bars),
            )
            .children(overall.into_iter().flat_map(|overall| {
                let divider = div()
                    .w(px(1.0))
                    .h(px(30.0))
                    .flex_shrink_0()
                    .bg(theme::line());
                [divider, overall]
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::duration;

    #[core::prelude::v1::test]
    fn time_left_reads_in_whole_units_never_zero() {
        assert_eq!(duration(0), "1 s", "under a second left still reads 1 s");
        assert_eq!(duration(35), "35 s");
        assert_eq!(duration(72), "1 min 12 s");
        assert_eq!(duration(3_900), "1 h 5 min");
    }
}
