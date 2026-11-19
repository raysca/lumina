//! "Building your library": the first scan of an empty catalog (v3).

use std::time::Instant;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::first_scan::FirstScanState;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::onboarding::{button, logo, welcome_column};
use crate::LuminaApp;

const JUST_ADDED_TILE: f32 = 56.0;

pub fn render(app: &LuminaApp, scan: &FirstScanState, cx: &mut Context<LuminaApp>) -> AnyElement {
    let root = app
        .grid_state
        .as_ref()
        .map(|state| state.engine.library_root().display().to_string())
        .unwrap_or_default();
    let current = scan
        .current_file
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fraction = scan.fraction();
    let counted = scan.total > 0 || scan.processed > 0;
    let thumbs: Vec<Option<std::sync::Arc<RenderImage>>> = match &app.grid_state {
        Some(grid) => {
            let cache = grid.texture_cache.lock();
            scan.just_added
                .iter()
                .map(|id| cache.peek(id).map(|t| t.image.clone()))
                .collect()
        }
        None => Vec::new(),
    };
    for (id, thumb) in scan.just_added.iter().zip(&thumbs) {
        if thumb.is_none() {
            app.queue_thumbnail_request(*id);
        }
    }

    welcome_column()
        .child(
            div()
                .id("first-scan")
                .debug_selector(|| "first-scan".to_string())
                .w(px(520.0))
                .flex()
                .flex_col()
                .items_start()
                .gap_5()
                .child(logo())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_2xl()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Building your library"),
                        )
                        .child(div().text_sm().text_color(theme::t2()).child(root)),
                )
                // Progress bar, the current file, "X of Y · about N s left".
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            div()
                                .id("first-scan-bar")
                                .debug_selector(|| "first-scan-bar".to_string())
                                .w_full()
                                .h(px(6.0))
                                .rounded_full()
                                .bg(theme::bg_2())
                                .child(
                                    div()
                                        .h_full()
                                        .rounded_full()
                                        .bg(theme::t2())
                                        .w(relative(fraction)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .gap_4()
                                .text_xs()
                                .child(
                                    div()
                                        .id("first-scan-file")
                                        .debug_selector(|| "first-scan-file".to_string())
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(theme::t2())
                                        .child(current),
                                )
                                .child(
                                    div()
                                        .id("first-scan-progress")
                                        .debug_selector(|| "first-scan-progress".to_string())
                                        .flex_none()
                                        .text_color(theme::t2())
                                        .child(scan.progress_line(Instant::now())),
                                ),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(stat("first-scan-photos", "Photos", scan.photos))
                        .child(stat("first-scan-videos", "Videos", scan.videos))
                        .child(stat("first-scan-raw", "RAW", scan.raw))
                        .child(stat("first-scan-skipped", "Skipped", scan.skipped)),
                )
                .when(!thumbs.is_empty(), |col| {
                    col.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(theme::section_label("JUST ADDED"))
                            .child(
                                div()
                                    .id("first-scan-just-added")
                                    .debug_selector(|| "first-scan-just-added".to_string())
                                    .flex()
                                    .flex_row()
                                    .gap_1p5()
                                    .children(thumbs.into_iter().map(|thumb| {
                                        div()
                                            .size(px(JUST_ADDED_TILE))
                                            .overflow_hidden()
                                            .bg(theme::bg_2())
                                            .children(thumb.map(|image| {
                                                img(image).size_full().object_fit(ObjectFit::Cover)
                                            }))
                                    })),
                            ),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(phase("Counting photos and videos", counted, !counted))
                        .child(phase("Reading dates, cameras and places", false, counted))
                        .child(phase("Ready to browse", false, false)),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .gap_3()
                        .child(
                            button("first-scan-open", "Open Library", true)
                                .tooltip(theme::tooltip_label("Browse now; the scan continues (↵)"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.dismiss_first_scan(cx);
                                })),
                        )
                        .child(
                            button("first-scan-stop", "Stop Scan", false)
                                .tooltip(theme::tooltip_label("Stop Scan (⌘.)"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.stop_first_scan(cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}

fn stat(id: &'static str, label: &'static str, value: usize) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .flex_1()
        .p(px(10.0))
        .rounded(theme::RADIUS_LG)
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line())
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(value.to_string()),
        )
        .child(div().text_xs().text_color(theme::t2()).child(label))
}

/// ✓ done, ● in progress, ○ waiting.
fn phase(label: &'static str, done: bool, active: bool) -> impl IntoElement {
    let color = if done || active {
        theme::t1()
    } else {
        theme::t2()
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .text_sm()
        .text_color(color)
        .child(if done {
            render_icon(Icon::Check, px(13.0), theme::t2()).into_any_element()
        } else {
            div()
                .size(px(8.0))
                .mx(px(2.5))
                .rounded_full()
                .map(|d| {
                    if active {
                        d.bg(theme::t1())
                    } else {
                        d.border_1().border_color(theme::line_2())
                    }
                })
                .into_any_element()
        })
        .child(label)
}
