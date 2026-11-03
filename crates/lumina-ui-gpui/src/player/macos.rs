#![allow(dead_code)]
use super::backend::{VideoFrame, VideoMetadata, VideoPlayerBackend};
use anyhow::{anyhow, Result};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::Path;

extern "C" {
    fn lumina_player_create() -> *mut c_void;
    fn lumina_player_destroy(player: *mut c_void);
    fn lumina_player_load(
        player: *mut c_void,
        path: *const c_char,
        out_duration: *mut f64,
        out_width: *mut c_int,
        out_height: *mut c_int,
    ) -> c_int;
    fn lumina_player_play(player: *mut c_void);
    fn lumina_player_pause(player: *mut c_void);
    fn lumina_player_is_playing(player: *mut c_void) -> c_int;
    fn lumina_player_seek(player: *mut c_void, seconds: f64);
    fn lumina_player_step(player: *mut c_void, frames: i32);
    fn lumina_player_get_position(player: *mut c_void) -> f64;
    fn lumina_player_get_duration(player: *mut c_void) -> f64;
    fn lumina_player_set_rate(player: *mut c_void, rate: f32);
    fn lumina_player_get_rate(player: *mut c_void) -> f32;
    fn lumina_player_set_volume(player: *mut c_void, volume: f32);
    fn lumina_player_get_volume(player: *mut c_void) -> f32;
    fn lumina_player_set_muted(player: *mut c_void, muted: c_int);
    fn lumina_player_is_muted(player: *mut c_void) -> c_int;
    fn lumina_player_set_looping(player: *mut c_void, looping: c_int);
    fn lumina_player_is_looping(player: *mut c_void) -> c_int;
    fn lumina_player_copy_frame(
        player: *mut c_void,
        out_width: *mut c_int,
        out_height: *mut c_int,
        out_data: *mut *mut u8,
        out_len: *mut c_int,
    ) -> c_int;
    fn lumina_player_copy_frame_direct(
        player: *mut c_void,
        dst: *mut u8,
        max_len: c_int,
        out_width: *mut c_int,
        out_height: *mut c_int,
        out_len: *mut c_int,
    ) -> c_int;
    fn lumina_player_free_frame(data: *mut u8);
}

pub struct AvFoundationPlayer {
    handle: *mut c_void,
    cached_width: u32,
    cached_height: u32,
}

unsafe impl Send for AvFoundationPlayer {}

impl AvFoundationPlayer {
    pub fn new() -> Self {
        let handle = unsafe { lumina_player_create() };
        Self {
            handle,
            cached_width: 0,
            cached_height: 0,
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

        let ok = unsafe {
            lumina_player_load(
                self.handle,
                c_path.as_ptr(),
                &mut duration,
                &mut width,
                &mut height,
            )
        };

        if ok != 1 {
            return Err(anyhow!("Failed to load video in AVFoundation: {:?}", path));
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

    fn duration(&self) -> f64 {
        unsafe { lumina_player_get_duration(self.handle) }
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

    fn next_frame(&mut self) -> Option<VideoFrame> {
        let expected_len = (self.cached_width as usize)
            .checked_mul(self.cached_height as usize)
            .and_then(|wh| wh.checked_mul(4))
            .unwrap_or(0);
        if expected_len > 0 {
            let mut vec = vec![0u8; expected_len];
            let mut width = 0i32;
            let mut height = 0i32;
            let mut len = 0i32;
            let ok = unsafe {
                lumina_player_copy_frame_direct(
                    self.handle,
                    vec.as_mut_ptr(),
                    vec.len() as c_int,
                    &mut width,
                    &mut height,
                    &mut len,
                )
            };
            if ok == 1 && len > 0 {
                self.cached_width = width as u32;
                self.cached_height = height as u32;
                vec.truncate(len as usize);
                return Some(VideoFrame {
                    width: width as u32,
                    height: height as u32,
                    data: vec,
                });
            }
        }

        let mut width = 0i32;
        let mut height = 0i32;
        let mut data_ptr: *mut u8 = std::ptr::null_mut();
        let mut len = 0i32;

        let ok = unsafe {
            lumina_player_copy_frame(
                self.handle,
                &mut width,
                &mut height,
                &mut data_ptr,
                &mut len,
            )
        };

        if ok != 1 || data_ptr.is_null() || len <= 0 {
            return None;
        }

        let slice = unsafe { std::slice::from_raw_parts(data_ptr, len as usize) };
        let vec = slice.to_vec();
        unsafe {
            lumina_player_free_frame(data_ptr);
        }

        self.cached_width = width as u32;
        self.cached_height = height as u32;

        Some(VideoFrame {
            width: width as u32,
            height: height as u32,
            data: vec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
