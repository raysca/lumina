//! Settings › Storage: thumbnail cache, settings file and log file.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section, value};
use super::tilde;
use crate::app::preferences::{thumbnail_cache_dir, SettingsButton};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::format_bytes;

pub(super) fn render_storage(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    // Read in the background; until the first reading lands, say so.
    let size = app.preferences_cache.map_or_else(
        || "Calculating…".to_string(),
        |usage| {
            let files = crate::i18n::t(&crate::i18n::plural_key(
                "preferences.thumbnails",
                usage.files,
            ))
            .replace("{n}", &usage.files.to_string());
            format!("{} · {files}", format_bytes(usage.bytes))
        },
    );
    let size_marker = size.clone();
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
                            app,
                            cx,
                            SettingsButton::RevealCache,
                            "Reveal Cache",
                            None,
                        )),
                )
                .debug_selector(|| "settings-row-cache".to_string()),
                row(
                    "Size on disk",
                    Some("Clearing is safe; thumbnails rebuild on their own."),
                    controls()
                        .child(
                            // Tests read the size as `preferences-cache-size-text:<size>`.
                            div()
                                .min_w_0()
                                .debug_selector(move || {
                                    format!("preferences-cache-size-text:{size_marker}")
                                })
                                .child(
                                    value(&size, true)
                                        .debug_selector(|| "preferences-cache-size".to_string()),
                                ),
                        )
                        .children(app.preferences_cache_cleared.then(|| {
                            div()
                                .debug_selector(|| "preferences-cache-cleared".to_string())
                                .flex_shrink_0()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1()
                                .text_size(theme::TEXT_SMALL)
                                .text_color(theme::positive())
                                .child(render_icon(Icon::Check, theme::ICON_XS, theme::positive()))
                                .child("Cleared")
                        }))
                        .child(row_button(
                            app,
                            cx,
                            SettingsButton::ClearCache,
                            // A second press while clearing does nothing; say why.
                            if app.preferences_cache_job.clearing {
                                "Clearing…"
                            } else {
                                "Clear Cache"
                            },
                            None,
                        )),
                )
                .debug_selector(|| "settings-row-cache-size".to_string()),
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
                        app,
                        cx,
                        SettingsButton::RevealConfig,
                        "Reveal Config",
                        None,
                    )),
            )
            .debug_selector(|| "settings-row-config".to_string())],
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
                        app,
                        cx,
                        SettingsButton::RevealLogs,
                        "Reveal Logs",
                        None,
                    )),
            )
            .debug_selector(|| "settings-row-logs".to_string())],
        ))
        .into_any_element()
}
