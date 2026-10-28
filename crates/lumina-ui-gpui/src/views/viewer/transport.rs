use gpui::*;

use super::utils::format_playback_speed;
use crate::app::types::format_time_fractional;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

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
        let current_time = this.video.playback_time;
        let progress_pct = if duration > 0.0 {
            (current_time / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let vol = this.video.volume.clamp(0.0, 1.0);

        div()
            .w_full()
            .max_w(px(880.0))
            .mb_4()
            .relative()
            .flex()
            .flex_col()
            .gap_2()
            // Floating Hover Preview Thumbnail Card (above scrubber, only when hovering)
            .child(
                if let (Some(hover_pct), Some(hover_time)) =
                    (this.video.hover_pct, this.video.hover_time)
                {
                    let hover_img = crate::app::types::generate_gradient_image(
                        (82, 148, 185),
                        (175, 205, 220),
                        144,
                        82,
                    );
                    div()
                        .id("video-hover-preview-card")
                        .absolute()
                        .bottom(px(84.0))
                        .left(relative((hover_pct - 0.08).clamp(0.01, 0.81)))
                        .w(px(144.0))
                        .h(px(82.0))
                        .rounded_md()
                        .overflow_hidden()
                        .border_1()
                        .border_color(theme::border_strong())
                        .bg(theme::bg_hud())
                        .shadow_2xl()
                        .child(img(hover_img).size_full().object_fit(ObjectFit::Cover))
                        .child(
                            div()
                                .absolute()
                                .bottom_1()
                                .left_0()
                                .right_0()
                                .flex()
                                .justify_center()
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .bg(theme::bg_media())
                                        .rounded_sm()
                                        .text_xs()
                                        .font_family(theme::FONT_MONO)
                                        .text_size(px(10.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::text())
                                        .child(format_time_fractional(hover_time)),
                                ),
                        )
                        .into_any_element()
                } else {
                    Empty.into_any_element()
                },
            )
            // Storyboard Filmstrip Scrubber Track
            .child({
                let num_slices = 16;
                let mut track_slices = div()
                    .size_full()
                    .flex()
                    .flex_row()
                    .rounded_md()
                    .overflow_hidden();
                for s in 0..num_slices {
                    let t = s as f32 / (num_slices - 1) as f32;
                    let r = (0x95 as f32 * (1.0 - t) + 0x3f as f32 * t) as u32;
                    let g = (0xc2 as f32 * (1.0 - t) + 0x73 as f32 * t) as u32;
                    let b = (0xdc as f32 * (1.0 - t) + 0x98 as f32 * t) as u32;
                    let col = (r << 16) | (g << 8) | b;
                    track_slices = track_slices.child(
                        div()
                            .flex_1()
                            .h_full()
                            .bg(rgb(col))
                            .border_r_1()
                            .border_color(theme::border_subtle()),
                    );
                }

                let mut hit_row = div().absolute().inset_0().flex().flex_row();
                for i in 0..32 {
                    let pct = (i as f32 + 0.5) / 32.0;
                    hit_row = hit_row.child(
                        div()
                            .id(ElementId::NamedInteger("scrub-seg".into(), i as u64))
                            .flex_1()
                            .h_full()
                            .on_mouse_move(cx.listener(move |this, _e: &MouseMoveEvent, _w, cx| {
                                this.set_video_hover_scrub(Some(pct), cx);
                            }))
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.seek_video_percent(pct as f64, cx);
                            })),
                    );
                }

                div()
                    .id("video-scrubber-track")
                    .w_full()
                    .h(px(34.0))
                    .rounded_md()
                    .border_1()
                    .border_color(theme::border())
                    .relative()
                    .cursor_pointer()
                    .child(track_slices)
                    // Hover needle indicator (only when hovering)
                    .child(if let Some(hover_pct) = this.video.hover_pct {
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
                    // Current Playhead needle indicator
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(relative(progress_pct as f32))
                            .w(px(2.5))
                            .bg(theme::accent())
                            .shadow_md(),
                    )
                    .child(hit_row)
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
                            // Monospace Time Display: 0:16.04 / 0:42.00
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
                            ),
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
                            // Amber Volume Slider (Supports 0.0 to 1.0)
                            .child(
                                div()
                                    .id("video-volume-slider")
                                    .w(px(68.0))
                                    .h(px(14.0))
                                    .cursor_pointer()
                                    .relative()
                                    .flex()
                                    .items_center()
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
                                    // 6 steps: 0.0, 0.2, 0.4, 0.6, 0.8, 1.0 (enables clean mute)
                                    .child({
                                        let mut v_hits =
                                            div().absolute().inset_0().flex().flex_row();
                                        for i in 0..=5 {
                                            let v_step = i as f32 / 5.0;
                                            v_hits = v_hits.child(
                                                div()
                                                    .id(ElementId::NamedInteger(
                                                        "vol-step".into(),
                                                        i as u64,
                                                    ))
                                                    .flex_1()
                                                    .h_full()
                                                    .on_click(cx.listener(
                                                        move |this, _e: &ClickEvent, _w, cx| {
                                                            this.set_video_volume(v_step, cx);
                                                        },
                                                    )),
                                            );
                                        }
                                        v_hits
                                    }),
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
