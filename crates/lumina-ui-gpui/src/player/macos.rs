use super::backend::{VideoFrame, VideoMetadata, VideoPlayerBackend, VideoSurface};
use anyhow::{anyhow, Result};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::Path;
use std::ptr::NonNull;

extern "C" {
    fn lumina_player_create() -> *mut c_void;
    fn lumina_player_destroy(player: *mut c_void);
    fn lumina_player_load(
        player: *mut c_void,
        path: *const c_char,
        wait_secs: f64,
        out_duration: *mut f64,
        out_width: *mut c_int,
        out_height: *mut c_int,
        out_error: *mut *mut c_char,
    ) -> c_int;
    fn lumina_player_take_error(player: *mut c_void) -> *mut c_char;
    fn lumina_player_play(player: *mut c_void);
    fn lumina_player_pause(player: *mut c_void);
    fn lumina_player_is_playing(player: *mut c_void) -> c_int;
    fn lumina_player_seek(player: *mut c_void, seconds: f64);
    fn lumina_player_step(player: *mut c_void, frames: i32);
    fn lumina_player_get_position(player: *mut c_void) -> f64;
    fn lumina_player_set_rate(player: *mut c_void, rate: f32);
    fn lumina_player_set_volume(player: *mut c_void, volume: f32);
    fn lumina_player_set_muted(player: *mut c_void, muted: c_int);
    fn lumina_player_set_looping(player: *mut c_void, looping: c_int);
    fn lumina_player_has_new_frame(player: *mut c_void) -> c_int;
    fn lumina_player_take_surface(player: *mut c_void) -> *mut c_void;
    fn lumina_player_copy_frame(
        player: *mut c_void,
        dst: *mut u8,
        capacity: usize,
        out_width: *mut c_int,
        out_height: *mut c_int,
    ) -> c_int;
    fn lumina_player_audio_track_count(player: *mut c_void) -> c_int;
    fn lumina_player_audio_track_name(player: *mut c_void, index: c_int) -> *mut c_char;
    fn lumina_player_selected_audio_track(player: *mut c_void) -> c_int;
    fn lumina_player_select_audio_track(player: *mut c_void, index: c_int);
    fn lumina_player_chapter_count(player: *mut c_void) -> c_int;
    fn lumina_player_chapter_start(player: *mut c_void, index: c_int) -> f64;
    fn lumina_player_chapter_title(player: *mut c_void, index: c_int) -> *mut c_char;
    fn lumina_player_free_string(s: *mut c_char);
    fn lumina_window_set_up_pip(ns_view: *mut c_void);
}

// Read back only by tests (see `VideoPlayerBackend::rate`).
#[cfg_attr(not(test), allow(dead_code))]
extern "C" {
    fn lumina_player_get_rate(player: *mut c_void) -> f32;
    fn lumina_player_get_volume(player: *mut c_void) -> f32;
    fn lumina_player_is_muted(player: *mut c_void) -> c_int;
    fn lumina_player_is_looping(player: *mut c_void) -> c_int;
}

/// How long a load waits for AVFoundation to say whether the item can play,
/// the same bound as the native tool timeout (`decoder::NATIVE_TOOL_TIMEOUT`).
/// Tests skip the wait: AVFoundation reports the item status on the main
/// queue, which the test harness never runs, so it would always time out.
const READY_WAIT_SECS: f64 = if cfg!(test) { 0.0 } else { 10.0 };

/// Shown in the playback-failed card when decoding fails mid-clip. The
/// AVFoundation error goes to the log only.
const PLAYBACK_FAILED_MESSAGE: &str =
    "This video stopped playing because part of it could not be decoded.";

/// Takes ownership of a malloc'd UTF-8 string from the bridge. Null is "".
fn take_bridge_string(raw: *mut c_char) -> String {
    if raw.is_null() {
        return String::new();
    }
    let text = unsafe { std::ffi::CStr::from_ptr(raw) }
        .to_string_lossy()
        .into_owned();
    unsafe { lumina_player_free_string(raw) };
    text
}

/// Bytes in a tightly packed BGRA frame; None for an empty or absurd size.
fn frame_len(width: u32, height: u32) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)
        .filter(|len| *len > 0)
}

pub struct AvFoundationPlayer {
    handle: *mut c_void,
    cached_width: u32,
    cached_height: u32,
    /// Test-only audio selection: set by `select_audio_track` and reported
    /// by `selected_audio_track`. An unloaded player has no tracks, so UI
    /// tests set or read this instead.
    #[cfg(test)]
    pub test_selected_audio: Option<usize>,
}

unsafe impl Send for AvFoundationPlayer {}

impl AvFoundationPlayer {
    pub fn new() -> Self {
        let handle = unsafe { lumina_player_create() };
        Self {
            handle,
            cached_width: 0,
            cached_height: 0,
            #[cfg(test)]
            test_selected_audio: None,
        }
    }
}

impl Default for AvFoundationPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AvFoundationPlayer {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                lumina_player_destroy(self.handle);
            }
            self.handle = std::ptr::null_mut();
        }
    }
}

