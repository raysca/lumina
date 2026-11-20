//! Settings › Downloads: how videos from links are downloaded and where
//! they are filed.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section};
use super::tilde;
use crate::app::preferences::SettingsButton;
use crate::app::LuminaApp;
use crate::config::DOWNLOAD_CONCURRENCY_CHOICES;
use crate::theme;

pub(super) fn render_downloads(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    // With no library open there is nowhere downloads go.
    let saving = app.downloads_folder().map(|folder| {
        section(
            "SAVING",
            None,
            [row(
                "Downloads go to",
                Some("Each download is filed by date and added to the library."),
                controls()
                    .child(path_pill("settings-downloads-path", tilde(&folder)))
                    .child(row_button(
                        app,
                        cx,
                        SettingsButton::RevealDownloads,
                        crate::i18n::platform::SHOW,
                        None,
                    )),
            )
            .debug_selector(|| "settings-row-downloads-path".to_string())],
        )
    });
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(section(
            "TRANSFERS",
            Some("Applies to videos imported from a link".into()),
            [row(
                "Simultaneous downloads",
                Some("Extra items wait in the queue until a slot frees up."),
                controls().child(concurrency_control(app, cx)),
            )
            .debug_selector(|| "settings-row-concurrency".to_string())],
        ))
        .children(saving)
        .into_any_element()
}

/// 1 / 2 / 3 / 5. A radio group for VoiceOver; ←/→ pick a segment while
/// Downloads is shown (`app/keymap/modals/`).
fn concurrency_control(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let current = app.config.download_concurrency;
    let mut control = theme::segmented("settings-concurrency")
        .debug_selector(|| "settings-concurrency".to_string())
        .role(Role::RadioGroup)
        .aria_label("Simultaneous downloads");
    for n in DOWNLOAD_CONCURRENCY_CHOICES {
        let on = n == current;
        let id = format!("settings-concurrency-{n}");
        let selector = id.clone();
        control = control.child(
            theme::segmented_item(SharedString::from(id), on)
                .debug_selector(move || selector.clone())
                .role(Role::RadioButton)
                .aria_label(SharedString::from(n.to_string()))
                .aria_toggled(if on { Toggled::True } else { Toggled::False })
                .child(n.to_string())
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_download_concurrency(n, cx);
                })),
        );
    }
    control
}
