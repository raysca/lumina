use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::{LuminaApp, UiMessage};
use gpui::*;

/// Number of clickable segments in the PiP scrubber hit row.
const PIP_SCRUB_SEGMENTS: usize = 16;

/// Fraction (0..=1) of the scrub track filled by the playback position.
///
/// The fill width must always be applied with `relative(fraction)` so it scales
/// with the PiP window; a hardcoded pixel width was the historical regression.
fn scrub_progress_fraction(current_time: f64, duration: f64) -> f32 {
    if duration > 0.0 {
        (current_time / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    }
}

/// Centre percentage (0..1) of segment `index` in a click-to-seek row.
fn scrub_segment_percent(index: usize, segments: usize) -> f64 {
    debug_assert!(segments > 0);
    (index as f64 + 0.5) / segments as f64
}

pub struct FloatingPipPlayerView {
    pub app: WeakEntity<LuminaApp>,
    /// This window's id, sent on close so the app can ignore a stale window.
    pub window_id: WindowId,
    pub ui_tx: async_channel::Sender<UiMessage>,
    pub _subscription: Option<Subscription>,
}

/// 20px icon-only PiP control; the tooltip names it.
fn pip_icon_button(id: &'static str, icon: Icon, tooltip: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .tooltip(theme::tooltip_label(tooltip))
        .size(px(20.0))
        .rounded(theme::RADIUS_MD)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(theme::bg_control_hover()))
        .child(render_icon(icon, px(12.0), theme::text()))
}

impl FloatingPipPlayerView {
    pub fn new(
        app: WeakEntity<LuminaApp>,
        ui_tx: async_channel::Sender<UiMessage>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _subscription = app.upgrade().map(|app_entity| {
            cx.observe(&app_entity, |_this, _entity, cx| {
                cx.notify();
            })
        });
        Self {
            app,
            window_id: window.window_handle().window_id(),
            ui_tx,
            _subscription,
        }
    }
}

impl Drop for FloatingPipPlayerView {
    fn drop(&mut self) {
        let _ = self
            .ui_tx
            .send_blocking(UiMessage::FloatingPipClosed(self.window_id));
    }
}

