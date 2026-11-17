//! Video loop range: in and out points for the open video.
//!
//! The range lasts as long as the player session and is never saved. It is
//! enforced on the frame tick (`UiMessage::VideoFrameTick`). While a range is
//! set the player's own end-of-file loop is off: that loop would restart at
//! zero, not at the in point, so the tick owns the end of the file too.

use gpui::*;
use std::time::{Duration, Instant};

use crate::app::types::format_time_fractional;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::player::VideoPlayerBackend;

/// How close to the end of the file counts as the end when no out point is
/// set. A player rarely reports exactly the duration.
const END_TOLERANCE_SECS: f64 = 0.05;

/// How long a wrap seek gets to land before it is issued again.
///
/// The player keeps reporting the old position until a zero-tolerance seek
/// completes, and a new seek cancels the one in flight, so reissuing it early
/// could keep the wrap from ever landing. This is only a watchdog for a seek
/// that never lands: the hold ends as soon as the position is back in range.
const WRAP_SEEK_WATCHDOG: Duration = Duration::from_secs(2);

/// In and out points in seconds. Either can be missing: the start of the
/// video then acts as in, and the end as out.
#[derive(Default)]
pub struct LoopRange {
    pub in_point: Option<f64>,
    pub out_point: Option<f64>,
    /// While a wrap seek is in flight: no new one before this time.
    pub(crate) settle_until: Option<Instant>,
}

impl LoopRange {
    pub fn is_set(&self) -> bool {
        self.in_point.is_some() || self.out_point.is_some()
    }

    fn start(&self) -> f64 {
        self.in_point.unwrap_or(0.0)
    }

    fn end(&self, duration: f64) -> f64 {
        self.out_point.unwrap_or(if duration > 0.0 {
            duration - END_TOLERANCE_SECS
        } else {
            f64::INFINITY
        })
    }
}

impl LuminaApp {
    /// Whether the viewer shows a video the loop commands can act on.
    pub(crate) fn loop_range_applies(&self) -> bool {
        self.is_fullscreen_viewer
            && !self.video.in_pip
            && self
                .selected_item
                .as_ref()
                .is_some_and(|item| item.media_type == lumina_core::MediaType::Video)
    }

    /// Marks the playhead as the loop start, unless it is not before the end.
    pub fn set_loop_in(&mut self, cx: &mut Context<Self>) {
        let at = self.current_player_position();
        if !at.is_finite() {
            return;
        }
        if at >= self.video.loop_range.end(self.video_scrub_duration()) {
            self.show_warning_toast(t("video.loop_start_after_end"), cx);
            return;
        }
        self.video.loop_range.in_point = Some(at);
        self.sync_player_looping();
        self.show_toast(
            t("video.loop_start_set").replace("{time}", &format_time_fractional(at)),
            cx,
        );
        cx.notify();
    }

    /// Marks the playhead as the loop end, unless it is not after the start.
    pub fn set_loop_out(&mut self, cx: &mut Context<Self>) {
        let at = self.current_player_position();
        if !at.is_finite() {
            return;
        }
        if at <= self.video.loop_range.start() {
            self.show_warning_toast(t("video.loop_end_before_start"), cx);
            return;
        }
        self.video.loop_range.out_point = Some(at);
        self.sync_player_looping();
        self.show_toast(
            t("video.loop_end_set").replace("{time}", &format_time_fractional(at)),
            cx,
        );
        cx.notify();
    }

    pub fn clear_loop_range(&mut self, cx: &mut Context<Self>) {
        self.video.loop_range = Default::default();
        self.sync_player_looping();
        cx.notify();
    }

    /// Gives the player its end-of-file loop only when no range owns the end.
    pub(crate) fn sync_player_looping(&mut self) {
        let looping = self.video.is_looping && !self.video.loop_range.is_set();
        if let Some(player) = self.video.player.as_mut() {
            player.set_looping(looping);
        }
    }

    /// Called on each playing tick. `ended` says the player stopped by
    /// itself. Returns true while the range owns what happens next, so the
    /// tick neither ends playback nor paints a frame from outside the range.
    pub(crate) fn enforce_loop_range(&mut self, ended: bool) -> bool {
        let duration = self.video_scrub_duration();
        let position = self.video.playback_time;
        let range = &mut self.video.loop_range;
        if !range.is_set() {
            return false;
        }
        if !(ended || position >= range.end(duration)) {
            // Any wrap seek has landed, so the next wrap need not wait.
            range.settle_until = None;
            return false;
        }
        let now = Instant::now();
        if range.settle_until.is_some_and(|until| now < until) {
            return true;
        }
        range.settle_until = Some(now + WRAP_SEEK_WATCHDOG);
        let start = range.start();
        self.seek_video_to_time(start);
        // A player that reached the end of the file has stopped.
        if let Some(player) = self.video.player.as_mut() {
            player.play();
        }
        true
    }
}
