use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::context_menu::ContextMenuState;
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

/// Additive zoom change for a trackpad pinch at `scale`. GPUI's `delta` is the
/// magnification change of one event (0.1 = +10%), so the change is
/// `scale * delta`; one event is bounded and non-finite input is ignored.
fn pinch_zoom_delta(pinch_delta: f32, scale: f32) -> f32 {
    if !pinch_delta.is_finite() || !scale.is_finite() {
        return 0.0;
    }
    scale * pinch_delta.clamp(-0.5, 0.5)
}

/// V3Video frame width; the transport card shares it.
pub(super) const VIDEO_FRAME_MAX_WIDTH: f32 = 1040.0;

/// V3Video: the video frame contain-fitted to at most 1040 wide and to the
/// stage left after the top bar, the docked info panel and the transport, so the
/// transport sits right below the picture and always stays on screen.
/// `None` for photos and immersive fullscreen, which fill the stage.
pub(super) fn video_frame_size(this: &LuminaApp) -> Option<(f32, f32)> {
    // ponytail: measured transport card (98) + its 12px top and 16px bottom
    // margins; update if the card grows.
    const TRANSPORT_BLOCK: f32 = 126.0;
    // The stage centres frame + transport, so a height-limited frame gets
    // 12px above it (the same as frame to transport) and 12px of slack below.
    const TOP_GAP: f32 = 24.0;
    // Room for the 40px prev/next buttons 26px from each edge, plus a 14px
    // gap to the frame.
    const NAV_ROOM: f32 = 160.0;
    let is_video = this
        .selected_item
        .as_ref()
        .is_some_and(|i| i.media_type == MediaType::Video);
    if !is_video || this.is_immersive_fullscreen {
        return None;
    }
    let aspect = this
        .viewer_image
        .as_ref()
        .map(|image| {
            let size = image.size(0);
            size.width.0 as f32 / size.height.0.max(1) as f32
        })
        .unwrap_or(16.0 / 9.0);
    let (_, _, win_w, win_h) = this.window_bounds.unwrap_or((0.0, 0.0, 1280.0, 840.0));
    let panel = if super::info_panel_shown(this) && !this.is_immersive_fullscreen {
        super::INFO_PANEL_WIDTH
    } else {
        0.0
    };
    let max_w = VIDEO_FRAME_MAX_WIDTH.min(win_w - panel - NAV_ROOM).max(1.0);
    let max_h = (win_h - super::top_bar::TOP_BAR_HEIGHT - TOP_GAP - TRANSPORT_BLOCK).max(1.0);
    let w = max_w.min(max_h * aspect);
    Some((w, w / aspect))
}

/// Corner radius of the photo at Fit (Design 02 "Detail").
const PHOTO_RADIUS: f32 = 3.0;

/// The photo's drop shadow (0 16 48) under the contain-fitted picture. The
/// media element fills the letterboxed canvas, so a box shadow on it would
/// outline the canvas; this paints under the fitted rect, measured in the
/// same frame. Fit only: a zoomed photo overflows the stage, which clips it.
fn photo_shadow(image_size: Size<DevicePixels>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if image_size.width.0 <= 0 || image_size.height.0 <= 0 {
                return;
            }
            let picture = ObjectFit::Contain.get_bounds(bounds, image_size);
            window.paint_drop_shadows(
                picture,
                Corners::all(px(PHOTO_RADIUS)),
                &[BoxShadow {
                    color: theme::media_scrim().into(),
                    offset: point(px(0.0), px(16.0)),
                    blur_radius: px(48.0),
                    spread_radius: px(0.0),
                    inset: false,
                }],
            );
        },
    )
    .absolute()
    .size_full()
}

pub struct ViewerViewport;

impl ViewerViewport {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let is_video = this
            .selected_item
            .as_ref()
            .map(|i| i.media_type == MediaType::Video)
            .unwrap_or(false);

