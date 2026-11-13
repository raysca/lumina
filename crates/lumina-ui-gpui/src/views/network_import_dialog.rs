//! Import Video from URL dialog: URL entry and the "already downloaded"
//! prompt. Downloads themselves show in the toolbar list.

use crate::app::text_input::TextTarget;
use gpui::*;

use crate::app::network_import::NetworkImportStage;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::import_dialog::dialog_footer_button;
use crate::views::page_header::header_icon_button;
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

const SUBTITLE: &str =
    "Paste a link to a video: a YouTube video, a page on another video site, or a video file.";

/// What one stage puts in the shared shell: header title, body, footer hint
/// and footer buttons.
struct StageParts {
    title: &'static str,
    body: Div,
    hint: Option<Div>,
    buttons: Vec<AnyElement>,
}

pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(stage) = app.network_import.stage.as_ref() else {
        return Empty.into_any_element();
    };

    let cancel = || {
        dialog_footer_button("net-import-cancel", "Cancel", "Esc", false).on_click(cx.listener(
            |this, _e: &ClickEvent, _w, cx| {
                this.close_network_import_dialog(cx);
            },
        ))
    };
    let text = |text: String| {
        div()
            .text_size(px(13.0))
            .line_height(px(20.0))
            .text_color(theme::t2())
            .child(text)
    };

    let stage = match stage {
        NetworkImportStage::Input { url, error } => {
            let inspect_url = url.clone();
            let body = div()
                .flex()
                .flex_col()
                .gap_3()
                .child(text(SUBTITLE.to_string()))
                .child(
                    div()
                        .id("net-import-url-input")
                        .debug_selector(|| "net-import-url-input".to_string())
                        .w_full()
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_2())
                        .border_1()
                        .border_color(if error.is_some() {
                            theme::danger()
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
                                    theme::t2()
                                } else {
                                    theme::t1()
                                })
                                .children(app.field_input_for(TextTarget::NetworkImportUrl)),
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
                                .text_color(theme::t2())
                                .hover(|s| s.text_color(theme::t1()))
                                .child("✕")
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_network_import_url(String::new(), cx);
                                }))
                        })),
                )
                .children(error.as_ref().map(|err| {
                    div()
                        .text_xs()
                        .text_color(theme::danger())
                        .child(err.clone())
                }));
            let hint = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(7.0))
                .child(render_icon(Icon::FolderDown, px(13.0), theme::t3()))
                .child(
                    div()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(10.5))
                        .text_color(theme::t3())
                        .child("Joins your downloads"),
                );
            let import = dialog_footer_button("net-import-inspect", "Import", "↵", true).on_click(
                cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.submit_network_url(inspect_url.clone(), cx);
                }),
            );
            StageParts {
                title: "Import Video from URL",
                body,
                hint: Some(hint),
                buttons: vec![cancel().into_any_element(), import.into_any_element()],
            }
        }
        NetworkImportStage::Checking { url } => StageParts {
            title: "Checking Link…",
            body: div().child(text(format!("Checking {}…", host_of(url)))),
            hint: None,
            buttons: vec![cancel().into_any_element()],
        },
        NetworkImportStage::AlreadyDownloaded { prior, .. } => {
            let media_id = prior.media_id;
            let again =
                dialog_footer_button("net-import-download-again", "Download Again", "⌘↵", false)
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.download_again_from_dialog(cx);
                    }));
            let show =
                dialog_footer_button("net-import-show-in-library", "Show in Library", "↵", true)
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.show_download_in_library(media_id, cx);
                    }));
            StageParts {
                title: "Already Downloaded",
                body: div().child(
                    text(format!(
                        "You downloaded this on {}.",
                        format_day(prior.imported_epoch_ms)
                    ))
                    .debug_selector(|| "net-import-already-downloaded".to_string()),
                ),
                hint: None,
                buttons: vec![
                    cancel().into_any_element(),
                    again.into_any_element(),
                    show.into_any_element(),
                ],
            }
        }
    };

    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(9.0))
        .pt(px(24.0))
        .px(px(26.0))
        .child(
            div()
                .flex_none()
                .size(px(30.0))
                .rounded(px(9.0))
                .bg(theme::accent_soft())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Link, px(16.0), theme::accent())),
        )
        .child(
            // ponytail: the design's -0.3 tracking is skipped; GPUI 0.3.11 has
            // no letter-spacing style.
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(19.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(stage.title),
        )
        .child(
            header_icon_button("net-import-close", Icon::X, "Close", Some("Esc"), false)
                .size(px(28.0))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.close_network_import_dialog(cx);
                })),
        );

    let footer = div()
        .debug_selector(|| "net-import-footer".to_string())
        .flex_none()
        .h(px(61.5))
        .px(px(22.0))
        .bg(theme::bg_2())
        .border_t_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .child(div().min_w_0().children(stage.hint))
        .child(div().flex().flex_row().gap_2().children(stage.buttons));

    div()
        .id("network-import-backdrop")
        .debug_selector(|| "network-import-backdrop".to_string())
        .absolute()
        .inset_0()
        .bg(theme::scrim())
        .flex()
        .items_center()
        .justify_center()
        // 16 px each side: below 676 px the card is the window minus 32.
        .px(px(16.0))
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.close_network_import_dialog(cx);
        }))
        .child(
            div()
                .id("network-import-card")
                .debug_selector(|| "network-import-card".to_string())
                .occlude()
                .w_full()
                .max_w(px(644.0))
                .bg(theme::bg_1())
                .border_1()
                .border_color(theme::line_2())
                .rounded(px(16.0))
                .overflow_hidden()
                // key_photo_scrim is exactly black at 70 %, the design's shadow.
                .shadow(vec![BoxShadow::new(
                    px(0.0),
                    px(24.0),
                    theme::key_photo_scrim().into(),
                )
                .blur_radius(px(64.0))])
                .flex()
                .flex_col()
                .on_click(cx.listener(|_this, _e: &ClickEvent, _w, _cx| {
                    // Stop click propagation to backdrop
                }))
                .child(header)
                .child(
                    div()
                        .px(px(26.0))
                        .pt(px(10.0))
                        .pb(px(22.0))
                        .child(stage.body),
                )
                .child(footer),
        )
        .into_any_element()
}
