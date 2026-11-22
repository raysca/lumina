use crate::app::types::format_clock;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::body::display_title;
use crate::views::viewer::transport::{drag_tracker, scrub_fraction};
use crate::{LuminaApp, UiMessage};
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

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

#[cfg(test)]
thread_local! {
    /// Window moves a test started; the test platform cannot move windows.
    pub(crate) static TEST_WINDOW_MOVES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

/// Drags the PiP window: a press anywhere on the video that no control
/// claims moves it.
fn start_drag(window: &Window) {
    #[cfg(test)]
    {
        let _ = window;
        TEST_WINDOW_MOVES.with(|moves| moves.set(moves.get() + 1));
    }
    #[cfg(not(test))]
    window.start_window_move();
}

/// The player keys while the PiP window is key (the viewer's bindings):
/// Space/K play, J/← back 5 s, → forward 5 s, ⌥←/⌥→ chapter or 5 s,
/// L loop, M mute; Escape closes the PiP (its window has no window buttons);
/// ↵ retries a clip that failed to load.
/// Returns whether the key was handled.
fn handle_pip_key(app: &mut LuminaApp, keystroke: &Keystroke, cx: &mut Context<LuminaApp>) -> bool {
    let m = keystroke.modifiers;
    if m.platform || m.control || m.shift {
        return false;
    }
    let key = keystroke.key.to_ascii_lowercase();
    match key.as_str() {
        "left" | "right" if m.alt => app.jump_chapter(key == "right", cx),
        _ if m.alt => return false,
        "space" | "k" => app.toggle_video_play(cx),
        "j" | "left" => app.seek_video_relative(-5.0, cx),
        "right" => app.seek_video_relative(5.0, cx),
        "l" => app.toggle_video_loop(cx),
        "m" => app.toggle_video_mute(cx),
        "escape" => app.close_pip(cx),
        // The failure message's Try Again.
        "enter" if app.video.load_error.is_some() => app.retry_video_load(cx),
        _ => return false,
    }
    true
}

pub struct FloatingPipPlayerView {
    pub app: WeakEntity<LuminaApp>,
    /// This window's id, sent on close so the app can ignore a stale window.
    pub window_id: WindowId,
    pub ui_tx: async_channel::Sender<UiMessage>,
    /// Holds focus so the player keys reach this window when it is key.
    focus_handle: FocusHandle,
    pub _subscription: Option<Subscription>,
}

/// 24px icon-only control over the video. Role, label and tooltip carry
/// the same text; the press never starts a window drag.
fn pip_control(
    id: &'static str,
    icon: Icon,
    icon_size: f32,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(label))
        .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
        .size(px(24.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(render_icon(icon, px(icon_size), theme::on_media()))
}

/// Round transport control (skip, play, mute).
fn pip_button(id: &'static str, icon: Icon, icon_size: f32, label: String) -> Stateful<Div> {
    pip_control(id, icon, icon_size, label)
        .rounded_full()
        .hover(|s| s.bg(theme::media_scrim()))
}

/// Square top-right control on a scrim (Expand, Close).
fn pip_chrome_button(id: &'static str, icon: Icon, label: String) -> Stateful<Div> {
    pip_control(id, icon, 12.0, label)
        .rounded(theme::RADIUS_MD)
        .bg(theme::media_scrim())
        .hover(|s| s.bg(theme::media_scrim().alpha(0.85)))
}

impl FloatingPipPlayerView {
    pub fn new(
        app: WeakEntity<LuminaApp>,
        ui_tx: async_channel::Sender<UiMessage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let _subscription = app.upgrade().map(|app_entity| {
            cx.observe(&app_entity, |_this, _entity, cx| {
                cx.notify();
            })
        });
        let focus_handle = cx.focus_handle();
        // Focus within this window only: the window opens without taking key.
        window.focus(&focus_handle, cx);
        Self {
            app,
            window_id: window.window_handle().window_id(),
            ui_tx,
            focus_handle,
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
                    .text_size(theme::TEXT_BODY)
                    .text_color(theme::t2())
                    .child(t("pip.main_window_closed"))
                    .into_any_element();
            }
        };

        // This window paints `video.pip_frame`; evict retired frames here
        // so playback cannot pile them up while the main window is hidden.
        let retired = app.update(cx, |app, _| std::mem::take(&mut app.retired_images));
        crate::app::viewer_actions::evict_retired_images(retired, window, cx);

        let app_read = app.read(cx);
        let video = &app_read.video;
        let preview_image = video.pip_frame.clone();
        let has_player = video.player.is_some();
        // A failed load says so instead of showing a stale or empty frame.
        let failed = video.load_error.is_some();
        let gone = video.load_file_gone;
        // A portrait PiP is too narrow for the line beside the button.
        let narrow = window.viewport_size().width < px(260.0);
        let title = video.item.as_ref().map(display_title).unwrap_or_default();
        let is_playing = video.is_playing;
        let is_muted = video.is_muted;
        let is_scrubbing = video.is_scrubbing;
        let has_chapters = !video.tracks.chapters.is_empty();
        // While scrubbing, the destination owns the playhead and readout.
        let current_time = video.scrub_time.unwrap_or(video.playback_time);
        let duration = video.duration;
        let progress = scrub_progress_fraction(current_time, duration);

        let on_app = |f: fn(&mut LuminaApp, &mut Context<LuminaApp>)| {
            let app = app.downgrade();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                if let Some(app) = app.upgrade() {
                    app.update(cx, f);
                }
            }
        };
        let app_expand = self.app.clone();
        let app_close = self.app.clone();
        let app_keys = self.app.clone();

        // The track surface is the first child, so its prepaint bounds map
        // pointer X to a 0..=1 fraction (as in the viewer transport).
        let track_bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));
        let scrub_track = div()
            .on_children_prepainted({
                let track_bounds = track_bounds.clone();
                move |child_bounds: Vec<Bounds<Pixels>>, _window, _cx| {
                    if let Some(bounds) = child_bounds.first() {
                        *track_bounds.borrow_mut() = Some(*bounds);
                    }
                }
            })
            .id("pip-scrub-track")
            .debug_selector(|| "pip-scrub-track".to_string())
            .role(Role::Slider)
            .aria_label(t("pip.seek"))
            .flex_1()
            .min_w_0()
            .h(px(14.0))
            .relative()
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, {
                let (app, track_bounds) = (app.downgrade(), track_bounds.clone());
                move |event: &MouseDownEvent, _w, cx| {
                    cx.stop_propagation();
                    if let (Some(app), Some(pct)) =
                        (app.upgrade(), scrub_fraction(event.position, &track_bounds))
                    {
                        app.update(cx, |app, cx| app.begin_video_scrub(pct, cx));
                    }
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let app = app.downgrade();
                move |_: &MouseUpEvent, _w, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |app, cx| app.end_video_scrub(cx));
                    }
                }
            })
            // A release can land off the track mid-drag.
            .on_mouse_up_out(MouseButton::Left, {
                let app = app.downgrade();
                move |_: &MouseUpEvent, _w, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |app, cx| app.end_video_scrub(cx));
                    }
                }
            })
            .child(div().absolute().inset_0())
            // 4px rail, played portion in accent.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(5.0))
                    .h(px(4.0))
                    .rounded_full()
                    .bg(theme::on_media().opacity(0.2))
                    .child(
                        div()
                            .h_full()
                            .w(relative(progress))
                            .rounded_full()
                            .bg(theme::accent()),
                    ),
            )
            // 10px knob centred on the playhead.
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(relative(progress))
                    .ml(px(-5.0))
                    .size(px(10.0))
                    .rounded_full()
                    .bg(theme::on_media()),
            )
            .child(drag_tracker(is_scrubbing, app.clone(), {
                let track_bounds = track_bounds.clone();
                move |this, position, cx| {
                    if let Some(pct) = scrub_fraction(position, &track_bounds) {
                        this.update_video_scrub(pct, cx);
                    }
                }
            }));

        let mono = |text: String, alpha: f32| {
            div()
                .flex_none()
                .text_size(theme::TEXT_CAPTION)
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_color(theme::on_media().opacity(alpha))
                .child(text)
        };

        div()
            .id("floating-pip-container")
            .track_focus(&self.focus_handle)
            .on_key_down(move |event: &KeyDownEvent, _w, cx| {
                let Some(app) = app_keys.upgrade() else {
                    return;
                };
                if app.update(cx, |app, cx| handle_pip_key(app, &event.keystroke, cx)) {
                    cx.stop_propagation();
                }
            })
            .size_full()
            .bg(theme::bg_media())
            // The design card: radius 10 and a line-2 outline. The window's
            // own shadow and corner clip come from macOS.
            .rounded(px(10.0))
            .border_1()
            .border_color(theme::line_2())
            .relative()
            .overflow_hidden()
            // Video surface; drags the window from anywhere.
            .child(
                div()
                    .size_full()
                    .on_mouse_down(MouseButton::Left, |_e, window, _cx| {
                        start_drag(window);
                    })
                    .child(if failed {
                        // The failure message sits over the fades, below.
                        div().into_any_element()
                    } else if let Some(img_render) = preview_image {
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
                            .text_size(theme::TEXT_SMALL)
                            .text_color(theme::t2())
                            .child(t(if has_player {
                                "pip.loading"
                            } else {
                                "pip.unplayable"
                            }))
                            .into_any_element()
                    }),
            )
            // 72px top fade: badge left, Expand and Close right.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(72.0))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(theme::media_scrim(), 0.0),
                        linear_color_stop(transparent_black(), 1.0),
                    ))
                    .on_mouse_down(MouseButton::Left, |_e, window, cx| {
                        start_drag(window);
                        cx.stop_propagation();
                    })
                    .flex()
                    .flex_row()
                    .items_start()
                    // Frame: badge 12 px from the card's outer edge; this fade
                    // starts inside the 1 px outline. The window buttons are
                    // hidden (`set_up_pip_window`).
                    .pt(px(11.0))
                    .px(px(11.0))
                    .gap(px(6.0))
                    .child(
                        // ponytail: GPUI has no letter-spacing; the design's
                        // 0.5 tracking is skipped.
                        div()
                            .debug_selector(|| "pip-badge".to_string())
                            .h(px(20.0))
                            .px(px(6.0))
                            .rounded(theme::RADIUS_SM)
                            .bg(theme::media_scrim())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(render_icon(Icon::Pip, theme::ICON_XS, theme::accent()))
                            .child(
                                div()
                                    .text_size(theme::TEXT_MICRO)
                                    .font_family(theme::FONT_SANS)
                                    .font_features(theme::tabular_figures())
                                    .text_color(theme::accent())
                                    .child(t("pip.badge")),
                            ),
                    )
                    .child(div().flex_1())
                    .child(
                        pip_chrome_button("pip-expand", Icon::Maximize, t("pip.expand")).on_click(
                            cx.listener(move |_this, _e: &ClickEvent, window, cx| {
                                if let Some(app_entity) = app_expand.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.expand_pip_to_viewer(cx);
                                    });
                                }
                                window.remove_window();
                            }),
                        ),
                    )
                    .child(
                        pip_chrome_button("pip-close", Icon::X, t("pip.close")).on_click(
                            cx.listener(move |_this, _e: &ClickEvent, window, cx| {
                                if let Some(app_entity) = app_close.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.close_pip(cx);
                                    });
                                }
                                window.remove_window();
                            }),
                        ),
                    ),
            )
            // 116px bottom fade: title over the controls row.
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(116.0))
                    .bg(linear_gradient(
                        0.0,
                        linear_color_stop(theme::media_scrim(), 0.0),
                        linear_color_stop(transparent_black(), 1.0),
                    ))
                    // The fade drags like the video; only the controls claim the press.
                    .on_mouse_down(MouseButton::Left, |_e, window, cx| {
                        start_drag(window);
                        cx.stop_propagation();
                    })
                    .flex()
                    .flex_col()
                    .justify_end()
                    .gap(px(8.0))
                    .px(px(14.0))
                    .pb(px(12.0))
                    .child(
                        div()
                            .debug_selector(|| "pip-title".to_string())
                            .truncate()
                            .text_size(theme::TEXT_BODY)
                            .font_family(theme::FONT_SANS)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::on_media())
                            .child(title),
                    )
                    .child(
                        div()
                            .debug_selector(|| "pip-controls".to_string())
                            .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                pip_button(
                                    "pip-skip-back",
                                    Icon::StepBack,
                                    13.0,
                                    t(if has_chapters {
                                        "pip.previous_chapter"
                                    } else {
                                        "pip.back"
                                    }),
                                )
                                .on_click(on_app(|app, cx| app.jump_chapter(false, cx))),
                            )
                            .child(
                                pip_button(
                                    "pip-play-pause",
                                    if is_playing { Icon::Pause } else { Icon::Play },
                                    16.0,
                                    t(if is_playing { "pip.pause" } else { "pip.play" }),
                                )
                                .on_click(on_app(LuminaApp::toggle_video_play)),
                            )
                            .child(
                                pip_button(
                                    "pip-skip-forward",
                                    Icon::StepForward,
                                    13.0,
                                    t(if has_chapters {
                                        "pip.next_chapter"
                                    } else {
                                        "pip.forward"
                                    }),
                                )
                                .on_click(on_app(|app, cx| app.jump_chapter(true, cx))),
                            )
                            .child(mono(format_clock(current_time, false), 1.0))
                            .child(scrub_track)
                            .child(mono(format_clock(duration, false), 0.7))
                            .child(
                                pip_button(
                                    "pip-mute",
                                    if is_muted {
                                        Icon::VolumeX
                                    } else {
                                        Icon::Volume
                                    },
                                    13.0,
                                    t(if is_muted { "pip.unmute" } else { "pip.mute" }),
                                )
                                .on_click(on_app(LuminaApp::toggle_video_mute)),
                            ),
                    ),
            )
            .children(
                failed.then(|| {
                    crate::views::viewer::pip_playback_failed(self.app.clone(), narrow, gone)
                }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: a glob import of gpui would bring gpui's `test`
    // proc-macro attribute into scope and shadow the builtin test harness.
    use super::scrub_progress_fraction;
    use crate::app::types::format_clock;

    #[test]
    fn scrub_progress_fraction_clamps_and_handles_zero_duration() {
        assert_eq!(scrub_progress_fraction(0.0, 0.0), 0.0);
        assert!((scrub_progress_fraction(15.0, 60.0) - 0.25).abs() < 1e-6);
        assert_eq!(scrub_progress_fraction(90.0, 60.0), 1.0);
        assert_eq!(scrub_progress_fraction(-5.0, 60.0), 0.0);
    }

    #[test]
    fn clock_reads_minutes_seconds_and_hours() {
        assert_eq!(format_clock(0.0, false), "0:00");
        assert_eq!(format_clock(78.9, false), "1:18");
        assert_eq!(format_clock(-3.0, false), "0:00");
        // The old local helper dropped the hours branch: a 90-minute clip
        // read "90:00" instead of "1:30:00".
        assert_eq!(format_clock(5400.0, false), "1:30:00");
    }
}
