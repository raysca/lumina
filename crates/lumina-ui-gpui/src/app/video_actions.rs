use gpui::*;
use lumina_core::MediaType;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::video_frame_to_gpui_image;
use crate::player::{self, VideoPlayerBackend};

/// Minimum interval between scrubbing seeks.
///
/// A drag emits pointer moves at display rate. AVFoundation seek plus frame
/// fetch is not free, so seeks are coalesced to this cadence while the visible
/// destination time and playhead still update on every move. Releasing the
/// pointer always commits the final destination immediately.
const SCRUB_SEEK_INTERVAL: Duration = Duration::from_millis(50);

/// Resolve a 0..=1 scrub fraction against the playback duration.
fn scrub_target_time(duration: f64, pct: f64) -> f64 {
    let duration = duration.max(0.0);
    (duration * pct.clamp(0.0, 1.0)).clamp(0.0, duration)
}

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
        // Closing the viewer, changing media, or otherwise stopping playback
        // must never leave a scrub drag armed against a stale destination.
        self.video.is_scrubbing = false;
        self.video.scrub_time = None;
        self.video.last_scrub_seek = None;
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
        let Some(player) = self.video.player.as_ref() else {
            return;
        };
        let max_d = if self.video.duration > 0.0 {
            self.video.duration
        } else {
            3600.0
        };
        let target = (player.position() + delta_secs).clamp(0.0, max_d);
        self.seek_video_to_time(target);
        cx.notify();
    }

    pub fn seek_video_percent(&mut self, pct: f64, cx: &mut Context<Self>) {
        if self.video.player.is_none() {
            return;
        }
        let target = scrub_target_time(self.video.duration, pct);
        self.seek_video_to_time(target);
        cx.notify();
    }

    /// Duration used for scrub math: player metadata when loaded, otherwise the
    /// selected item's catalog duration (the transport shows the same value).
    fn video_scrub_duration(&self) -> f64 {
        if self.video.duration > 0.0 {
            self.video.duration
        } else {
            self.selected_item
                .as_ref()
                .map(|item| item.duration_secs())
                .unwrap_or(0.0)
        }
    }

    /// Moves the player and the tracked position to target without fetching a
    /// frame. This is the cheap half of a seek, used for coalesced drag updates.
    fn apply_video_scrub_position(&mut self, target: f64) {
        self.video.playback_time = target;
        if let Some(ref mut player) = self.video.player {
            player.seek(target);
        }
    }

    /// Seeks and renders the resulting frame. This is the existing click/keyboard
    /// seek behavior and also runs when a scrub drag is committed on release.
    fn seek_video_to_time(&mut self, target: f64) {
        self.video.playback_time = target;
        if let Some(ref mut player) = self.video.player {
            player.seek(target);
            if let Some(frame) = player.next_frame() {
                if let Ok(img) = video_frame_to_gpui_image(frame) {
                    self.viewer_image = Some(img);
                }
            }
        }
    }

    /// Starts a direct scrub at pct (0..=1) and seeks the player there.
    ///
    /// Called on pointer-down over the timeline. The destination is tracked in
    /// scrub_time so the transport can render time/playhead feedback even when
    /// the backend is not loaded (for example while a video is still opening).
    pub fn begin_video_scrub(&mut self, pct: f64, cx: &mut Context<Self>) {
        let target = scrub_target_time(self.video_scrub_duration(), pct);
        self.video.is_scrubbing = true;
        self.video.scrub_time = Some(target);
        self.video.last_scrub_seek = Some(Instant::now());
        self.apply_video_scrub_position(target);
        cx.notify();
    }

    /// Updates the scrub destination while the pointer moves.
    ///
    /// scrub_time always follows the pointer; the player seek is coalesced to
    /// at most one per SCRUB_SEEK_INTERVAL so a drag cannot queue a seek per
    /// display frame. The final destination is committed by
    /// Self::end_video_scrub on release.
    pub fn update_video_scrub(&mut self, pct: f64, cx: &mut Context<Self>) {
        if !self.video.is_scrubbing {
            return;
        }
        let target = scrub_target_time(self.video_scrub_duration(), pct);
        self.video.scrub_time = Some(target);

        let now = Instant::now();
        let seek_due = self
            .video
            .last_scrub_seek
            .map(|last| now.saturating_duration_since(last) >= SCRUB_SEEK_INTERVAL)
            .unwrap_or(true);
        if seek_due {
            self.video.last_scrub_seek = Some(now);
            self.apply_video_scrub_position(target);
        }
        cx.notify();
    }

    /// Ends the scrub drag and commits the last destination as a real seek.
    ///
    /// A no-op when no drag is active, so releasing the pointer elsewhere in
    /// the app can safely call this.
    pub fn end_video_scrub(&mut self, cx: &mut Context<Self>) {
        if !self.video.is_scrubbing {
            return;
        }
        self.video.is_scrubbing = false;
        self.video.last_scrub_seek = None;
        if let Some(target) = self.video.scrub_time.take() {
            self.seek_video_to_time(target);
        }
        cx.notify();
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
