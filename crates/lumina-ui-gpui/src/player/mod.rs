pub mod backend;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub mod dummy;

pub use backend::{VideoFrame, VideoPlayerBackend};

#[cfg(target_os = "macos")]
pub type NativeVideoPlayer = macos::AvFoundationPlayer;

#[cfg(target_os = "linux")]
pub type NativeVideoPlayer = linux::GStreamerPlayer;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub type NativeVideoPlayer = dummy::DummyPlayer;

pub fn create_native_player() -> NativeVideoPlayer {
    NativeVideoPlayer::new()
}
