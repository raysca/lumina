//! Import Video from URL dialog: URL entry and the "already downloaded"
//! prompt. Downloads themselves show in the toolbar list.

use gpui::*;

use crate::app::network_import::NetworkImportStage;
use crate::theme;
use crate::LuminaApp;

/// "Oct 2, 2026" in local time.
fn format_day(epoch_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(epoch_ms)
        .map(|day| {
            day.with_timezone(&chrono::Local)
                .format("%b %-d, %Y")
                .to_string()
        })
        .unwrap_or_else(|| "an earlier day".to_string())
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
            .child(theme::kbd("Esc"))
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
                            button("net-import-inspect", "Import")
                                .bg(theme::accent())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::accent_fg())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.submit_network_url(inspect_url.clone(), cx);
                                })),
                        ),
                )
        }
        NetworkImportStage::Checking { url } => div()
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
                    .child(format!("Checking {}…", host_of(url))),
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
        NetworkImportStage::AlreadyDownloaded { prior, .. } => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::text())
                    .child("Already Downloaded"),
            )
            .child(
                div()
                    .debug_selector(|| "net-import-already-downloaded".to_string())
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(format!(
                        "You downloaded this on {}.",
                        format_day(prior.imported_epoch_ms)
                    )),
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
                        button("net-import-download-again", "Download Again")
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .text_color(theme::text())
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.download_again_from_dialog(cx);
                            })),
                    ),
            ),
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
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.close_network_import_dialog(cx);
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
