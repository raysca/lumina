//! The Downloads screen's rows. Active and attention rows
//! are full cards with progress; queued and completed rows are compact. A
//! click on a row focuses it; its buttons act.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::net_import::DownloadPhase;
use lumina_core::DownloadState;

use crate::app::downloads::{row_menu_items, DownloadEntry};
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::downloads_panel::time_left;
use crate::views::network_import_preview::host_of;
use crate::views::page_header::{popover_under, text_marker};
use crate::LuminaApp;

/// The row's outer box: focus ring, click to focus, and its automation
/// name. Every row type keeps `downloads-screen-row-{id}`.
fn row_shell(app: &LuminaApp, entry: &DownloadEntry, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let id = entry.row.id;
    let focused = app.downloads.screen_focused == Some(id);
    div()
        .id(("downloads-screen-row", id as u64))
        .debug_selector(move || format!("downloads-screen-row-{id}"))
        .role(Role::ListItem)
        .aria_label(title_of(entry))
        .aria_selected(focused)
        .flex()
        .flex_row()
        .items_center()
        .rounded(px(12.0))
        .border_1()
        .border_color(if focused {
            theme::focus_ring()
        } else {
            theme::line()
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.downloads.screen_focused = Some(id);
            cx.notify();
        }))
}

pub(crate) fn title_of(entry: &DownloadEntry) -> SharedString {
    entry
        .row
        .title
        .clone()
        .unwrap_or_else(|| entry.row.url.clone())
        .into()
}

/// A bg-3 placeholder with the film glyph, or the item's thumbnail.
pub(crate) fn thumb(app: &LuminaApp, entry: &DownloadEntry, w: f32, h: f32) -> Div {
    let row = &entry.row;
    let texture = row
        .media_id
        .filter(|_| row.in_library)
        .and_then(|id| crate::views::albums_overview::albums::thumbnail(app, id));
    let (id, shown) = (row.id, if texture.is_some() { "image" } else { "film" });
    div()
        .debug_selector(move || format!("downloads-thumb-{shown}-{id}"))
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
        .rounded(theme::RADIUS_MD)
        .overflow_hidden()
        .bg(theme::bg_3())
        .flex()
        .items_center()
        .justify_center()
        .child(match texture {
            Some(texture) => img(texture)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => render_icon(Icon::Film, px(h * 0.4), theme::t3()).into_any_element(),
        })
}

/// A 30×30 icon button (with `text` beside the icon when given). Its
/// tooltip is `label`; VoiceOver hears `label` and the row's `title`
/// ("Pause My video"), since every row has the same buttons.
fn icon_button(
    selector: String,
    icon: Icon,
    label: String,
    title: &SharedString,
    text: Option<String>,
    (bg, fg): (Rgba, Rgba),
) -> Stateful<Div> {
    let name = SharedString::from(format!("{label} {title}"));
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(name)
        .tooltip(theme::tooltip_label(label))
        .h(px(30.0))
        .min_w(px(30.0))
        .flex_shrink_0()
        .px(px(if text.is_some() { 10.0 } else { 0.0 }))
        .gap(px(6.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_LG)
        .bg(bg)
        .text_size(px(12.0))
        .text_color(fg)
        .cursor_pointer()
        .hover(|s| s.opacity(0.85))
        .child(render_icon(icon, px(14.0), fg))
        .children(text)
}

fn plain() -> (Rgba, Rgba) {
    (theme::bg_2(), theme::t2())
}

/// ✕: cancels a running row or removes any row.
fn remove_button(id: i64, title: &SharedString, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    icon_button(
        format!("downloads-screen-remove-{id}"),
        Icon::X,
        t("downloads.remove"),
        title,
        None,
        plain(),
    )
    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
        cx.stop_propagation();
        this.cancel_or_remove_download(id, cx);
    }))
}

/// A mono 11 line.
fn mono(text: String, color: Rgba) -> Div {
    div()
        .min_w_0()
        .font_family(theme::FONT_MONO)
        .text_size(px(11.0))
        .text_color(color)
        .child(text)
}

