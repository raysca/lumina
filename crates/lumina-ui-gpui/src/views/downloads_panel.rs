//! The downloads dropdown under the toolbar button: Downloading, Waiting,
//! Paused, Failed and Earlier, each row with its status and actions.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::net_import::{DownloadPhase, NetworkDownloadProgress};
use lumina_core::DownloadState;

use crate::app::downloads::{DownloadEntry, DownloadSurface};
use crate::app::types::format_bytes;
use crate::theme;
use crate::views::downloads_popover;
use crate::LuminaApp;

/// "2 min left", from what is left and the current speed.
pub(crate) fn time_left(progress: &NetworkDownloadProgress) -> Option<String> {
    let total = progress.total_bytes?;
    if progress.speed_bytes_per_sec == 0 || total <= progress.bytes_downloaded {
        return None;
    }
    let secs = (total - progress.bytes_downloaded) / progress.speed_bytes_per_sec;
    Some(match secs {
        0..=59 => "under a minute left".to_string(),
        60..=3599 => format!("{} min left", secs / 60),
        _ => format!("{} h {} min left", secs / 3600, (secs % 3600) / 60),
    })
}

/// The grey line under a row's title.
fn status(entry: &DownloadEntry, first_waiting: bool, highlighted: bool) -> String {
    if highlighted {
        return "Already in your downloads".to_string();
    }
    let row = &entry.row;
    match row.state {
        DownloadState::Running => match entry.progress.as_ref() {
            None => "Getting ready…".to_string(),
            Some(p) if p.phase == DownloadPhase::Finalizing => "Finishing…".to_string(),
            Some(p) if p.phase == DownloadPhase::Connecting || p.bytes_downloaded == 0 => {
                "Getting ready…".to_string()
            }
            Some(p) => {
                let mut text = match p.total_bytes {
                    Some(total) => format!(
                        "{} of {}",
                        format_bytes(p.bytes_downloaded),
                        format_bytes(total)
                    ),
                    None => format_bytes(p.bytes_downloaded),
                };
                if let Some(left) = time_left(p) {
                    text.push_str(" · ");
                    text.push_str(&left);
                }
                text
            }
        },
        DownloadState::Waiting if first_waiting => "Next up".to_string(),
        DownloadState::Waiting => "Waiting".to_string(),
        DownloadState::Paused => "Paused".to_string(),
        DownloadState::Failed => row.error.clone().unwrap_or_else(|| "Failed".to_string()),
        DownloadState::Done if row.in_library => "Added to library".to_string(),
        DownloadState::Done => "Removed from library".to_string(),
        DownloadState::Duplicate => "Already in your library".to_string(),
    }
}

/// A small text button on a row ("Retry", "Resume", "✕").
fn row_button(id: String, label: &'static str, tooltip: &'static str) -> Stateful<Div> {
    let selector = id.clone();
    div()
        .id(SharedString::from(id))
        .debug_selector(move || selector.clone())
        .flex_shrink_0()
        .px_1p5()
        .py_0p5()
        .rounded(theme::RADIUS_SM)
        .cursor_pointer()
        .text_xs()
        .text_color(theme::t2())
        .hover(|s| s.bg(theme::bg_3()).text_color(theme::t1()))
        .tooltip(theme::tooltip_label(tooltip))
        .child(label)
}

