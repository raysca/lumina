use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

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
            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
                if e.click_count() >= 2 {
                    this.toggle_viewer_zoom(cx);
                }
            }))
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
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _w, cx| {
                        let dy = match e.delta {
                            ScrollDelta::Pixels(p) => f32::from(p.y),
                            ScrollDelta::Lines(p) => p.y * 10.0,
                        };
                        if dy.abs() < 0.01 {
                            return;
                        }
                        this.zoom_viewer_by(dy.signum() * 0.25, cx);
                    }))
                    .child(if let Some(render_img) = preview_image {
                        if is_zoomed {
                            div()
                                .id("media-canvas-zoomed")
                                .relative()
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
                                .child(
                                    img(render_img)
                                        .max_w_full()
                                        .max_h_full()
                                        .object_fit(ObjectFit::Contain),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    img(render_img)
                                        .max_w_full()
                                        .max_h_full()
                                        .object_fit(ObjectFit::Contain),
                                )
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
                    // Large Circular Play Indicator (Centered on Video when paused)
                    .child(if is_video && !this.video.is_playing {
                        div()
                            .id("viewer-center-play-indicator")
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
                    }),
            )
    }
}
