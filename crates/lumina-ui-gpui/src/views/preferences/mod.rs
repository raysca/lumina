//! Settings: a full-window page with its own category sidebar, a TopBar per
//! category and scrolling grouped label/value/action rows.

use gpui::*;

use crate::app::preferences::PreferencesTab;
use crate::app::LuminaApp;
use crate::icons::Icon;
use crate::theme;
use crate::views::page_header::{header_icon_button, page_header};

mod about;
mod general;
mod rows;
mod storage;

/// Width of the settings sidebar (frame `l85pQN`).
const SIDEBAR_WIDTH: f32 = 252.0;

/// How a category shows in the sidebar and its TopBar.
struct Category {
    label: &'static str,
    icon: Icon,
    selector: &'static str,
    meta: &'static str,
}

fn category(tab: PreferencesTab) -> Category {
    match tab {
        PreferencesTab::General => Category {
            label: "General",
            icon: Icon::Settings,
            selector: "settings-category-general",
            meta: "Library and external editor",
        },
        PreferencesTab::Storage => Category {
            label: "Storage",
            icon: Icon::Database,
            selector: "settings-category-storage",
            meta: "Thumbnail cache, settings file and logs",
        },
        PreferencesTab::About => Category {
            label: "About",
            icon: Icon::Info,
            selector: "settings-category-about",
            meta: "Version, licence and acknowledgements",
        },
    }
}

/// The settings page. It covers the whole window (library or welcome
/// screen) and takes every click, so nothing behind it reacts.
pub fn render_preferences_modal(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.preferences_open {
        return Empty.into_any_element();
    }
    let tab = app.preferences_tab;
    let content = match tab {
        PreferencesTab::General => general::render_general(app, cx),
        PreferencesTab::Storage => storage::render_storage(app, cx),
        PreferencesTab::About => about::render_about(cx),
    };
    let Category { label, meta, .. } = category(tab);
    let close = header_icon_button("preferences-close", Icon::X, "Close", Some("Esc"), false)
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.close_preferences(cx);
        }));

    div()
        .id("settings-page")
        .debug_selector(|| "settings-page".to_string())
        .occlude()
        .absolute()
        .inset_0()
        .bg(theme::bg_0())
        .flex()
        .flex_row()
        .child(render_sidebar(tab, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                // The library's banner is hidden under this page.
                .child(app.render_error_banner(cx))
                .child(page_header(Empty, label, Some(meta.into()), close, false))
                .child(
                    div()
                        .id("preferences-body")
                        .debug_selector(|| "preferences-body".to_string())
                        .flex_1()
                        .min_h(px(0.0))
                        .px(px(32.0))
                        .py(px(16.0))
                        .flex()
                        .flex_col()
                        .overflow_y_scroll()
                        .child(content),
                ),
        )
        .into_any_element()
}

/// The 252 px category sidebar: wordmark, "SETTINGS" and one nav item per
/// category.
fn render_sidebar(selected: PreferencesTab, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    // Clear the traffic lights, as the library sidebar's brand row does.
    let inset = if cfg!(target_os = "macos") { 72.0 } else { 2.0 };
    let mut categories = div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(theme::section_label("SETTINGS").px(px(10.0)).pb(px(7.0)));
    for tab in PreferencesTab::ALL {
        let Category {
            label,
            icon,
            selector,
            ..
        } = category(tab);
        categories = categories.child(
            theme::nav_item(selector.to_string(), icon, label, 0, false, tab == selected).on_click(
                cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_preferences_tab(tab, cx);
                }),
            ),
        );
    }
    div()
        .debug_selector(|| "settings-sidebar".to_string())
        .w(px(SIDEBAR_WIDTH))
        .h_full()
        .flex_shrink_0()
        .bg(theme::bg_1())
        .border_r_1()
        .border_color(theme::line())
        .py(px(16.0))
        .px(px(12.0))
        .flex()
        .flex_col()
        .gap(px(18.0))
        .child(
            div()
                .pl(px(inset))
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(18.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child("Lumina"),
        )
        .child(categories)
}

/// Shortens a path under the home directory to `~/…` for display.
pub(crate) fn tilde(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_owned())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