/// Active and attention rows: thumbnail, title over status, a progress
/// track and the meta line, then Pause / Resume / Retry and ✕.
pub(crate) fn card_row(
    app: &LuminaApp,
    entry: &DownloadEntry,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let row = &entry.row;
    let id = row.id;
    let failed = row.state == DownloadState::Failed;
    let host = host_of(&row.url).to_string();
    let progress = entry.progress.as_ref();
    let fraction = progress.and_then(|p| {
        p.total_bytes
            .filter(|t| *t > 0)
            .map(|t| p.bytes_downloaded.min(t) as f32 / t as f32)
    });
    let (status, tone) = match row.state {
        DownloadState::Running => match progress {
            Some(p) if p.phase == DownloadPhase::Finalizing => {
                (t("downloads.finishing"), theme::positive())
            }
            Some(p) if p.phase != DownloadPhase::Connecting && p.bytes_downloaded > 0 => {
                let mut text = match fraction {
                    Some(f) => format!("{}%", (f * 100.0).floor() as u32),
                    None => format_bytes(p.bytes_downloaded),
                };
                if let Some(left) = time_left(p) {
                    text = format!("{text} · {left}");
                }
                (text, theme::accent())
            }
            _ => (t("downloads.getting_ready"), theme::t2()),
        },
        DownloadState::Paused => (t("downloads.paused"), theme::t2()),
        _ => (t("downloads.failed"), theme::danger()),
    };
    let meta = if app.downloads.highlight == Some(id) {
        t("downloads.already_in_downloads")
    } else if failed {
        let mut parts = vec![row.error.clone().unwrap_or_default()];
        if let Some(when) = row.finished_epoch_ms.and_then(clock) {
            parts.push(t_with("downloads.last_tried", &[&when]));
        }
        parts.retain(|p| !p.is_empty());
        parts.join(" · ")
    } else {
        let mut parts = vec![host];
        if let Some(p) = progress.filter(|_| row.state == DownloadState::Running) {
            if let Some(total) = p.total_bytes {
                parts.push(t_with(
                    "downloads.status_of",
                    &[&format_bytes(p.bytes_downloaded), &format_bytes(total)],
                ));
            }
            if p.speed_bytes_per_sec > 0 {
                parts.push(format!("{}/s", format_bytes(p.speed_bytes_per_sec)));
            }
        }
        parts.join(" · ")
    };
    let track = (!failed).then(|| {
        let fill = if row.state == DownloadState::Running {
            theme::accent()
        } else {
            theme::t3()
        };
        div()
            .h(px(4.0))
            .w_full()
            .rounded(px(2.0))
            .bg(theme::bg_3())
            .child(
                div()
                    .h_full()
                    .rounded(px(2.0))
                    .bg(fill)
                    .w(relative(fraction.unwrap_or(0.0))),
            )
    });
    let title = title_of(entry);
    let action = match row.state {
        DownloadState::Running => Some(
            icon_button(
                format!("download-pause-{id}"),
                Icon::Pause,
                t("downloads.pause"),
                &title,
                None,
                plain(),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.pause_download(id, cx);
            })),
        ),
        DownloadState::Paused => Some(
            icon_button(
                format!("download-resume-{id}"),
                Icon::Play,
                t("downloads.resume"),
                &title,
                None,
                (theme::accent_soft(), theme::accent()),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.resume_download(id, cx);
            })),
        ),
        DownloadState::Failed if row.can_retry => Some(
            icon_button(
                format!("download-retry-{id}"),
                Icon::RotateCcw,
                t("downloads.retry"),
                &title,
                Some(t("downloads.retry")),
                (theme::danger_soft(), theme::danger()),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.retry_download(id, cx);
            })),
        ),
        _ => None,
    };
    let remove = remove_button(id, &title, cx);
    row_shell(app, entry, cx)
        .px(px(16.0))
        .py(px(14.0))
        .gap(px(14.0))
        .bg(theme::bg_1())
        .when(failed && app.downloads.screen_focused != Some(id), |r| {
            r.border_color(theme::danger_outline())
        })
        .child(thumb(app, entry, 72.0, 46.0))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::t1())
                                .child(title),
                        )
                        .child(mono(status, tone).flex_shrink_0()),
                )
                .children(track)
                .child(
                    mono(meta.clone(), theme::t3())
                        .children(text_marker("downloads-row-meta", &SharedString::from(meta))),
                ),
        )
        .children(action)
        .child(remove)
}

/// "14:05" in local time.
fn clock(epoch_ms: i64) -> Option<String> {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_millis_opt(epoch_ms)
        .single()
        .map(|time| time.format("%H:%M").to_string())
}

/// A 52 high compact row: `lead` (position or check badge), thumbnail,
/// title over meta; callers add the buttons.
fn compact_row(
    app: &LuminaApp,
    entry: &DownloadEntry,
    lead: Div,
    meta: String,
    title_color: Rgba,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    row_shell(app, entry, cx)
        .h(px(52.0))
        .px(px(14.0))
        .gap(px(12.0))
        .bg(theme::bg_1())
        .child(lead)
        .child(thumb(app, entry, 54.0, 34.0))
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
                        .text_size(px(13.0))
                        .text_color(title_color)
                        .child(title_of(entry)),
                )
                .child(mono(meta, theme::t3()).truncate()),
        )
}

