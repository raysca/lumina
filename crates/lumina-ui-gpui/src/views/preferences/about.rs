//! Settings › About: version, licence, acknowledgements, and updates
//! (macOS: Check for Updates / Restart to Update, and Check Daily).

use gpui::prelude::FluentBuilder;
use gpui::*;

use super::rows::row_button;
use crate::app::preferences::SettingsButton;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

pub(super) fn render_about(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
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
                .child(render_icon(Icon::Sparkles, px(38.0), theme::on_accent())),
        )
        .child(
            div()
                .text_size(theme::TEXT_TITLE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child("Lumina"),
        )
        .child(
            div()
                .debug_selector(|| "preferences-version".to_string())
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_SMALL)
                .text_color(theme::t2())
                .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
        )
        .child(
            div()
                .max_w(px(380.0))
                .text_size(theme::TEXT_BODY)
                .text_center()
                .text_color(theme::t2())
                .child(
                    "A local-first photo and video manager. No account, no telemetry, \
                     and no network use you didn't ask for. Your files stay where they are.",
                ),
        )
        .child(
            div()
                .pt(px(6.0))
                .flex()
                .flex_row()
                .gap_2()
                .child(row_button(
                    app,
                    cx,
                    SettingsButton::License,
                    "MIT License",
                    None,
                ))
                .child(row_button(
                    app,
                    cx,
                    SettingsButton::Acknowledgements,
                    "Acknowledgements",
                    None,
                )),
        )
        .when(crate::app::updates::supported(), |about| {
            about.child(render_updates(app, cx))
        })
        .into_any_element()
}

/// The update button (by state), Check Daily, and the status line.
fn render_updates(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Div {
    let button = app.update_button();
    let label = match button {
        SettingsButton::RestartToUpdate => "Restart to Update",
        SettingsButton::ReleasePage => "Open Release Page",
        _ => "Check for Updates",
    };
    let on = app.config.auto_update_check;
    let status = app.update_status();
    div()
        .pt(px(14.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(row_button(app, cx, button, label, None))
                .child(
                    row_button(
                        app,
                        cx,
                        SettingsButton::AutoUpdates,
                        "Check Daily",
                        on.then_some(Icon::Check),
                    )
                    .tooltip(theme::tooltip_label(
                        "Look for a new version when Lumina opens, once a day",
                    ))
                    .aria_toggled(if on {
                        Toggled::True
                    } else {
                        Toggled::False
                    }),
                ),
        )
        .when(!status.is_empty(), |s| {
            s.child(
                div()
                    .debug_selector(|| "preferences-update-status".to_string())
                    .max_w(px(380.0))
                    .text_size(theme::TEXT_SMALL)
                    .text_center()
                    .text_color(theme::t2())
                    .children(text_marker(
                        "preferences-update-status",
                        &status.clone().into(),
                    ))
                    .child(status),
            )
        })
}
