//! Settings › General: the launch view, the external editor and the
//! version.

use gpui::*;

use super::rows::{controls, row, row_button, section, value};
use crate::app::LuminaApp;
use crate::config::LaunchView;
use crate::theme;

pub(super) fn render_general(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let editor_set = app.config.validated_external_editor().is_some();
    div()
        .flex()
        .flex_col()
        .gap(px(14.0))
        .child(section(
            "GENERAL",
            None,
            [row(
                "Default view on launch",
                Some("Where Lumina opens after a restart."),
                controls().child(launch_view_control(app, cx)),
            )],
        ))
        .child(section(
            "EXTERNAL EDITOR",
            None,
            [row(
                "Editor",
                Some("Edits happen in that app; Edit a Copy keeps the original."),
                controls()
                    .child(
                        value(&app.external_editor_label(), false)
                            .text_color(if editor_set { theme::t1() } else { theme::t2() })
                            .debug_selector(|| "preferences-external-editor".to_string()),
                    )
                    .child(row_button(
                        cx,
                        "preferences-choose-editor",
                        "Choose Editor…",
                        None,
                        |this, cx| this.choose_external_editor(cx),
                    )),
            )
            .debug_selector(|| "settings-row-editor".to_string())],
        ))
        .child(section(
            "UPDATES",
            Some(format!("Lumina {}", env!("CARGO_PKG_VERSION")).into()),
            [],
        ))
        .into_any_element()
}

/// Library or Last viewed. A radio group for VoiceOver; ←/→ pick a
/// segment while General is shown (`app/keymap/modals.rs`).
fn launch_view_control(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let current = app.config.launch_view;
    theme::segmented("settings-launch-view")
        .debug_selector(|| "settings-launch-view".to_string())
        .role(Role::RadioGroup)
        .aria_label("Default view on launch")
        .child(launch_view_item(
            cx,
            LaunchView::Library,
            "settings-launch-view-library",
            "Library",
            current,
        ))
        .child(launch_view_item(
            cx,
            LaunchView::LastViewed,
            "settings-launch-view-last",
            "Last viewed",
            current,
        ))
}

fn launch_view_item(
    cx: &mut Context<LuminaApp>,
    view: LaunchView,
    id: &'static str,
    label: &'static str,
    current: LaunchView,
) -> Stateful<Div> {
    let on = view == current;
    theme::segmented_item(id, on)
        .debug_selector(move || id.to_string())
        .role(Role::RadioButton)
        .aria_label(label)
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .child(label)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.set_launch_view(view, cx);
        }))
}
