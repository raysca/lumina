//! The downloads popover's rows (frame `RXoOa`): Active rows with a
//! thumbnail, meta line, state-coloured track and buttons; Earlier rows
//! with a check badge and Show in Library. A click focuses a row and runs
//! its main action, except on a running row (Resume starts over).

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::net_import::DownloadPhase;
use lumina_core::DownloadState;

use crate::app::downloads::DownloadEntry;
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::downloads_popover::named_popover_button;
use crate::views::downloads_rows::{thumb, title_of};
use crate::views::network_import_preview::host_of;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// "4 min ago", or the date once it is a day old.
pub(crate) fn finished_label(epoch_ms: i64) -> String {
    use chrono::TimeZone;
    let elapsed = (chrono::Utc::now().timestamp_millis() - epoch_ms).max(0) as u64;
    if elapsed < 86_400_000 {
        return crate::app::preferences::ago(std::time::Duration::from_millis(elapsed));
    }
    chrono::Local
        .timestamp_millis_opt(epoch_ms)
        .single()
        .map(|time| time.format("%b %-d, %Y").to_string())
        .unwrap_or_default()
}

/// The row's box: `download-row-{id}`, bg-2 while focused or highlighted,
/// click to focus and run the main action.
fn shell(app: &LuminaApp, entry: &DownloadEntry, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let id = entry.row.id;
    let on = app.downloads.focused == Some(id) || app.downloads.highlight == Some(id);
    div()
        .id(SharedString::from(format!("download-row-{id}")))
        .debug_selector(move || format!("download-row-{id}"))
        .role(Role::ListItem)
        .aria_label(title_of(entry))
        .aria_selected(on)
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .cursor_pointer()
        .when(on, |el| el.bg(theme::bg_2()))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.downloads.focused = Some(id);
            // ↵ pauses a running row; a stray click must not.
            let running = this
                .downloads
                .entries
                .iter()
                .any(|e| e.row.id == id && e.row.state == DownloadState::Running);
            if !running {
                this.download_main_action(id, cx);
            }
            cx.notify();
        }))
}

/// A mono 10 meta line, with its text marker for tests.
fn meta_line(id: i64, text: String, color: Rgba) -> Div {
    let marker = SharedString::from(format!("{id}:{text}"));
    div()
        .debug_selector(move || format!("download-meta-{id}"))
        .relative()
        .min_w_0()
        .font_family(theme::FONT_MONO)
        .text_size(px(10.0))
        .text_color(color)
        .children(text_marker("download-meta", &marker))
        .child(text)
}

/// A 25 px row button whose VoiceOver name adds the row's title.
fn row_button(
    selector: String,
    icon: Icon,
    label: String,
    title: &SharedString,
    colors: (Rgba, Rgba),
) -> Stateful<Div> {
    let name = SharedString::from(format!("{label} {title}"));
    named_popover_button(selector, icon, label, name, colors)
}

/// The meta line of an active row, and its colour.
fn active_meta(app: &LuminaApp, entry: &DownloadEntry, first_waiting: bool) -> (String, Rgba) {
    let row = &entry.row;
    if app.downloads.highlight == Some(row.id) {
        return (t("downloads.already_in_downloads"), theme::t3());
    }
    let text = match (row.state, entry.progress.as_ref()) {
        (DownloadState::Running, Some(p)) if p.phase == DownloadPhase::Finalizing => {
            t("downloads.finishing")
        }
        (DownloadState::Running, Some(p))
            if p.phase != DownloadPhase::Connecting && p.bytes_downloaded > 0 =>
        {
            let mut parts = vec![host_of(&row.url).to_string()];
            parts.push(match p.total_bytes {
                Some(total) => format!(
                    "{}/{}",
                    format_bytes(p.bytes_downloaded),
                    format_bytes(total)
                ),
                None => format_bytes(p.bytes_downloaded),
            });
            if p.speed_bytes_per_sec > 0 {
                parts.push(format!("{}/s", format_bytes(p.speed_bytes_per_sec)));
            }
            parts.join(" · ")
        }
        (DownloadState::Running, _) => t("downloads.getting_ready"),
        (DownloadState::Paused, _) => t("downloads.paused_restart"),
        (DownloadState::Waiting, _) if first_waiting => t("downloads.next_up_row"),
        (DownloadState::Waiting, _) => t("downloads.waiting"),
        _ => {
            let reason = row.error.clone().unwrap_or_else(|| t("downloads.failed"));
            return (reason, theme::danger());
        }
    };
    (text, theme::t3())
}

