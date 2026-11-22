//! The docked transport's controls row: Play, ±5 s and the time
//! on the left; speed, mute, audio, loop and full screen on the right. The
//! frame-step buttons are gone; `,` and `.` and the palette still step.

use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

use super::transport::{drag_tracker, scrub_fraction};
use super::utils::format_playback_speed;
use crate::app::types::format_clock;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// Height of the controls row.
pub(super) const CONTROLS_HEIGHT: f32 = 40.0;
/// Round transport buttons.
const BUTTON: f32 = 30.0;
/// The accent Play button.
const PLAY: f32 = 38.0;

/// A 30px round transport button with a 16px icon; the tooltip names the key.
fn round_button(id: &'static str, icon: Icon, label: String, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(label))
        .flex_none()
        .size(px(BUTTON))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(on, |b| b.bg(theme::bg_3()))
        .hover(|s| s.bg(theme::bg_3()))
        .child(render_icon(
            icon,
            theme::ICON_MD,
            if on { theme::accent() } else { theme::t1() },
        ))
}

/// The controls row under the scrubber.
pub(super) fn render(
    this: &LuminaApp,
    current_time: f64,
    duration: f64,
    cx: &mut Context<LuminaApp>,
) -> Div {
    let video = &this.video;
    let (elapsed, total) = (
        format_clock(current_time, false),
        format_clock(duration, false),
    );
    let readout: SharedString = format!("{elapsed} / {total}").into();
    let play_label = t(if video.is_playing {
        "video.pause"
    } else {
        "video.play"
    });

    let left = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.0))
        .child(
            div()
                .id("video-play-btn")
                .debug_selector(|| "video-play".to_string())
                .role(Role::Button)
                .aria_label(play_label.clone())
                .tooltip(theme::tooltip_label(play_label))
                .flex_none()
                .size(px(PLAY))
                .rounded_full()
                .bg(theme::accent())
                .hover(|s| s.bg(theme::accent_hover()))
                .flex()
                .justify_center()
                .items_center()
                .cursor_pointer()
                .child(render_icon(
                    if video.is_playing {
                        Icon::Pause
                    } else {
                        Icon::Play
                    },
                    theme::ICON_MD,
                    theme::on_accent(),
                ))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_video_play(cx);
                })),
        )
        .child(
            round_button("video-seek-back", Icon::RotateCcw, t("video.back_5"), false).on_click(
                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.seek_video_relative(-5.0, cx);
                }),
            ),
        )
        .child(
            round_button(
                "video-seek-fwd",
                Icon::RotateCw,
                t("video.forward_5"),
                false,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.seek_video_relative(5.0, cx);
            })),
        )
        .child(
            div()
                .id("video-time")
                .debug_selector(|| "video-time".to_string())
                .flex_none()
                .flex()
                .flex_row()
                .gap(px(4.0))
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_SMALL)
                .children(crate::views::page_header::text_marker(
                    "video-time",
                    &readout,
                ))
                .child(div().text_color(theme::t1()).child(elapsed))
                .child(div().text_color(theme::t3()).child(format!("/ {total}"))),
        );

    let speed = div()
        .relative()
        .child(
            hud_pill("video-speed-btn", "video-speed", t("video.speed"))
                .aria_expanded(video.speed_menu_open)
                .bg(theme::bg_2())
                .border_1()
                .border_color(theme::line())
                .when(video.speed_menu_open, |b| b.bg(theme::bg_3()))
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_CAPTION)
                .text_color(theme::t2())
                .child(format_playback_speed(video.playback_speed))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_video_speed_menu(cx);
                })),
        )
        .when(video.speed_menu_open, |zone| {
            zone.child(render_speed_menu(this, cx))
        });

    let right = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(speed)
        .child(
            round_button(
                "video-mute",
                if video.is_muted {
                    Icon::VolumeX
                } else {
                    Icon::Volume
                },
                t(if video.is_muted {
                    "video.unmute"
                } else {
                    "video.mute"
                }),
                video.is_muted,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.toggle_video_mute(cx);
            })),
        )
        .child(volume_slider(this, cx))
        .when(video.tracks.has_audio_choice(), |zone| {
            zone.child(
                div()
                    .relative()
                    .child(render_audio_button(this, cx))
                    .when(video.tracks.audio_menu_open, |zone| {
                        zone.child(render_audio_menu(this, cx))
                    }),
            )
        })
        .child(
            round_button(
                "video-loop",
                Icon::Repeat,
                t("video.loop"),
                video.is_looping,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.toggle_video_loop(cx);
            })),
        )
        .child(
            round_button(
                "video-fullscreen",
                if this.is_immersive_fullscreen {
                    Icon::Minimize
                } else {
                    Icon::Maximize
                },
                t("video.fullscreen"),
                false,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, window, cx| {
                this.toggle_immersive_fullscreen(Some(window), cx);
            })),
        );

    div()
        .w_full()
        .h(px(CONTROLS_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_4()
        .child(left)
        .child(right)
}

/// The 62x4 volume slider beside mute: press or drag sets the level; the
/// drag saves on release. Its keyboard path is ↑/↓ and the palette.
fn volume_slider(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    // The track is the first child; its prepaint bounds map pointer X.
    let track: Rc<RefCell<Option<Bounds<Pixels>>>> = Rc::new(RefCell::new(None));
    let label = t("video.volume");
    div()
        .on_children_prepainted({
            let track = track.clone();
            move |bounds: Vec<Bounds<Pixels>>, _window, _cx| {
                if let Some(bounds) = bounds.first() {
                    *track.borrow_mut() = Some(*bounds);
                }
            }
        })
        .id("video-volume")
        .debug_selector(|| "video-volume".to_string())
        .role(Role::Slider)
        .aria_label(label.clone())
        .aria_numeric_value(f64::from(this.video.volume))
        .aria_min_numeric_value(0.0)
        .aria_max_numeric_value(1.0)
        .aria_numeric_value_step(0.1)
        .tooltip(theme::tooltip_label(label))
        .flex_none()
        .w(px(62.0))
        // A taller hit area than the 4px track.
        .h(px(16.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .child(
            div()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(theme::volume_track())
                .child(
                    div()
                        .h_full()
                        .w(relative(this.video.volume.clamp(0.0, 1.0)))
                        .rounded_full()
                        .bg(theme::t1()),
                ),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener({
                let track = track.clone();
                move |this, event: &MouseDownEvent, _w, cx| {
                    if let Some(level) = scrub_fraction(event.position, &track) {
                        this.begin_volume_drag(level as f32, cx);
                    }
                }
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, cx| this.end_volume_drag(cx)),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, cx| this.end_volume_drag(cx)),
        )
        .child(drag_tracker(
            this.video.volume_drag,
            cx.entity(),
            move |this, position, cx| {
                if let Some(level) = scrub_fraction(position, &track) {
                    this.apply_video_volume(level as f32, cx);
                }
            },
        ))
}

/// Speed menu, hanging above the speed pill.
fn render_speed_menu(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .id("video-speed-popover")
        .debug_selector(|| "video-speed-popover".to_string())
        .occlude()
        .absolute()
        .bottom(px(34.0))
        .left_0()
        .w(px(102.0))
        .bg(theme::bg_float())
        .border_1()
        .border_color(theme::line_2())
        .rounded(theme::RADIUS_LG)
        .shadow_2xl()
        .py_1()
        .flex()
        .flex_col()
        .children(crate::config::VIDEO_SPEEDS.into_iter().map(|rate| {
            let is_cur = (this.video.playback_speed - rate).abs() < 0.05;
            div()
                .id(ElementId::NamedInteger(
                    "speed-opt".into(),
                    (rate * 100.0) as u64,
                ))
                .px_3()
                .py_1()
                .hover(|s| s.bg(theme::bg_3()))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .text_size(theme::TEXT_SMALL)
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_color(if is_cur { theme::accent() } else { theme::t1() })
                .child(if is_cur {
                    div().flex().items_center().child(render_icon(
                        Icon::Check,
                        theme::ICON_XS,
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

/// The Audio menu button; shown only when the file offers a choice. Its
/// keyboard path is the palette's "Audio Track: {name}" rows.
/// The HUD's 28px pill: speed and audio share this shape.
fn hud_pill(
    id: &'static str,
    selector: &'static str,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    div()
        .id(id)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(label))
        .h(px(28.0))
        .px(px(10.0))
        .rounded(px(7.0))
        .flex()
        .items_center()
        .hover(|s| s.bg(theme::bg_3()))
        .cursor_pointer()
}

fn render_audio_button(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    hud_pill(
        "video-audio-btn",
        "video-audio",
        t("video.audio_menu_tooltip"),
    )
    .aria_expanded(this.video.tracks.audio_menu_open)
    .when(this.video.tracks.audio_menu_open, |b| b.bg(theme::bg_3()))
    .text_size(theme::TEXT_SMALL)
    .text_color(theme::t1())
    .child(t("video.audio_menu"))
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.toggle_audio_menu(cx);
    }))
}

/// Audio track menu, hanging above the Audio button like the speed menu.
fn render_audio_menu(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let tracks = &this.video.tracks;
    // Asked while the menu is open, not cached at load (see current_audio_track).
    let playing = this.current_audio_track();
    div()
        .id("video-audio-popover")
        .debug_selector(|| "video-audio-popover".to_string())
        .role(Role::Menu)
        .aria_label(t("video.audio_menu_tooltip"))
        .occlude()
        .absolute()
        .bottom(px(34.0))
        .right_0()
        .min_w(px(160.0))
        .max_w(px(280.0))
        .bg(theme::bg_float())
        .border_1()
        .border_color(theme::line_2())
        .rounded(theme::RADIUS_LG)
        .shadow_2xl()
        .py_1()
        .flex()
        .flex_col()
        .children((0..tracks.audio.len()).map(|index| {
            let is_cur = playing == Some(index);
            let name = tracks.audio_name(index);
            div()
                .id(ElementId::NamedInteger("audio-opt".into(), index as u64))
                .debug_selector(move || format!("video-audio-track-{index}"))
                .role(Role::MenuItemRadio)
                .aria_toggled(if is_cur {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .aria_label(name.clone())
                .px_3()
                .py_1()
                .hover(|s| s.bg(theme::bg_3()))
                .cursor_pointer()
                .flex()
                .flex_row()
                .items_center()
                .gap_1p5()
                .text_size(theme::TEXT_SMALL)
                .text_color(if is_cur { theme::accent() } else { theme::t1() })
                .child(if is_cur {
                    div()
                        .debug_selector(|| "video-audio-check".to_string())
                        .flex()
                        .items_center()
                        .child(render_icon(Icon::Check, theme::ICON_XS, theme::accent()))
                } else {
                    div().w(px(12.0))
                })
                .child(div().truncate().child(name))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.select_audio_track(index, cx);
                }))
        }))
}
