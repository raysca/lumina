use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::utils::format_playback_speed;
use crate::app::types::format_time_fractional;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// Convert a window-space pointer position into a 0..=1 fraction of the scrub
/// track. Bounds are captured during prepaint, so drag mapping stays correct
/// across resizes and layout changes without hard-coded geometry.
fn scrub_fraction(
    position: Point<Pixels>,
    track_bounds: &Rc<RefCell<Option<Bounds<Pixels>>>>,
) -> Option<f64> {
    let bounds = (*track_bounds.borrow())?;
    let width: f32 = bounds.size.width.into();
    if width <= 0.0 {
        return None;
    }
    let left: f32 = bounds.origin.x.into();
    let pointer_x: f32 = position.x.into();
    Some(f64::from(((pointer_x - left) / width).clamp(0.0, 1.0)))
}

/// While `active`, forwards every pressed-button mouse move in the window to
/// `on_move`. `on_mouse_move` only fires while its element is hovered, so a
/// drag on a thin track would freeze as soon as the pointer drifted off it.
fn drag_tracker(
    active: bool,
    app: Entity<LuminaApp>,
    on_move: impl Fn(&mut LuminaApp, Point<Pixels>, &mut Context<LuminaApp>) + 'static,
) -> impl IntoElement {
    let on_move = Rc::new(on_move);
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            if !active {
                return;
            }
            let (app, on_move) = (app.clone(), on_move.clone());
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
                if phase == DispatchPhase::Bubble && event.pressed_button == Some(MouseButton::Left)
                {
                    app.update(cx, |this, cx| on_move(this, event.position, cx));
                }
            });
        },
    )
    .absolute()
    .size_0()
}

/// 32px transport hit area with a 16px icon; the tooltip names the key.
fn transport_button(
    id: &'static str,
    icon: Icon,
    tooltip: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(tooltip))
        .size(px(32.0))
        .rounded(theme::RADIUS_MD)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(on, |b| b.bg(theme::selection_bg()))
        .hover(|s| s.bg(theme::bg_control_hover()))
        .child(render_icon(
            icon,
            px(16.0),
            if on { theme::accent() } else { theme::text() },
        ))
}

/// Narrowest transport card that still fits its three control zones.
const TRANSPORT_MIN_WIDTH: f32 = 560.0;

pub struct ViewerTransport;

