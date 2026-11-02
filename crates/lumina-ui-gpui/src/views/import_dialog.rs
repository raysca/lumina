//! The import dialog: checking the dropped or picked items, then what will be
//! copied and what skipped.

use gpui::*;

use crate::app::import::ImportDialog;
use crate::app::types::format_bytes;
use crate::theme;
use crate::LuminaApp;

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The dialog for `import.dialog`, or nothing when it is unset.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(dialog) = app.import.dialog.as_ref() else {
        return Empty.into_any_element();
    };

    let button = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .h(px(30.0))
            .px_3()
            .rounded(theme::RADIUS_MD)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_xs()
            .child(label)
    };
    let cancel = |label: &'static str| {
        button("import-cancel", label)
            .bg(theme::bg_control())
            .hover(|s| s.bg(theme::bg_control_hover()))
            .text_color(theme::text())
            .child(theme::kbd("Esc"))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.cancel_import_dialog(cx);
            }))
    };
    let confirm = |label: &'static str| {
        button("import-confirm", label)
            .bg(theme::accent())
            .hover(|s| s.opacity(0.9))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::accent_fg())
            .child(theme::kbd("↵"))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.confirm_import(cx);
            }))
    };
    let line = |text: String| div().text_xs().text_color(theme::text_muted()).child(text);

    let mut bar = None;
    let (title, lines, buttons): (String, Vec<String>, Vec<AnyElement>) = match dialog {
        ImportDialog::Planning { count, checked } => {
            let title = match *checked {
                None => format!("Checking {}…", plural(*count, "item", "items")),
                Some((done, total)) if done < total => {
                    bar = Some(done as f32 / total as f32);
                    format!("Checking… {done} of {}", plural(total, "file", "files"))
                }
                Some(_) => {
                    bar = Some(1.0);
                    "Looking for duplicates…".to_string()
                }
            };
            (title, Vec::new(), vec![cancel("Cancel").into_any_element()])
        }
        ImportDialog::Ready(plan) => {
            let mut lines = Vec::new();
            if plan.duplicates + plan.in_library > 0 {
                lines.push(format!(
                    "{} already in your library will be skipped",
                    plan.duplicates + plan.in_library
                ));
            }
            if plan.unsupported > 0 {
                lines.push(format!(
                    "{} will be skipped",
                    plural(plan.unsupported, "unsupported file", "unsupported files")
                ));
            }
            if plan.unreadable > 0 {
                lines.push(format!(
                    "{} could not be read",
                    plural(plan.unreadable, "file", "files")
                ));
            }
            if plan.files.is_empty() {
                (
                    "Nothing new to import".to_string(),
                    lines,
                    vec![confirm("OK").into_any_element()],
                )
            } else {
                lines.push(
                    "Copied into Imports in your library folder, sorted by date taken. \
                     The originals are not changed."
                        .to_string(),
                );
                (
                    format!(
                        "Import {} ({})",
                        plural(plan.files.len(), "item", "items"),
                        format_bytes(plan.bytes)
                    ),
                    lines,
                    vec![
                        cancel("Cancel").into_any_element(),
                        confirm("Import").into_any_element(),
                    ],
                )
            }
        }
    };

    div()
        .id("import-dialog-backdrop")
        .debug_selector(|| "import-dialog-backdrop".to_string())
        .absolute()
        .inset_0()
        .occlude()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_center()
        .p_8()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.cancel_import_dialog(cx);
            }),
        )
        .child(
            div()
                .id("import-dialog")
                .debug_selector(|| "import-dialog".to_string())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .w(px(420.0))
                .p_5()
                .bg(theme::bg_dialog())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(theme::RADIUS_LG)
                .shadow_2xl()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .child(title),
                        )
                        .children(lines.into_iter().map(line)),
                )
                .children(
                    bar.map(|fraction| progress_bar("import-check-progress", fraction, 380.0)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .children(buttons),
                ),
        )
        .into_any_element()
}

/// A thin determinate bar, `fraction` (0–1) of `width` filled.
pub fn progress_bar(id: &'static str, fraction: f32, width: f32) -> Div {
    div()
        .debug_selector(move || id.to_string())
        .w(px(width))
        .h(px(4.0))
        .rounded(px(2.0))
        .bg(theme::bg_control())
        .child(
            div()
                .h_full()
                .w(px(width * fraction.clamp(0.0, 1.0)))
                .rounded(px(2.0))
                .bg(theme::accent()),
        )
}

/// A full-window target for files dragged in from Finder: invisible until a
/// drag is over the window, then "Drop to import".
pub fn render_drop_target(cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .id("import-drop-target")
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_center()
        .opacity(0.0)
        .drag_over::<ExternalPaths>(|style, _, _, _| {
            style
                .opacity(1.0)
                .bg(theme::modal_dim())
                .border_2()
                .border_color(theme::accent())
        })
        // ExternalPaths cannot be built in tests; this only forwards.
        .on_drop(cx.listener(|this, paths: &ExternalPaths, _w, cx| {
            this.begin_import(paths.paths().to_vec(), cx);
        }))
        .child(
            div()
                .px_4()
                .py_2()
                .rounded(theme::RADIUS_LG)
                .bg(theme::bg_dialog())
                .text_base()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text())
                .child("Drop to import"),
        )
}
