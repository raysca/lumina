//! Controls that float over the viewer stage (v3): the zoom pill at the bottom
//! and the previous/next buttons on the sides.

use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::ViewerZoomPreset;

/// Bottom-center zoom pill: − Fit 100% 200% 400% + with the live readout.
pub fn render_zoom_hud(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let state = this.viewer_state;
    div()
        .absolute()
        .bottom(px(24.0))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .id("viewer-zoom-hud")
                .debug_selector(|| "viewer-zoom-hud".to_string())
                // Clicks on the pill must not start a pan or a double-click zoom.
                .occlude()
                .p(px(4.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .rounded(px(8.0))
                .bg(theme::bg_hud())
                .border_1()
                .border_color(theme::border_strong())
                .shadow_lg()
                .text_xs()
                .child(step_button(
                    "viewer-zoom-out",
                    "Zoom out (−)",
                    "−",
                    cx,
                    |this, cx| this.zoom_out_viewer(cx),
                ))
                .children(
                    ViewerZoomPreset::ALL
                        .into_iter()
                        .map(|preset| preset_button(preset, state.matches_preset(preset), cx)),
                )
                .child(step_button(
                    "viewer-zoom-in",
                    "Zoom in (+)",
                    "+",
                    cx,
                    |this, cx| this.zoom_in_viewer(cx),
                ))
                .child(
                    div()
                        .id("viewer-zoom-label")
                        .debug_selector(|| "viewer-zoom-label".to_string())
                        .tooltip(theme::tooltip_label(
                            "Zoom as a percentage of actual pixels",
                        ))
                        .min_w(px(44.0))
                        .px(px(6.0))
                        .text_center()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.0))
                        .text_color(theme::text_subtle())
                        .child(state.zoom_label()),
                )
                .child(
                    div()
                        .id("viewer-zoom-reset")
                        .debug_selector(|| "viewer-zoom-reset".to_string())
                        .tooltip(theme::tooltip_label("Reset zoom and pan (0)"))
                        .size(px(28.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .child(render_icon(Icon::RotateCcw, px(12.0), theme::text_muted()))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.reset_viewer_zoom(cx);
                        })),
                ),
        )
        .into_any_element()
}

fn preset_button(
    preset: ViewerZoomPreset,
    active: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let id = match preset {
        ViewerZoomPreset::Fit => "viewer-zoom-fit",
        ViewerZoomPreset::Actual => "viewer-zoom-actual",
        ViewerZoomPreset::TwoX => "viewer-zoom-2x",
        ViewerZoomPreset::FourX => "viewer-zoom-4x",
    };
    let button = div()
        .id(id)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(preset.tooltip()))
        .h(px(28.0))
        .px(px(12.0))
        .flex()
        .items_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .child(preset.label())
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.apply_viewer_zoom_preset(preset, cx);
        }));
    if active {
        button
            .bg(theme::selection_bg())
            .text_color(theme::selection_fg())
    } else {
        button
            .text_color(theme::text_muted())
            .hover(|s| s.bg(theme::bg_control_hover()).text_color(theme::text()))
    }
}

fn step_button(
    id: &'static str,
    tooltip: &'static str,
    glyph: &'static str,
    cx: &mut Context<LuminaApp>,
    action: fn(&mut LuminaApp, &mut Context<LuminaApp>),
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(tooltip))
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .text_sm()
        .text_color(theme::text_muted())
        .hover(|s| s.bg(theme::bg_control_hover()).text_color(theme::text()))
        .child(glyph)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| action(this, cx)))
}

/// Round previous/next buttons centred on the stage's left and right edges.
/// They call the same navigation as [ and ] (←/→ seek on video).
pub fn render_nav_buttons(cx: &mut Context<LuminaApp>) -> [AnyElement; 2] {
    [
        nav_button("viewer-prev", "Previous ([)", Icon::ChevronLeft, -1, cx)
            .left(px(40.0))
            .into_any_element(),
        nav_button("viewer-next", "Next (])", Icon::ChevronRight, 1, cx)
            .right(px(40.0))
            .into_any_element(),
    ]
}

fn nav_button(
    id: &'static str,
    tooltip: &'static str,
    icon: Icon,
    delta: isize,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(tooltip))
        .occlude()
        .absolute()
        .top(relative(0.5))
        .mt(px(-20.0))
        .size(px(40.0))
        .rounded_full()
        .bg(theme::bg_app())
        .border_1()
        .border_color(theme::border())
        .hover(|s| s.bg(theme::bg_control()))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(icon, px(16.0), theme::text_muted()))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.navigate_selection(delta, cx);
        }))
}

/// Bottom-center pill shown instead of the transport while PiP owns the
/// player: this video can be played in PiP, or, when it is the PiP video,
/// brought back into the viewer.
pub fn render_pip_handoff(
    this: &LuminaApp,
    item: &lumina_core::MediaItem,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let is_pip_video = this.video.item.as_ref().is_some_and(|v| v.id == item.id);
    let (label, action) = if is_pip_video {
        ("Show Here", "Playing in Picture-in-Picture")
    } else {
        (
            "Play in Picture-in-Picture",
            "Picture-in-Picture is playing another video",
        )
    };
    let target = item.clone();
    div()
        .absolute()
        .bottom(px(24.0))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            theme::hud_panel("viewer-pip-handoff")
                .debug_selector(|| "viewer-pip-handoff".to_string())
                .occlude()
                .h(px(40.0))
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(render_icon(Icon::Pip, px(14.0), theme::text_muted()))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(action),
                )
                .child(
                    theme::control_primary("viewer-pip-handoff-btn")
                        .debug_selector(|| "viewer-pip-handoff-btn".to_string())
                        .tooltip(theme::tooltip_label(if is_pip_video {
                            "Move the video from Picture-in-Picture into the viewer"
                        } else {
                            "Play this video in Picture-in-Picture (Space)"
                        }))
                        .child(label)
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            if is_pip_video {
                                this.expand_pip_to_viewer(cx);
                            } else {
                                this.play_in_pip(target.clone(), cx);
                            }
                        })),
                ),
        )
        .into_any_element()
}
