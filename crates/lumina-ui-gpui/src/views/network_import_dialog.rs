//! Network import dialog view: URL entry, inspection preview, download progress.

use gpui::*;

use crate::app::network_import::NetworkImportStage;
use crate::app::types::format_bytes;
use crate::theme;
use crate::LuminaApp;
use lumina_core::net_import::DownloadPhase;

/// Formats seconds into MM:SS or HH:MM:SS.
fn format_duration(seconds: f64) -> String {
    let total_secs = seconds.round() as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}

/// The host part of a URL ("www.youtube.com"), for status text.
fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(stage) = app.network_import.stage.as_ref() else {
        return Empty.into_any_element();
    };

    let button = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .h(px(30.0))
            .px_3()
            .rounded(theme::RADIUS_MD)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_xs()
            .child(label)
    };

    let cancel = |label: &'static str| {
        button("net-import-cancel", label)
            .bg(theme::bg_control())
            .hover(|s| s.bg(theme::bg_control_hover()))
            .text_color(theme::text())
            // A download takes Esc twice (see `dismiss_network_import`).
            .child(theme::kbd(if label == "Cancel Download" {
                "Esc Esc"
            } else {
                "Esc"
            }))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.close_network_import_dialog(cx);
            }))
    };

    let content = match stage {
        NetworkImportStage::Input { url, error } => {
            let inspect_url = url.clone();
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text())
                        .child("Import Video from URL"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("Paste a link to a video: a YouTube video, a page on another video site, or a video file."),
                )
                .child(
                    div()
                        .id("net-import-url-input")
                        .debug_selector(|| "net-import-url-input".to_string())
                        .w_full()
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_control())
                        .border_1()
                        .border_color(if error.is_some() {
                            theme::danger_border()
                        } else {
                            theme::focus_ring()
                        })
                        .rounded(theme::RADIUS_MD)
                        .flex()
                        .items_center()
                        .gap_2()
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_1()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_xs()
                                .text_color(if url.is_empty() {
                                    theme::text_muted()
                                } else {
                                    theme::text()
                                })
                                .child(if url.is_empty() {
                                    "Paste a video link (⌘V) |".to_string()
                                } else {
                                    format!("{url}|")
                                }),
                        )
                        .children((!url.is_empty()).then(|| {
                            div()
                                .id("net-import-clear-btn")
                                .debug_selector(|| "net-import-clear-btn".to_string())
                                .cursor_pointer()
                                .px_1()
                                .py_0p5()
                                .rounded(theme::RADIUS_SM)
                                .text_xs()
                                .text_color(theme::text_muted())
                                .hover(|s| s.text_color(theme::text()))
                                .child("✕")
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_network_import_url(String::new(), cx);
                                }))
                        })),
                )
                .children(error.as_ref().map(|err| {
                    div()
                        .text_xs()
                        .text_color(theme::danger_text())
                        .child(err.clone())
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .mt_2()
                        .child(cancel("Cancel"))
                        .child(
                            button("net-import-inspect", "Continue")
                                .bg(theme::accent())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::accent_fg())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.inspect_network_url(inspect_url.clone(), cx);
                                })),
                        ),
                )
        }
        NetworkImportStage::Inspecting { url } => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::text())
                    .child("Checking Link…"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(format!("Getting video details from {}…", host_of(url))),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .mt_2()
                    .child(cancel("Cancel")),
            ),
        NetworkImportStage::Ready {
            info,
            target_path,
            is_favorite,
            already_imported,
        } => {
            let is_fav = *is_favorite;
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .child("Import Video"),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded(theme::RADIUS_SM)
                                .bg(theme::bg_control())
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::accent())
                                .child(info.kind.label()),
                        ),
                )
                .child(
                    div()
                        .p_3()
                        .bg(theme::bg_control())
                        .rounded(theme::RADIUS_MD)
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text())
                                .child(info.title.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_3()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .children(info.duration_secs.map(|d| {
                                    div().child(format!("Duration: {}", format_duration(d)))
                                }))
                                .children(info.expected_bytes.map(|b| {
                                    div().child(format!("Est. Size: {}", format_bytes(b)))
                                }))
                                .children(
                                    info.uploader
                                        .as_ref()
                                        .map(|u| div().child(format!("Uploader: {u}"))),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Saves to your library's Imports folder as"),
                        )
                        .child(
                            div().text_xs().text_color(theme::text()).child(
                                target_path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                            ),
                        ),
                )
                .children(already_imported.as_ref().map(|path| {
                    div()
                        .debug_selector(|| "net-import-already-imported".to_string())
                        .text_xs()
                        .text_color(theme::accent())
                        .child(format!(
                            "Already in your library as {}. Importing again makes a second copy.",
                            path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default()
                        ))
                }))
                .child(
                    div()
                        .id("net-import-favorite-toggle")
                        .debug_selector(|| "net-import-favorite-toggle".to_string())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.toggle_network_import_favorite(cx);
                        }))
                        .child(
                            div()
                                .w(px(16.0))
                                .h(px(16.0))
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(if is_fav {
                                    theme::accent()
                                } else {
                                    theme::border()
                                })
                                .bg(if is_fav {
                                    theme::accent()
                                } else {
                                    theme::bg_control()
                                })
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(theme::accent_fg())
                                .child(if is_fav { "✓" } else { "" }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text())
                                .child("Add to Favorites"),
                        )
                        .child(theme::kbd("F")),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .mt_2()
                        .child(cancel("Cancel"))
                        .child(
                            button("net-import-confirm", "Download & Import")
                                .bg(theme::accent())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::accent_fg())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.confirm_network_download(cx);
                                })),
                        ),
                )
        }
        NetworkImportStage::Downloading {
            title,
            progress,
            confirm_cancel,
        } => {
            let pct = progress.percent().unwrap_or(0.0);
            let status = match progress.phase {
                DownloadPhase::Downloading => {
                    let mut text = format!(
                        "{} · {}",
                        progress.message,
                        format_bytes(progress.bytes_downloaded)
                    );
                    if let Some(total) = progress.total_bytes {
                        text.push_str(&format!(" of {}", format_bytes(total)));
                    }
                    if progress.speed_bytes_per_sec > 0 {
                        text.push_str(&format!(
                            " · {}/s",
                            format_bytes(progress.speed_bytes_per_sec)
                        ));
                    }
                    text
                }
                _ => format!("{}…", progress.message),
            };
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text())
                        .child("Downloading Video…"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::text())
                        .child(title.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded(px(3.0))
                        .bg(theme::bg_control())
                        .child(
                            div()
                                .h_full()
                                .w(relative((pct / 100.0).clamp(0.0, 1.0)))
                                .rounded(px(3.0))
                                .bg(theme::accent()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(status)
                        .child(if pct > 0.0 {
                            format!("{pct:.1}%")
                        } else {
                            String::new()
                        }),
                )
                .children(confirm_cancel.then(|| {
                    div()
                        .debug_selector(|| "net-import-confirm-cancel".to_string())
                        .text_xs()
                        .text_color(theme::accent())
                        .child("Press Esc again to stop the download.")
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .mt_2()
                        .child(cancel("Cancel Download")),
                )
        }
        NetworkImportStage::MissingHelper { url } => {
            let retry_url = url.clone();
            // Say what the user can do, never which tool is missing.
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text())
                        .child("Can't Download This Video"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(lumina_core::net_import::MISSING_PART),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .mt_2()
                        .child(cancel("Close"))
                        .child(
                            button("net-import-retry", "Check Again")
                                .bg(theme::accent())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::accent_fg())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.inspect_network_url(retry_url.clone(), cx);
                                })),
                        ),
                )
        }
    };

    div()
        .id("network-import-backdrop")
        .debug_selector(|| "network-import-backdrop".to_string())
        .absolute()
        .inset_0()
        .bg(theme::modal_dim())
        .flex()
        .items_center()
        .justify_center()
        // A stray click outside never stops a running download.
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.dismiss_network_import(false, cx);
        }))
        .child(
            div()
                .id("network-import-card")
                .debug_selector(|| "network-import-card".to_string())
                .occlude()
                .w(px(520.0))
                .p_5()
                .bg(theme::bg_dialog())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(theme::RADIUS_LG)
                .shadow_2xl()
                .on_click(cx.listener(|_this, _e: &ClickEvent, _w, _cx| {
                    // Stop click propagation to backdrop
                }))
                .child(content),
        )
        .into_any_element()
}
