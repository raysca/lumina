use anyhow::{anyhow, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zune_jpeg::JpegDecoder;

use crate::{DecodedFrame, MediaId, MediaType, PixelFormat};

/// RAII guard that removes a file on drop.
struct TempFile(std::path::PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// RAII guard that removes a directory tree on drop.
struct TempDir(std::path::PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Native helpers (sips/QuickLook) must never hang a decode worker.
const NATIVE_TOOL_TIMEOUT: Duration = Duration::from_secs(10);

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<ExitStatus> {
    command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn()?;
    let _reaper = live_children::track(child.id());
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!("native image tool timed out after {:?}", timeout));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// Kills still-running native tools when the process exits.
///
/// The timeout above only fires while the waiting worker is alive. If the
/// process exits mid-wait (app quit during a scan, a test finishing), the child
/// is reparented to launchd and qlmanage, which never exits on a file it can't
/// read, would leak forever. An `atexit` hook kills whatever is still tracked.
/// SIGKILL of the parent still leaks; macOS has no parent-death signal.
#[cfg(unix)]
mod live_children {
    use std::collections::HashSet;
    use std::sync::{Mutex, Once};

    static PIDS: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
    static HOOK: Once = Once::new();

    extern "C" {
        fn atexit(callback: extern "C" fn()) -> i32;
        fn kill(pid: i32, signal: i32) -> i32;
    }

    extern "C" fn kill_tracked() {
        let pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
        for &pid in pids.iter().flatten() {
            // SAFETY: plain syscall on a pid we spawned and have not reaped.
            unsafe { kill(pid as i32, 9) };
        }
    }

    pub(super) struct Tracked(u32);

    impl Drop for Tracked {
        fn drop(&mut self) {
            let mut pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
            pids.get_or_insert_with(HashSet::new).remove(&self.0);
        }
    }

    pub(super) fn track(pid: u32) -> Tracked {
        // SAFETY: registers a function with no captured state.
        HOOK.call_once(|| unsafe {
            atexit(kill_tracked);
        });
        let mut pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
        pids.get_or_insert_with(HashSet::new).insert(pid);
        Tracked(pid)
    }

    #[cfg(test)]
    pub(super) fn is_tracked(pid: u32) -> bool {
        let pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
        pids.as_ref().is_some_and(|pids| pids.contains(&pid))
    }
}

#[cfg(not(unix))]
mod live_children {
    pub(super) fn track(_pid: u32) {}
}

#[derive(Clone)]
pub struct DecoderPipeline {
    pool: Arc<ThreadPool>,
}

impl DecoderPipeline {
    pub fn new(num_threads: usize) -> Result<Self> {
        let threads = if num_threads == 0 {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .max(2)
        } else {
            num_threads
        };

        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .thread_name(|i| format!("lumina-decode-{}", i))
            .build()?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    pub fn spawn<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.pool.spawn(task);
    }

    pub fn decode_image(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        let bytes = fs::read(path)?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // High-speed SIMD path for JPEGs
        if ext == "jpg" || ext == "jpeg" {
            if let Ok(frame) = Self::decode_jpeg_simd(id, path, &bytes, target_size) {
                return Ok(frame);
            }
        }

        // For large non-JPEG files (> 8 MB), decode hardware-scaled via sips first to avoid full decode OOM
        if bytes.len() > 8 * 1024 * 1024 {
            if let Ok(frame) = Self::decode_via_sips(id, path, target_size) {
                return Ok(frame);
            }
        }

        // Fallback to image crate
        let img = Self::apply_exif_orientation(image::load_from_memory(&bytes)?, &bytes);
        Self::resize_to_frame(id, img, target_size)
    }

    fn read_exif_orientation(bytes: &[u8]) -> u32 {
        let mut cursor = std::io::Cursor::new(bytes);
        if let Ok(exif) = exif::Reader::new().read_from_container(&mut cursor) {
            if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
                if let Some(v) = field.value.get_uint(0) {
                    return v;
                }
            }
        }
        1
    }

    fn apply_exif_orientation(img: DynamicImage, bytes: &[u8]) -> DynamicImage {
        match Self::read_exif_orientation(bytes) {
            2 => img.fliph(),
            3 => img.rotate180(),
            4 => img.flipv(),
            5 => img.rotate90().fliph(),
            6 => img.rotate90(),
            7 => img.fliph().rotate90(),
            8 => img.rotate270(),
            _ => img,
        }
    }

    fn decode_jpeg_simd(
        id: MediaId,
        path: &Path,
        bytes: &[u8],
        target_size: u32,
    ) -> Result<DecodedFrame> {
        let mut decoder = JpegDecoder::new(bytes);
        decoder.decode_headers()?;
        let info = decoder
            .info()
            .ok_or_else(|| anyhow!("Missing JPEG header info"))?;

        let orig_width = info.width as u32;
        let orig_height = info.height as u32;

        // Cap longest edge: if larger than 2560px, decode scaled image via sips to prevent OOM
        if (orig_width > 2560 || orig_height > 2560)
            && (orig_width > target_size || orig_height > target_size)
        {
            if let Ok(frame) = Self::decode_via_sips(id, path, target_size) {
                return Ok(frame);
            }
        }

        let pixels = decoder.decode()?;
        // zune-jpeg returns RGB
        let img = image::RgbImage::from_raw(orig_width, orig_height, pixels)
            .ok_or_else(|| anyhow!("Failed to create RgbImage from decoded JPEG bytes"))?;

        let dynamic = Self::apply_exif_orientation(DynamicImage::ImageRgb8(img), bytes);
        Self::resize_to_frame(id, dynamic, target_size)
    }

    pub fn decode_via_sips(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_out = std::env::temp_dir().join(format!("lumina_sips_{}_{}.png", id.0, nanos));
        let _guard = TempFile(temp_out.clone());

        let status = run_with_timeout(
            Command::new("sips")
                .arg("-s")
                .arg("format")
                .arg("png")
                .arg("-Z")
                .arg(target_size.to_string())
                .arg(path)
                .arg("--out")
                .arg(&temp_out),
            NATIVE_TOOL_TIMEOUT,
        );

        if let Ok(status) = status {
            if status.success() && temp_out.exists() {
                if let Ok(bytes) = fs::read(&temp_out) {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        return Self::resize_to_frame(id, img, target_size);
                    }
                }
            }
        }
        Err(anyhow!("sips decode failed"))
    }

    pub fn decode_video(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        // Use macOS quicklook thumbnail generator (instant native GPU/AVFoundation thumbnail)
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_dir = std::env::temp_dir().join(format!("lumina_thumb_{}_{}", id.0, nanos));
        let _ = fs::create_dir_all(&temp_dir);
        let _guard = TempDir(temp_dir.clone());

        let size_arg = target_size.max(256).to_string();
        let status = run_with_timeout(
            Command::new("qlmanage")
                .arg("-t")
                .arg("-s")
                .arg(&size_arg)
                .arg("-o")
                .arg(&temp_dir)
                .arg(path),
            NATIVE_TOOL_TIMEOUT,
        );

        if let Ok(status) = status {
            if status.success() {
                // Find generated thumbnail image
                if let Ok(entries) = fs::read_dir(&temp_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if let Some(ext) = p.extension() {
                            if ext == "png" || ext == "jpg" {
                                if let Ok(bytes) = fs::read(&p) {
                                    if let Ok(img) = image::load_from_memory(&bytes) {
                                        return Self::resize_to_frame(id, img, target_size);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow!("QuickLook thumbnail failed"))
    }

    pub fn decode_media(
        id: MediaId,
        media_type: MediaType,
        path: &Path,
        target_size: u32,
    ) -> Result<DecodedFrame> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match media_type {
            MediaType::Image => {
                // HEIC/HEIF formats lack pure-Rust decoding in image crate; use native sips ImageIO hardware decode first
                if ext == "heic" || ext == "heif" {
                    return Self::decode_via_sips(id, path, target_size)
                        .or_else(|_| Self::decode_video(id, path, target_size));
                }
                Self::decode_image(id, path, target_size)
                    .or_else(|_| Self::decode_video(id, path, target_size))
            }
            MediaType::Video => Self::decode_video(id, path, target_size),
            MediaType::Raw => {
                // Try native sips hardware thumbnail first for RAW camera formats to avoid full RAW decode
                Self::decode_via_sips(id, path, target_size)
                    .or_else(|_| Self::decode_video(id, path, target_size))
            }
        }
    }

    /// Rotates a decoded frame clockwise by `quarters` quarter turns.
    ///
    /// This is a display-only remap of the decoded pixel buffer; the media file
    /// is never touched. Frames whose buffer is not a 4-byte-per-pixel layout are
    /// returned unchanged rather than reinterpreted as pixels.
    pub fn rotate_frame(frame: DecodedFrame, quarters: u8) -> DecodedFrame {
        let quarters = crate::normalize_quarter_turns(quarters as i32);
        if quarters == 0 {
            return frame;
        }

        let width = frame.width as usize;
        let height = frame.height as usize;
        const BYTES_PER_PIXEL: usize = 4;
        let expected = width.saturating_mul(height).saturating_mul(BYTES_PER_PIXEL);
        if width == 0 || height == 0 || frame.data.len() != expected {
            return frame;
        }

        let (rotated_width, rotated_height) = if quarters == 2 {
            (width, height)
        } else {
            (height, width)
        };
        let mut data = vec![0u8; expected];
        for y in 0..height {
            for x in 0..width {
                let source = (y * width + x) * BYTES_PER_PIXEL;
                let (target_x, target_y) = match quarters {
                    // Clockwise: the left column becomes the top row.
                    1 => (height - 1 - y, x),
                    // Half turn: both axes mirrored.
                    2 => (width - 1 - x, height - 1 - y),
                    // Counter-clockwise: the top row becomes the left column.
                    _ => (y, width - 1 - x),
                };
                let target = (target_y * rotated_width + target_x) * BYTES_PER_PIXEL;
                data[target..target + BYTES_PER_PIXEL]
                    .copy_from_slice(&frame.data[source..source + BYTES_PER_PIXEL]);
            }
        }

        DecodedFrame {
            width: rotated_width as u32,
            height: rotated_height as u32,
            data,
            ..frame
        }
    }

    fn resize_to_frame(id: MediaId, img: DynamicImage, target_size: u32) -> Result<DecodedFrame> {
        let (w, h) = img.dimensions();
        let resized = if w > target_size || h > target_size {
            img.resize(target_size, target_size, FilterType::Triangle)
        } else {
            img
        };

        let rgba = resized.to_rgba8();
        let (width, height) = rgba.dimensions();
        let mut data = rgba.into_raw();

        // Pre-swizzle RGBA8 -> BGRA8 on the background Rayon thread so DiskCache stores
        // native Metal BGRA8Unorm bytes and UI-thread texture upload is zero-copy.
        for chunk in data.as_chunks_mut::<4>().0 {
            chunk.swap(0, 2);
        }

        Ok(DecodedFrame {
            id,
            width,
            height,
            format: PixelFormat::Bgra8,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: a native tool still running when the process exits must be
    /// killed, not left orphaned (thousands of hung qlmanage processes once
    /// exhausted the per-user process limit).
    #[cfg(unix)]
    #[test]
    fn native_tool_dies_when_the_process_exits() {
        const CHILD_ENV: &str = "LUMINA_NATIVE_TOOL_ORPHAN_PIDFILE";
        if let Ok(pidfile) = std::env::var(CHILD_ENV) {
            let script = format!(
                "echo $$ > '{pidfile}.tmp' && mv '{pidfile}.tmp' '{pidfile}' && exec sleep 30"
            );
            std::thread::spawn(move || {
                let _ = run_with_timeout(
                    Command::new("sh").args(["-c", &script]),
                    Duration::from_secs(60),
                );
            });
            while !Path::new(&pidfile).exists() {
                std::thread::sleep(Duration::from_millis(10));
            }
            // The shell can write its pid before spawn() returns and the pid is
            // tracked; exiting in that gap would test nothing.
            let pid: u32 = fs::read_to_string(&pidfile)
                .expect("tool wrote its pid")
                .trim()
                .parse()
                .expect("pid is a number");
            let deadline = Instant::now() + Duration::from_secs(10);
            while !live_children::is_tracked(pid) {
                if Instant::now() >= deadline {
                    std::process::exit(2);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            std::process::exit(0);
        }

        let pidfile = std::env::temp_dir().join(format!(
            "lumina_orphan_{}_{}.pid",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "decoder::tests::native_tool_dies_when_the_process_exits",
            ])
            .env(CHILD_ENV, &pidfile)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("re-run test binary as the exiting parent");
        assert!(status.success(), "helper process must exit cleanly");
        let pid = fs::read_to_string(&pidfile).expect("helper wrote the tool pid");
        let _ = fs::remove_file(&pidfile);
        let alive = || {
            Command::new("kill")
                .args(["-0", pid.trim()])
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        while alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if alive() {
            let _ = Command::new("kill").args(["-9", pid.trim()]).status();
            panic!(
                "native tool {} outlived the process that spawned it",
                pid.trim()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn native_tool_timeout_kills_slow_child() {
        let started = Instant::now();
        let err = run_with_timeout(
            Command::new("sh").args(["-c", "sleep 5"]),
            Duration::from_millis(100),
        )
        .expect_err("slow command must time out");
        assert!(err.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[cfg(unix)]
    #[test]
    fn native_tool_returns_fast_status() {
        let status = run_with_timeout(
            Command::new("sh").args(["-c", "exit 0"]),
            Duration::from_secs(2),
        )
        .expect("fast command should run");
        assert!(status.success());
    }

    fn test_frame(width: u32, height: u32, pixels: &[[u8; 4]]) -> DecodedFrame {
        let mut data = Vec::with_capacity(pixels.len() * 4);
        for pixel in pixels {
            data.extend_from_slice(pixel);
        }
        DecodedFrame {
            id: MediaId(1),
            width,
            height,
            format: PixelFormat::Bgra8,
            data,
        }
    }

    fn flatten(pixels: &[[u8; 4]]) -> Vec<u8> {
        pixels.iter().flatten().copied().collect()
    }

    #[test]
    fn quarter_turn_normalization_wraps_signed_values() {
        assert_eq!(crate::normalize_quarter_turns(0), 0);
        assert_eq!(crate::normalize_quarter_turns(1), 1);
        assert_eq!(crate::normalize_quarter_turns(3), 3);
        assert_eq!(crate::normalize_quarter_turns(4), 0, "a full turn is 0");
        assert_eq!(
            crate::normalize_quarter_turns(-1),
            3,
            "Rotate Left from 0 must wrap to 3"
        );
        assert_eq!(crate::normalize_quarter_turns(-4), 0);
        assert_eq!(crate::normalize_quarter_turns(7), 3);
        assert_eq!(crate::normalize_quarter_turns(i32::MIN), 0);
    }

    #[test]
    fn rotate_frame_maps_pixels_for_each_quarter_turn() {
        let a = [10, 11, 12, 13];
        let b = [20, 21, 22, 23];
        let c = [30, 31, 32, 33];
        let d = [40, 41, 42, 43];
        let pixels = [a, b, c, d];

        let source = test_frame(2, 2, &pixels);
        let original_data = source.data.clone();
        let right = DecoderPipeline::rotate_frame(source, 1);
        assert_eq!((right.width, right.height), (2, 2));
        assert_eq!(right.data, flatten(&[c, a, d, b]), "clockwise turn");

        let left = DecoderPipeline::rotate_frame(test_frame(2, 2, &pixels), 3);
        assert_eq!(left.data, flatten(&[b, d, a, c]), "counter-clockwise turn");

        let half = DecoderPipeline::rotate_frame(test_frame(2, 2, &pixels), 2);
        assert_eq!(half.data, flatten(&[d, c, b, a]), "half turn");

        let mut repeated = test_frame(2, 2, &pixels);
        for _ in 0..4 {
            repeated = DecoderPipeline::rotate_frame(repeated, 1);
        }
        assert_eq!(repeated.data, original_data, "four turns are the identity");

        let round_trip = DecoderPipeline::rotate_frame(right, 3);
        assert_eq!(
            round_trip.data, original_data,
            "a left turn must undo a right turn"
        );
    }

    #[test]
    fn rotate_frame_swaps_the_long_edge_for_quarter_turns() {
        let wide = test_frame(2, 1, &[[1, 2, 3, 4], [5, 6, 7, 8]]);
        let quarter = DecoderPipeline::rotate_frame(wide, 1);
        assert_eq!(
            (quarter.width, quarter.height),
            (1, 2),
            "a 90° turn must swap width and height"
        );
        assert_eq!(
            quarter.data,
            flatten(&[[1, 2, 3, 4], [5, 6, 7, 8]]),
            "the left pixel becomes the top pixel"
        );

        let half = DecoderPipeline::rotate_frame(
            test_frame(
                3,
                2,
                &[
                    [1, 1, 1, 1],
                    [2, 2, 2, 2],
                    [3, 3, 3, 3],
                    [4, 4, 4, 4],
                    [5, 5, 5, 5],
                    [6, 6, 6, 6],
                ],
            ),
            2,
        );
        assert_eq!((half.width, half.height), (3, 2), "a half turn keeps size");
    }

    #[test]
    fn rotate_frame_leaves_unsupported_buffers_unchanged() {
        let odd = DecodedFrame {
            id: MediaId(1),
            width: 2,
            height: 1,
            format: PixelFormat::Rgba8,
            data: vec![1, 2, 3],
        };
        let rotated = DecoderPipeline::rotate_frame(odd, 1);
        assert_eq!((rotated.width, rotated.height), (2, 1));
        assert_eq!(rotated.data, vec![1, 2, 3]);
    }
}
