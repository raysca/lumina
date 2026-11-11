//! "Downloads are still running" before quitting or opening another library.

use gpui::*;

use crate::app::downloads::DownloadsConfirm;
use crate::theme;
use crate::LuminaApp;

/// The prompt for `downloads.confirm`, or nothing when it is unset.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(confirm) = app.downloads.confirm.as_ref() else {
        return Empty.into_any_element();
    };
    let count = app.running_or_waiting_count();
    let running = if count == 1 {
        "1 download is still running.".to_string()
    } else {
        format!("{count} downloads are still running.")
    };
    let (question, action) = match confirm {
        DownloadsConfirm::Quit => ("Quit anyway?", "Quit"),
        DownloadsConfirm::OpenLibrary(_) => ("Open another library anyway?", "Open"),
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

    div()
        .id("downloads-confirm-backdrop")
        .debug_selector(|| "downloads-confirm-backdrop".to_string())
        .absolute()
        .inset_0()
        .occlude()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_center()
        .p_8()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.dismiss_downloads_prompt(cx);
            }),
        )
        .child(
            div()
                .id("downloads-confirm-card")
                .debug_selector(|| "downloads-confirm-card".to_string())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .w(px(400.0))
                .p_5()
                .bg(theme::bg_dialog())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(theme::RADIUS_LG)
                .shadow_2xl()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::text())
                                .child(format!("{running} {question}")),
                        )
                        .child(div().text_xs().text_color(theme::text_muted()).child(
                            "They stop now and wait in your downloads, paused, for you to resume.",
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .child(
                            button("downloads-confirm-cancel", "Cancel")
                                .bg(theme::bg_control())
                                .hover(|s| s.bg(theme::bg_control_hover()))
                                .text_color(theme::text())
                                .child(theme::kbd("Esc"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.dismiss_downloads_prompt(cx);
                                })),
                        )
                        .child(
                            button("downloads-confirm-ok", action)
                                .bg(theme::accent())
                                .hover(|s| s.bg(theme::accent_hover()))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::accent_fg())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.confirm_downloads_prompt(cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}