impl VideoPlayerBackend for AvFoundationPlayer {
    fn load(&mut self, path: &Path) -> Result<VideoMetadata> {
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|e| anyhow!("Invalid path string: {}", e))?;

        let mut duration = 0.0f64;
        let mut width = 0i32;
        let mut height = 0i32;
        let mut error: *mut c_char = std::ptr::null_mut();

        let ok = unsafe {
            lumina_player_load(
                self.handle,
                c_path.as_ptr(),
                READY_WAIT_SECS,
                &mut duration,
                &mut width,
                &mut height,
                &mut error,
            )
        };

        let error = take_bridge_string(error);
        if ok != 1 {
            if !error.is_empty() {
                log::warn!("AVFoundation could not load {}: {error}", path.display());
            }
            return Err(anyhow!(
                "The system player could not open {}",
                path.display()
            ));
        }

        self.cached_width = width.max(0) as u32;
        self.cached_height = height.max(0) as u32;

        Ok(VideoMetadata {
            duration_secs: duration,
            width: self.cached_width,
            height: self.cached_height,
        })
    }

    fn play(&mut self) {
        unsafe { lumina_player_play(self.handle) };
    }

    fn pause(&mut self) {
        unsafe { lumina_player_pause(self.handle) };
    }

    fn is_playing(&self) -> bool {
        unsafe { lumina_player_is_playing(self.handle) == 1 }
    }

    fn seek(&mut self, seconds: f64) {
        unsafe { lumina_player_seek(self.handle, seconds) };
    }

    fn step_frames(&mut self, frames: i32) {
        unsafe { lumina_player_step(self.handle, frames) };
    }

    fn position(&self) -> f64 {
        unsafe { lumina_player_get_position(self.handle) }
    }

    fn set_rate(&mut self, rate: f32) {
        unsafe { lumina_player_set_rate(self.handle, rate) };
    }

    fn rate(&self) -> f32 {
        unsafe { lumina_player_get_rate(self.handle) }
    }

    fn set_volume(&mut self, volume: f32) {
        unsafe { lumina_player_set_volume(self.handle, volume) };
    }

    fn volume(&self) -> f32 {
        unsafe { lumina_player_get_volume(self.handle) }
    }

    fn set_muted(&mut self, muted: bool) {
        unsafe { lumina_player_set_muted(self.handle, if muted { 1 } else { 0 }) };
    }

    fn is_muted(&self) -> bool {
        unsafe { lumina_player_is_muted(self.handle) == 1 }
    }

    fn set_looping(&mut self, looping: bool) {
        unsafe { lumina_player_set_looping(self.handle, if looping { 1 } else { 0 }) };
    }

    fn is_looping(&self) -> bool {
        unsafe { lumina_player_is_looping(self.handle) == 1 }
    }

    /// Copies the newest snapshot, allocating only when one is ready. The
    /// frame's `Vec` becomes the GPUI image, so each frame needs its own;
    /// it is sized from the last frame and not zeroed (the bridge writes
    /// every byte before it is read).
    fn next_frame(&mut self) -> Option<VideoFrame> {
        if unsafe { lumina_player_has_new_frame(self.handle) } != 1 {
            return None;
        }
        // The second pass covers a frame whose size differs from the last
        // one (the bridge keeps it until a big enough buffer comes).
        for _ in 0..2 {
            let capacity = frame_len(self.cached_width, self.cached_height)?;
            let mut data = Vec::<u8>::with_capacity(capacity);
            let mut width = 0i32;
            let mut height = 0i32;
            let status = unsafe {
                lumina_player_copy_frame(
                    self.handle,
                    data.as_mut_ptr(),
                    capacity,
                    &mut width,
                    &mut height,
                )
            };
            let (width, height) = (width.max(0) as u32, height.max(0) as u32);
            match status {
                1 => {
                    let len = frame_len(width, height)?;
                    // SAFETY: the bridge wrote `len` <= `capacity` bytes.
                    unsafe { data.set_len(len) };
                    self.cached_width = width;
                    self.cached_height = height;
                    return Some(VideoFrame {
                        width,
                        height,
                        data,
                    });
                }
                -1 => {
                    self.cached_width = width;
                    self.cached_height = height;
                }
                _ => return None,
            }
        }
        None
    }

    fn take_surface(&mut self) -> Option<VideoSurface> {
        let raw = unsafe { lumina_player_take_surface(self.handle) };
        // SAFETY: the bridge hands over a +1 retained CVPixelBuffer (or null).
        NonNull::new(raw.cast()).map(|buffer| unsafe { VideoSurface::from_raw(buffer) })
    }

    fn take_playback_error(&mut self) -> Option<String> {
        let raw = unsafe { lumina_player_take_error(self.handle) };
        if raw.is_null() {
            return None;
        }
        log::error!("AVFoundation playback error: {}", take_bridge_string(raw));
        Some(PLAYBACK_FAILED_MESSAGE.to_string())
    }

    fn audio_tracks(&self) -> Vec<String> {
        let count = unsafe { lumina_player_audio_track_count(self.handle) };
        (0..count)
            .map(|i| take_bridge_string(unsafe { lumina_player_audio_track_name(self.handle, i) }))
            .collect()
    }

    fn selected_audio_track(&self) -> Option<usize> {
        #[cfg(test)]
        if self.test_selected_audio.is_some() {
            return self.test_selected_audio;
        }
        usize::try_from(unsafe { lumina_player_selected_audio_track(self.handle) }).ok()
    }

    fn select_audio_track(&mut self, index: usize) {
        #[cfg(test)]
        {
            self.test_selected_audio = Some(index);
        }
        if let Ok(index) = c_int::try_from(index) {
            unsafe { lumina_player_select_audio_track(self.handle, index) };
        }
    }

    fn chapters(&self) -> Vec<(f64, String)> {
        let count = unsafe { lumina_player_chapter_count(self.handle) };
        (0..count)
            .map(|i| unsafe {
                (
                    lumina_player_chapter_start(self.handle, i),
                    take_bridge_string(lumina_player_chapter_title(self.handle, i)),
                )
            })
            .collect()
    }
}

