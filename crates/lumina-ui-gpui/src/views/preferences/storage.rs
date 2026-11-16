//! Settings › Storage: thumbnail cache, settings file and log file.

use gpui::*;

use super::{button, group, note, row, section_title, tilde, value};
use crate::app::preferences::thumbnail_cache_dir;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::format_bytes;

pub(super) fn render_storage(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let usage = app.preferences_cache.unwrap_or_default();
    let size = format!(
        "{} · {} thumbnail{}",
        format_bytes(usage.bytes),
        usage.files,
        if usage.files == 1 { "" } else { "s" }
    );
    div()
        .flex()
        .flex_col()
        .child(section_title("THUMBNAIL CACHE", false))
        .child(group([
            row("Location", value(&tilde(&thumbnail_cache_dir()), true)).child(button(
                cx,
                "preferences-reveal-cache",
                "Reveal Cache",
                |this, cx| this.reveal_cache_directory(cx),
            )),
            row(
                "Size on disk",
                value(&size, false).debug_selector(|| "preferences-cache-size".to_string()),
            )
            .children(app.preferences_cache_cleared.then(|| {
                div()
                    .debug_selector(|| "preferences-cache-cleared".to_string())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .text_color(theme::positive())
                    .child(render_icon(Icon::Check, px(12.0), theme::positive()))
                    .child("Cleared")
            }))
            .child(button(
                cx,
                "preferences-clear-cache",
                "Clear Cache",
                |this, cx| this.clear_thumbnail_cache(cx),
            )),
        ]))
        .child(note(
            "Thumbnails rebuild automatically; clearing only costs decode time.",
        ))
        .child(section_title("SETTINGS FILE", true))
        .child(group([row(
            "Config",
            value(&tilde(&crate::config::AppConfig::config_path()), true),
        )
        .child(button(
            cx,
            "preferences-reveal-config",
            "Reveal Config",
            |this, cx| this.reveal_config(cx),
        ))]))
        .child(section_title("DIAGNOSTICS", true))
        .child(group([row(
            "Log file",
            value(&tilde(&crate::logging::log_file_path()), true),
        )
        .child(button(
            cx,
            "preferences-reveal-logs",
            "Reveal Logs",
            |this, cx| this.reveal_logs(cx),
        ))]))
        .into_any_element()
}
