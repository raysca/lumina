//! Settings › Library: where the library lives, how big it is and what it
//! takes on disk.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section, value};
use super::storage_card::render_storage_card;
use crate::app::preferences::SettingsButton;
use crate::app::types::format_bytes;
use crate::app::LuminaApp;
use crate::icons::Icon;

pub(super) fn render_library(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let storage = app.library_storage();
    let items = crate::i18n::items(app.total_library_count);
    // The size waits for the first reading; the count is known at once.
    let size = match storage {
        Some(s) => format!("{} · {items}", format_bytes(s.library_bytes())),
        None => items,
    };
    let marker = size.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        // No reading yet (or none for this library): no card.
        .children(storage.map(|storage| render_storage_card(app, storage, cx)))
        .child(section(
            "LIBRARY",
            None,
            [
                row(
                    "Library location",
                    Some("The folder this library lives in."),
                    controls()
                        .child(path_pill("settings-path-library", app.library_path_label()))
                        .child(row_button(
                            app,
                            cx,
                            SettingsButton::OpenAnother,
                            "Open Another…",
                            None,
                        )),
                )
                .debug_selector(|| "settings-row-library".to_string()),
                row(
                    "Library size",
                    None,
                    controls()
                        .child(
                            div()
                                .min_w_0()
                                .debug_selector(|| "preferences-indexed".to_string())
                                .child(value(&size, true).debug_selector(move || {
                                    format!("preferences-indexed-text:{marker}")
                                })),
                        )
                        .child(row_button(
                            app,
                            cx,
                            SettingsButton::Rescan,
                            "Rescan Library",
                            Some(Icon::Wrench),
                        )),
                )
                .debug_selector(|| "settings-row-library-size".to_string()),
            ],
        ))
        .into_any_element()
}