        let is_missing = this
            .selected_item
            .as_ref()
            .is_some_and(|i| this.missing_items.contains(&i.id));

        // The decode gave up on this item: its grid thumbnail, or the
        // viewer's own full-size decode.
        let is_failed = this.selected_item.as_ref().is_some_and(|i| {
            this.viewer_failed == Some(i.id)
                || this
                    .grid_state
                    .as_ref()
                    .is_some_and(|state| state.is_failed(i.id))
        });

        let preview_image = this.viewer_image.clone();
        let scale = this.viewer_state.zoom_scale;
        let (pan_x, pan_y) = this.viewer_state.pan_offset;
        let is_zoomed = scale > 1.0;
        let is_dragging = this.viewer_state.is_dragging;
        // A fixed margin clear of the 40px prev/next buttons: 1264x760 at
        // 1440x900 like V3Photo, growing with larger windows (no size cap).
        // ponytail: the inset stays while zoomed, so zoom geometry keeps one
        // canvas size; cost: zoomed photos are clipped to it, not full-bleed.
        let inset = !is_video && !this.is_immersive_fullscreen;
        let viewport = match video_frame_size(this) {
            Some((w, h)) => div().flex_none().w(px(w)).h(px(h)),
            None => div().flex_1().w_full(),
        };
        viewport
            .id("viewer-viewport")
            .debug_selector(|| "viewer-viewport".to_string())
            .relative()
            .overflow_hidden()
            .when(inset, |d| d.px(px(88.0)).py(px(44.0)))
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
                this.wake_immersive_hud(cx);
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
            // The viewer's right-click menu holds what the slim top bar does
            // not: Open, Reveal, Edit, Rotate (v3).
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, e: &MouseDownEvent, _w, cx| {
                    // The menu acts on the shown item only: drop any grid
                    // multi-selection so Delete/Rate cannot reach hidden items.
                    // The viewer can be open without a grid index.
                    // Edit mode has its own bar; the menu's Delete and
                    // Rotate must not act under the draft.
                    // A slideshow has no menu: its Delete and Rotate would
                    // act on a slide behind the timer.
                    if this.edits.session.is_some() || this.slideshow.is_some() {
                        return;
                    }
                    if let Some(item) = this.selected_item.clone() {
                        this.selected_indices.clear();
                        this.selected_indices.extend(this.selected_index);
                        this.selection_anchor = this.selected_index;
                        // One item now: its own summary and tags (a large
                        // selection's may still be loading).
                        this.update_batch_summary();
                        this.refresh_selected_tags();
                        this.context_menu_state = Some(ContextMenuState {
                            position: e.position,
                            item,
                            hovered: None,
                        });
                        cx.notify();
                    }
                }),
            )
            // The double-click zoom handler lives on the media element itself
            // (see below), not on this full-bleed viewport: a double-click in
            // the letterbox around the picture must not zoom (BUG 14).
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
                    // Pinch zooms at the pinch centre; each event is an
                    // increment, so the phase is ignored.
                    .on_pinch(cx.listener(|this, e: &PinchEvent, w, cx| {
                        let viewport_size = w.viewport_size();
                        let viewport = (
                            f32::from(viewport_size.width),
                            f32::from(viewport_size.height),
                        );
                        let centre = (f32::from(e.position.x), f32::from(e.position.y));
                        let delta = pinch_zoom_delta(e.delta, this.viewer_state.zoom_scale);
                        this.zoom_viewer_at(delta, centre, viewport, cx);
                        cx.stop_propagation();
                    }))
                    // Scroll pans while zoomed (nothing at Fit), like Photos;
                    // ⌘-scroll zooms at the pointer so a mouse wheel can zoom.
                    .on_scroll(cx.listener(|this, e: &ScrollEvent, w, cx| {
                        if !e.modifiers.secondary(cx) {
                            let d = e.delta.pixel_delta(w.line_height());
                            this.pan_viewer_by(f32::from(d.x), f32::from(d.y), cx);
                            return;
                        }
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
                        let image_size = render_img.size(0);
                        let media = img(render_img)
                            .id("viewer-media")
                            .debug_selector(|| "viewer-media".to_string())
                            .object_fit(ObjectFit::Contain)
                            // GPUI rounds the fitted picture, not the
                            // letterboxed element.
                            .when(!is_video, |m| m.rounded(px(PHOTO_RADIUS)))
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
                                .relative()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(!is_video, |fit| fit.child(photo_shadow(image_size)))
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
                            .rounded(theme::RADIUS_LG)
                            .overflow_hidden()
                            .bg(theme::bg_media())
                            .into_any_element()
                    } else if is_failed {
                        // The decode gave up: say so, and how to see the file.
                        div()
                            .debug_selector(|| "viewer-stage-failed".to_string())
                            .size_full()
                            .rounded(theme::RADIUS_LG)
                            .bg(theme::bg_media())
                            .flex()
                            .flex_col()
                            .justify_center()
                            .items_center()
                            .gap_2()
                            .child(render_icon(Icon::TriangleAlert, px(28.0), theme::t2()))
                            .child(
                                div()
                                    .text_color(theme::t1())
                                    .child(crate::i18n::t("viewer.preview_failed")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme::t2())
                                    .child(crate::i18n::t("viewer.open_default_hint")),
                            )
                            .into_any_element()
                    } else {
                        // Neutral full stage until the frame arrives: the same
                        // size as the picture, so stepping never jumps (the
                        // swap to the full frame has no fade, Decision 8).
                        div()
                            .debug_selector(|| "viewer-stage-placeholder".to_string())
                            .size_full()
                            .rounded(theme::RADIUS_LG)
                            .bg(theme::bg_media())
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
                    // Crop mode's rectangle and handles, over the frame.
                    .children(super::crop_overlay::render(this, cx))
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
                                .border_color(theme::line_2())
                                .hover(|s| s.bg(theme::bg_float()).border_color(theme::t2()))
                                .flex()
                                .justify_center()
                                .items_center()
                                .cursor_pointer()
                                .shadow_xl()
                                .child(render_icon(Icon::Play, px(28.0), theme::t1()))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.toggle_video_play(cx);
                                }))
                                .into_any_element()
                        } else {
                            Empty.into_any_element()
                        },
                    ),
            )
            // The cached thumbnail may still show; say why it goes no further.
            .when(is_missing, |d| {
                d.child(
                    div()
                        .debug_selector(|| "viewer-missing".to_string())
                        .absolute()
                        .bottom(px(24.0))
                        .px_3()
                        .py_2()
                        .rounded(theme::RADIUS_MD)
                        .bg(theme::bg_float())
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(Icon::TriangleAlert, px(14.0), theme::danger()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme::t1())
                                .child("The original file is missing."),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{pinch_zoom_delta, scroll_zoom_delta};

    #[core::prelude::v1::test]
    fn pinch_zoom_delta_is_multiplicative_and_finite() {
        assert!((pinch_zoom_delta(0.1, 1.0) - 0.1).abs() < 1e-6);
        assert!(
            (pinch_zoom_delta(0.1, 8.0) / pinch_zoom_delta(0.1, 1.0) - 8.0).abs() < 1e-4,
            "the same pinch must change zoom by the same ratio at any scale"
        );
        assert!(pinch_zoom_delta(-0.1, 2.0) < 0.0, "pinching in zooms out");
        assert_eq!(pinch_zoom_delta(5.0, 1.0), 0.5, "one event stays bounded");
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                pinch_zoom_delta(bad, 1.0),
                0.0,
                "non-finite pinch must not zoom"
            );
            assert_eq!(
                pinch_zoom_delta(0.1, bad),
                0.0,
                "non-finite scale must not zoom"
            );
        }
    }

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
