//! Preferences modal: library/diagnostics, storage, about.

use gpui::*;

use crate::app::preferences::{thumbnail_cache_dir, PreferencesTab};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::format_bytes;

pub fn render_preferences_modal(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.preferences_open {
        return Empty.into_any_element();
    }
    let tab = app.preferences_tab;
    let mut tabs = div().flex().flex_row().gap_1().px_5().pt_3();
    for (candidate, label, selector) in [
        (
            PreferencesTab::General,
            "General",
            "preferences-tab-general",
        ),
        (
            PreferencesTab::Storage,
            "Storage",
            "preferences-tab-storage",
        ),
        (PreferencesTab::About, "About", "preferences-tab-about"),
    ] {
        let selected = tab == candidate;
        tabs = tabs.child(
            div()
                .id(selector)
                .debug_selector(move || selector.to_string())
                .px_3()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .text_color(if selected {
                    theme::accent()
                } else {
                    theme::text_muted()
                })
                .bg(if selected {
                    theme::selection_bg()
                } else {
                    theme::bg_pane()
                })
                .hover(|s| s.bg(theme::bg_control_hover()))
                .child(label)
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_preferences_tab(candidate, cx);
                })),
        );
    }

    let content = match tab {
        PreferencesTab::General => render_general(app, cx),
        PreferencesTab::Storage => render_storage(app, cx),
        PreferencesTab::About => render_about(app, cx),
    };

    div()
        .id("preferences-overlay")
        .debug_selector(|| "preferences-overlay".to_string())
        .absolute()
        .inset_0()
        .bg(theme::scrim())
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
                .w(px(560.0))
                .max_h(px(560.0))
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border())
                .rounded_xl()
                .shadow_2xl()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .px_5()
                        .pt_4()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .child("Preferences"),
                        )
                        .child(
                            div()
                                .id("preferences-close")
                                .debug_selector(|| "preferences-close".to_string())
                                .cursor_pointer()
                                .child(render_icon(Icon::X, px(12.0), theme::text()))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_preferences(cx);
                                })),
                        ),
                )
                .child(tabs)
                .child(
                    div()
                        .id("preferences-body")
                        .p_5()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .overflow_y_scroll()
                        .child(content),
                ),
        )
        .into_any_element()
}

fn render_general(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(section_title("LIBRARY"))
        .child(field("Library folder", &app.library_path_label()))
        .child(field("Items indexed", &app.total_library_count.to_string()))
        .child(button_row(
            cx,
            vec![
                ("preferences-rescan", "Rescan Library", |this, cx| {
                    this.rescan_library(cx)
                }),
                (
                    "preferences-reveal-library",
                    "Reveal in Finder",
                    |this, cx| this.reveal_library(cx),
                ),
            ],
        ))
        .child(section_title("DIAGNOSTICS"))
        .child(field(
            "Log file",
            &crate::logging::log_file_path().display().to_string(),
        ))
        .child(button_row(
            cx,
            vec![("preferences-reveal-logs", "Reveal Logs", |this, cx| {
                this.reveal_logs(cx)
            })],
        ))
        .into_any_element()
}

fn render_storage(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let cache_path = thumbnail_cache_dir();
    let size = app.preferences_cache_bytes.unwrap_or(0);
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(section_title("THUMBNAIL CACHE"))
        .child(field("Location", &cache_path.display().to_string()))
        .child(field("Size on disk", &format_bytes(size)))
        .child(button_row(
            cx,
            vec![
                ("preferences-clear-cache", "Clear Cache", |this, cx| {
                    this.clear_thumbnail_cache(cx)
                }),
                ("preferences-reveal-cache", "Reveal Cache", |this, cx| {
                    this.reveal_cache_directory(cx)
                }),
            ],
        ))
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("Thumbnails rebuild automatically; clearing only costs decode time."),
        )
        .into_any_element()
}

fn render_about(_app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text())
                .child("Lumina"),
        )
        .child(field("Version", env!("CARGO_PKG_VERSION")))
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("Local-first photo & video manager. No cloud, no telemetry."),
        )
        .child(section_title("SETTINGS FILE"))
        .child(field(
            "Config",
            &crate::config::AppConfig::config_path()
                .display()
                .to_string(),
        ))
        .child(button_row(
            cx,
            vec![("preferences-reveal-config", "Reveal Config", |this, cx| {
                this.reveal_config(cx)
            })],
        ))
        .into_any_element()
}

fn section_title(text: &str) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(FontWeight::BOLD)
        .text_color(theme::text_subtle())
        .child(text.to_string())
}

fn field(label: &str, value: &str) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(theme::text())
                .child(value.to_string()),
        )
}

type PreferencesAction = fn(&mut LuminaApp, &mut Context<LuminaApp>);

fn button_row(
    cx: &mut Context<LuminaApp>,
    buttons: Vec<(&'static str, &'static str, PreferencesAction)>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap_2()
        .children(buttons.into_iter().map(|(id, label, action)| {
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .px_3()
                .py_1()
                .rounded_md()
                .bg(theme::bg_control())
                .hover(|s| s.bg(theme::bg_control_hover()))
                .cursor_pointer()
                .text_xs()
                .text_color(theme::text())
                .child(label)
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    action(this, cx);
                }))
        }))
}
