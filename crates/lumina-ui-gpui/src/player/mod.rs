pub mod backend;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(not(target_os = "macos"))]
pub mod dummy;

pub use backend::{VideoFrame, VideoPlayerBackend};

#[cfg(target_os = "macos")]
pub type NativeVideoPlayer = macos::AvFoundationPlayer;

#[cfg(not(target_os = "macos"))]
pub type NativeVideoPlayer = dummy::DummyPlayer;

pub fn create_native_player() -> NativeVideoPlayer {
    NativeVideoPlayer::new()
}
