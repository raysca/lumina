//! Settings: a full-window page with its own category sidebar, a TopBar per
//! category and scrolling grouped label/value/action rows.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::preferences::{PreferencesTab, SettingsButton};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::{header_icon_button, page_header_with_meta_font};

mod about;
mod downloads;
mod general;
mod library;
mod rows;
mod storage;
mod storage_card;

/// Width of the settings sidebar.
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
            meta: "Launch and external editor",
        },
        PreferencesTab::Library => Category {
            label: "Library",
            icon: Icon::Library,
            selector: "settings-category-library",
            meta: "Where the library lives and how big it is",
        },
        PreferencesTab::Downloads => Category {
            label: "Downloads",
            icon: Icon::Download,
            selector: "settings-category-downloads",
            meta: "How videos from links are downloaded and filed away",
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
        PreferencesTab::Library => library::render_library(app, cx),
        PreferencesTab::Downloads => downloads::render_downloads(app, cx),
        PreferencesTab::Storage => storage::render_storage(app, cx),
        PreferencesTab::About => about::render_about(app, cx),
    };
    let Category { label, meta, .. } = category(tab);
    let close = header_icon_button("preferences-close", Icon::X, "Close", Some("Esc"), false)
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.close_preferences(cx);
        }));
    // Reset puts no Storage or About value back, so it is General's alone.
    let actions = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .when(tab == PreferencesTab::General, |s| {
            s.child(reset_button(app, cx))
        })
        .child(close);
    let confirm = if app.preferences_reset_confirm {
        crate::views::smart_album_delete::confirm_dialog(
            "settings-reset",
            "Reset settings to defaults?".into(),
            "Panels, grid size, sort orders, layout and grouping, and the view on \
             launch go back to how they first were. Your library and files are not \
             affected."
                .into(),
            ("Cancel".into(), "Reset".into()),
            LuminaApp::cancel_reset_preferences,
            LuminaApp::confirm_reset_preferences,
            cx,
        )
    } else {
        Empty.into_any_element()
    };

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
                .child(page_header_with_meta_font(
                    None,
                    label,
                    Some(meta.into()),
                    theme::FONT_SANS,
                    actions,
                    false,
                ))
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
        .child(confirm)
        .into_any_element()
}

/// The General TopBar's "Reset to defaults" (h 32, r 8, `bg_2`, line
/// outline, refresh icon, 12.5).
fn reset_button(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let button = SettingsButton::Reset;
    div()
        .id(button.selector())
        .debug_selector(move || button.selector())
        .role(Role::Button)
        .aria_label(SharedString::from("Reset to defaults"))
        .h(px(32.0))
        .px(px(12.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(rows::focus_border(app, button, theme::line()))
        .bg(theme::bg_2())
        .hover(|s| s.bg(theme::bg_3()))
        .whitespace_nowrap()
        .text_size(px(12.5))
        .text_color(theme::t1())
        .cursor_pointer()
        .child(render_icon(Icon::RefreshCw, px(14.0), theme::t2()))
        .child("Reset to defaults")
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            button.press(this, cx);
        }))
}

/// The 252 px category sidebar: wordmark, "SETTINGS" and one nav item per
/// category.
fn render_sidebar(selected: PreferencesTab, cx: &mut Context<LuminaApp>) -> impl IntoElement {
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
        .pb(px(16.0))
        .px(px(crate::views::sidebar::SIDEBAR_PADDING_X))
        .flex()
        .flex_col()
        .gap(px(crate::views::sidebar::SIDEBAR_GAP))
        .child(crate::views::sidebar::render_brand("settings-brand", None))
        .child(categories)
}

/// Shortens a path under the home directory to `~/…` for display.
pub(crate) fn tilde(path: &std::path::Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_owned())) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