impl Render for FloatingPipPlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = match self.app.upgrade() {
            Some(app) => app,
            None => {
                return div()
                    .size_full()
                    .bg(theme::bg_media())
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_sm()
                    .text_color(theme::text_subtle())
                    .child("Main library window closed")
                    .into_any_element();
            }
        };

        // This window paints `video.pip_frame`; evict retired frames here
        // so playback cannot pile them up while the main window is hidden.
        let retired = app.update(cx, |app, _| std::mem::take(&mut app.retired_images));
        crate::app::viewer_actions::evict_retired_images(retired, window, cx);

        let app_read = app.read(cx);
        let preview_image = app_read.video.pip_frame.clone();
        let has_player = app_read.video.player.is_some();
        let title = app_read
            .video
            .item
            .as_ref()
            .map(|item| item.file_name.clone())
            .unwrap_or_default();
        let is_playing = app_read.video.is_playing;
        let current_time = app_read.video.playback_time;
        let duration = app_read.video.duration;
        let time_progress = scrub_progress_fraction(current_time, duration);

        let time_display = format!(
            "{}:{:02} / {}:{:02}",
            (current_time.max(0.0) as u64) / 60,
            (current_time.max(0.0) as u64) % 60,
            (duration.max(0.0) as u64) / 60,
            (duration.max(0.0) as u64) % 60
        );

        let app_toggle_play = self.app.clone();
        let app_return_viewer = self.app.clone();
        let app_close = self.app.clone();

        div()
            .id("floating-pip-container")
            .size_full()
            .bg(theme::bg_media())
            .relative()
            .overflow_hidden()
            // Video Frame Surface (Draggable by window)
            .child(
                div()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, |_e, window, _cx| {
                        window.start_window_move();
                    })
                    .child(if let Some(img_render) = preview_image {
                        div()
                            .size_full()
                            .child(img(img_render).size_full().object_fit(ObjectFit::Contain))
                            .into_any_element()
                    } else {
                        div()
                            .size_full()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_xs()
                            .text_color(theme::text_subtle())
                            .child(if has_player {
                                "Loading video…"
                            } else {
                                "This video can't be played"
                            })
                            .into_any_element()
                    }),
            )
            // Title bar (always visible): mono filename, Expand, Close.
            .child(
                div()
                    .absolute()
                    // Presses on the controls must not reach the video surface,
                    // which starts a window drag on mouse-down.
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(26.0))
                    .px_2()
                    .bg(theme::scrim())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.0))
                            .font_family(theme::FONT_MONO)
                            .text_color(theme::text_muted())
                            .child(title),
                    )
                    .child(
                        pip_icon_button(
                            "pip-return-to-viewer-btn",
                            Icon::Maximize,
                            "Expand to viewer",
                        )
                        .debug_selector(|| "pip-expand".to_string())
                        .on_click(cx.listener(
                            move |_this, _e: &ClickEvent, window, cx| {
                                if let Some(app_entity) = app_return_viewer.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.expand_pip_to_viewer(cx);
                                    });
                                }
                                window.remove_window();
                            },
                        )),
                    )
                    .child(
                        pip_icon_button("pip-close-btn", Icon::X, "Close")
                            .debug_selector(|| "pip-close".to_string())
                            .on_click(cx.listener(move |_this, _e: &ClickEvent, window, cx| {
                                if let Some(app_entity) = app_close.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.close_pip(cx);
                                    });
                                }
                                window.remove_window();
                            })),
                    ),
            )
            // Control pill (always visible), inset 8px: play, track, time.
            .child(
                div()
                    .absolute()
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
                    .bottom(px(8.0))
                    .left(px(8.0))
                    .right(px(8.0))
                    .h(px(28.0))
                    .px_1()
                    .rounded_full()
                    .bg(theme::scrim())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        pip_icon_button(
                            "pip-play-pause-btn",
                            if is_playing { Icon::Pause } else { Icon::Play },
                            if is_playing {
                                "Pause (Space)"
                            } else {
                                "Play (Space)"
                            },
                        )
                        .debug_selector(|| "pip-play-pause".to_string())
                        .on_click(cx.listener(
                            move |_this, _e: &ClickEvent, _w, cx| {
                                if let Some(app_entity) = app_toggle_play.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.toggle_video_play(cx);
                                    });
                                }
                            },
                        )),
                    )
                    // 3px track with a click-to-seek hit row over it.
                    .child({
                        let mut hit_row = div().absolute().inset_0().flex().flex_row();
                        for i in 0..PIP_SCRUB_SEGMENTS {
                            let pct = scrub_segment_percent(i, PIP_SCRUB_SEGMENTS);
                            let app_seek = self.app.clone();
                            hit_row = hit_row.child(
                                div()
                                    .id(ElementId::NamedInteger("pip-scrub-seg".into(), i as u64))
                                    .flex_1()
                                    .h_full()
                                    .on_click(cx.listener(
                                        move |_this, _e: &ClickEvent, _w, cx| {
                                            if let Some(app_entity) = app_seek.upgrade() {
                                                app_entity.update(cx, |app, cx| {
                                                    app.seek_video_percent(pct, cx);
                                                });
                                            }
                                        },
                                    )),
                            );
                        }
                        div()
                            .id("pip-scrub-track")
                            .debug_selector(|| "pip-scrub-track".to_string())
                            .flex_1()
                            .h(px(14.0))
                            .relative()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(3.0))
                                    .rounded_full()
                                    .bg(theme::bg_control_hover())
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(time_progress))
                                            .rounded_full()
                                            .bg(theme::accent()),
                                    ),
                            )
                            .child(hit_row)
                    })
                    .child(
                        div()
                            .pr_2()
                            .text_size(px(11.0))
                            .font_family(theme::FONT_MONO)
                            .text_color(theme::text_muted())
                            .child(time_display),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: a glob import of gpui would bring gpui's `test`
    // proc-macro attribute into scope and shadow the builtin test harness.
    use super::{scrub_progress_fraction, scrub_segment_percent, PIP_SCRUB_SEGMENTS};

    #[test]
    fn scrub_progress_fraction_clamps_and_handles_zero_duration() {
        assert_eq!(scrub_progress_fraction(0.0, 0.0), 0.0);
        assert!((scrub_progress_fraction(15.0, 60.0) - 0.25).abs() < 1e-6);
        assert_eq!(scrub_progress_fraction(90.0, 60.0), 1.0);
        assert_eq!(scrub_progress_fraction(-5.0, 60.0), 0.0);
    }

    #[test]
    fn scrub_segment_percent_returns_segment_centres() {
        assert!((scrub_segment_percent(0, 16) - 0.03125).abs() < 1e-9);
        assert!((scrub_segment_percent(7, 16) - 0.46875).abs() < 1e-9);
        assert!((scrub_segment_percent(15, 16) - 0.96875).abs() < 1e-9);
        for i in 0..PIP_SCRUB_SEGMENTS {
            let p = scrub_segment_percent(i, PIP_SCRUB_SEGMENTS);
            assert!(p > 0.0 && p < 1.0);
        }
    }
}
