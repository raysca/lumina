use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::ViewerCanvas;

/// Wheel-to-zoom gain. Pixel deltas become fractional zoom changes instead of
/// fixed 0.25 steps, so a trackpad scroll is smooth while the keyboard/HUD keep
/// their deterministic steps.
const SCROLL_ZOOM_SENSITIVITY: f32 = 0.0025;

/// Additive zoom change for a wheel delta at `scale`. The change is
/// multiplicative (`scale * e^(k*dy)`), so the same gesture feels equally fast
/// at 1x and at 12x.
fn scroll_zoom_delta(raw_delta: f32, scale: f32) -> f32 {
    scale * ((raw_delta * SCROLL_ZOOM_SENSITIVITY).clamp(-0.6, 0.6).exp() - 1.0)
}

pub struct ViewerViewport;

impl ViewerViewport {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let is_video = this
            .selected_item
            .as_ref()
            .map(|i| i.media_type == MediaType::Video)
            .unwrap_or(false);

        let is_immersive = this.is_immersive_fullscreen;
        let hud_visible = this.immersive_hud_visible;
        let preview_image = this.viewer_image.clone();
        let scale = this.viewer_state.zoom_scale;
        let (pan_x, pan_y) = this.viewer_state.pan_offset;
        let is_zoomed = scale > 1.0;
        let is_dragging = this.viewer_state.is_dragging;

