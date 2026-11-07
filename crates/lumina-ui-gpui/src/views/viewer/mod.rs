use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::inspector::InspectorView;

pub mod crop_overlay;
pub mod edit_bar;
pub mod scrub_preview;
pub mod stage_controls;
pub mod top_bar;
pub mod transport;
pub mod utils;
pub mod viewport;

pub use crate::views::viewer_state::ViewerState;
pub use top_bar::ViewerTopBar;
pub use transport::ViewerTransport;
pub use viewport::ViewerViewport;

/// Floating info drawer width and its gap to the window edge.
pub(super) const DRAWER_WIDTH: f32 = 340.0;
pub(super) const DRAWER_INSET: f32 = 16.0;

pub struct MediaViewer;

impl MediaViewer {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
        let item = match this.selected_item.as_ref() {
            Some(it) => it,
            None => return Empty.into_any_element(),
        };

        let is_video = item.media_type == MediaType::Video;
        // The viewer is always the fullscreen overlay, so the inspector drawer
        // simply follows the app's inspector state.
        let is_inspector_open = this.inspector_open;
        let is_immersive = this.is_immersive_fullscreen;
        let hud_visible = this.immersive_hud_visible;
        let show_chrome = !is_immersive || hud_visible;
        // Edit mode swaps the top bar for the edit bar and hides what could
        // step to, or act on, another photo (nav buttons, info drawer).
        let editing = this.edits.session.is_some();
        let show_drawer = is_inspector_open && show_chrome && !editing;

        let backdrop = div()
            .id("viewer-backdrop")
            .size_full()
            .bg(if is_immersive {
                theme::bg_media()
            } else {
                theme::bg_viewer()
            })
            .flex()
            .flex_col()
            .justify_between()
            .items_center()
            .overflow_hidden();

        backdrop
            .on_mouse_move(cx.listener(|this, _e: &MouseMoveEvent, _w, cx| {
                this.wake_immersive_hud(cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                    if this.viewer_state.is_dragging {
                        this.end_viewer_drag(cx);
                    }
                }),
            )
            // 1. Top Bar
            .child(if editing {
                edit_bar::render(this, cx)
            } else if is_immersive && !hud_visible {
                Empty.into_any_element()
            } else if is_immersive {
                // The HUD fades back in after an idle hide.
                crate::motion::fade_in(
                    div().w_full().child(ViewerTopBar::render(this, item, cx)),
                    "viewer-hud-top-in",
                    crate::motion::HUD_IN,
                )
                .into_any_element()
            } else {
                ViewerTopBar::render(this, item, cx).into_any_element()
            })
            // 2. Center Viewport
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    // The info drawer floats at the right; reserve its width
                    // so it never covers the media, next button or transport.
                    // Not in immersive mode: the drawer hides with the HUD
                    // there, and the photo must not resize each time.
                    .when(is_inspector_open && !is_immersive && !editing, |stage| {
                        stage.pr(px(DRAWER_WIDTH + DRAWER_INSET * 2.0))
                    })
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    // The media frame and its prev/next buttons share a box,
                    // so the buttons centre on the picture, not on the
                    // picture plus the video transport below it.
                    .child(
                        div()
                            .relative()
                            // A framed video takes its own height so the
                            // transport follows it; photos fill the stage.
                            .map(|frame| match viewport::video_frame_size(this) {
                                Some(_) => frame.flex_none(),
                                None => frame.flex_1(),
                            })
                            .min_h_0()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .child(ViewerViewport::render(this, cx))
                            .when(show_chrome && !editing, |frame| {
                                frame.children(stage_controls::render_nav_buttons(cx))
                            }),
                    )
                    .when(show_chrome && !is_video, |stage| {
                        stage.children(stage_controls::render_zoom_controls(this, cx))
                    })
                    // Video Transport Bar (Scrubber + Dock). While PiP owns the
                    // player the viewer shows the poster and a hand-off pill.
                    .child(if is_video && show_chrome && this.video.in_pip {
                        stage_controls::render_pip_handoff(this, item, cx).into_any_element()
                    } else if is_video && show_chrome && is_immersive {
                        crate::motion::fade_in(
                            div()
                                .w_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .child(ViewerTransport::render(this, cx)),
                            "viewer-hud-transport-in",
                            crate::motion::HUD_IN,
                        )
                        .into_any_element()
                    } else if is_video && show_chrome {
                        ViewerTransport::render(this, cx).into_any_element()
                    } else {
                        Empty.into_any_element()
                    }),
            )
            // 4. Floating Inspector Drawer (when opened)
            .child(if show_drawer {
                div()
                    .id("viewer-info-drawer")
                    .debug_selector(|| "viewer-info-drawer".to_string())
                    // The drawer floats over the viewport; keep clicks inside it
                    // from reaching the media canvas or the grid behind.
                    .occlude()
                    .absolute()
                    .top(px(60.0))
                    .right(px(DRAWER_INSET))
                    .bottom(px(DRAWER_INSET))
                    .w(px(DRAWER_WIDTH))
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(theme::RADIUS_LG)
                    .shadow_2xl()
                    .overflow_hidden()
                    .child(InspectorView::render(this, cx))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            .into_any_element()
    }
}
