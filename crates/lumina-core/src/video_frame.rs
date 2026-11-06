//! Video thumbnails through AVFoundation, in process (`video_frame_macos.m`).
//!
//! This replaced a `qlmanage` subprocess, which hung until its timeout on
//! some videos and left their cards blank. macOS only for now.

use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;
use std::sync::atomic::AtomicI32;
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
        pub fn lumina_video_frames(
            path: *const c_char,
            max_size: c_int,
            times: *const c_double,
            count: c_int,
            tolerance_s: c_double,
            timeout_s: c_double,
            cancel: *const i32,
            out: *mut FrameOut,
        ) -> c_int;
        pub fn lumina_video_frame_free(data: *mut u8);
    }

    /// `LuminaFrameOut` in `video_frame_macos.m`.
    #[repr(C)]
    pub struct FrameOut {
        pub time: c_double,
        pub width: c_int,
        pub height: c_int,
        pub data: *mut u8,
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

/// Most frames a preview strip holds: one per second of video, up to this.
const STRIP_MAX_FRAMES: usize = 48;

/// Frames spread evenly over a video of `duration_secs`, for the scrubber's
/// hover preview: `clamp(ceil(duration), 2, 48)` of them, each fitted inside
/// `max_size` square, as BGRA8, in ascending time order. Each frame snaps to
/// the nearest keyframe within half the spacing, which keeps one batch fast.
///
/// Set `cancel` to a non-zero value from another thread to stop early; the
/// frames decoded so far are returned. Errors when the duration is not
/// positive and finite, the file cannot be read, or no frame was decoded.
/// Frame ids are `MediaId(0)`: strip frames belong to no grid card.
pub fn video_preview_strip(
    path: &Path,
    duration_secs: f64,
    max_size: u32,
    cancel: &AtomicI32,
) -> Result<Vec<(f64, DecodedFrame)>> {
    if !duration_secs.is_finite() || duration_secs <= 0.0 {
        bail!("a preview strip needs a positive duration, got {duration_secs}");
    }
    let count = (duration_secs.ceil() as usize).clamp(2, STRIP_MAX_FRAMES);
    let spacing = duration_secs / count as f64;
    let times: Vec<f64> = (0..count).map(|i| (i as f64 + 0.5) * spacing).collect();
    decode_at(
        path,
        &times,
        max_size,
        Duration::try_from_secs_f64(spacing / 2.0)
            .with_context(|| format!("duration {duration_secs} s is out of range"))?,
        crate::decoder::NATIVE_TOOL_TIMEOUT,
        cancel,
    )
}

/// Frames of the video at `path` near each of `times` (seconds), within
/// `tolerance` either side, fitted inside `max_size` square, as BGRA8. The
/// whole batch gives up after `timeout`; a non-zero `cancel` stops it early.
/// Returns the frames decoded by then in ascending actual time, or an error
/// when there are none.
#[cfg(target_os = "macos")]
pub(crate) fn decode_at(
    path: &Path,
    times: &[f64],
    max_size: u32,
    tolerance: Duration,
    timeout: Duration,
    cancel: &AtomicI32,
) -> Result<Vec<(f64, DecodedFrame)>> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes())?;
    let max_size = i32::try_from(max_size)?;
    let count = i32::try_from(times.len())?;
    let mut out: Vec<ffi::FrameOut> = (0..times.len())
        .map(|_| ffi::FrameOut {
            time: 0.0,
            width: 0,
            height: 0,
            data: std::ptr::null_mut(),
        })
        .collect();
    // SAFETY: the path is a valid C string, `times` and `out` hold `count`
    // elements, and all three outlive the call. `cancel` points at the
    // AtomicI32's i32 storage (same size and alignment as i32); the bridge
    // only reads it, with an atomic load, and only before it returns, so a
    // concurrent `store` from another thread is not a data race. Late
    // completion callbacks write into a holder the bridge retains, never
    // into `out`.
    let filled = unsafe {
        ffi::lumina_video_frames(
            c_path.as_ptr(),
            max_size,
            times.as_ptr(),
            count,
            tolerance.as_secs_f64(),
            timeout.as_secs_f64(),
            cancel.as_ptr(),
            out.as_mut_ptr(),
        )
    };
    let filled = usize::try_from(filled).unwrap_or(0).min(out.len());
    let frames = out[..filled]
        .iter()
        .map(|slot| {
            let len = slot.width as usize * slot.height as usize * 4;
            // SAFETY: each filled slot holds a malloc'd buffer of exactly
            // width * height * 4 bytes, copied here and then freed once.
            let data = unsafe {
                let bytes = std::slice::from_raw_parts(slot.data, len).to_vec();
                ffi::lumina_video_frame_free(slot.data);
                bytes
            };
            let frame = DecodedFrame {
                id: MediaId(0),
                width: slot.width as u32,
                height: slot.height as u32,
                format: PixelFormat::Bgra8,
                data,
            };
            (slot.time, frame)
        })
        .collect::<Vec<_>>();
    if frames.is_empty() {
        bail!("AVFoundation decoded no frames");
    }
    Ok(frames)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn decode_at(
    _path: &Path,
    _times: &[f64],
    _max_size: u32,
    _tolerance: Duration,
    _timeout: Duration,
    _cancel: &AtomicI32,
) -> Result<Vec<(f64, DecodedFrame)>> {
    Err(anyhow!("video preview strips are only available on macOS"))
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

    /// AVFoundation's first use in a process takes 1-7 s to load. Timed
    /// tests pay that once here so they measure the call, not the cold start.
    fn warm_avfoundation() {
        static WARM: std::sync::Once = std::sync::Once::new();
        WARM.call_once(|| {
            let _ = decode(MediaId(0), &fixture(), 32, Duration::from_secs(10));
        });
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lumina_strip_{tag}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir must be creatable");
        dir
    }

    #[test]
    fn strip_returns_sorted_frames_within_size() {
        let cancel = AtomicI32::new(0);
        let frames = video_preview_strip(&fixture(), 2.0, 32, &cancel)
            .expect("the 2 s H.264 fixture must give a strip");
        assert!(
            (1..=2).contains(&frames.len()),
            "a 2 s video asks for 2 frames, got {}",
            frames.len()
        );
        assert!(
            frames.windows(2).all(|w| w[0].0 <= w[1].0),
            "strip frames come in ascending time order"
        );
        for (time, frame) in &frames {
            assert!(
                (0.0..=2.0).contains(time),
                "frame time {time} must lie inside the 2 s video"
            );
            assert!(
                frame.width.max(frame.height) <= 32,
                "the 64x48 frame must be fitted inside 32 px, got {}x{}",
                frame.width,
                frame.height
            );
            assert_eq!(frame.data.len(), (frame.width * frame.height * 4) as usize);
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
                "a strip frame must hold picture data"
            );
        }
    }

    #[test]
    fn strip_of_missing_file_errors() {
        warm_avfoundation();
        let dir = temp_dir("missing");
        let started = std::time::Instant::now();
        let result = video_preview_strip(&dir.join("nope.mp4"), 2.0, 32, &AtomicI32::new(0));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err(), "a missing file gives no strip");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "a missing file must fail on its own, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn strip_of_undecodable_file_errors_fast() {
        let dir = temp_dir("garbage");
        let bogus = dir.join("vp9-stand-in.mp4");
        // Deterministic noise: no container AVFoundation can read.
        let mut x: u32 = 0x1234_5678;
        let bytes: Vec<u8> = (0..64 * 1024)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        std::fs::write(&bogus, bytes).expect("temp file must be writable");
        warm_avfoundation();
        let started = std::time::Instant::now();
        let result = video_preview_strip(&bogus, 30.0, 256, &AtomicI32::new(0));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err(), "random bytes must not produce a strip");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "an undecodable file must fail fast, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn strip_with_zero_or_nan_duration_errors() {
        let cancel = AtomicI32::new(0);
        for duration in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(
                video_preview_strip(&fixture(), duration, 32, &cancel).is_err(),
                "duration {duration} must be rejected"
            );
        }
    }

    #[test]
    fn strip_cancel_flag_stops_batch_early() {
        warm_avfoundation();
        let cancel = AtomicI32::new(1);
        let started = std::time::Instant::now();
        let result = video_preview_strip(&fixture(), 2.0, 32, &cancel);
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "a cancelled strip must return at once, took {:?}",
            started.elapsed()
        );
        assert!(
            result.map_or(true, |frames| frames.len() < 2),
            "a strip cancelled before it starts must not decode every frame"
        );
    }
}