/// The 3 px track's fill colour and width: accent while downloading,
/// positive while finishing, danger when failed, empty otherwise.
fn track(entry: &DownloadEntry) -> Div {
    let progress = entry
        .progress
        .as_ref()
        .filter(|_| entry.row.state == DownloadState::Running);
    let (fill, width) = match (entry.row.state, progress) {
        (DownloadState::Running, Some(p)) if p.phase == DownloadPhase::Finalizing => {
            (theme::positive(), 1.0)
        }
        (DownloadState::Running, Some(p)) => (
            theme::accent(),
            p.total_bytes
                .filter(|t| *t > 0)
                .map_or(0.0, |t| p.bytes_downloaded.min(t) as f32 / t as f32),
        ),
        (DownloadState::Failed, _) => (theme::danger(), 1.0),
        _ => (theme::t3(), 0.0),
    };
    div()
        .h(px(3.0))
        .w_full()
        .rounded_full()
        .bg(theme::bg_3())
        .child(div().h_full().rounded_full().bg(fill).w(relative(width)))
}

/// Running, paused, waiting and failed rows.
pub(crate) fn active_row(
    app: &LuminaApp,
    entry: &DownloadEntry,
    first_waiting: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let row = &entry.row;
    let id = row.id;
    let (meta, meta_color) = active_meta(app, entry, first_waiting);
    let failed = row.state == DownloadState::Failed;
    let title = title_of(entry);
    let action = match row.state {
        DownloadState::Running => Some(
            row_button(
                format!("download-pause-{id}"),
                Icon::Pause,
                t("downloads.pause"),
                &title,
                (theme::bg_3(), theme::t2()),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.pause_download(id, cx);
            })),
        ),
        DownloadState::Paused => Some(
            row_button(
                format!("download-resume-{id}"),
                Icon::Play,
                t("downloads.resume"),
                &title,
                (theme::accent_soft(), theme::accent()),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.resume_download(id, cx);
            })),
        ),
        DownloadState::Failed if row.can_retry => Some(
            row_button(
                format!("download-retry-{id}"),
                Icon::RotateCcw,
                t("downloads.retry"),
                &title,
                (theme::danger_soft(), theme::danger()),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.retry_download(id, cx);
            })),
        ),
        _ => None,
    };
    let remove = remove_button(id, &title, row.state, cx);
    shell(app, entry, cx)
        .p(px(10.0))
        .gap(px(9.0))
        .rounded(px(9.0))
        .child(thumb(app, entry, 56.0, 34.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .child(title),
                )
                // A failure reason ends with what to do; let it wrap.
                .child(meta_line(id, meta, meta_color).map(|el| {
                    if failed {
                        el.line_clamp(3).text_ellipsis()
                    } else {
                        el.truncate()
                    }
                }))
                .child(track(entry)),
        )
        .children(action)
        .child(remove)
}

/// ✕: cancels a running row or removes any row.
fn remove_button(
    id: i64,
    title: &SharedString,
    state: DownloadState,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let label = if state == DownloadState::Running {
        t("downloads.cancel")
    } else {
        t("downloads.remove")
    };
    row_button(
        format!("download-remove-{id}"),
        Icon::X,
        label,
        title,
        (theme::bg_3(), theme::t2()),
    )
    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
        cx.stop_propagation();
        this.cancel_or_remove_download(id, cx);
    }))
}

/// Done and duplicate rows: 44 high, check badge, Show in Library (or
/// Download Again on a duplicate) and ✕.
pub(crate) fn earlier_row(
    app: &LuminaApp,
    entry: &DownloadEntry,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let row = &entry.row;
    let id = row.id;
    let meta = if app.downloads.highlight == Some(id) {
        t("downloads.already_in_downloads")
    } else {
        match (row.state, row.in_library) {
            (DownloadState::Duplicate, _) => t("downloads.already_in_library"),
            (_, false) => t("downloads.removed_from_library"),
            // ponytail: no size; the row stores none. Add it if the
            // catalog ever keeps the file size on the download row.
            (_, true) => match row.finished_epoch_ms {
                Some(ms) => t_with("downloads.added", &[&finished_label(ms)]),
                None => t("downloads.added_plain"),
            },
        }
    };
    let title = title_of(entry);
    let plain = (theme::bg_1(), theme::t2());
    let action = match row.state {
        DownloadState::Duplicate => Some(
            row_button(
                format!("download-again-{id}"),
                Icon::Download,
                t("downloads.download_again"),
                &title,
                plain,
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.download_again(id, cx);
            })),
        ),
        _ if row.in_library => Some(
            row_button(
                format!("download-reveal-{id}"),
                Icon::Folder,
                t("downloads.show_in_library"),
                &title,
                plain,
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.download_main_action(id, cx);
            })),
        ),
        _ => None,
    };
    let remove = remove_button(id, &title, row.state, cx);
    shell(app, entry, cx)
        .h(px(44.0))
        .px(px(10.0))
        .gap(px(10.0))
        .rounded(px(8.0))
        .child(
            div()
                .size(px(18.0))
                .flex_shrink_0()
                .rounded_full()
                .bg(theme::positive_soft())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Check, px(11.0), theme::positive())),
        )
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
                        .text_size(px(12.5))
                        .text_color(theme::t1())
                        .child(title),
                )
                .child(meta_line(id, meta, theme::t3()).truncate()),
        )
        .children(action)
        .child(remove)
}
