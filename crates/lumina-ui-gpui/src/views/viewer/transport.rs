use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::app::LuminaApp;
use crate::theme;

/// Convert a window-space pointer position into a 0..=1 fraction of the scrub
/// track. Bounds are captured during prepaint, so drag mapping stays correct
/// across resizes and layout changes without hard-coded geometry.
pub(crate) fn scrub_fraction(
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
pub(crate) fn drag_tracker(
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

/// Padding above and below the transport's rows.
const PAD_Y: f32 = 16.0;
/// Gap between the transport's rows.
const ROW_GAP: f32 = 11.0;
/// Height of the scrub track.
const TRACK_HEIGHT: f32 = 24.0;

/// The docked transport's full height (border, padding, rows), so the video
/// frame can be sized to the stage left above it.
pub(super) fn transport_height(_this: &LuminaApp) -> f32 {
    1.0 + 2.0 * PAD_Y + TRACK_HEIGHT + ROW_GAP + super::transport_controls::CONTROLS_HEIGHT
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
        // Scrub feedback follows the drag destination, else the hover point.
        let time_badge = if is_scrubbing {
            scrub_time.map(|time| (progress_pct as f32, time))
        } else {
            hover_pct.zip(hover_time)
        };

        // The loop range as 0..=1 of the track; a missing point is the file's
        // start or end.
        let range = &this.video.loop_range;
        let loop_band = (range.is_set() && duration > 0.0).then(|| {
            let start = (range.in_point.unwrap_or(0.0) / duration).clamp(0.0, 1.0) as f32;
            let end = (range.out_point.unwrap_or(duration) / duration).clamp(0.0, 1.0) as f32;
            (start, end - start)
        });

        // Chapter starts as 0..=1 of the track. The first chapter usually
        // starts at 0, where a tick would only mark the track's edge.
        let chapter_ticks: Vec<f32> = if duration > 0.0 {
            this.video
                .tracks
                .chapters
                .iter()
                .map(|(start, _)| start / duration)
                .filter(|pct| *pct > 0.0 && *pct < 1.0)
                .map(|pct| pct as f32)
                .collect()
        } else {
            Vec::new()
        };
        let menu_open = this.video.speed_menu_open || this.video.tracks.audio_menu_open;

        div()
            .id("video-transport")
            .debug_selector(|| "video-transport".to_string())
            .on_hover(cx.listener(|this, on: &bool, _w, _cx| this.set_pointer_on_hud(*on)))
            // Docked across the stage, under the frame (Design 06).
            .w_full()
            .flex_none()
            .px(px(28.0))
            .py(px(PAD_Y))
            .border_t_1()
            .border_color(theme::line())
            .relative()
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .child(div().w_full().flex().flex_row().child({
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
                    .flex_1()
                    .h(px(TRACK_HEIGHT))
                    .rounded(theme::RADIUS_MD)
                    .border_1()
                    .border_color(if is_scrubbing {
                        theme::t2()
                    } else {
                        theme::line()
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
                    .child(
                        div()
                            .size_full()
                            .rounded(theme::RADIUS_MD)
                            .bg(theme::bg_2()),
                    )
                    // Loop range, under the played tint.
                    .children(loop_band.map(|(start, width)| {
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .debug_selector(|| "video-loop-range".to_string())
                            .left(relative(start))
                            .w(relative(width))
                            .border_x_1()
                            .border_color(theme::accent())
                            .bg(theme::bg_3())
                    }))
                    .children(chapter_ticks.into_iter().enumerate().map(|(i, pct)| {
                        div()
                            .absolute()
                            .top(px(4.0))
                            .bottom(px(4.0))
                            .debug_selector(move || format!("video-chapter-tick-{}", i + 1))
                            .left(relative(pct))
                            .w(px(1.5))
                            .bg(theme::t2())
                    }))
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
                            .bg(theme::accent_soft()),
                    )
                    // Hover needle indicator, hidden while a scrub owns the playhead.
                    .child(if let Some(hover_pct) = hover_pct {
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(relative(hover_pct))
                            .w(px(1.5))
                            .bg(theme::t1())
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
                    // Time badge, with the nearest strip frame once there
                    // is one, anchored directly above the track.
                    .children(time_badge.map(|(badge_pct, badge_time)| {
                        super::scrub_preview::render(this, badge_pct, badge_time)
                    }))
            }))
            // Dismiss layer for the speed and audio menus, under the controls.
            .when(menu_open, |card| {
                card.child(
                    div()
                        .id("video-speed-dismiss")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                                this.video.speed_menu_open = false;
                                this.video.tracks.audio_menu_open = false;
                                cx.notify();
                            }),
                        ),
                )
            })
            .child(super::transport_controls::render(
                this,
                current_time,
                duration,
                cx,
            ))
    }
}