/// A queued row: its place in line, Move up (not on the first) and ✕.
pub(crate) fn queued_row(
    app: &LuminaApp,
    entry: &DownloadEntry,
    n: usize,
    before: Option<&&DownloadEntry>,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let id = entry.row.id;
    let host = host_of(&entry.row.url);
    let meta = match before {
        None => format!("{host} · {}", t("downloads.next_up")),
        Some(prev) => format!(
            "{host} · {}",
            t_with("downloads.starts_after", &[&title_of(prev)])
        ),
    };
    let lead = mono(format!("{}", n + 1), theme::t3()).w(px(16.0));
    let title = title_of(entry);
    let remove = remove_button(id, &title, cx);
    compact_row(app, entry, lead, meta, theme::t2(), cx)
        .when(before.is_some() && id > 0, |r| {
            r.child(
                icon_button(
                    format!("downloads-screen-up-{id}"),
                    Icon::ArrowUp,
                    t("downloads.move_up"),
                    &title,
                    None,
                    plain(),
                )
                .aria_keyshortcuts("Alt+Meta+ArrowUp")
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.move_download_up(id, cx);
                })),
            )
        })
        .child(remove)
}

/// A completed row: check badge, the item's thumbnail, Show in Library
/// and the "…" menu.
pub(crate) fn completed_row(
    app: &LuminaApp,
    entry: &DownloadEntry,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let row = &entry.row;
    let id = row.id;
    let meta = match (row.state, row.in_library) {
        (DownloadState::Duplicate, _) => t("downloads.already_in_library"),
        (_, false) => t("downloads.removed_from_library"),
        (_, true) => {
            let elapsed = row
                .finished_epoch_ms
                .map(|ms| (chrono::Utc::now().timestamp_millis() - ms).max(0) as u64)
                .unwrap_or(0);
            t_with(
                "downloads.added",
                &[&crate::app::preferences::ago(
                    std::time::Duration::from_millis(elapsed),
                )],
            )
        }
    };
    let lead = div()
        .size(px(20.0))
        .flex_shrink_0()
        .rounded_full()
        .bg(theme::positive_soft())
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(Icon::Check, px(12.0), theme::positive()));
    let items = row_menu_items(entry);
    let menu = app
        .downloads
        .row_menu
        .filter(|(open, _)| *open == id)
        .map(|(_, at)| row_menu(id, &items, at, cx));
    let title = title_of(entry);
    compact_row(app, entry, lead, meta, theme::t1(), cx)
        .when(items.contains(&"show"), |r| {
            r.child(
                icon_button(
                    format!("downloads-screen-show-{id}"),
                    Icon::FolderOpen,
                    t("downloads.show_in_library"),
                    &title,
                    None,
                    plain(),
                )
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.download_main_action(id, cx);
                })),
            )
        })
        .child(
            icon_button(
                format!("downloads-screen-more-{id}"),
                Icon::Ellipsis,
                t("downloads.more"),
                &title,
                None,
                plain(),
            )
            .relative()
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                cx.stop_propagation();
                this.toggle_download_row_menu(id, cx);
            }))
            .children(menu.map(|card| popover_under(Anchor::TopRight, card))),
        )
}

/// Show in Library, Download Again (duplicates) and Remove from List.
fn row_menu(
    id: i64,
    items: &[&'static str],
    highlight: usize,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let marker: SharedString = id.to_string().into();
    theme::menu("downloads-row-menu")
        .debug_selector(|| "downloads-row-menu".to_string())
        .role(Role::Menu)
        .aria_label(t("downloads.row_menu"))
        .w(px(180.0))
        .occlude()
        .children(text_marker("downloads-row-menu", &marker))
        .children(items.iter().enumerate().map(|(i, &item)| {
            let selector = format!("downloads-row-menu-{item}");
            let label = t(match item {
                "show" => "downloads.show_in_library",
                "again" => "downloads.download_again",
                _ => "downloads.remove",
            });
            theme::menu_item(SharedString::from(selector.clone()), i == highlight)
                .debug_selector(move || selector.clone())
                .role(Role::MenuItem)
                .aria_label(SharedString::from(label.clone()))
                .child(label)
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.run_download_row_menu(id, item, cx);
                }))
        }))
}
