//! Settings › Library: where the library lives and how big it is.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section, value};
use crate::app::LuminaApp;
use crate::icons::Icon;

pub(super) fn render_library(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let size = crate::i18n::items(app.total_library_count);
    let marker = size.clone();
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
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
                            cx,
                            "preferences-change-library",
                            "Open Another…",
                            None,
                            |this, cx| this.pick_folder_dialog(cx),
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
                            cx,
                            "preferences-rescan",
                            "Rescan Library",
                            Some(Icon::Wrench),
                            |this, cx| this.rescan_library(cx),
                        )),
                )
                .debug_selector(|| "settings-row-library-size".to_string()),
            ],
        ))
        .into_any_element()
}