fn render_row(
    entry: &DownloadEntry,
    first_waiting: bool,
    app: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let row = &entry.row;
    let id = row.id;
    let focused = app.downloads.focused == Some(id);
    let highlighted = app.downloads.highlight == Some(id);
    let title = row.title.clone().unwrap_or_else(|| row.url.clone());
    let fraction = (row.state == DownloadState::Running)
        .then(|| entry.progress.as_ref().and_then(|p| p.percent()))
        .flatten()
        .map(|percent| (percent / 100.0).clamp(0.0, 1.0));
    let selector = format!("download-row-{id}");

    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .w_full()
        .min_w_0()
        .px_2()
        .py_1p5()
        .rounded(theme::RADIUS_MD)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .when(focused || highlighted, |el| el.bg(theme::bg_3()))
        .when(!(focused || highlighted), |el| {
            // A lighter step than the focused row (bg_3), so keyboard focus stays visible.
            el.hover(|s| s.bg(theme::bg_2()))
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.downloads.focused = Some(id);
            // ↵ pauses a running row; a stray click must not, since Resume
            // starts over.
            let running = this
                .downloads
                .entries
                .iter()
                .any(|e| e.row.id == id && e.row.state == DownloadState::Running);
            if !running {
                this.download_main_action(id, cx);
            }
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .truncate()
                        .text_xs()
                        .text_color(theme::t1())
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(if row.state == DownloadState::Failed {
                            theme::danger()
                        } else {
                            theme::t2()
                        })
                        // A failure reason ends with what to do; let it wrap.
                        .map(|el| {
                            if row.state == DownloadState::Failed {
                                el.line_clamp(4).text_ellipsis()
                            } else {
                                el.truncate()
                            }
                        })
                        .child(status(entry, first_waiting, highlighted)),
                )
                .children(fraction.map(|fraction| {
                    div()
                        .h(px(3.0))
                        .w_full()
                        .rounded_full()
                        .bg(theme::bg_2())
                        .child(
                            div()
                                .h_full()
                                .w(relative(fraction))
                                .rounded_full()
                                .bg(theme::accent()),
                        )
                })),
        )
        .when(row.state == DownloadState::Failed && row.can_retry, |el| {
            el.child(
                row_button(format!("download-retry-{id}"), "Retry", "Try again").on_click(
                    cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        cx.stop_propagation();
                        this.retry_download(id, cx);
                    }),
                ),
            )
        })
        .when(row.state == DownloadState::Paused, |el| {
            el.child(
                row_button(format!("download-resume-{id}"), "Resume", "Start over").on_click(
                    cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        cx.stop_propagation();
                        this.resume_download(id, cx);
                    }),
                ),
            )
        })
        .when(row.state == DownloadState::Duplicate, |el| {
            el.child(
                row_button(
                    format!("download-again-{id}"),
                    "Download Again",
                    "Make a second copy (⌘↵)",
                )
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.download_again(id, cx);
                })),
            )
        })
        .child(
            row_button(
                format!("download-remove-{id}"),
                "✕",
                if row.state == DownloadState::Running {
                    "Cancel download"
                } else {
                    "Remove from list"
                },
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.cancel_or_remove_download(id, cx);
            })),
        )
        .into_any_element()
}

/// The dismiss backdrop behind the downloads list. The list itself hangs
/// from the header's Downloads button (`panel_card`), which shows whenever
/// there are rows; the list only opens, and stays open, while there are.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.downloads.open {
        return Empty.into_any_element();
    }
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .id("downloads-backdrop")
                .debug_selector(|| "downloads-backdrop".to_string())
                // Covers the toolbar too, so a second click on the icon
                // closes the list rather than reopening it.
                .occlude()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.close_downloads_panel(cx);
                    }),
                ),
        )
        .into_any_element()
}

/// The downloads popover: 420 wide (less on a narrow window), at most 550
/// high. Header and footer stay put; only the rows scroll.
pub fn panel_card(app: &LuminaApp, window: &Window, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let first_waiting = app
        .downloads
        .entries
        .iter()
        .find(|e| e.row.state == DownloadState::Waiting)
        .map(|e| e.row.id);
    // Each section head and row is a direct child: `scroll_to_item` counts
    // them (`move_download_focus`), so nothing else may go in here.
    let mut body = div()
        .id("downloads-panel-body")
        .debug_selector(|| "downloads-panel-body".to_string())
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&app.downloads.scroll)
        .p_1()
        .flex()
        .flex_col()
        .gap_0p5();
    for (label, rows) in app.download_sections(DownloadSurface::Panel) {
        body = body.child(if label == "Earlier" {
            downloads_popover::earlier_head(cx)
        } else {
            theme::section_label(label.to_uppercase())
                .px_2()
                .pt_2()
                .pb_0p5()
        });
        for entry in rows {
            body = body.child(render_row(
                entry,
                first_waiting == Some(entry.row.id),
                app,
                cx,
            ));
        }
    }

    let viewport = window.viewport_size();
    div()
        .debug_selector(|| "downloads-panel".to_string())
        .id("downloads-panel")
        .role(Role::Dialog)
        .aria_label(crate::i18n::t("downloads.title"))
        .occlude()
        .w(px(420.0).min(viewport.width - px(32.0)))
        // Room for the toolbar above it.
        .max_h(px(550.0).min(viewport.height - px(80.0)))
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line_2())
        .rounded(px(14.0))
        .shadow_2xl()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(downloads_popover::header(app, cx))
        .child(body)
        .child(downloads_popover::footer(cx))
}
