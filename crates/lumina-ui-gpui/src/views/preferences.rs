//! Preferences sheet (v3): icon tab strip, grouped label/value/action rows.

use gpui::*;

use crate::app::preferences::{ago, thumbnail_cache_dir, PreferencesTab};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::format_bytes;

pub fn render_preferences_modal(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.preferences_open {
        return Empty.into_any_element();
    }
    let tab = app.preferences_tab;
    let mut tabs = div()
        .flex()
        .flex_row()
        .justify_center()
        .gap_1()
        .pt(px(6.0))
        .pb(px(10.0))
        .bg(theme::bg_raised())
        .border_b_1()
        .border_color(theme::border_subtle());
    for (candidate, label, icon, selector) in [
        (
            PreferencesTab::General,
            "General",
            Icon::Settings,
            "preferences-tab-general",
        ),
        (
            PreferencesTab::Storage,
            "Storage",
            Icon::Database,
            "preferences-tab-storage",
        ),
        (
            PreferencesTab::About,
            "About",
            Icon::Info,
            "preferences-tab-about",
        ),
    ] {
        let selected = tab == candidate;
        let fg = if selected {
            theme::selection_fg()
        } else {
            theme::text_muted()
        };
        tabs = tabs.child(
            div()
                .id(selector)
                .debug_selector(move || selector.to_string())
                .w(px(84.0))
                .h(px(54.0))
                .rounded(px(8.0))
                .cursor_pointer()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .text_xs()
                .text_color(fg)
                .bg(if selected {
                    theme::selection_bg()
                } else {
                    theme::bg_raised()
                })
                .hover(|s| s.bg(theme::bg_control_hover()))
                .child(render_icon(icon, px(20.0), fg))
                .child(label)
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_preferences_tab(candidate, cx);
                })),
        );
    }

    let content = match tab {
        PreferencesTab::General => render_general(app, cx),
        PreferencesTab::Storage => render_storage(app, cx),
        PreferencesTab::About => render_about(cx),
    };
    let window_h = app.window_bounds.map_or(900.0, |(_, _, _, h)| h);

    div()
        .id("preferences-overlay")
        .debug_selector(|| "preferences-overlay".to_string())
        .absolute()
        .inset_0()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.close_preferences(cx);
            }),
        )
        .child(
            div()
                .id("preferences-card")
                .debug_selector(|| "preferences-card".to_string())
                .occlude()
                .w(px(680.0))
                .h(px((window_h - 48.0).clamp(360.0, 600.0)))
                .bg(theme::bg_raised())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(px(10.0))
                .shadow_2xl()
                .overflow_hidden()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .h(px(38.0))
                        .flex_shrink_0()
                        .px(px(12.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .bg(theme::bg_raised())
                        .child(div().size(px(20.0)))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .justify_center()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(match tab {
                                    PreferencesTab::General => "General",
                                    PreferencesTab::Storage => "Storage",
                                    PreferencesTab::About => "About Lumina",
                                }),
                        )
                        .child(
                            div()
                                .id("preferences-close")
                                .debug_selector(|| "preferences-close".to_string())
                                .size(px(20.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|s| s.bg(theme::bg_control_hover()))
                                .tooltip(theme::tooltip_label("Close (Esc)"))
                                .child(render_icon(Icon::X, px(12.0), theme::text_muted()))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_preferences(cx);
                                })),
                        ),
                )
                .child(tabs)
                .child(
                    div()
                        .id("preferences-body")
                        .flex_1()
                        .min_h(px(0.0))
                        .px(px(26.0))
                        .py(px(22.0))
                        .flex()
                        .flex_col()
                        .overflow_y_scroll()
                        .child(content),
                ),
        )
        .into_any_element()
}

fn render_general(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
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
                .text_color(if editor_set {
                    theme::text()
                } else {
                    theme::text_subtle()
                })
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

fn render_storage(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
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
                    .text_color(theme::success_text())
                    .child(render_icon(Icon::Check, px(12.0), theme::success_text()))
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

fn render_about(cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .pt(px(30.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(72.0))
                .rounded(px(16.0))
                .bg(theme::accent())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Sparkles, px(38.0), theme::accent_fg())),
        )
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text())
                .child("Lumina"),
        )
        .child(
            div()
                .debug_selector(|| "preferences-version".to_string())
                .font_family(theme::FONT_MONO)
                .text_xs()
                .text_color(theme::text_muted())
                .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
        )
        .child(
            div()
                .max_w(px(380.0))
                .text_sm()
                .text_center()
                .text_color(theme::text_muted())
                .child(
                    "A local-first photo and video manager. No account, no network, \
                     no telemetry. Your files stay where they are.",
                ),
        )
        .child(
            div()
                .pt(px(6.0))
                .flex()
                .flex_row()
                .gap_2()
                .child(button(
                    cx,
                    "preferences-license",
                    "MIT License",
                    |this, cx| this.open_bundled_doc("LICENSE", cx),
                ))
                .child(button(
                    cx,
                    "preferences-acknowledgements",
                    "Acknowledgements",
                    |this, cx| this.open_bundled_doc("THIRD_PARTY_NOTICES.md", cx),
                )),
        )
        .into_any_element()
}

/// Shortens a path under the home directory to `~/…` for display.
fn tilde(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_owned())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

fn section_title(text: &'static str, spaced: bool) -> Div {
    theme::section_label(text)
        .mt(px(if spaced { 22.0 } else { 0.0 }))
        .mb(px(8.0))
}

/// A bordered group of rows with dividers between them.
fn group(rows: impl IntoIterator<Item = Div>) -> Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(8.0))
        .border_1()
        .border_color(theme::border_subtle())
        .bg(theme::bg_app())
        .children(rows.into_iter().enumerate().map(|(i, row)| {
            if i == 0 {
                row
            } else {
                row.border_t_1().border_color(theme::border_subtle())
            }
        }))
}

/// One 44px row: fixed-width label, truncating value, then action buttons.
fn row(label: &'static str, value: Div) -> Div {
    div()
        .min_h(px(44.0))
        .px(px(14.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .text_sm()
        .child(
            div()
                .w(px(130.0))
                .flex_shrink_0()
                .text_color(theme::text_muted())
                .child(label),
        )
        .child(value)
}

fn value(text: &str, mono: bool) -> Div {
    let v = div()
        .flex_1()
        .min_w(px(0.0))
        .truncate()
        .text_color(theme::text())
        .child(text.to_string());
    if mono {
        v.font_family(theme::FONT_MONO).text_xs()
    } else {
        v
    }
}

fn note(text: &'static str) -> Div {
    div()
        .mt(px(10.0))
        .mx(px(2.0))
        .text_xs()
        .text_color(theme::text_subtle())
        .child(text)
}

type PreferencesAction = fn(&mut LuminaApp, &mut Context<LuminaApp>);

fn button(
    cx: &mut Context<LuminaApp>,
    id: &'static str,
    label: &'static str,
    action: PreferencesAction,
) -> Stateful<Div> {
    theme::control(id)
        .debug_selector(move || id.to_string())
        .flex_shrink_0()
        .child(label)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            action(this, cx);
        }))
}
