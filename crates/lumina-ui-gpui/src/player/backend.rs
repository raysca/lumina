use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
}

pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    /// BGRA8 formatted pixel buffer (directly compatible with GPUI Metal atlas)
    pub data: Vec<u8>,
}

pub trait VideoPlayerBackend: Send {
    fn load(&mut self, path: &Path) -> Result<VideoMetadata>;
    fn play(&mut self);
    fn pause(&mut self);
    fn is_playing(&self) -> bool;
    fn seek(&mut self, seconds: f64);
    /// Steps by whole frames of the video's own rate (negative steps back).
    /// ponytail: the default assumes 30 fps; the macOS player overrides it
    /// with AVPlayerItem's native frame stepping.
    fn step_frames(&mut self, frames: i32) {
        let target = (self.position() + f64::from(frames) / 30.0).max(0.0);
        self.seek(target);
    }
    fn position(&self) -> f64;
    fn set_rate(&mut self, rate: f32);
    fn set_volume(&mut self, volume: f32);
    fn set_muted(&mut self, muted: bool);
    fn set_looping(&mut self, looping: bool);
    // The app keeps its own copy of these settings; the getters read the
    // player back so tests can check what the UI applied.
    #[cfg_attr(not(test), allow(dead_code))]
    fn rate(&self) -> f32;
    #[cfg_attr(not(test), allow(dead_code))]
    fn volume(&self) -> f32;
    #[cfg_attr(not(test), allow(dead_code))]
    fn is_muted(&self) -> bool;
    #[cfg_attr(not(test), allow(dead_code))]
    fn is_looping(&self) -> bool;
    /// Fetches the latest decoded video frame if available.
    fn next_frame(&mut self) -> Option<VideoFrame>;
    /// Display names of the loaded file's alternative audio tracks, in order.
    /// Empty when the file offers no choice. A name can be empty.
    fn audio_tracks(&self) -> Vec<String> {
        Vec::new()
    }
    /// Index into `audio_tracks` of the track that plays, when known.
    fn selected_audio_track(&self) -> Option<usize> {
        None
    }
    /// Plays audio track `index` of `audio_tracks` instead of the current one.
    fn select_audio_track(&mut self, _index: usize) {}
    /// A playback error hit after a successful load (decoding failed
    /// mid-clip), as a sentence for the user, returned once. The UI then
    /// shows the clip as failed, like a failed load.
    fn take_playback_error(&mut self) -> Option<String> {
        None
    }
    /// Chapter starts in seconds with their titles, as the file stores them.
    fn chapters(&self) -> Vec<(f64, String)> {
        Vec::new()
    }
}
