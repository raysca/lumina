//! The import dialog: checking the dropped or picked items, then what will be
//! copied and what skipped.

use gpui::*;

use crate::app::import::ImportDialog;
use crate::app::types::format_bytes;
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The dialog for `import.dialog`, or nothing when it is unset.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(dialog) = app.import.dialog.as_ref() else {
        return Empty.into_any_element();
    };

    let cancel = |label: &'static str| {
        dialog_footer_button("import-cancel", label, "Esc", false).on_click(cx.listener(
            |this, _e: &ClickEvent, _w, cx| {
                this.cancel_import_dialog(cx);
            },
        ))
    };
    let confirm = |label: &'static str| {
        dialog_footer_button("import-confirm", label, "↵", true).on_click(cx.listener(
            |this, _e: &ClickEvent, _w, cx| {
                this.confirm_import(cx);
            },
        ))
    };
    let line = |text: String| div().text_xs().text_color(theme::t2()).child(text);

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
                .bg(theme::bg_1())
                .border_1()
                .border_color(theme::line_2())
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
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::t1())
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

/// A dialog footer button: 36 high, r9, Inter 13, with a 22×20 key hint.
/// Primary is the accent action; the other is bg-3 with a `line_2` outline.
/// `id` is also the debug selector; `label` is the accessible name.
pub fn dialog_footer_button(
    id: &'static str,
    label: &'static str,
    kbd: &'static str,
    primary: bool,
) -> Stateful<Div> {
    dialog_footer_button_with(id, label, kbd, primary, true)
}

/// [`dialog_footer_button`], drawn disabled when `enabled` is false: dimmed,
/// with no hover or pointer (tests see an `{id}-text:disabled` marker). It
/// keeps whatever click handler the caller adds.
pub fn dialog_footer_button_with(
    id: &'static str,
    label: &'static str,
    kbd: &'static str,
    primary: bool,
    enabled: bool,
) -> Stateful<Div> {
    let (hint_bg, hint_fg) = if primary {
        (theme::on_accent_kbd(), theme::on_accent())
    } else {
        (theme::bg_0(), theme::t3())
    };
    let button = div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .h(px(36.0))
        .px(px(12.0))
        .rounded(px(9.0))
        .border_1()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(9.0))
        .text_size(px(13.0));
    let (hover_bg, button) = if primary {
        (
            theme::accent_hover(),
            button
                .bg(theme::accent())
                .border_color(theme::accent())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::on_accent()),
        )
    } else {
        (
            theme::plate(),
            button
                .bg(theme::bg_3())
                .border_color(theme::line_2())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::t1()),
        )
    };
    let button = if enabled {
        button.hover(move |s| s.bg(hover_bg)).cursor_pointer()
    } else {
        button
            .opacity(0.45)
            .children(text_marker(id, &"disabled".into()))
    };
    button.child(label).child(
        div()
            .min_w(px(22.0))
            .h(px(20.0))
            .px(px(4.0))
            .rounded(px(5.0))
            .bg(hint_bg)
            .flex()
            .items_center()
            .justify_center()
            .font_family(theme::FONT_MONO)
            .text_size(px(11.0))
            .font_weight(FontWeight::NORMAL)
            .text_color(hint_fg)
            .child(kbd),
    )
}

/// A thin determinate bar, `fraction` (0–1) of `width` filled.
pub fn progress_bar(id: &'static str, fraction: f32, width: f32) -> Div {
    div()
        .debug_selector(move || id.to_string())
        .w(px(width))
        .h(px(4.0))
        .rounded(px(2.0))
        .bg(theme::bg_2())
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
                .bg(theme::bg_1())
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child("Drop to import"),
        )
}
