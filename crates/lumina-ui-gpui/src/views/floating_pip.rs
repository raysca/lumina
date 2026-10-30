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
    pub is_hovered: bool,
    pub ui_tx: async_channel::Sender<UiMessage>,
    pub _subscription: Option<Subscription>,
}

impl FloatingPipPlayerView {
    pub fn new(
        app: WeakEntity<LuminaApp>,
        ui_tx: async_channel::Sender<UiMessage>,
        cx: &mut Context<Self>,
    ) -> Self {
        let _subscription = app.upgrade().map(|app_entity| {
            cx.observe(&app_entity, |_this, _entity, cx| {
                cx.notify();
            })
        });
        Self {
            app,
            is_hovered: false,
            ui_tx,
            _subscription,
        }
    }
}

impl Drop for FloatingPipPlayerView {
    fn drop(&mut self) {
        let _ = self.ui_tx.send_blocking(UiMessage::FloatingPipClosed);
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

        // This window paints `viewer_image` too; evict retired frames here
        // so playback cannot pile them up while the main window is hidden.
        let retired = app.update(cx, |app, _| std::mem::take(&mut app.retired_viewer_images));
        crate::app::viewer_actions::evict_retired_viewer_images(retired, window, cx);

        let app_read = app.read(cx);
        let preview_image = app_read.viewer_image.clone();
        let is_playing = app_read.video.is_playing;
        let is_muted = app_read.video.is_muted;
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
        let app_toggle_mute = self.app.clone();
        let app_return_viewer = self.app.clone();
        let app_close = self.app.clone();
        let is_hovered = self.is_hovered;

        div()
            .id("floating-pip-container")
            .size_full()
            .bg(theme::bg_media())
            .relative()
            .overflow_hidden()
            .on_hover(cx.listener(|this, hovered: &bool, _w, cx| {
                if this.is_hovered != *hovered {
                    this.is_hovered = *hovered;
                    cx.notify();
                }
            }))
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
                            .child("Loading video...")
                            .into_any_element()
                    }),
            )
            // Hover Overlay: Top Control Bar (Window Drag, Return to Fullscreen, Close)
            .child(if is_hovered {
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .p_2()
                    .bg(theme::scrim())
                    .border_b_1()
                    .border_color(theme::border_subtle())
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::accent())
                                    .child(render_icon(Icon::Pip, px(12.0), theme::accent())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text())
                                    .child("PICTURE-IN-PICTURE"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            // Return to Main Viewer Button
                            .child(
                                div()
                                    .id("pip-return-to-viewer-btn")
                                    .tooltip(theme::tooltip_label("Expand to main viewer"))
                                    .px_2()
                                    .py_0p5()
                                    .bg(theme::bg_control())
                                    .hover(|s| s.bg(theme::accent()).text_color(theme::accent_fg()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(render_icon(Icon::ExternalLink, px(12.0), theme::text()))
                                    .child("Expand")
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
                            // Close PiP Button
                            .child(
                                div()
                                    .id("pip-close-btn")
                                    .tooltip(theme::tooltip_label("Close PiP"))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded_md()
                                    .hover(|s| s.bg(theme::danger()))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child(render_icon(Icon::X, px(11.0), theme::text()))
                                    .on_click(cx.listener(
                                        move |_this, _e: &ClickEvent, window, cx| {
                                            if let Some(app_entity) = app_close.upgrade() {
                                                app_entity.update(cx, |app, cx| {
                                                    app.floating_pip_closed(cx);
                                                });
                                            }
                                            window.remove_window();
                                        },
                                    )),
                            ),
                    )
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            // Hover Overlay: Bottom Playback Controls
            .child(if is_hovered {
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .p_2()
                    .bg(theme::scrim())
                    .border_t_1()
                    .border_color(theme::border_subtle())
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    // Mini scrubber track (interactive click-to-seek + relative width)
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
                            .w_full()
                            .h(px(6.0))
                            .bg(theme::bg_control_hover())
                            .rounded_full()
                            .cursor_pointer()
                            .relative()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(time_progress as f32))
                                    .bg(theme::accent())
                                    .rounded_full(),
                            )
                            .child(hit_row)
                    })
                    // Bottom actions row: Play/Pause, Time, Mute
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    // Play / Pause Button
                                    .child(
                                        div()
                                            .id("pip-play-pause-btn")
                                            .px_2()
                                            .py_0p5()
                                            .bg(if is_playing {
                                                theme::accent()
                                            } else {
                                                theme::bg_control()
                                            })
                                            .hover(|s| {
                                                s.bg(theme::accent()).text_color(theme::accent_fg())
                                            })
                                            .rounded_md()
                                            .cursor_pointer()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::text())
                                            .child(render_icon(
                                                if is_playing { Icon::Pause } else { Icon::Play },
                                                px(11.0),
                                                theme::text(),
                                            ))
                                            .child(if is_playing { "Pause" } else { "Play" })
                                            .on_click(cx.listener(
                                                move |_this, _e: &ClickEvent, _w, cx| {
                                                    if let Some(app_entity) =
                                                        app_toggle_play.upgrade()
                                                    {
                                                        app_entity.update(cx, |app, cx| {
                                                            app.toggle_video_play(cx);
                                                        });
                                                    }
                                                },
                                            )),
                                    )
                                    // Time Indicator
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child(time_display),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    // Mute Button
                                    .child(
                                        div()
                                            .id("pip-mute-btn")
                                            .px_2()
                                            .py_0p5()
                                            .bg(if is_muted {
                                                theme::danger_bg()
                                            } else {
                                                theme::bg_control()
                                            })
                                            .hover(|s| s.bg(theme::bg_control_hover()))
                                            .rounded_md()
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::text())
                                            .child(render_icon(
                                                if is_muted {
                                                    Icon::VolumeX
                                                } else {
                                                    Icon::Volume
                                                },
                                                px(12.0),
                                                theme::text(),
                                            ))
                                            .on_click(cx.listener(
                                                move |_this, _e: &ClickEvent, _w, cx| {
                                                    if let Some(app_entity) =
                                                        app_toggle_mute.upgrade()
                                                    {
                                                        app_entity.update(cx, |app, cx| {
                                                            app.toggle_video_mute(cx);
                                                        });
                                                    }
                                                },
                                            )),
                                    ),
                            ),
                    )
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
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