        div()
            .id("viewer-viewport")
            .debug_selector(|| "viewer-viewport".to_string())
            .relative()
            .flex_1()
            .w_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            // Drag Panning Event Handlers
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _w, cx| {
                    let x: f32 = e.position.x.into();
                    let y: f32 = e.position.y.into();
                    this.start_viewer_drag(x, y, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _w, cx| {
                if this.is_immersive_fullscreen && !this.immersive_hud_visible {
                    this.immersive_hud_visible = true;
                }
                if this.viewer_state.is_dragging {
                    let x: f32 = e.position.x.into();
                    let y: f32 = e.position.y.into();
                    this.drag_viewer_to(x, y, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                    this.end_viewer_drag(cx);
                }),
            )
            // The double-click zoom handler lives on the media element itself
            // (see below), not on this full-bleed viewport: a double-click in
            // the letterbox around the picture must not zoom (BUG 14).
            // Left Chevron Button
            .child(if !is_immersive || hud_visible {
                div()
                    .id("viewer-prev-btn")
                    .debug_selector(|| "viewer-prev".to_string())
                    .tooltip(theme::tooltip_label("Previous item"))
                    .absolute()
                    .left_6()
                    .size(px(40.0))
                    .rounded_full()
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border())
                    .hover(|s| s.bg(theme::bg_control_hover()).text_color(theme::accent()))
                    .flex()
                    .justify_center()
                    .items_center()
                    .cursor_pointer()
                    .text_xl()
                    .text_color(theme::text_muted())
                    .child(render_icon(
                        Icon::ChevronLeft,
                        px(20.0),
                        theme::text_muted(),
                    ))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.navigate_selection(-1, cx);
                    }))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            // Right Chevron Button
            .child(if !is_immersive || hud_visible {
                div()
                    .id("viewer-next-btn")
                    .debug_selector(|| "viewer-next".to_string())
                    .tooltip(theme::tooltip_label("Next item"))
                    .absolute()
                    .right_6()
                    .size(px(40.0))
                    .rounded_full()
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border())
                    .hover(|s| s.bg(theme::bg_control_hover()).text_color(theme::accent()))
                    .flex()
                    .justify_center()
                    .items_center()
                    .cursor_pointer()
                    .text_xl()
                    .text_color(theme::text_muted())
                    .child(render_icon(
                        Icon::ChevronRight,
                        px(20.0),
                        theme::text_muted(),
                    ))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.navigate_selection(1, cx);
                    }))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            // Center Media Canvas: Image or Video Viewport
            .child(
                div()
                    .id("media-canvas-fit")
                    .relative()
                    .flex_1()
                    // Without these the flex item grows to the image's
                    // intrinsic size instead of fitting it to the window.
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, w, cx| {
                        let dy = match e.delta {
                            ScrollDelta::Pixels(p) => f32::from(p.y),
                            // A wheel notch is one line: ~50px zooms ~13%.
                            ScrollDelta::Lines(p) => p.y * 50.0,
                        };
                        if dy.abs() < 0.01 {
                            return;
                        }
                        // Keep the content point under the cursor fixed while
                        // zooming. The position is in window-content pixels; the
                        // anchor is the captured canvas center (the window
                        // center until the canvas has painted once).
                        let cursor = (f32::from(e.position.x), f32::from(e.position.y));
                        let viewport_size = w.viewport_size();
                        let viewport = (
                            f32::from(viewport_size.width),
                            f32::from(viewport_size.height),
                        );
                        let delta = scroll_zoom_delta(dy, this.viewer_state.zoom_scale);
                        this.zoom_viewer_at(delta, cursor, viewport, cx);
                    }))
                    .child(if let Some(render_img) = preview_image {
                        // Zoom must grow the bitmap, not just its wrapper: the
                        // Img fills the scaled wrapper (which is scale x the
                        // canvas) while keeping ObjectFit::Contain. At Fit the
                        // Img fills the canvas itself.
                        let media = img(render_img)
                            .id("viewer-media")
                            .debug_selector(|| "viewer-media".to_string())
                            .object_fit(ObjectFit::Contain)
                            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
                                // The element fills the canvas; only the
                                // visible media (not its letterbox) toggles.
                                if e.click_count() >= 2
                                    && this.viewer_state.hits_media(e.position())
                                {
                                    this.toggle_viewer_zoom(cx);
                                }
                            }));
                        if is_zoomed {
                            div()
                                .id("media-canvas-zoomed")
                                .relative()
                                // A zoomed wrapper is larger than the canvas
                                // by design; never let flex shrink it back.
                                .flex_none()
                                .left(px(pan_x))
                                .top(px(pan_y))
                                .w(relative(scale))
                                .h(relative(scale))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor(if is_dragging {
                                    CursorStyle::ClosedHand
                                } else {
                                    CursorStyle::OpenHand
                                })
                                .child(media.size_full())
                                .into_any_element()
                        } else {
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                // Fit fills the canvas (contain), enlarging
                                // small media like Photos does.
                                .child(media.size_full())
                                .into_any_element()
                        }
                    } else if is_video {
                        // Neutral canvas while the first video frame decodes.
                        div()
                            .w_full()
                            .h_full()
                            .max_w(px(1060.0))
                            .max_h(px(596.0))
                            .rounded_lg()
                            .overflow_hidden()
                            .bg(theme::bg_media())
                            .into_any_element()
                    } else {
                        div()
                            .w(px(400.0))
                            .h(px(400.0))
                            .bg(theme::bg_pane())
                            .rounded_lg()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_color(theme::text_subtle())
                            .child("Loading media...")
                            .into_any_element()
                    })
                    .child({
                        // Captures the canvas geometry the zoom math needs.
                        let app = cx.entity();
                        let media_px = this.viewer_image.as_ref().map(|image| {
                            let size = image.size(0);
                            (size.width.0 as f32, size.height.0 as f32)
                        });
                        let source_long_px = this
                            .selected_item
                            .as_ref()
                            .map_or(0.0, |item| item.width.max(item.height) as f32);
                        canvas(
                            move |bounds, window, cx| {
                                let geometry = media_px.and_then(|(w, h)| {
                                    ViewerCanvas::new(
                                        (
                                            f32::from(bounds.size.width),
                                            f32::from(bounds.size.height),
                                        ),
                                        (
                                            f32::from(bounds.center().x),
                                            f32::from(bounds.center().y),
                                        ),
                                        w / h,
                                        if source_long_px > 0.0 {
                                            source_long_px
                                        } else {
                                            w.max(h)
                                        },
                                        window.scale_factor(),
                                    )
                                });
                                app.update(cx, |this, _| this.viewer_state.canvas = geometry);
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full()
                    })
                    // Large Circular Play Indicator (Centered on Video when paused).
                    // Not while PiP owns the player: this is a poster then.
                    .child(
                        if is_video && !this.video.in_pip && !this.video.is_playing {
                            div()
                                .id("viewer-center-play-indicator")
                                .debug_selector(|| "viewer-center-play".to_string())
                                .absolute()
                                .size(px(68.0))
                                .rounded_full()
                                .bg(theme::scrim())
                                .border_1()
                                .border_color(theme::border_strong())
                                .hover(|s| s.bg(theme::bg_hud()).border_color(theme::accent()))
                                .flex()
                                .justify_center()
                                .items_center()
                                .cursor_pointer()
                                .shadow_xl()
                                .child(render_icon(Icon::Play, px(28.0), theme::text()))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.toggle_video_play(cx);
                                }))
                                .into_any_element()
                        } else {
                            Empty.into_any_element()
                        },
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::scroll_zoom_delta;

    #[core::prelude::v1::test]
    fn scroll_zoom_is_proportional_and_signed() {
        let small = scroll_zoom_delta(10.0, 1.0);
        let large = scroll_zoom_delta(40.0, 1.0);
        assert!(
            (small - 0.0253).abs() < 1e-3,
            "10px of wheel must be about 2.5%"
        );
        assert!(large > small, "a larger wheel delta must zoom more");
        assert!(
            scroll_zoom_delta(-10.0, 1.0) < 0.0,
            "scrolling back zooms out"
        );
        assert!(scroll_zoom_delta(0.0, 1.0).abs() < 1e-6);
        assert!(
            scroll_zoom_delta(10_000.0, 1.0) <= 0.6_f32.exp() - 1.0 + 1e-6,
            "one event must stay bounded even with a huge delta"
        );
    }

    #[core::prelude::v1::test]
    fn scroll_zoom_is_multiplicative() {
        let at_one = scroll_zoom_delta(10.0, 1.0);
        let at_eight = scroll_zoom_delta(10.0, 8.0);
        assert!(
            (at_eight / at_one - 8.0).abs() < 1e-3,
            "the same gesture must change zoom by the same ratio at any scale"
        );
        let out_and_back =
            (1.0 + scroll_zoom_delta(25.0, 1.0)) * (1.0 + scroll_zoom_delta(-25.0, 1.0));
        assert!(
            (out_and_back - 1.0).abs() < 1e-4,
            "in then out must return to the start"
        );
    }
}
