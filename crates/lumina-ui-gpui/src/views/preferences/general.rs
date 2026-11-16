//! Settings › General: the library rows and the external editor.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section, value};
use crate::app::preferences::ago;
use crate::app::LuminaApp;
use crate::theme;

pub(super) fn render_general(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let mut indexed = app.total_library_count.to_string();
    // ponytail: session-only; persist the scan time if users want it across launches.
    if let Some(elapsed) = app.last_scan_at.map(|at| at.elapsed()) {
        indexed.push_str(&format!(" · last scan {}", ago(elapsed)));
    }
    let editor_set = app.config.validated_external_editor().is_some();
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(section(
            "LIBRARY",
            None,
            [
                row(
                    "Library folder",
                    None,
                    controls()
                        .child(path_pill("settings-path-library", app.library_path_label()))
                        .child(row_button(
                            cx,
                            "preferences-change-library",
                            "Change…",
                            |this, cx| this.pick_folder_dialog(cx),
                        )),
                )
                .debug_selector(|| "settings-row-library".to_string()),
                row(
                    "Items indexed",
                    None,
                    controls()
                        .child(
                            value(&indexed, true)
                                .debug_selector(|| "preferences-indexed".to_string()),
                        )
                        .child(row_button(
                            cx,
                            "preferences-rescan",
                            "Rescan Library",
                            |this, cx| this.rescan_library(cx),
                        )),
                ),
            ],
        ))
        .child(section(
            "EXTERNAL EDITOR",
            None,
            [row(
                "Editor",
                Some("Edits happen in that app; Edit a Copy keeps the original."),
                controls()
                    .child(
                        value(&app.external_editor_label(), false)
                            .text_color(if editor_set { theme::t1() } else { theme::t2() })
                            .debug_selector(|| "preferences-external-editor".to_string()),
                    )
                    .child(row_button(
                        cx,
                        "preferences-choose-editor",
                        "Choose Editor…",
                        |this, cx| this.choose_external_editor(cx),
                    )),
            )
            .debug_selector(|| "settings-row-editor".to_string())],
        ))
        .into_any_element()
}