impl ViewerTransport {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let duration = if this.video.duration > 0.0 {
            this.video.duration
        } else {
            this.selected_item
                .as_ref()
                .map(|i| i.duration_secs())
                .unwrap_or(0.0)
        };
        let is_scrubbing = this.video.is_scrubbing;
        let scrub_time = this.video.scrub_time;
        // While scrubbing, the destination owns the playhead and time readout so
        // feedback follows the pointer even when seeks are coalesced.
        let current_time = if is_scrubbing {
            scrub_time.unwrap_or(this.video.playback_time)
        } else {
            this.video.playback_time
        };
        let progress_pct = if duration > 0.0 {
            (current_time / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Hover markers yield to the scrub destination while dragging.
        let hover_pct = if is_scrubbing {
            None
        } else {
            this.video.hover_pct
        };
        let hover_time = if is_scrubbing {
            None
        } else {
            this.video.hover_time
        };
        // Honest scrub feedback: the native bridge cannot produce a decoded
        // still for an arbitrary scrub position, so the transport shows a
        // time-only badge next to the playhead instead of a synthetic image.
        let time_badge = if is_scrubbing {
            scrub_time.map(|time| (progress_pct as f32, time))
        } else {
            hover_pct.zip(hover_time)
        };

        let time_label = |t: f64| {
            div()
                .w(px(56.0))
                .flex_shrink_0()
                .font_family(theme::FONT_MONO)
                .text_size(px(11.0))
                .text_color(theme::text_muted())
                .child(format_time_fractional(t))
        };

        div()
            .id("video-transport")
            .debug_selector(|| "video-transport".to_string())
            .on_hover(cx.listener(|this, on: &bool, _w, _cx| this.set_pointer_on_hud(*on)))
            .w_full()
            // As wide as the video frame, but never too narrow for the three
            // control zones (portrait and tiny frames).
            .max_w(px(super::viewport::video_frame_size(this)
                .map_or(super::viewport::VIDEO_FRAME_MAX_WIDTH, |(w, _)| {
                    w.max(TRANSPORT_MIN_WIDTH)
                })))
            .mt(px(12.0))
            .mb_4()
            .p_3()
            .bg(theme::bg_app())
            .border_1()
            .border_color(theme::border())
            .rounded(theme::RADIUS_LG)
            .relative()
            .flex()
            .flex_col()
            .gap_2()
            // Time-only scrub/hover feedback. No image is invented: this is
            // exactly the destination timestamp the transport is seeking to.
            .child(if let Some((badge_pct, badge_time)) = time_badge {
                div()
                    .id("video-time-badge")
                    .debug_selector(|| "video-time-badge".to_string())
                    .absolute()
                    .top(px(-26.0))
                    .left(relative((badge_pct - 0.06).clamp(0.01, 0.85)))
                    .px_2()
                    .py_0p5()
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(theme::RADIUS_SM)
                    .shadow_lg()
                    .text_xs()
                    .font_family(theme::FONT_MONO)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::text())
                    .child(format_time_fractional(badge_time))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            // Scrubber row: elapsed · track · total.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(time_label(current_time))
                    // Timeline scrubber track: press, drag, and release map
                    // directly to a timeline position. Pointer moves update the
                    // visible destination on every event while seeks are
                    // coalesced in LuminaApp::update_video_scrub. No thumbnail
                    // storyboard is fabricated.
                    .child({
                        // The full-size track surface is the first child, so its
                        // prepaint bounds are the track rect used to map pointer X to a
                        // 0..=1 timeline fraction.
                        let track_bounds: Rc<RefCell<Option<Bounds<Pixels>>>> =
                            Rc::new(RefCell::new(None));

                        div()
                            .on_children_prepainted({
                                let track_bounds = track_bounds.clone();
                                move |child_bounds: Vec<Bounds<Pixels>>, _window, _cx| {
                                    if let Some(bounds) = child_bounds.first() {
                                        *track_bounds.borrow_mut() = Some(*bounds);
                                    }
                                }
                            })
                            .id("video-scrubber-track")
                            .debug_selector(|| "video-scrubber".to_string())
                            .flex_1()
                            .h(px(24.0))
                            .rounded(theme::RADIUS_MD)
                            .border_1()
                            .border_color(if is_scrubbing {
                                theme::accent()
                            } else {
                                theme::border()
                            })
                            .relative()
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener({
                                    let track_bounds = track_bounds.clone();
                                    move |this, event: &MouseDownEvent, _window, cx| {
                                        if let Some(pct) =
                                            scrub_fraction(event.position, &track_bounds)
                                        {
                                            this.begin_video_scrub(pct, cx);
                                        }
                                    }
                                }),
                            )
                            .on_mouse_move(cx.listener({
                                let track_bounds = track_bounds.clone();
                                move |this, event: &MouseMoveEvent, _window, cx| {
                                    let Some(pct) = scrub_fraction(event.position, &track_bounds)
                                    else {
                                        return;
                                    };
                                    // An active scrub is driven by the drag tracker.
                                    if !this.video.is_scrubbing {
                                        this.set_video_hover_scrub(Some(pct as f32), cx);
                                    }
                                }
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                                    this.end_video_scrub(cx);
                                }),
                            )
                            // A release can land outside the track mid-drag; the
                            // capture-phase out-handler commits the final destination there.
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                                    this.end_video_scrub(cx);
                                }),
                            )
                            .on_hover(cx.listener(|this, hovered: &bool, _window, cx| {
                                if !*hovered && !this.video.is_scrubbing {
                                    this.set_video_hover_scrub(None, cx);
                                }
                            }))
                            .child(
                                div()
                                    .size_full()
                                    .rounded(theme::RADIUS_MD)
                                    .bg(theme::bg_control()),
                            )
                            // Played portion: a 22% amber tint up to the playhead.
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left_0()
                                    .debug_selector(|| "video-played".to_string())
                                    .w(relative(progress_pct as f32))
                                    .rounded_l_md()
                                    .bg(rgba(0xF0B54B38)),
                            )
                            // Hover needle indicator, hidden while a scrub owns the playhead.
                            .child(if let Some(hover_pct) = hover_pct {
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(relative(hover_pct))
                                    .w(px(1.5))
                                    .bg(theme::text())
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            })
                            .child(drag_tracker(is_scrubbing, cx.entity(), {
                                let track_bounds = track_bounds.clone();
                                move |this, position, cx| {
                                    if let Some(pct) = scrub_fraction(position, &track_bounds) {
                                        this.update_video_scrub(pct, cx);
                                    }
                                }
                            }))
                            // Current playhead: follows the scrub destination while dragging.
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left(relative(progress_pct as f32))
                                    .w(px(if is_scrubbing { 3.5 } else { 2.5 }))
                                    .bg(theme::accent())
                                    .shadow_md(),
                            )
                    })
                    .child(time_label(duration)),
            )
            // Dismiss layer for the speed menu, under the controls.
            .when(this.video.speed_menu_open, |card| {
                card.child(
                    div()
                        .id("video-speed-dismiss")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                                this.video.speed_menu_open = false;
                                cx.notify();
                            }),
                        ),
                )
            })
            // 3-zone controls: [mute · speed] | [−5s · ‹ · play · › · +5s] | [loop · PiP · full screen]
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .relative()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                transport_button(
                                    "video-mute",
                                    if this.video.is_muted {
                                        Icon::VolumeX
                                    } else {
                                        Icon::Volume
                                    },
                                    if this.video.is_muted {
                                        "Unmute (M)"
                                    } else {
                                        "Mute (M)"
                                    },
                                    this.video.is_muted,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_mute(cx);
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .id("video-speed-btn")
                                    .debug_selector(|| "video-speed".to_string())
                                    .tooltip(theme::tooltip_label("Playback speed"))
                                    .h(px(32.0))
                                    .px_2()
                                    .rounded(theme::RADIUS_MD)
                                    .flex()
                                    .items_center()
                                    .when(this.video.speed_menu_open, |b| {
                                        b.bg(theme::selection_bg())
                                    })
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_family(theme::FONT_MONO)
                                    .text_color(theme::text())
                                    .child(format_playback_speed(this.video.playback_speed))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_speed_menu(cx);
                                    })),
                            )
                            .when(this.video.speed_menu_open, |zone| {
                                zone.child(render_speed_menu(this, cx))
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            .child(
                                transport_button(
                                    "video-seek-back",
                                    Icon::RotateCcw,
                                    "Back 5 seconds (←)",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.seek_video_relative(-5.0, cx);
                                    },
                                )),
                            )
                            .child(
                                transport_button(
                                    "video-step-back",
                                    Icon::StepBack,
                                    "Previous frame (,)",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.step_video_frame(-1, cx);
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .id("video-play-btn")
                                    .debug_selector(|| "video-play".to_string())
                                    .tooltip(theme::tooltip_label(if this.video.is_playing {
                                        "Pause (Space)"
                                    } else {
                                        "Play (Space)"
                                    }))
                                    .mx_1()
                                    .size(px(40.0))
                                    .rounded_full()
                                    .bg(theme::text())
                                    .hover(|s| s.bg(theme::text_muted()))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .child(render_icon(
                                        if this.video.is_playing {
                                            Icon::Pause
                                        } else {
                                            Icon::Play
                                        },
                                        px(16.0),
                                        theme::bg_media(),
                                    ))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_play(cx);
                                    })),
                            )
                            .child(
                                transport_button(
                                    "video-step-fwd",
                                    Icon::StepForward,
                                    "Next frame (.)",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.step_video_frame(1, cx);
                                    },
                                )),
                            )
                            .child(
                                transport_button(
                                    "video-seek-fwd",
                                    Icon::RotateCw,
                                    "Forward 5 seconds (→)",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.seek_video_relative(5.0, cx);
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_end()
                            .gap_1()
                            .child(
                                transport_button(
                                    "video-loop",
                                    Icon::Repeat,
                                    "Loop (L)",
                                    this.video.is_looping,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_loop(cx);
                                    },
                                )),
                            )
                            .child(
                                transport_button(
                                    "video-pip",
                                    Icon::Pip,
                                    "Picture in Picture (⌥P)",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        if let Some(item) = this.selected_item.clone() {
                                            this.play_in_pip(item, cx);
                                        }
                                    },
                                )),
                            )
                            .child(
                                transport_button(
                                    "video-fullscreen",
                                    if this.is_immersive_fullscreen {
                                        Icon::Minimize
                                    } else {
                                        Icon::Maximize
                                    },
                                    "Full screen (⌃⌘F)",
                                    false,
                                )
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, window, cx| {
                                        this.toggle_immersive_fullscreen(Some(window), cx);
                                    },
                                )),
                            ),
                    ),
            )
    }
}

