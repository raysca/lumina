//! Controls that float over the viewer stage (v3): the Fit button and zoom
//! navigator while zoomed, and the previous/next buttons on the sides.

use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MIN_SCALE};

/// While zoomed (v3): a round "Fit to window" button at the bottom and the
/// navigator (with Fit · Fill · 1:1 buttons) at the top right. At Fit the
/// stage shows no zoom chrome; pinch, ⌘-scroll, +/−, ⌘0–⌘4, ⌘K and Z zoom
/// (a plain scroll only pans when zoomed).
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
                .tooltip(theme::tooltip_label("Fit to window (⌘0)"))
                // Clicks on the button must not start a pan.
                .occlude()
                .size(px(40.0))
                .rounded_full()
                .bg(theme::bg_float())
                .border_1()
                .border_color(theme::line_2())
                .shadow_lg()
                .hover(|s| s.bg(theme::bg_3()))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Minimize, px(16.0), theme::t1()))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.reset_viewer_zoom(cx);
                })),
        );
    Some(
        div()
            .absolute()
            .inset_0()
            .child(fit)
            .children(render_navigator(this, cx))
            .into_any_element(),
    )
}

const NAVIGATOR_EDGE: f32 = 180.0;
/// Gap from the navigator to the stage's top-right corner.
const NAVIGATOR_INSET: f32 = 16.0;

/// Overview (long edge 180px) of the picture with an amber rect on the
/// visible region, and the Fit · Fill · 1:1 preset buttons below it.
fn render_navigator(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<AnyElement> {
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
    // Windowed, the info panel is docked beside the stage; immersive, it
    // floats over the stage's right edge, so the navigator moves left of it.
    let right = if this.is_immersive_fullscreen && super::info_panel_shown(this) {
        super::INFO_PANEL_WIDTH + NAVIGATOR_INSET
    } else {
        NAVIGATOR_INSET
    };
    let presets = [
        (ViewerZoomPreset::Fit, "fit", "zoom.preset_fit"),
        (ViewerZoomPreset::Fill, "fill", "zoom.preset_fill"),
        (ViewerZoomPreset::Actual, "actual", "zoom.preset_actual"),
    ];
    let buttons = presets.map(|(preset, slug, key)| {
        let id = format!("viewer-zoom-preset-{slug}");
        let hint = t(&format!("{key}_hint"));
        let active = (this.viewer_state.zoom_scale - preset.scale(Some(canvas))).abs() < 0.005;
        theme::segmented_item(SharedString::from(id.clone()), active)
            .debug_selector(move || id.clone())
            .role(Role::Button)
            .aria_label(hint.clone())
            .tooltip(theme::tooltip_label(hint))
            .child(t(key))
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                this.apply_viewer_zoom_preset(preset, cx);
            }))
    });
    let overview = div()
        .id("viewer-navigator")
        .debug_selector(|| "viewer-navigator".to_string())
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .border_1()
        .border_color(theme::line_2())
        .bg(theme::bg_float())
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
                .text_color(theme::t1())
                .child(this.viewer_state.zoom_label()),
        );
    Some(
        div()
            // Clicks on the navigator or its buttons must not start a pan.
            .occlude()
            .absolute()
            .top(px(NAVIGATOR_INSET))
            .right(px(right))
            .flex()
            .flex_col()
            .items_end()
            .gap_2()
            .child(overview)
            .child(theme::segmented("viewer-zoom-presets").children(buttons))
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
        .bg(theme::bg_0())
        .border_1()
        .border_color(theme::line())
        .hover(|s| s.bg(theme::bg_2()))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(icon, px(16.0), theme::t2()))
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
                .child(render_icon(Icon::Pip, px(14.0), theme::t2()))
                .child(div().text_xs().text_color(theme::t2()).child(action))
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
