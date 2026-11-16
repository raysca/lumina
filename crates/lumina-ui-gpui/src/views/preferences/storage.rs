//! Settings › Storage: thumbnail cache, settings file and log file.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section, value};
use super::tilde;
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
        .gap(px(14.0))
        .child(section(
            "THUMBNAIL CACHE",
            None,
            [
                row(
                    "Location",
                    None,
                    controls()
                        .child(path_pill(
                            "settings-path-cache",
                            tilde(&thumbnail_cache_dir()),
                        ))
                        .child(row_button(
                            cx,
                            "preferences-reveal-cache",
                            "Reveal Cache",
                            |this, cx| this.reveal_cache_directory(cx),
                        )),
                ),
                row(
                    "Size on disk",
                    Some("Clearing is safe; thumbnails rebuild on their own."),
                    controls()
                        .child(
                            value(&size, true)
                                .debug_selector(|| "preferences-cache-size".to_string()),
                        )
                        .children(app.preferences_cache_cleared.then(|| {
                            div()
                                .debug_selector(|| "preferences-cache-cleared".to_string())
                                .flex_shrink_0()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .text_size(px(11.5))
                                .text_color(theme::positive())
                                .child(render_icon(Icon::Check, px(12.0), theme::positive()))
                                .child("Cleared")
                        }))
                        .child(row_button(
                            cx,
                            "preferences-clear-cache",
                            "Clear Cache",
                            |this, cx| this.clear_thumbnail_cache(cx),
                        )),
                ),
            ],
        ))
        .child(section(
            "SETTINGS FILE",
            None,
            [row(
                "Config",
                None,
                controls()
                    .child(path_pill(
                        "settings-path-config",
                        tilde(&crate::config::AppConfig::config_path()),
                    ))
                    .child(row_button(
                        cx,
                        "preferences-reveal-config",
                        "Reveal Config",
                        |this, cx| this.reveal_config(cx),
                    )),
            )],
        ))
        .child(section(
            "DIAGNOSTICS",
            None,
            [row(
                "Log file",
                None,
                controls()
                    .child(path_pill(
                        "settings-path-logs",
                        tilde(&crate::logging::log_file_path()),
                    ))
                    .child(row_button(
                        cx,
                        "preferences-reveal-logs",
                        "Reveal Logs",
                        |this, cx| this.reveal_logs(cx),
                    )),
            )],
        ))
        .into_any_element()
}
