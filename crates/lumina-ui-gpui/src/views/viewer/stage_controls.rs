//! Controls that float over the viewer stage (v3): the Fit button and zoom
//! navigator while zoomed, and the previous/next buttons on the sides.

use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::ZOOM_MIN_SCALE;

/// While zoomed (v3): a round "Fit to window" button at the bottom and the
/// navigator at the top right. At Fit the stage shows no zoom chrome; the
/// wheel, +/−, 0/1/2/4 and Z zoom.
pub fn render_zoom_controls(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<AnyElement> {
    let state = this.viewer_state;
    if state.zoom_scale <= ZOOM_MIN_SCALE {
        return None;
    }
    let fit = div()
        .absolute()
        .bottom(px(24.0))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .id("viewer-zoom-fit")
                .debug_selector(|| "viewer-zoom-fit".to_string())
                .tooltip(theme::tooltip_label("Fit to window (0)"))
                // Clicks on the button must not start a pan.
                .occlude()
                .size(px(40.0))
                .rounded_full()
                .bg(theme::bg_hud())
                .border_1()
                .border_color(theme::border_strong())
                .shadow_lg()
                .hover(|s| s.bg(theme::bg_control_hover()))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Minimize, px(16.0), theme::text()))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.reset_viewer_zoom(cx);
                })),
        );
    Some(
        div()
            .absolute()
            .inset_0()
            .child(fit)
            .children(render_navigator(this))
            .into_any_element(),
    )
}

const NAVIGATOR_EDGE: f32 = 180.0;

/// Overview (long edge 180px) of the picture with an amber rect on the visible region.
fn render_navigator(this: &LuminaApp) -> Option<AnyElement> {
    let (x, y, w, h) = this.viewer_state.visible_region()?;
    let canvas = this.viewer_state.canvas?;
    let image = this.viewer_image.clone()?;
    let aspect = canvas.fitted.0 / canvas.fitted.1;
    // Long edge 180px; the short edge keeps room for the zoom label.
    let (width, height) = if aspect >= 1.0 {
        (NAVIGATOR_EDGE, (NAVIGATOR_EDGE / aspect).max(24.0))
    } else {
        ((NAVIGATOR_EDGE * aspect).max(24.0), NAVIGATOR_EDGE)
    };
    // Keep clear of the floating info drawer.
    let right = if this.inspector_open {
        super::DRAWER_WIDTH + super::DRAWER_INSET * 2.0
    } else {
        super::DRAWER_INSET
    };
    Some(
        div()
            .id("viewer-navigator")
            .debug_selector(|| "viewer-navigator".to_string())
            .occlude()
            .absolute()
            .top(px(16.0))
            .right(px(right))
            .w(px(width))
            .h(px(height))
            .rounded(theme::RADIUS_MD)
            .overflow_hidden()
            .border_1()
            .border_color(theme::border_strong())
            .bg(theme::bg_hud())
            .shadow_lg()
            .child(img(image).size_full().object_fit(ObjectFit::Fill))
            .child(
                div()
                    .debug_selector(|| "viewer-navigator-region".to_string())
                    .absolute()
                    .left(relative(x))
                    .top(relative(y))
                    .w(relative(w))
                    .h(relative(h))
                    .border_2()
                    .border_color(theme::accent()),
            )
            .child(
                div()
                    .id("viewer-zoom-label")
                    .debug_selector(|| "viewer-zoom-label".to_string())
                    .absolute()
                    .right(px(4.0))
                    .bottom(px(4.0))
                    .px(px(4.0))
                    .rounded(theme::RADIUS_SM)
                    .bg(theme::scrim())
                    .font_family(theme::FONT_MONO)
                    .text_size(px(11.0))
                    .text_color(theme::text())
                    .child(this.viewer_state.zoom_label()),
            )
            .into_any_element(),
    )
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