/// Sets up the PiP window: keeps it on screen while Lumina is inactive (GPUI
/// makes floating windows NSPanels, which macOS hides when the app loses
/// focus) and hides the window buttons, since the card draws its own Close.
/// No-op without a native AppKit window (tests).
pub fn set_up_pip_window(window: &gpui::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
        // SAFETY: GPUI hands out its live NSView, borrowed for this call on
        // the main thread; the bridge only reads its window and sets flags.
        unsafe { lumina_window_set_up_pip(appkit.ns_view.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_len_is_tight_bgra_and_rejects_empty_or_overflowing_sizes() {
        assert_eq!(frame_len(1000, 562), Some(1000 * 562 * 4));
        assert_eq!(
            frame_len(0, 1080),
            None,
            "an unknown size asks for no buffer"
        );
        assert_eq!(
            frame_len(u32::MAX, u32::MAX),
            None,
            "an absurd size must not overflow"
        );
    }

    #[test]
    fn test_avfoundation_player_lifecycle() {
        let mut player = AvFoundationPlayer::new();
        assert!(!player.is_playing());
        player.set_volume(0.5);
        assert!((player.volume() - 0.5).abs() < 0.01);
        player.set_rate(1.5);
        assert!((player.rate() - 1.5).abs() < 0.01);
        player.set_looping(true);
        assert!(player.is_looping());
        player.set_looping(false);
        assert!(!player.is_looping());
        player.set_muted(true);
        assert!(player.is_muted());
        player.set_muted(false);
        assert!(!player.is_muted());
    }

    #[test]
    fn test_avfoundation_player_load_and_extract_frame() {
        let Ok(path_str) = std::env::var("LUMINA_TEST_VIDEO") else {
            return;
        };
        let test_video = Path::new(&path_str);
        if !test_video.exists() {
            return;
        }

        let mut player = AvFoundationPlayer::new();
        let meta = player.load(test_video).expect("Load real video failed");
        assert!(meta.duration_secs > 0.0);
        assert!(meta.width > 0);
        assert!(meta.height > 0);

        player.seek(0.1);
        std::thread::sleep(std::time::Duration::from_millis(100));

        let frame = player.next_frame();
        assert!(
            frame.is_some(),
            "Expected decoded video frame from AVFoundation"
        );
        let f = frame.unwrap();
        assert_eq!(f.width, meta.width);
        assert_eq!(f.height, meta.height);
        assert_eq!(f.data.len(), (meta.width * meta.height * 4) as usize);
    }

    /// The bridge on a real file: three 2 s chapters titled One/Two/Three and
    /// two audio tracks, English then French. Make it with any local tool and
    /// run with `--ignored` and LUMINA_TEST_TRACKS_VIDEO set. Track names are
    /// AVFoundation's display names, which come from the track language.
    #[test]
    #[ignore = "needs LUMINA_TEST_TRACKS_VIDEO: two audio tracks and three chapters"]
    fn bridge_reads_audio_tracks_and_chapters_from_a_real_file() {
        let path = std::env::var("LUMINA_TEST_TRACKS_VIDEO")
            .expect("LUMINA_TEST_TRACKS_VIDEO must name the test file");
        let mut player = AvFoundationPlayer::new();
        player.load(Path::new(&path)).expect("the test file loads");

        let tracks = player.audio_tracks();
        assert_eq!(tracks, ["English", "French"], "both audio tracks, named");
        assert_eq!(player.selected_audio_track(), Some(0), "the first plays");
        player.select_audio_track(1);
        // Clear the test record so the read below asks AVFoundation.
        player.test_selected_audio = None;
        assert_eq!(player.selected_audio_track(), Some(1), "the switch lands");

        let chapters = player.chapters();
        let titles: Vec<&str> = chapters.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(titles, ["One", "Two", "Three"], "{chapters:?}");
        for ((start, _), expected) in chapters.iter().zip([0.0, 2.0, 4.0]) {
            assert!((start - expected).abs() < 0.05, "{chapters:?}");
        }
    }
}