/// Speed menu, anchored above the speed button in the left zone.
fn render_speed_menu(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .id("video-speed-popover")
        .debug_selector(|| "video-speed-popover".to_string())
        .occlude()
        .absolute()
        .bottom(px(40.0))
        .left(px(32.0))
        .w(px(102.0))
        .bg(theme::bg_hud())
        .border_1()
        .border_color(theme::border_strong())
        .rounded(theme::RADIUS_LG)
        .shadow_2xl()
        .py_1()
        .flex()
        .flex_col()
        .children([0.25, 0.5, 1.0, 1.5, 2.0, 4.0].into_iter().map(|rate| {
            let is_cur = (this.video.playback_speed - rate).abs() < 0.05;
            div()
                .id(ElementId::NamedInteger(
                    "speed-opt".into(),
                    (rate * 100.0) as u64,
                ))
                .px_3()
                .py_1()
                .hover(|s| s.bg(theme::bg_control_hover()))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(if is_cur {
                    theme::accent()
                } else {
                    theme::text()
                })
                .child(if is_cur {
                    div().flex().items_center().child(render_icon(
                        Icon::Check,
                        px(12.0),
                        theme::accent(),
                    ))
                } else {
                    div().w(px(12.0))
                })
                .child(format_playback_speed(rate))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_video_speed(rate, cx);
                }))
        }))
}
