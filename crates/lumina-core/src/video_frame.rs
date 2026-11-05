//! Video thumbnails through AVFoundation, in process (`video_frame_macos.m`).
//!
//! This replaced a `qlmanage` subprocess, which hung until its timeout on
//! some videos and left their cards blank. macOS only for now.

use anyhow::{anyhow, Result};
use std::path::Path;
use std::time::Duration;

use crate::{DecodedFrame, MediaId, PixelFormat};

#[cfg(target_os = "macos")]
mod ffi {
    use std::os::raw::{c_char, c_double, c_int};

    extern "C" {
        pub fn lumina_video_frame(
            path: *const c_char,
            max_size: c_int,
            timeout_seconds: c_double,
            out_width: *mut c_int,
            out_height: *mut c_int,
            out_data: *mut *mut u8,
        ) -> c_int;
        pub fn lumina_video_frame_free(data: *mut u8);
    }
}

/// A frame of the video at `path`, fitted inside `target_size` square, as
/// BGRA8. Gives up after `timeout` so a stuck decode cannot hold a worker.
#[cfg(target_os = "macos")]
pub(crate) fn decode(
    id: MediaId,
    path: &Path,
    target_size: u32,
    timeout: Duration,
) -> Result<DecodedFrame> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes())?;
    let max_size = i32::try_from(target_size)?;
    let (mut width, mut height, mut data) = (0, 0, std::ptr::null_mut());
    // SAFETY: the path is a valid C string; the out-pointers live for the call.
    let ok = unsafe {
        ffi::lumina_video_frame(
            c_path.as_ptr(),
            max_size,
            timeout.as_secs_f64(),
            &mut width,
            &mut height,
            &mut data,
        )
    };
    if ok == 0 || data.is_null() {
        return Err(anyhow!("AVFoundation could not read a frame"));
    }
    let len = width as usize * height as usize * 4;
    // SAFETY: on success the bridge returns a malloc'd buffer of exactly
    // width * height * 4 bytes, copied here and then freed once.
    let bytes = unsafe {
        let bytes = std::slice::from_raw_parts(data, len).to_vec();
        ffi::lumina_video_frame_free(data);
        bytes
    };
    Ok(DecodedFrame {
        id,
        width: width as u32,
        height: height as u32,
        format: PixelFormat::Bgra8,
        data: bytes,
    })
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn decode(
    _id: MediaId,
    _path: &Path,
    _target_size: u32,
    _timeout: Duration,
) -> Result<DecodedFrame> {
    Err(anyhow!("video thumbnails are only available on macOS"))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../lumina-ui-gpui/tests/fixtures/clip.mp4")
    }

    #[test]
    fn decodes_a_frame_of_a_real_mp4_without_upscaling() {
        let frame = decode(MediaId(7), &fixture(), 320, Duration::from_secs(10))
            .expect("AVFoundation must decode the 64x48 H.264 fixture");
        assert_eq!(frame.id, MediaId(7));
        assert_eq!(
            (frame.width, frame.height),
            (64, 48),
            "a small video keeps its size; it is never upscaled"
        );
        assert_eq!(frame.data.len(), 64 * 48 * 4, "one BGRA pixel per 4 bytes");
        assert!(
            frame.data.as_chunks::<4>().0.iter().all(|px| px[3] == 255),
            "video frames are opaque"
        );
        assert!(
            frame
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .any(|px| px[..3] != [0, 0, 0]),
            "the frame must hold picture data, not an empty buffer"
        );
    }

    #[test]
    fn fits_large_frames_inside_the_target_size() {
        let frame = decode(MediaId(1), &fixture(), 32, Duration::from_secs(10))
            .expect("the fixture must decode at a small size");
        assert_eq!(
            (frame.width, frame.height),
            (32, 24),
            "the frame is fitted inside the target square, keeping 4:3"
        );
    }

    #[test]
    fn a_file_that_is_not_a_video_fails_instead_of_hanging() {
        let dir = std::env::temp_dir().join(format!("lumina_video_frame_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir must be creatable");
        let bogus = dir.join("not-a-video.mp4");
        std::fs::write(&bogus, b"definitely not an mp4").expect("temp file must be writable");
        let started = std::time::Instant::now();
        let result = decode(MediaId(2), &bogus, 320, Duration::from_secs(10));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err(), "garbage bytes must not produce a frame");
        assert!(
            started.elapsed() < Duration::from_secs(9),
            "an unreadable file must fail on its own, not by timing out"
        );
    }
}
