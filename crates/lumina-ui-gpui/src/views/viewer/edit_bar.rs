//! The edit-mode bar that replaces the viewer's top bar: Cancel, rotate,
//! exposure, Auto, Crop (with the aspect presets while cropping), Revert to
//! Original and Done. Keys are in
//! `app/keymap/modals/`; state and actions in `app/quick_edit.rs`.

use gpui::*;

use crate::app::crop::CropAspect;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::ViewerRotation;

type Handler = fn(&mut LuminaApp, &mut Context<LuminaApp>);

/// A labelled bar button: role, label and tooltip (with its key) per §11.
fn bar_button(
    id: &'static str,
    base: fn(&'static str) -> Stateful<Div>,
    label: &'static str,
    tooltip: &'static str,
    on_click: Handler,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    base(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label)
        .tooltip(theme::tooltip_label(tooltip))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| on_click(this, cx)))
}

/// "+0.7 EV", "0.0 EV", "−1.3 EV".
fn exposure_label(ev: f32) -> String {
    if ev.abs() < 0.05 {
        "0.0 EV".to_string()
    } else {
        format!("{ev:+.1} EV").replace('-', "−")
    }
}

pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(session) = this.edits.session.as_ref() else {
        return Empty.into_any_element();
    };
    let draft = session.draft;
    let crop_aspect = session.crop.map(|crop| crop.aspect);
    let icon = |icon: Icon| render_icon(icon, theme::ICON_MD, theme::t1());
    let exposure = exposure_label(draft.exposure_ev);

    let tools = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            bar_button(
                "edit-rotate-left",
                theme::control_ghost,
                "Rotate Left",
                "Rotate Left (⇧R)",
                |this, cx| this.rotate_edit_draft(ViewerRotation::Left, cx),
                cx,
            )
            .child(icon(Icon::RotateCcw)),
        )
        .child(
            bar_button(
                "edit-rotate-right",
                theme::control_ghost,
                "Rotate Right",
                "Rotate Right (R)",
                |this, cx| this.rotate_edit_draft(ViewerRotation::Right, cx),
                cx,
            )
            .child(icon(Icon::RotateCw)),
        )
        .child(div().w(px(12.0)))
        .child(
            bar_button(
                "edit-exposure-down",
                theme::control_ghost,
                "Darker",
                "Darker, ⅓ stop (−)",
                |this, cx| this.step_edit_exposure(-1, cx),
                cx,
            )
            .child(icon(Icon::Minus)),
        )
        .child(
            div()
                .id("edit-exposure-value")
                .debug_selector(|| "edit-exposure-value".to_string())
                .role(Role::Status)
                .aria_label(format!("Exposure {exposure}"))
                .w(px(64.0))
                .flex()
                .justify_center()
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_color(theme::t1())
                .child(exposure),
        )
        .child(
            bar_button(
                "edit-exposure-up",
                theme::control_ghost,
                "Brighter",
                "Brighter, ⅓ stop (=)",
                |this, cx| this.step_edit_exposure(1, cx),
                cx,
            )
            .child(icon(Icon::Plus)),
        )
        .child(div().w(px(12.0)))
        .child(
            bar_button(
                "edit-auto",
                if draft.auto_enhance {
                    theme::control_on
                } else {
                    theme::control
                },
                if draft.auto_enhance {
                    "Auto Enhance, on"
                } else {
                    "Auto Enhance, off"
                },
                "Auto Enhance (A)",
                LuminaApp::toggle_edit_auto,
                cx,
            )
            .gap_1()
            .child(render_icon(Icon::Sparkles, theme::ICON_SM, theme::t1()))
            .child("Auto"),
        )
        .child(
            bar_button(
                "edit-crop",
                if crop_aspect.is_some() {
                    theme::control_on
                } else {
                    theme::control
                },
                if crop_aspect.is_some() {
                    "Crop, on"
                } else {
                    "Crop, off"
                },
                "Crop (C); ⇧C applies and keeps editing",
                LuminaApp::toggle_crop,
                cx,
            )
            .child("Crop"),
        )
        .child(
            bar_button(
                "edit-revert",
                theme::control,
                "Revert to Original",
                "Revert to Original",
                LuminaApp::revert_edit_draft,
                cx,
            )
            .child("Revert to Original"),
        );

    let bar = div()
        .id("edit-bar")
        .debug_selector(|| "edit-bar".to_string())
        .role(Role::Toolbar)
        .aria_label("Edit photo")
        .w_full()
        .h(px(theme::TITLEBAR_HEIGHT + 1.0)) // + its bottom border
        .px_4()
        .bg(theme::bg_0())
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .text_size(theme::TEXT_SMALL)
        .text_color(theme::t1())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                // Traffic lights spacer, as in the top bar.
                .child(div().w(px(64.0)))
                .child(
                    bar_button(
                        "edit-cancel",
                        theme::control,
                        "Cancel",
                        "Cancel (Esc)",
                        LuminaApp::cancel_edit,
                        cx,
                    )
                    .child("Cancel"),
                ),
        )
        .child(tools)
        .child(
            bar_button(
                "edit-done",
                theme::control_primary,
                "Done",
                "Done (Enter)",
                LuminaApp::finish_edit,
                cx,
            )
            .font_weight(FontWeight::MEDIUM)
            .child("Done"),
        );
    // The aspect presets get their own row while cropping, so the bar fits
    // the 960 pt minimum window.
    div()
        .w_full()
        .flex()
        .flex_col()
        .child(bar)
        .children(crop_aspect.map(|current| aspect_buttons(current, cx)))
        .into_any_element()
}

/// The aspect presets, shown while cropping; the current one is on.
fn aspect_buttons(current: CropAspect, cx: &mut Context<LuminaApp>) -> Div {
    div()
        .w_full()
        .h(px(40.0))
        .bg(theme::bg_0())
        .border_b_1()
        .border_color(theme::line())
        .justify_center()
        .text_size(theme::TEXT_SMALL)
        .text_color(theme::t1())
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .children(CropAspect::ALL.into_iter().map(|aspect| {
            let id = SharedString::from(format!("edit-aspect-{}", aspect.name()));
            let base: fn(SharedString) -> Stateful<Div> = if aspect == current {
                theme::control_on
            } else {
                theme::control
            };
            base(id.clone())
                .debug_selector(move || id.to_string())
                .role(Role::Button)
                .aria_label(format!("Crop aspect {}", aspect.label()))
                .tooltip(theme::tooltip_label(aspect.tooltip()))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_crop_aspect(aspect, cx)
                }))
                .child(aspect.label())
        }))
}

#[cfg(test)]
mod tests {
    use super::exposure_label;

    #[core::prelude::v1::test]
    fn exposure_label_reads_in_tenths() {
        assert_eq!(exposure_label(0.0), "0.0 EV");
        assert_eq!(exposure_label(2.0 / 3.0), "+0.7 EV");
        assert_eq!(exposure_label(-4.0 / 3.0), "−1.3 EV");
    }
}
