use gpui::*;
use lumina_core::MediaType;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::video_frame_to_gpui_image;
use crate::player::{self, VideoPlayerBackend};

impl LuminaApp {
    pub fn open_floating_pip(&mut self, cx: &mut Context<Self>) {
        let is_video = self
            .selected_item
            .as_ref()
            .map(|i| i.media_type == MediaType::Video)
            .unwrap_or(false);

        if !is_video {
            return;
        }

        // Close fullscreen viewer so user can browse grid
        self.is_fullscreen_viewer = false;
        self.viewer_state.reset();

        if self.floating_pip_window.is_some() {
            cx.notify();
            return;
        }

        // Defer window creation to next tick of GPUI event loop
        let _ = self.ui_tx.send_blocking(UiMessage::OpenFloatingPip);
        cx.notify();
    }

    pub fn expand_pip_to_viewer(&mut self, cx: &mut Context<Self>) {
        self.floating_pip_window = None;
        self.is_fullscreen_viewer = true;
        self.viewer_state.reset();
        cx.notify();
    }

    pub fn floating_pip_closed(&mut self, cx: &mut Context<Self>) {
        self.floating_pip_window = None;
        if !self.is_fullscreen_viewer {
            self.stop_video_player();
        }
        cx.notify();
    }

    pub fn load_video_player(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.stop_video_player();
        let mut player = player::create_native_player();
        if let Ok(meta) = player.load(path) {
            player.set_looping(self.video.is_looping);
            player.set_volume(self.video.volume);
            player.set_muted(self.video.is_muted);
            player.set_rate(self.video.playback_speed);
            self.video.duration = meta.duration_secs;
            self.video.playback_time = 0.0;
            if let Some(frame) = player.next_frame() {
                if let Ok(img) = video_frame_to_gpui_image(frame) {
                    self.viewer_image = Some(img);
                }
            }
            self.video.player = Some(player);
        }
        cx.notify();
    }

    pub fn stop_video_player(&mut self) {
        self.stop_video_ticker();
        if let Some(ref mut player) = self.video.player {
            player.pause();
        }
        self.video.player = None;
        self.video.is_playing = false;
    }

    pub fn start_video_ticker(&mut self) {
        self.stop_video_ticker();
        let active = Arc::new(AtomicBool::new(true));
        self.video.ticker_active = Some(active.clone());
        let frame_pending = self.video.frame_pending.clone();
        frame_pending.store(false, Ordering::Release);
        let ui_tx = self.ui_tx.clone();
        let handle = std::thread::Builder::new()
            .name("lumina-video-ticker".to_string())
            .spawn(move || {
                while active.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(16));
                    if !active.load(Ordering::Relaxed) {
                        break;
                    }
                    // Backpressure: drop tick if previous frame has not yet been processed by the UI thread
                    if frame_pending
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                        .is_ok()
                        && ui_tx.send_blocking(UiMessage::VideoFrameTick).is_err()
                    {
                        break;
                    }
                }
            })
            .ok();
        self.video.ticker_handle = handle;
    }

    pub fn stop_video_ticker(&mut self) {
        if let Some(active) = self.video.ticker_active.take() {
            active.store(false, Ordering::Relaxed);
            self.video.ticker_handle = None;
        }
    }

    pub fn toggle_video_play(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut player) = self.video.player {
            if self.video.is_playing {
                player.pause();
                self.video.is_playing = false;
                self.stop_video_ticker();
            } else {
                player.play();
                self.video.is_playing = true;
                self.start_video_ticker();
            }
            cx.notify();
        } else {
            self.video.is_playing = !self.video.is_playing;
            cx.notify();
        }
    }

    pub fn seek_video_relative(&mut self, delta_secs: f64, cx: &mut Context<Self>) {
        if let Some(ref mut player) = self.video.player {
            let max_d = if self.video.duration > 0.0 {
                self.video.duration
            } else {
                3600.0
            };
            let target = (player.position() + delta_secs).clamp(0.0, max_d);
            player.seek(target);
            self.video.playback_time = target;
            if let Some(frame) = player.next_frame() {
                if let Ok(img) = video_frame_to_gpui_image(frame) {
                    self.viewer_image = Some(img);
                }
            }
            cx.notify();
        }
    }

    pub fn seek_video_percent(&mut self, pct: f64, cx: &mut Context<Self>) {
        if let Some(ref mut player) = self.video.player {
            let max_duration = self.video.duration.max(0.0);
            let target = (max_duration * pct.clamp(0.0, 1.0)).clamp(0.0, max_duration);
            player.seek(target);
            self.video.playback_time = target;
            if let Some(frame) = player.next_frame() {
                if let Ok(img) = video_frame_to_gpui_image(frame) {
                    self.viewer_image = Some(img);
                }
            }
            cx.notify();
        }
    }

    pub fn step_video_frame(&mut self, frames: isize, cx: &mut Context<Self>) {
        let delta = (frames as f64) * (1.0 / 30.0);
        self.seek_video_relative(delta, cx);
    }

    #[cfg(test)]
    pub fn cycle_video_speed(&mut self, cx: &mut Context<Self>) {
        self.video.playback_speed = match self.video.playback_speed {
            s if s < 0.75 => 1.0,
            s if s < 1.25 => 1.5,
            s if s < 1.75 => 2.0,
            _ => 0.5,
        };
        if let Some(ref mut player) = self.video.player {
            player.set_rate(self.video.playback_speed);
        }
        cx.notify();
    }

    pub fn toggle_video_mute(&mut self, cx: &mut Context<Self>) {
        self.video.is_muted = !self.video.is_muted;
        if let Some(ref mut player) = self.video.player {
            player.set_muted(self.video.is_muted);
        }
        cx.notify();
    }

    pub fn toggle_video_loop(&mut self, cx: &mut Context<Self>) {
        self.video.is_looping = !self.video.is_looping;
        if let Some(ref mut player) = self.video.player {
            player.set_looping(self.video.is_looping);
        }
        cx.notify();
    }

    pub fn set_video_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        let v = volume.clamp(0.0, 1.0);
        self.video.volume = v;
        if v > 0.0 && self.video.is_muted {
            self.video.is_muted = false;
        }
        if let Some(ref mut player) = self.video.player {
            player.set_volume(v);
            player.set_muted(self.video.is_muted);
        }
        cx.notify();
    }

    pub fn set_video_speed(&mut self, speed: f32, cx: &mut Context<Self>) {
        self.video.playback_speed = speed;
        self.video.speed_menu_open = false;
        if let Some(ref mut player) = self.video.player {
            player.set_rate(speed);
        }
        cx.notify();
    }

    pub fn toggle_video_speed_menu(&mut self, cx: &mut Context<Self>) {
        self.video.speed_menu_open = !self.video.speed_menu_open;
        cx.notify();
    }

    pub fn set_video_hover_scrub(&mut self, hover_pct: Option<f32>, cx: &mut Context<Self>) {
        if let Some(pct) = hover_pct {
            let duration = if self.video.duration > 0.0 {
                self.video.duration
            } else {
                self.selected_item
                    .as_ref()
                    .map(|i| i.duration_secs())
                    .unwrap_or(0.0)
            };
            let pct_clamped = pct.clamp(0.0, 1.0);
            self.video.hover_pct = Some(pct_clamped);
            self.video.hover_time = Some(duration * (pct_clamped as f64));
        } else {
            self.video.hover_pct = None;
            self.video.hover_time = None;
        }
        cx.notify();
    }
}
