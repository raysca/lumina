use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::theme;
use crate::views::inspector::InspectorView;

pub mod chapter_labels;
pub mod crop_overlay;
pub mod edit_bar;
pub mod filmstrip;
pub mod scrub_preview;
pub mod stage_controls;
pub mod top_bar;
pub mod transport;
pub mod transport_controls;
pub mod utils;
pub mod viewport;

pub use crate::views::viewer_state::ViewerState;
pub use top_bar::ViewerTopBar;
pub use transport::ViewerTransport;
pub use viewport::ViewerViewport;

/// Docked info panel width (Design 02 "Detail").
pub(super) const INFO_PANEL_WIDTH: f32 = 340.0;

/// Whether the viewer shows the info panel. Edit mode and a slideshow hide
/// it, and immersive mode hides it with the HUD. Windowed, it is docked
/// beside the stage; immersive, it floats over the stage below the top bar,
/// so the photo does not resize each time the HUD wakes.
pub(super) fn info_panel_shown(this: &LuminaApp) -> bool {
    this.inspector_open
        && (!this.is_immersive_fullscreen || this.immersive_hud_visible)
        && this.edits.session.is_none()
        && this.slideshow.is_none()
}

/// The stage column's width: the window minus the docked info panel.
/// `None` before the window bounds are known.
pub(super) fn stage_width(this: &LuminaApp) -> Option<f32> {
    let (_, _, width, _) = this.window_bounds?;
    let docked = info_panel_shown(this) && !this.is_immersive_fullscreen;
    Some(if docked {
        width - INFO_PANEL_WIDTH
    } else {
        width
    })
}

pub struct MediaViewer;

impl MediaViewer {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
        let item = match this.selected_item.as_ref() {
            Some(it) => it,
            None => return Empty.into_any_element(),
        };

        let is_video = item.media_type == MediaType::Video;
        let is_immersive = this.is_immersive_fullscreen;
        let hud_visible = this.immersive_hud_visible;
        let show_chrome = !is_immersive || hud_visible;
        // Edit mode swaps the top bar for the edit bar and hides what could
        // step to, or act on, another photo (nav buttons, info panel).
        let editing = this.edits.session.is_some();
        // A slideshow keeps only Back: no step, mark, zoom, play or PiP
        // control acts behind its timer (its keys are Space, ←/→ and Esc).
        let presenting = this.slideshow.is_some();
        let show_panel = info_panel_shown(this);

        let backdrop = div()
            .id("viewer-backdrop")
            .size_full()
            .bg(if is_immersive {
                theme::bg_media()
            } else {
                theme::bg_stage()
            })
            .flex()
            .flex_row()
            .overflow_hidden();

        // Stage column: top bar, then the media with its controls.
        let stage = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
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
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    // The media frame and its prev/next buttons share a box,
                    // so the buttons centre on the picture.
                    .child(
                        div()
                            .relative()
                            // A framed video takes its own size, centred
                            // above the transport; photos fill the stage.
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
                            .when(show_chrome && !editing && !presenting, |frame| {
                                frame.children(stage_controls::render_nav_buttons(this, cx))
                            }),
                    )
                    .when(show_chrome && !is_video && !presenting, |stage| {
                        stage.children(stage_controls::render_zoom_controls(this, cx))
                    })
                    // While PiP owns the player the viewer shows the poster
                    // and a hand-off pill instead of the transport.
                    .when(
                        is_video && show_chrome && !presenting && this.video.in_pip,
                        |stage| stage.child(stage_controls::render_pip_handoff(this, item, cx)),
                    ),
            )
            // 3. The transport, docked across the stage under the frame.
            .child(
                if presenting || !is_video || !show_chrome || this.video.in_pip {
                    Empty.into_any_element()
                } else if is_immersive {
                    crate::motion::fade_in(
                        div().w_full().child(ViewerTransport::render(this, cx)),
                        "viewer-hud-transport-in",
                        crate::motion::HUD_IN,
                    )
                    .into_any_element()
                } else {
                    ViewerTransport::render(this, cx).into_any_element()
                },
            )
            // 4. The filmstrip under the media.
            .children(filmstrip::render(this, cx));

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
            .child(stage)
            // 5. Info panel: a full-height column beside the stage.
            .child(if show_panel {
                div()
                    .id("viewer-info-drawer")
                    .debug_selector(|| "viewer-info-drawer".to_string())
                    // Keep clicks inside the panel from reaching the media
                    // canvas or the grid behind.
                    .occlude()
                    .flex_none()
                    .w(px(INFO_PANEL_WIDTH))
                    .when(!is_immersive, |panel| panel.h_full())
                    .role(Role::Complementary)
                    .aria_label(t("viewer.info_panel"))
                    // Immersive: float over the stage below the top bar, so
                    // its Favourite and Info buttons stay clickable.
                    .when(is_immersive, |panel| {
                        panel
                            .absolute()
                            .top(px(top_bar::TOP_BAR_HEIGHT))
                            .bottom_0()
                            .right_0()
                    })
                    .bg(theme::bg_1())
                    .border_l_1()
                    .border_color(theme::line())
                    .overflow_hidden()
                    .child(InspectorView::render(this, cx))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            // The star popover's dismiss backdrop; the card hangs from the
            // star button and paints above it (deferred).
            .when(this.rate_popover_shown(), |viewer| {
                viewer.child(
                    div()
                        .id("viewer-rate-backdrop")
                        .debug_selector(|| "viewer-rate-backdrop".to_string())
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                                this.close_rate_popover(cx);
                            }),
                        ),
                )
            })
            .into_any_element()
    }
}
