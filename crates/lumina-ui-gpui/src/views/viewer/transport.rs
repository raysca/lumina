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

/// Map a window-space pointer position onto the volume track's 0..=1 range.
fn volume_fraction(
    position: Point<Pixels>,
    track_bounds: &Rc<RefCell<Option<Bounds<Pixels>>>>,
) -> Option<f32> {
    let bounds = (*track_bounds.borrow())?;
    let width: f32 = bounds.size.width.into();
    if width <= 0.0 {
        return None;
    }
    let left: f32 = bounds.origin.x.into();
    let pointer_x: f32 = position.x.into();
    Some(((pointer_x - left) / width).clamp(0.0, 1.0))
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

        let vol = this.video.volume.clamp(0.0, 1.0);
        let volume_bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));

        div()
            .w_full()
            .max_w(px(880.0))
            .mb_4()
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
                    .bottom(px(84.0))
                    .left(relative((badge_pct - 0.06).clamp(0.01, 0.85)))
                    .px_2()
                    .py_0p5()
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded_sm()
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
            // Timeline scrubber track: press, drag, and release map directly to
            // a timeline position. Pointer moves update the visible destination
            // on every event while seeks are coalesced in
            // LuminaApp::update_video_scrub. The background is a plain track;
            // no thumbnail storyboard is fabricated.
            .child({
                // The full-size track surface is the first child, so its
                // prepaint bounds are the track rect used to map pointer X to a
                // 0..=1 timeline fraction.
                let track_bounds: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));

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
                    .w_full()
                    .h(px(34.0))
                    .rounded_md()
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
                                if let Some(pct) = scrub_fraction(event.position, &track_bounds) {
                                    this.begin_video_scrub(pct, cx);
                                }
                            }
                        }),
                    )
                    .on_mouse_move(cx.listener({
                        let track_bounds = track_bounds.clone();
                        move |this, event: &MouseMoveEvent, _window, cx| {
                            let Some(pct) = scrub_fraction(event.position, &track_bounds) else {
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
                    .child(div().size_full().rounded_md().bg(theme::bg_control()))
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
            // Controls Row
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .text_color(theme::text())
                    // Left Controls: Replay, Frame Step Back, Play/Pause, Frame Step Fwd, Forward, Time Readout
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            // Replay -5s
                            .child(
                                div()
                                    .id("video-seek-back-btn")
                                    .p_1()
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(theme::accent()))
                                    .text_sm()
                                    .child(render_icon(Icon::RotateCcw, px(14.0), theme::text()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.seek_video_relative(-5.0, cx);
                                    })),
                            )
                            // Step Frame -1
                            .child(
                                div()
                                    .id("video-step-back-btn")
                                    .tooltip(theme::tooltip_label("Previous frame"))
                                    .p_1()
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(theme::accent()))
                                    .text_sm()
                                    .child(render_icon(Icon::StepBack, px(14.0), theme::text()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.step_video_frame(-1, cx);
                                    })),
                            )
                            // Play/Pause Button - Prominent pill with black icon
                            .child(
                                div()
                                    .id("video-play-btn")
                                    .debug_selector(|| "video-play".to_string())
                                    .w(px(38.0))
                                    .h(px(26.0))
                                    .bg(theme::text())
                                    .hover(|s| s.bg(theme::text_muted()))
                                    .rounded_md()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::bg_media())
                                    .child(render_icon(
                                        if this.video.is_playing {
                                            Icon::Pause
                                        } else {
                                            Icon::Play
                                        },
                                        px(14.0),
                                        theme::bg_media(),
                                    ))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_play(cx);
                                    })),
                            )
                            // Step Frame +1
                            .child(
                                div()
                                    .id("video-step-fwd-btn")
                                    .tooltip(theme::tooltip_label("Next frame"))
                                    .p_1()
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(theme::accent()))
                                    .text_sm()
                                    .child(render_icon(Icon::StepForward, px(14.0), theme::text()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.step_video_frame(1, cx);
                                    })),
                            )
                            // Forward +5s
                            .child(
                                div()
                                    .id("video-seek-fwd-btn")
                                    .p_1()
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(theme::accent()))
                                    .text_sm()
                                    .child(render_icon(Icon::RotateCw, px(14.0), theme::text()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.seek_video_relative(5.0, cx);
                                    })),
                            )
                            // Monospace Time Display: 0:16.04 / 0:42.00. While
                            // scrubbing this shows the pointer destination, not the
                            // backend's last decoded position.
                            .child(
                                div()
                                    .ml_2()
                                    .font_family(theme::FONT_MONO)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text_muted())
                                    .child(format!(
                                        "{} / {}",
                                        format_time_fractional(current_time),
                                        format_time_fractional(duration)
                                    )),
                            )
                            // Explicit scrub-state pill so the drag is unmistakable
                            // beyond the accent border and playhead.
                            .child(if is_scrubbing {
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .bg(theme::selection_bg())
                                    .rounded_sm()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::accent())
                                    .child("SCRUB")
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    )
                    // Right Controls: Volume Icon, Volume Slider, Speed, Loop, PiP, Fullscreen
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2p5()
                            // Speaker Icon
                            .child(
                                div()
                                    .id("video-mute-btn")
                                    .debug_selector(|| "video-mute".to_string())
                                    .tooltip(theme::tooltip_label("Mute (M)"))
                                    .cursor_pointer()
                                    .text_sm()
                                    .child(render_icon(
                                        if this.video.is_muted {
                                            Icon::VolumeX
                                        } else {
                                            Icon::Volume
                                        },
                                        px(14.0),
                                        theme::text(),
                                    ))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_mute(cx);
                                    })),
                            )
                            // Amber Volume Slider: continuous 0.0..1.0 mapping with a
                            // 24px hit area (the visible track is 4px) and pointer drag.
                            .child(
                                div()
                                    .on_children_prepainted({
                                        let volume_bounds = volume_bounds.clone();
                                        move |child_bounds: Vec<Bounds<Pixels>>, _window, _cx| {
                                            if let Some(bounds) = child_bounds.first() {
                                                *volume_bounds.borrow_mut() = Some(*bounds);
                                            }
                                        }
                                    })
                                    .id("video-volume-slider")
                                    .debug_selector(|| "video-volume-slider".to_string())
                                    .w(px(88.0))
                                    .h(px(24.0))
                                    .cursor_pointer()
                                    .relative()
                                    .flex()
                                    .items_center()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener({
                                            let volume_bounds = volume_bounds.clone();
                                            move |this, event: &MouseDownEvent, _window, cx| {
                                                if let Some(pct) =
                                                    volume_fraction(event.position, &volume_bounds)
                                                {
                                                    this.video.adjusting_volume = true;
                                                    this.set_video_volume(pct, cx);
                                                }
                                            }
                                        }),
                                    )
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                                            this.video.adjusting_volume = false;
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_up_out(
                                        MouseButton::Left,
                                        cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                                            this.video.adjusting_volume = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(4.0))
                                            .bg(theme::bg_control())
                                            .rounded_full()
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(vol))
                                                    .bg(theme::accent())
                                                    .rounded_full(),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .left(relative((vol - 0.08).clamp(0.0, 0.86)))
                                            .size(px(10.0))
                                            .rounded_full()
                                            .bg(theme::text())
                                            .shadow_sm(),
                                    )
                                    // Last child: the track bounds come from the first.
                                    .child(drag_tracker(
                                        this.video.adjusting_volume,
                                        cx.entity(),
                                        {
                                            let volume_bounds = volume_bounds.clone();
                                            move |this, position, cx| {
                                                if let Some(pct) =
                                                    volume_fraction(position, &volume_bounds)
                                                {
                                                    this.set_video_volume(pct, cx);
                                                }
                                            }
                                        },
                                    )),
                            )
                            // Speed Button: 1 ×
                            .child(
                                div()
                                    .id("video-speed-btn")
                                    .tooltip(theme::tooltip_label("Playback speed"))
                                    .px_2()
                                    .py_1()
                                    .bg(if this.video.speed_menu_open {
                                        theme::selection_bg()
                                    } else {
                                        theme::bg_control()
                                    })
                                    .border_1()
                                    .border_color(if this.video.speed_menu_open {
                                        theme::accent()
                                    } else {
                                        theme::border()
                                    })
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_family(theme::FONT_MONO)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(format_playback_speed(this.video.playback_speed))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_speed_menu(cx);
                                    })),
                            )
                            // Loop Toggle Pill
                            .child(
                                div()
                                    .id("video-loop-btn")
                                    .debug_selector(|| "video-loop".to_string())
                                    .tooltip(theme::tooltip_label("Loop playback"))
                                    .px_2()
                                    .py_1()
                                    .bg(if this.video.is_looping {
                                        theme::selection_bg()
                                    } else {
                                        theme::bg_control()
                                    })
                                    .border_1()
                                    .border_color(if this.video.is_looping {
                                        theme::accent()
                                    } else {
                                        theme::border()
                                    })
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if this.video.is_looping {
                                        theme::accent()
                                    } else {
                                        theme::text_muted()
                                    })
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .child(render_icon(
                                        Icon::Repeat,
                                        px(11.0),
                                        if this.video.is_looping {
                                            theme::accent()
                                        } else {
                                            theme::text_muted()
                                        },
                                    ))
                                    .child("Loop")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_video_loop(cx);
                                    })),
                            )
                            // Picture-in-Picture Button
                            .child(
                                div()
                                    .id("video-pip-btn")
                                    .debug_selector(|| "video-pip".to_string())
                                    .tooltip(theme::tooltip_label("Open Picture-in-Picture (⌥P)"))
                                    .px_2()
                                    .py_1()
                                    .bg(theme::bg_control())
                                    .border_1()
                                    .border_color(theme::border())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(render_icon(Icon::Pip, px(12.0), theme::text()))
                                    .child("PIP")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.open_floating_pip(cx);
                                    })),
                            )
                            // Fullscreen Toggle Button
                            .child(
                                div()
                                    .id("video-fullscreen-btn")
                                    .debug_selector(|| "video-fullscreen".to_string())
                                    .tooltip(theme::tooltip_label("Toggle fullscreen"))
                                    .w(px(28.0))
                                    .h(px(26.0))
                                    .bg(theme::bg_control())
                                    .border_1()
                                    .border_color(theme::border())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child(render_icon(
                                        if this.is_immersive_fullscreen {
                                            Icon::Minimize
                                        } else {
                                            Icon::Maximize
                                        },
                                        px(13.0),
                                        theme::text(),
                                    ))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
                                        this.toggle_immersive_fullscreen(Some(window), cx);
                                    })),
                            ),
                    ),
            )
            // Floating Speed Popover Menu (Directly above speed button)
            .child(if this.video.speed_menu_open {
                div()
                    .id("video-speed-popover-layer")
                    .child(
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
                    .child(
                        div()
                            .id("video-speed-popover")
                            .absolute()
                            .bottom(px(46.0))
                            .right(px(148.0))
                            .w(px(102.0))
                            .bg(theme::bg_hud())
                            .border_1()
                            .border_color(theme::border())
                            .rounded_xl()
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
                                    .justify_between()
                                    .text_xs()
                                    .font_family(theme::FONT_MONO)
                                    .text_color(if is_cur {
                                        theme::accent()
                                    } else {
                                        theme::text()
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap_1p5()
                                            .child(if is_cur {
                                                div().flex().items_center().child(render_icon(
                                                    Icon::Check,
                                                    px(12.0),
                                                    theme::accent(),
                                                ))
                                            } else {
                                                div().w(px(12.0)).child("")
                                            })
                                            .child(format_playback_speed(rate)),
                                    )
                                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                        this.set_video_speed(rate, cx);
                                    }))
                            })),
                    )
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
    }
}
