pub mod backend;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

// Only a Windows port builds this arm. It is kept compiled-out so the
// `VideoPlayerBackend` contract has a no-op implementation to port against;
// no CI job builds it today, so a change there is unverified until that port
// starts (AGENTS §5.4).
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub mod dummy;

pub use backend::{VideoFrame, VideoPlayerBackend, VideoSurface};

#[cfg(target_os = "macos")]
pub type NativeVideoPlayer = macos::AvFoundationPlayer;

#[cfg(target_os = "linux")]
pub type NativeVideoPlayer = linux::GStreamerPlayer;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub type NativeVideoPlayer = dummy::DummyPlayer;

pub fn create_native_player() -> NativeVideoPlayer {
    NativeVideoPlayer::new()
}
