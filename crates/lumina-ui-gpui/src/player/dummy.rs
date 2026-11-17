use super::backend::{VideoFrame, VideoMetadata, VideoPlayerBackend};
use anyhow::{anyhow, Result};
use std::path::Path;

pub struct DummyPlayer;

impl DummyPlayer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoPlayerBackend for DummyPlayer {
    fn load(&mut self, _path: &Path) -> Result<VideoMetadata> {
        Err(anyhow!("Video playback not supported on this platform"))
    }
    fn play(&mut self) {}
    fn pause(&mut self) {}
    fn is_playing(&self) -> bool {
        false
    }
    fn seek(&mut self, _seconds: f64) {}
    fn position(&self) -> f64 {
        0.0
    }
    fn set_rate(&mut self, _rate: f32) {}
    fn rate(&self) -> f32 {
        1.0
    }
    fn set_volume(&mut self, _volume: f32) {}
    fn volume(&self) -> f32 {
        1.0
    }
    fn set_muted(&mut self, _muted: bool) {}
    fn is_muted(&self) -> bool {
        false
    }
    fn set_looping(&mut self, _looping: bool) {}
    fn is_looping(&self) -> bool {
        false
    }
    fn next_frame(&mut self) -> Option<VideoFrame> {
        None
    }
}
