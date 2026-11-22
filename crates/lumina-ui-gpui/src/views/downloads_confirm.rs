//! "Downloads are still running" before quitting or opening another library.

use gpui::*;

use crate::app::downloads::DownloadsConfirm;
use crate::theme;
use crate::views::import_dialog::dialog_footer_button;
use crate::LuminaApp;

/// The prompt for `downloads.confirm`, or nothing when it is unset.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(confirm) = app.downloads.confirm.as_ref() else {
        return Empty.into_any_element();
    };
    let count = app.downloads_cut_by_leaving();
    let running = crate::i18n::t(&crate::i18n::plural_key("downloads.still_running", count))
        .replace("{n}", &count.to_string());
    let (question, action) = match confirm {
        DownloadsConfirm::Quit => ("Quit anyway?", "Quit"),
        DownloadsConfirm::OpenLibrary(_) => ("Open another library anyway?", "Open"),
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
                .bg(theme::bg_1())
                .border_1()
                .border_color(theme::line_2())
                .rounded(px(16.0))
                .overflow_hidden()
                .shadow_2xl()
                .flex()
                .flex_col()
                .child(
                    div()
                        .p(px(22.0))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(theme::TEXT_LARGE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::t1())
                                .child(format!("{running} {question}")),
                        )
                        .child(div().text_size(theme::TEXT_BODY).text_color(theme::t2()).child(
                            "They stop now and wait in your downloads, paused, for you to resume.",
                        )),
                )
                // The footer bar, as in the Import Video from URL dialog.
                .child(
                    div()
                        .debug_selector(|| "downloads-confirm-footer".to_string())
                        .h(px(61.5))
                        .px(px(22.0))
                        .bg(theme::bg_2())
                        .border_t_1()
                        .border_color(theme::line())
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(
                            dialog_footer_button(
                                "downloads-confirm-cancel",
                                "Cancel",
                                "Esc",
                                false,
                            )
                            .on_click(cx.listener(
                                |this, _e: &ClickEvent, _w, cx| {
                                    this.dismiss_downloads_prompt(cx);
                                },
                            )),
                        )
                        .child(
                            dialog_footer_button("downloads-confirm-ok", action, "↵", true)
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.confirm_downloads_prompt(cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}
