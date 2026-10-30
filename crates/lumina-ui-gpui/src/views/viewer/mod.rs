use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::inspector::InspectorView;

pub mod stage_controls;
pub mod top_bar;
pub mod transport;
pub mod utils;
pub mod viewport;

pub use crate::views::viewer_state::ViewerState;
pub use top_bar::ViewerTopBar;
pub use transport::ViewerTransport;
pub use viewport::ViewerViewport;

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
                if this.is_immersive_fullscreen && !this.immersive_hud_visible {
                    this.immersive_hud_visible = true;
                    cx.notify();
                }
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
            .child(if is_immersive && !hud_visible {
                Empty.into_any_element()
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
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(ViewerViewport::render(this, cx))
                    // v3: prev/next on the stage edges and, for stills, a
                    // floating zoom pill. There is no filmstrip tray.
                    .when(show_chrome, |stage| {
                        stage.children(stage_controls::render_nav_buttons(cx))
                    })
                    .when(show_chrome && !is_video, |stage| {
                        stage.child(stage_controls::render_zoom_hud(this, cx))
                    })
                    // Video Transport Bar (Scrubber + Dock). While PiP owns the
                    // player the viewer shows the poster and a hand-off pill.
                    .child(if is_video && show_chrome && this.video.in_pip {
                        stage_controls::render_pip_handoff(this, item, cx).into_any_element()
                    } else if is_video && show_chrome {
                        ViewerTransport::render(this, cx).into_any_element()
                    } else {
                        Empty.into_any_element()
                    }),
            )
            // 4. Floating Inspector Drawer (when opened)
            .child(if is_inspector_open && (!is_immersive || hud_visible) {
                div()
                    .id("viewer-info-drawer")
                    .debug_selector(|| "viewer-info-drawer".to_string())
                    // The drawer floats over the viewport; keep clicks inside it
                    // from reaching the media canvas or the grid behind.
                    .occlude()
                    .absolute()
                    .top(px(60.0))
                    .right_4()
                    .bottom(px(16.0))
                    .w(px(340.0))
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded_xl()
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
