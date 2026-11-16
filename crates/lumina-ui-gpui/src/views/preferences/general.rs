//! Settings › General: the library rows and the external editor.

use gpui::*;

use super::{button, group, note, row, section_title, value};
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
        .child(section_title("LIBRARY", false))
        .child(group([
            row("Library folder", value(&app.library_path_label(), true)).child(button(
                cx,
                "preferences-change-library",
                "Change…",
                |this, cx| this.pick_folder_dialog(cx),
            )),
            row(
                "Items indexed",
                value(&indexed, false).debug_selector(|| "preferences-indexed".to_string()),
            )
            .child(button(
                cx,
                "preferences-rescan",
                "Rescan Library",
                |this, cx| this.rescan_library(cx),
            )),
        ]))
        .child(section_title("EXTERNAL EDITOR", true))
        .child(group([row(
            "Editor",
            value(&app.external_editor_label(), false)
                .text_color(if editor_set { theme::t1() } else { theme::t2() })
                .debug_selector(|| "preferences-external-editor".to_string()),
        )
        .child(button(
            cx,
            "preferences-choose-editor",
            "Choose Editor…",
            |this, cx| this.choose_external_editor(cx),
        ))]))
        .child(note(
            "Used by 'Edit in External Editor'. Lumina never changes your original \
             files; edits happen in the app you choose.",
        ))
        .into_any_element()
}
