//! Settings › Downloads: how videos from links are downloaded.

use gpui::*;

use super::rows::{controls, row, section};
use crate::app::LuminaApp;
use crate::config::DOWNLOAD_CONCURRENCY_CHOICES;
use crate::theme;

pub(super) fn render_downloads(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    section(
        "TRANSFERS",
        Some("Applies to videos imported from a link".into()),
        [row(
            "Simultaneous downloads",
            Some("Extra items wait in the queue until a slot frees up."),
            controls().child(concurrency_control(app, cx)),
        )
        .debug_selector(|| "settings-row-concurrency".to_string())],
    )
    .into_any_element()
}

/// 1 / 2 / 3 / 5. A radio group for VoiceOver; ←/→ pick a segment while
/// Downloads is shown (`app/keymap/modals.rs`).
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
