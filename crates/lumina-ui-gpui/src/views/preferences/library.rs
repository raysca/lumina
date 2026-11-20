//! Settings › Library: where the library lives, how big it is and what it
//! takes on disk.

use gpui::*;

use super::rows::{controls, path_pill, row, row_button, section, value};
use super::storage_card::render_storage_card;
use crate::app::preferences::SettingsButton;
use crate::app::types::format_bytes;
use crate::app::LuminaApp;
use crate::icons::Icon;
use crate::theme;

pub(super) fn render_library(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let storage = app.library_storage();
    let items = crate::i18n::plural("header.items", app.total_library_count);
    // The size waits for the first reading; the count is known at once.
    let size = match storage {
        Some(s) => format!("{} · {items}", format_bytes(s.library_bytes())),
        None => items,
    };
    let marker = size.clone();
    let open_root = app
        .grid_state
        .as_ref()
        .map(|grid| grid.engine.library_root().to_path_buf());
    let known = app.known_libraries();
    let known_rows: Vec<Div> = if known.is_empty() {
        vec![div()
            .min_h(px(43.5))
            .flex()
            .items_center()
            .text_size(px(11.5))
            .text_color(theme::t2())
            .child("No libraries yet.")]
    } else {
        known
            .iter()
            .enumerate()
            .map(|(index, path)| {
                known_library_row(
                    app,
                    cx,
                    index,
                    path,
                    open_root.as_deref() == Some(path.as_path()),
                    app.recent_unavailable(path),
                )
            })
            .collect()
    };
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
        .child(section(
            "KNOWN LIBRARIES",
            Some(
                "Remove a library's catalog and thumbnails. Its photos stay where they are.".into(),
            ),
            known_rows,
        ))
        .into_any_element()
}

/// One known library: name, path, state, and its Remove… button.
fn known_library_row(
    app: &LuminaApp,
    cx: &mut Context<LuminaApp>,
    index: usize,
    path: &std::path::Path,
    is_open: bool,
    unavailable: bool,
) -> Div {
    let name = crate::app::recents::library_name(path);
    let full_path = path.display().to_string();
    let path_marker = full_path.clone();
    let row_selector = format!("settings-known-library-{index}");
    div()
        .debug_selector(move || row_selector.clone())
        .min_h(px(43.5))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(24.0))
        .child(
            div()
                .flex_1()
                .min_w(px(240.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .child(name),
                )
                .child(
                    div()
                        .debug_selector(move || {
                            format!("settings-known-library-path-{index}:{path_marker}")
                        })
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis_middle()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.5))
                        .text_color(theme::t3())
                        .child(full_path),
                ),
        )
        .children(is_open.then(|| {
            div()
                .flex_shrink_0()
                .text_size(px(11.0))
                .text_color(theme::positive())
                .child("Open now")
        }))
        .children(unavailable.then(|| {
            div()
                .flex_shrink_0()
                .text_size(px(11.0))
                .text_color(theme::t2())
                .child("Not available")
        }))
        .child(controls().child(row_button(
            app,
            cx,
            SettingsButton::RemoveLibrary(index),
            "Remove…",
            Some(Icon::Trash),
        )))
}
