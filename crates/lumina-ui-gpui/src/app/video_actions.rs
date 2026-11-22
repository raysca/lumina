use gpui::*;
use lumina_core::{MediaId, MediaItem, MediaType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::bridge::video_frame_to_gpui_image;
use crate::config::resume_start_time;
use crate::player::VideoPlayerBackend;

/// Minimum interval between scrubbing seeks.
///
/// A drag emits pointer moves at display rate. AVFoundation seek plus frame
/// fetch is not free, so seeks are coalesced to this cadence while the visible
/// destination time and playhead still update on every move. Releasing the
/// pointer always commits the final destination immediately.
const SCRUB_SEEK_INTERVAL: Duration = Duration::from_millis(50);

/// Playback progress (in seconds) between background resume-point updates.
///
/// Frame ticks arrive at display rate; writing the resume map every frame would
/// be pointless churn. A fixed playback-progress interval keeps updates
/// deterministic and free of wall-clock timers, and pause/stop/close always
/// commit the final position.
const RESUME_PROGRESS_INTERVAL: f64 = 5.0;

/// The UI's playback preferences, applied to each newly loaded player.
#[derive(Clone, Copy)]
pub(crate) struct VideoSettings {
    looping: bool,
    volume: f32,
    muted: bool,
    rate: f32,
}

impl VideoSettings {
    pub(crate) fn of(video: &super::state::VideoPlaybackState) -> Self {
        Self {
            looping: video.is_looping,
            volume: video.volume,
            muted: video.is_muted,
            rate: video.playback_speed,
        }
    }

    pub(super) fn apply(self, player: &mut impl VideoPlayerBackend) {
        player.set_looping(self.looping);
        player.set_volume(self.volume);
        player.set_muted(self.muted);
        player.set_rate(self.rate);
    }
}

/// How long a paused player is polled for the frame at a seek destination.
///
/// `AVPlayer.seekToTime` with zero tolerance completes asynchronously, so the
/// destination frame is not guaranteed to exist when the seek returns. The
/// viewer therefore polls for a bounded window after a seek instead of
/// blocking or leaving a timer running forever. Playback keeps its normal
/// continuous frame loop and does not use this window.
const SEEK_REFRESH_WINDOW: Duration = Duration::from_millis(300);

/// Resolve a 0..=1 scrub fraction against the playback duration.
fn scrub_target_time(duration: f64, pct: f64) -> f64 {
    let duration = duration.max(0.0);
    (duration * pct.clamp(0.0, 1.0)).clamp(0.0, duration)
}

impl LuminaApp {
    /// Duration used to validate a resume position: player metadata first,
    /// then the session item's catalog duration before the player loads.
    fn resume_duration(&self) -> f64 {
        if self.video.duration.is_finite() && self.video.duration > 0.0 {
            return self.video.duration;
        }
        self.video
            .item
            .as_ref()
            .or(self.selected_item.as_ref())
            .filter(|item| item.media_type == MediaType::Video)
            .map(|item| item.duration_secs())
            .filter(|secs| secs.is_finite() && *secs > 0.0)
            .unwrap_or(0.0)
    }

    /// Media id whose player owns the current resume point.
    ///
    /// The grid tracks the id that was actually loaded, so switching videos
    /// cannot attribute the old position to the new selection. Tests that run
    /// without a catalog fall back to the selected video; production always has
    /// a grid state while a library is open.
    fn active_resume_media_id(&self) -> Option<MediaId> {
        if let Some(state) = self.grid_state.as_ref() {
            return state.video_resume.current();
        }
        #[cfg(test)]
        {
            self.selected_item
                .as_ref()
                .filter(|item| item.media_type == MediaType::Video)
                .map(|item| item.id)
        }
        #[cfg(not(test))]
        {
            None
        }
    }

    /// Best known current playback position. The backend position wins when it
    /// is usable; the tracked playback time covers a missing or not-yet-loaded
    /// player (seeks and tests).
    pub(crate) fn current_player_position(&self) -> f64 {
        match self.video.player.as_ref().map(|player| player.position()) {
            Some(position) if position.is_finite() && position > 0.0 => position,
            _ => self.video.playback_time,
        }
    }

    /// Records a resume point for the active video. Returns false when the
    /// value is invalid, the duration is unknown, or there is no active media.
    fn record_video_resume(&self, seconds: f64) -> bool {
        let duration = self.resume_duration();
        if !seconds.is_finite() || seconds <= 0.0 || duration <= 0.0 {
            return false;
        }
        if let Some(state) = self.grid_state.as_ref() {
            return state.video_resume.record_current(seconds, duration);
        }
        #[cfg(test)]
        {
            let Some(id) = self.active_resume_media_id() else {
                return false;
            };
            self.config.resume_positions.record(id, seconds, duration);
            true
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    /// Commits the current playback position to the resume map.
    fn persist_video_resume_now(&self) -> bool {
        self.record_video_resume(self.current_player_position())
    }

    /// Throttled resume update during playback, called from the frame tick.
    ///
    /// Updates only when the position moved by at least RESUME_PROGRESS_INTERVAL
    /// since the last saved point, then persists through the normal config save
    /// path so a quit or crash loses at most one interval. A position inside the
    /// end margin clears the stale point once so a finished video does not
    /// reopen near the end.
    pub(crate) fn note_video_resume_progress(&mut self) {
        let position = self.video.playback_time;
        if !position.is_finite() || position <= 0.0 {
            return;
        }
        let duration = self.resume_duration();
        let Some(id) = self.active_resume_media_id() else {
            return;
        };
        let saved = self
            .grid_state
            .as_ref()
            .and_then(|state| state.video_resume.saved(id))
            .or_else(|| self.config.resume_positions.get(id));

        if resume_start_time(Some(position), duration) <= 0.0 {
            // At the start (first second) or inside the end margin the saved
            // point is stale for this playback; clear it once.
            if saved.is_some() && self.record_video_resume(position) {
                self.save_config();
            }
            return;
        }
        if let Some(saved) = saved {
            if (position - saved).abs() < RESUME_PROGRESS_INTERVAL {
                return;
            }
        }
        if self.record_video_resume(position) {
            self.save_config();
        }
    }

    /// The open clip cannot play: a failed load, or a decode error mid-clip.
    /// Drops the player and shows the playback-failed card (or the missing
    /// card when the file is gone). Keeps `video.item` so Try Again and
    /// Reveal work.
    pub(crate) fn record_video_failure(&mut self, error: String) {
        self.stop_video_ticker();
        // A mid-clip failure keeps the position reached, as every stop does
        // (a failed load has no player and no position to keep).
        if self.video.player.is_some() && self.persist_video_resume_now() {
            self.save_config();
        }
        if let Some(mut player) = self.video.player.take() {
            player.pause();
        }
        self.video.surface = None;
        // A failed clip has no resume point to finish.
        if let Some(state) = self.grid_state.as_ref() {
            state.video_resume.finish();
        }
        self.video.is_playing = false;
        self.video.load_error = Some(error);
        // ponytail: one stat on the UI thread, where the load already
        // failed; the catalog marks it missing when the watcher sees it.
        self.video.load_file_gone = !self
            .video
            .item
            .as_ref()
            .is_some_and(|item| item.path.exists());
        // No popover left open above the dimmed transport.
        self.video.speed_menu_open = false;
        self.video.tracks.audio_menu_open = false;
    }

    pub fn stop_video_player(&mut self) {
        self.video.load_generation = self.video.load_generation.wrapping_add(1);
        self.stop_video_ticker();
        if let Some(ref mut player) = self.video.player {
            player.pause();
        }
        // Commit the position before dropping the handle. This runs on viewer
        // close, media change, floating-PiP close, and library shutdown.
        self.persist_video_resume_now();
        if !self.is_fullscreen_viewer {
            // The viewer is closing (or PiP is): drop resume points whose media
            // left the catalog. The save that follows this stop persists the
            // pruned map. Keep the set bounded so this stays a short batch.
            if let Some(state) = self
                .grid_state
                .as_ref()
                .filter(|state| !state.video_resume.positions.is_empty())
            {
                let engine = state.engine.clone();
                state
                    .video_resume
                    .prune_missing(|id| engine.get_item_by_id(id).is_some());
            }
        }
        if let Some(state) = self.grid_state.as_ref() {
            state.video_resume.finish();
        }
        self.video.player = None;
        self.video.surface = None;
        self.video.item = None;
        self.video.pending_load_seek = None;
        self.video.is_playing = false;
        // Closing the viewer, changing media, or otherwise stopping playback
        // must never leave a scrub drag armed against a stale destination.
        self.video.is_scrubbing = false;
        self.video.scrub_time = None;
        self.video.last_scrub_seek = None;
        self.video.seek_refresh_until = None;
        self.video.resume_after_scrub = false;
        // A range belongs to one open video.
        self.video.loop_range = Default::default();
        // So are its tracks, chapters and audio choice.
        self.video.tracks = Default::default();
        // And its load failure.
        self.video.load_error = None;
        self.video.load_file_gone = false;
        self.video.failed_focus = None;
        self.clear_scrub_strip();
    }

    /// Shows a decoded video frame wherever the session is presented.
    pub(crate) fn show_video_frame(&mut self, image: Arc<RenderImage>) {
        if !self.video.in_pip {
            self.set_viewer_image(Some(image));
            return;
        }
        if let Some(old) = self.video.pip_frame.replace(image) {
            self.retire_image(old);
        }
    }

    /// Starts the frame ticker only when it is not already running.
    ///
    /// The paused post-seek refresh arms a deadline on every coalesced seek, so
    /// restarting the thread each time would churn. `ticker_active` is the
    /// authoritative running flag (cleared by `stop_video_ticker`).
    fn ensure_video_ticker(&mut self) {
        if self.video.ticker_active.is_none() {
            self.start_video_ticker();
        }
    }

    /// Arms the bounded post-seek frame refresh for a paused player.
    ///
    /// While playing, the normal playback ticker already refreshes frames, so
    /// this is a no-op. Seek code calls it after every real seek (including the
    /// coalesced seeks issued during a scrub drag) so the picture can catch up
    /// without blocking the UI thread.
    pub(super) fn arm_seek_refresh(&mut self) {
        if self.video.is_playing || self.video.player.is_none() {
            return;
        }
        self.video.seek_refresh_until = Some(Instant::now() + SEEK_REFRESH_WINDOW);
        self.video.step_from = None;
        self.ensure_video_ticker();
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
        // A clip that failed to load has nothing to play.
        if self.video_unplayable() {
            return;
        }
        // A play/pause the user chose (Space mid-drag, PiP controls) wins
        // over the scrub's own resume on release.
        self.video.resume_after_scrub = false;
        if let Some(ref mut player) = self.video.player {
            if self.video.is_playing {
                player.pause();
                self.video.is_playing = false;
                self.stop_video_ticker();
                // Pausing is a deliberate stopping point: commit the position
                // through the normal config save path instead of waiting for
                // the next periodic save.
                self.persist_video_resume_now();
                self.save_config();
            } else {
                player.play();
                self.video.is_playing = true;
                // Continuous playback owns frame updates; a paused seek refresh
                // must not race the play ticker.
                self.video.seek_refresh_until = None;
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
            // Still loading: the seek lands when the player does.
            if self.video.item.is_some() && self.video.load_error.is_none() {
                let duration = self.video_scrub_duration();
                let maximum = if duration > 0.0 { duration } else { 3600.0 };
                let target = (self.video.playback_time + delta_secs).clamp(0.0, maximum);
                self.seek_video_to_time(target);
                cx.notify();
            }
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

    /// Test helper: seek to `pct` of the duration (PiP now scrubs through
    /// `begin_video_scrub` like the viewer).
    #[cfg(test)]
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
    pub(crate) fn video_scrub_duration(&self) -> f64 {
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
        let mut seeked = false;
        if let Some(ref mut player) = self.video.player {
            player.seek(target);
            seeked = true;
        } else if self.video.item.is_some() && self.video.load_error.is_none() {
            self.video.pending_load_seek = Some(target);
        }
        if seeked {
            self.arm_seek_refresh();
        }
    }

    /// Seeks and renders the resulting frame. This is the existing click/keyboard
    /// seek behavior and also runs when a scrub drag is committed on release.
    pub(crate) fn seek_video_to_time(&mut self, target: f64) {
        self.video.playback_time = target;
        let mut rendered = false;
        let mut seeked = false;
        let mut frame = None;
        if let Some(ref mut player) = self.video.player {
            player.seek(target);
            seeked = true;
            // Fast path: AVFoundation sometimes has the destination frame
            // ready synchronously (for example when it is already decoded).
            if let Some(img) = player
                .next_frame()
                .and_then(|frame| video_frame_to_gpui_image(frame).ok())
            {
                frame = Some(img);
            }
            if let Some(surface) = player.take_surface() {
                self.video.surface = Some(surface);
            }
        } else if self.video.item.is_some() && self.video.load_error.is_none() {
            self.video.pending_load_seek = Some(target);
        }
        if let Some(img) = frame {
            self.show_video_frame(img);
            rendered = true;
        }
        if rendered {
            self.video.seek_refresh_until = None;
        } else if seeked {
            self.arm_seek_refresh();
        }
    }

    /// Starts a direct scrub at pct (0..=1) and seeks the player there.
    ///
    /// Called on pointer-down over the timeline. The destination is tracked in
    /// scrub_time so the transport can render time/playhead feedback even when
    /// the backend is not loaded (for example while a video is still opening).
    pub fn begin_video_scrub(&mut self, pct: f64, cx: &mut Context<Self>) {
        let target = scrub_target_time(self.video_scrub_duration(), pct);
        // Scrubbing a playing video pauses it (no audio thrash); release
        // resumes. Not toggle_video_play: a press is no stopping point to save.
        if self.video.is_playing {
            if let Some(ref mut player) = self.video.player {
                player.pause();
            }
            self.video.is_playing = false;
            self.stop_video_ticker();
            self.video.resume_after_scrub = true;
        }
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
        if std::mem::take(&mut self.video.resume_after_scrub) && !self.video.is_playing {
            self.toggle_video_play(cx);
        }
        cx.notify();
    }

    /// Steps whole frames at the video's real frame rate, then shows the
    /// frame the player landed on.
    pub fn step_video_frame(&mut self, frames: isize, cx: &mut Context<Self>) {
        // Frame stepping is a paused operation (QuickTime pauses too).
        if self.video.is_playing {
            self.toggle_video_play(cx);
        }
        let Some(player) = self.video.player.as_mut() else {
            return;
        };
        let from = player.position();
        player.step_frames(frames as i32);
        // Stepping is asynchronous in AVFoundation; re-seeking here could undo
        // it, so poll until the position moves off `from` (see the ticker).
        self.video.playback_time = player.position();
        self.arm_seek_refresh();
        self.video.step_from = Some(from);
        cx.notify();
    }

    /// Test helper: the next speed, through the real (saving) path.
    #[cfg(test)]
    pub fn cycle_video_speed(&mut self, cx: &mut Context<Self>) {
        let next = match self.video.playback_speed {
            s if s < 0.75 => 1.0,
            s if s < 1.25 => 1.5,
            s if s < 1.75 => 2.0,
            _ => 0.5,
        };
        self.set_video_speed(next, cx);
    }

    /// Mute or unmute. A clip that failed to load is left alone.
    pub fn toggle_video_mute(&mut self, cx: &mut Context<Self>) {
        if self.video_unplayable() {
            return;
        }
        self.video.is_muted = !self.video.is_muted;
        if let Some(ref mut player) = self.video.player {
            player.set_muted(self.video.is_muted);
        }
        self.config.video_muted = self.video.is_muted;
        self.save_config();
        cx.notify();
    }

    /// Loop on or off. A clip that failed to load is left alone.
    pub fn toggle_video_loop(&mut self, cx: &mut Context<Self>) {
        if self.video_unplayable() {
            return;
        }
        self.video.is_looping = !self.video.is_looping;
        self.sync_player_looping();
        cx.notify();
    }

    pub fn set_video_speed(&mut self, speed: f32, cx: &mut Context<Self>) {
        self.video.playback_speed = speed;
        self.video.speed_menu_open = false;
        if let Some(ref mut player) = self.video.player {
            player.set_rate(speed);
        }
        self.config.video_speed = Some(speed);
        self.save_config();
        cx.notify();
    }

    pub fn toggle_video_speed_menu(&mut self, cx: &mut Context<Self>) {
        self.video.speed_menu_open = !self.video.speed_menu_open;
        self.video.tracks.audio_menu_open = false;
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

impl LuminaApp {
    /// Replaces the shown item with a fresh catalog copy, keeping the size the
    /// player reported for an open video (the catalog has none for videos).
    pub(crate) fn set_selected_from_catalog(&mut self, mut item: MediaItem) {
        if let Some(playing) = self.video.item.as_ref().filter(|v| v.id == item.id) {
            fill_missing_dimensions(&mut item, playing.width, playing.height);
        }
        self.selected_item = Some(item);
    }
}

/// Takes the player's frame size when the catalog has none.
pub(super) fn fill_missing_dimensions(item: &mut MediaItem, width: u32, height: u32) {
    if (item.width == 0 || item.height == 0) && width > 0 && height > 0 {
        item.width = width;
        item.height = height;
    }
}

#[cfg(test)]
mod dimension_tests {
    use super::fill_missing_dimensions;

    #[core::prelude::v1::test]
    fn player_dimensions_fill_only_missing_ones() {
        let mut item = crate::app::types::get_sample_items()
            .into_iter()
            .find(|i| i.media_type == lumina_core::MediaType::Video)
            .expect("the sample set has a video");
        item.width = 0;
        item.height = 0;
        fill_missing_dimensions(&mut item, 0, 0);
        assert_eq!(
            (item.width, item.height),
            (0, 0),
            "no size from the player keeps —"
        );
        fill_missing_dimensions(&mut item, 1920, 1080);
        assert_eq!(
            (item.width, item.height),
            (1920, 1080),
            "missing dims are filled"
        );
        fill_missing_dimensions(&mut item, 640, 480);
        assert_eq!(
            (item.width, item.height),
            (1920, 1080),
            "catalog dims are never overwritten"
        );
    }
}
