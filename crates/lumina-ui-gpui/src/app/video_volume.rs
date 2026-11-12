//! Video volume: the transport slider, ↑/↓ in the viewer and the palette's
//! Volume Up/Down. The level is kept in the config for the next video.

use gpui::*;

use crate::app::LuminaApp;
use crate::player::VideoPlayerBackend;

/// One ↑/↓ or palette step.
const VOLUME_STEP: f32 = 0.1;

impl LuminaApp {
    /// Sets the volume (clamped to 0..=1) on the player without saving it;
    /// a slider drag calls this on every move.
    pub(crate) fn apply_video_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        if volume.is_nan() {
            return;
        }
        self.video.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = self.video.player.as_mut() {
            player.set_volume(self.video.volume);
        }
        cx.notify();
    }

    /// Sets the volume and saves it, like speed and mute. A save error
    /// shows in the error banner; the player keeps the new level.
    pub fn set_video_volume(&mut self, volume: f32, cx: &mut Context<Self>) {
        self.apply_video_volume(volume, cx);
        self.config.video_volume = Some(self.video.volume);
        self.save_config();
    }

    /// Steps the volume up or down by 0.1 (↑/↓, Volume Up/Down).
    pub fn nudge_video_volume(&mut self, up: bool, cx: &mut Context<Self>) {
        let step = if up { VOLUME_STEP } else { -VOLUME_STEP };
        self.set_video_volume(self.video.volume + step, cx);
    }

    /// Starts a slider drag at `volume`.
    pub(crate) fn begin_volume_drag(&mut self, volume: f32, cx: &mut Context<Self>) {
        self.video.volume_drag = true;
        self.apply_video_volume(volume, cx);
    }

    /// Ends a slider drag and saves the level it left.
    pub(crate) fn end_volume_drag(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.video.volume_drag) {
            self.set_video_volume(self.video.volume, cx);
        }
    }
}
