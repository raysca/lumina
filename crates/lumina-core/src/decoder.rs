use anyhow::{anyhow, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageDecoder};
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

/// Native helpers (sips, QuickLook, AVFoundation) must never hang a decode worker.
pub(crate) const NATIVE_TOOL_TIMEOUT: Duration = Duration::from_secs(10);

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<ExitStatus> {
    command.stdout(Stdio::null()).stderr(Stdio::null());
    wait_with_timeout(command.spawn()?, timeout)
}

/// Waits for `child`, killing it after `timeout`.
fn wait_with_timeout(mut child: std::process::Child, timeout: Duration) -> Result<ExitStatus> {
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

/// The pixel size of an image as macOS reads it (`sips -g`), for a file
/// whose catalog size is unknown (an EXIF-less HEIC or RAW). `None` when the
/// tool is missing, fails or times out. The output is two short lines, far
/// below a pipe buffer, so reading it after the exit cannot deadlock.
pub(crate) fn native_pixel_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut child = Command::new("sips")
        .args(["-g", "pixelWidth", "-g", "pixelHeight"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    if !wait_with_timeout(child, NATIVE_TOOL_TIMEOUT)
        .ok()?
        .success()
    {
        return None;
    }
    let mut text = String::new();
    stdout.read_to_string(&mut text).ok()?;
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(key)?.trim().parse::<u32>().ok())
    };
    Some((value("pixelWidth:")?, value("pixelHeight:")?))
}

/// Kills still-running native tools when the process exits.
///
/// The timeout above only fires while the waiting worker is alive. If the
/// process exits mid-wait (app quit during a scan, a test finishing), the child
/// is reparented to launchd and qlmanage, which never exits on a file it can't
/// read, would leak forever. An `atexit` hook kills whatever is still tracked.
/// Network downloads (curl, yt-dlp) run in their own process group and are
/// tracked by group: the bundled yt-dlp is a PyInstaller launcher whose
/// Python child (and ffmpeg) would survive a kill of the launcher alone.
/// SIGKILL of the parent still leaks; macOS has no parent-death signal.
#[cfg(unix)]
pub(crate) mod live_children {
    use std::collections::HashSet;
    use std::sync::{Mutex, Once};

    /// Positive: one process. Negative: a whole process group.
    static PIDS: Mutex<Option<HashSet<i32>>> = Mutex::new(None);
    static HOOK: Once = Once::new();

    extern "C" {
        fn atexit(callback: extern "C" fn()) -> i32;
        fn kill(pid: i32, signal: i32) -> i32;
    }

    extern "C" fn kill_tracked() {
        let pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
        for &pid in pids.iter().flatten() {
            // SAFETY: plain syscall on a pid we spawned and have not reaped.
            unsafe { kill(pid, 9) };
        }
    }

    pub(crate) struct Tracked(i32);

    impl Drop for Tracked {
        fn drop(&mut self) {
            let mut pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
            pids.get_or_insert_with(HashSet::new).remove(&self.0);
        }
    }

    fn insert(id: i32) -> Tracked {
        // SAFETY: registers a function with no captured state.
        HOOK.call_once(|| unsafe {
            atexit(kill_tracked);
        });
        let mut pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
        pids.get_or_insert_with(HashSet::new).insert(id);
        Tracked(id)
    }

    pub(crate) fn track(pid: u32) -> Tracked {
        insert(pid as i32)
    }

    /// Tracks the process group led by `pid` (spawned with `process_group(0)`).
    pub(crate) fn track_group(pid: u32) -> Tracked {
        insert(-(pid as i32))
    }

    /// Stops a child spawned as a group leader and everything it started:
    /// SIGTERM to the group (the PyInstaller launcher forwards it to Python,
    /// which cleans up), up to 2 s to exit, then SIGKILL to what is left.
    pub(crate) fn stop_group(child: &mut std::process::Child) {
        let group = -(child.id() as i32);
        // SAFETY: signals a process group this process created.
        unsafe { kill(group, 15) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // The launcher may be gone while its children are not.
        // SAFETY: as above; ESRCH when the group is already empty is fine.
        unsafe { kill(group, 9) };
        let _ = child.wait();
    }

    #[cfg(test)]
    pub(super) fn is_tracked(pid: u32) -> bool {
        let pids = PIDS.lock().unwrap_or_else(|e| e.into_inner());
        pids.as_ref()
            .is_some_and(|pids| pids.contains(&(pid as i32)))
    }
}

#[cfg(not(unix))]
pub(crate) mod live_children {
    pub(crate) fn track(_pid: u32) {}
    pub(crate) fn track_group(_pid: u32) {}
    pub(crate) fn stop_group(child: &mut std::process::Child) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// The system sRGB profile that `sips -m` matches every decode to.
const SRGB_PROFILE: &str = "/System/Library/ColorSync/Profiles/sRGB Profile.icc";

/// Whether an ICC profile is sRGB, judged by its description ("sRGB
/// IEC61966-2.1", "sRGB built-in", ...). A profile whose description cannot
/// be read counts as not sRGB, so it is color matched rather than shown as
/// is. (A file whose profile the decoder cannot extract at all reports no
/// profile and is treated as untagged, that is, as sRGB.)
fn is_srgb_profile(icc: &[u8]) -> bool {
    icc_description(icc).is_some_and(|desc| desc.contains("sRGB"))
}

/// The `desc` tag of an ICC profile: ASCII in a v2 `desc` type, the first
/// UTF-16BE record of a v4 `mluc` type.
fn icc_description(icc: &[u8]) -> Option<String> {
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(icc.get(at..at + 4)?.try_into().ok()?))
    };
    let count = u32_at(128)? as usize;
    let tag = (0..count.min(256)).find_map(|i| {
        let entry = 132 + 12 * i;
        (icc.get(entry..entry + 4)? == b"desc").then_some(entry)
    })?;
    let start = u32_at(tag + 4)? as usize;
    let body = icc.get(start..start.checked_add(u32_at(tag + 8)? as usize)?)?;
    let field = |at: usize| -> Option<usize> {
        Some(u32::from_be_bytes(body.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    match body.get(..4)? {
        b"desc" => {
            let text = body.get(12..field(8)?.checked_add(12)?)?;
            let text = text.split(|&b| b == 0).next()?;
            Some(String::from_utf8_lossy(text).into_owned())
        }
        b"mluc" if field(8)? > 0 => {
            let (len, offset) = (field(20)?, field(24)?);
            let units: Vec<u16> = body
                .get(offset..offset.checked_add(len)?)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&pair| u16::from_be_bytes(pair))
                .collect();
            String::from_utf16(&units).ok()
        }
        _ => None,
    }
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
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_jpeg = ext == "jpg" || ext == "jpeg";

        // For large non-JPEG files (> 8 MB), decode hardware-scaled via sips first to avoid full decode OOM
        // without reading the entire large file into a heap buffer first.
        if !is_jpeg {
            if let Ok(metadata) = fs::metadata(path) {
                if metadata.len() > 8 * 1024 * 1024 {
                    if let Ok(frame) = Self::decode_via_sips(id, path, target_size) {
                        return Ok(frame);
                    }
                }
            }
        }

        let bytes = fs::read(path)?;

        // High-speed SIMD path for JPEGs
        if is_jpeg {
            if let Ok(frame) = Self::decode_jpeg_simd(id, path, &bytes, target_size) {
                return Ok(frame);
            }
        }

        // Fallback to image crate (PNG, WebP, and JPEGs zune rejects).
        let mut decoder = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()?
            .into_decoder()?;
        let (width, height) = decoder.dimensions();
        let icc = decoder.icc_profile().ok().flatten();
        if let Some(frame) = Self::color_match_via_sips(id, path, target_size, icc, width, height) {
            return Ok(frame);
        }
        // `into_decoder` skips the output-buffer guard that `decode()` has;
        // without it a tiny file declaring a huge size is allocated (and an
        // allocation failure aborts the process).
        image::Limits::default().reserve(decoder.total_bytes())?;
        let img = Self::apply_exif_orientation(DynamicImage::from_decoder(decoder)?, &bytes);
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

        let icc = decoder.icc_profile();
        if let Some(frame) =
            Self::color_match_via_sips(id, path, target_size, icc, orig_width, orig_height)
        {
            return Ok(frame);
        }

        let pixels = decoder.decode()?;
        // zune-jpeg returns RGB
        let img = image::RgbImage::from_raw(orig_width, orig_height, pixels)
            .ok_or_else(|| anyhow!("Failed to create RgbImage from decoded JPEG bytes"))?;

        let dynamic = Self::apply_exif_orientation(DynamicImage::ImageRgb8(img), bytes);
        Self::resize_to_frame(id, dynamic, target_size)
    }

    /// The in-process decoders (zune-jpeg, the image crate) ignore color
    /// profiles, so a wide-gamut file (Display P3 from an iPhone or a macOS
    /// screenshot, Adobe RGB from a camera) decoded as if it were sRGB looks
    /// dull. When `icc` is present and not sRGB, decode through sips, which
    /// color matches; `None` (no profile, sRGB, or sips failed) means decode
    /// in process as before. Not scaled past the image's own size, as the
    /// in-process path never upscales either.
    // ponytail: a sips process plus a PNG round trip costs about 50 ms per
    // decode against about 5 ms for zune at 1600x1200 (release). Upgrade path
    // if wide-gamut libraries feel slow: in-process ImageIO, a CGImageSource
    // drawn into an sRGB CGBitmapContext as `video_frame_macos.m` does.
    fn color_match_via_sips(
        id: MediaId,
        path: &Path,
        target_size: u32,
        icc: Option<Vec<u8>>,
        width: u32,
        height: u32,
    ) -> Option<DecodedFrame> {
        if icc.is_none_or(|icc| is_srgb_profile(&icc)) {
            return None;
        }
        Self::decode_via_sips(id, path, target_size.min(width.max(height)))
            .map_err(|err| {
                log::warn!(
                    "colour conversion failed for {}, showing it unconverted: {err:#}",
                    path.display()
                );
            })
            .ok()
    }

    /// Decodes through macOS `sips`, scaled to fit `target_size` and color
    /// matched to sRGB (frames and the cache hold sRGB; untagged and sRGB
    /// sources come back unchanged).
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
                .arg("-m")
                .arg(SRGB_PROFILE)
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
                        // sips keeps the source pixels unrotated and copies the
                        // orientation into the PNG's eXIf chunk; apply it, as
                        // the in-process decoders do.
                        let img = Self::apply_exif_orientation(img, &bytes);
                        return Self::resize_to_frame(id, img, target_size);
                    }
                }
            }
        }
        Err(anyhow!("sips decode failed"))
    }

    /// A video's thumbnail: a frame decoded in process by AVFoundation.
    pub fn decode_video(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        crate::video_frame::decode(id, path, target_size, NATIVE_TOOL_TIMEOUT)
    }

    /// QuickLook's thumbnail of `path`, for images the decoders above cannot
    /// read. (Videos go through [`Self::decode_video`]: qlmanage hung on some.)
    fn decode_via_quicklook(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
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
                        .or_else(|_| Self::decode_via_quicklook(id, path, target_size));
                }
                Self::decode_image(id, path, target_size)
                    .or_else(|_| Self::decode_via_quicklook(id, path, target_size))
            }
            MediaType::Video => Self::decode_video(id, path, target_size),
            MediaType::Raw => {
                // Try native sips hardware thumbnail first for RAW camera formats to avoid full RAW decode
                Self::decode_via_sips(id, path, target_size)
                    .or_else(|_| Self::decode_via_quicklook(id, path, target_size))
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
pub(crate) mod tests {
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

    /// Regression: videos went to a qlmanage subprocess that hung until its
    /// timeout on some files, leaving their cards blank.
    #[cfg(target_os = "macos")]
    #[test]
    fn videos_decode_in_process_through_avfoundation() {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../lumina-ui-gpui/tests/fixtures/clip.mp4");
        let frame = DecoderPipeline::decode_media(MediaId(3), MediaType::Video, &fixture, 320)
            .expect("a video thumbnail must decode");
        assert_eq!(
            (frame.width, frame.height, frame.format),
            (64, 48, PixelFormat::Bgra8),
            "video frames come back at their own size in the texture format"
        );
    }

    /// A JPEG whose left half is red and right half blue, tagged EXIF
    /// Orientation 6 (display turned 90° clockwise: the left half on top).
    pub(crate) fn orientation_six_jpeg(path: &Path, width: u32, height: u32) {
        let image = image::RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                image::Rgb([230, 20, 20])
            } else {
                image::Rgb([20, 20, 230])
            }
        });
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode_image(&image)
            .expect("fixture encodes");
        // Big-endian TIFF, IFD0 with one entry: Orientation (0x0112) = 6.
        let mut app1 = b"Exif\0\0MM\0\x2a\0\0\0\x08\0\x01".to_vec();
        app1.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0, 0, 0, 0, 0]);
        let mut segment = vec![0xff, 0xe1];
        segment.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        segment.extend_from_slice(&app1);
        // After SOI and the encoder's APP0 (JFIF) segment.
        let app0_end = 4 + u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
        jpeg.splice(app0_end..app0_end, segment);
        fs::write(path, jpeg).expect("fixture writes");
    }

    #[test]
    fn icc_description_reads_v2_desc_and_v4_mluc() {
        let profiles = Path::new("/System/Library/ColorSync/Profiles");
        if profiles.is_dir() {
            // sRGB Profile.icc is a v2 `desc` tag, Display P3.icc a v4 `mluc`.
            let read = |name: &str| fs::read(profiles.join(name)).expect("system ICC profile");
            let srgb = read("sRGB Profile.icc");
            let p3 = read("Display P3.icc");
            assert_eq!(icc_description(&srgb).as_deref(), Some("sRGB IEC61966-2.1"));
            assert_eq!(icc_description(&p3).as_deref(), Some("Display P3"));
            assert!(is_srgb_profile(&srgb), "sRGB is not converted");
            assert!(!is_srgb_profile(&p3), "Display P3 must be converted");
            let desc = 132
                + 12 * (0..)
                    .find(|i| &srgb[132 + 12 * i..136 + 12 * i] == b"desc")
                    .expect("desc tag");
            let be = |at: usize| u32::from_be_bytes(srgb[at..at + 4].try_into().unwrap()) as usize;
            let (body, body_len) = (be(desc + 4), be(desc + 8));
            for len in 0..srgb.len() {
                let parsed = icc_description(&srgb[..len]);
                if len < body + body_len {
                    assert_eq!(
                        parsed, None,
                        "a profile cut to {len} bytes, inside its desc tag, has no description"
                    );
                }
            }
            let mut huge = srgb.clone();
            huge[body + 8..body + 12].copy_from_slice(&u32::MAX.to_be_bytes());
            assert_eq!(icc_description(&huge), None, "a huge desc length");
        }
        assert_eq!(icc_description(b"not a profile"), None);
        assert_eq!(icc_description(&[0xff; 200]), None, "bad tag count");
        assert!(
            !is_srgb_profile(b"not a profile"),
            "an unparseable profile counts as non-sRGB"
        );
    }

    /// `jpeg` with `icc` spliced in as a single APP2 ICC_PROFILE segment.
    fn with_icc(jpeg: &[u8], icc: &[u8]) -> Vec<u8> {
        let mut payload = b"ICC_PROFILE\0\x01\x01".to_vec();
        payload.extend_from_slice(icc);
        let mut segment = vec![0xff, 0xe2];
        segment.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        segment.extend_from_slice(&payload);
        let mut out = jpeg.to_vec();
        out.splice(2..2, segment);
        out
    }

    /// Regression for the sRGB conversion: an untagged or sRGB-tagged JPEG
    /// keeps its pixels (no second conversion, no shift), while a Display P3
    /// one is converted.
    #[cfg(target_os = "macos")]
    #[test]
    fn srgb_and_untagged_jpegs_are_not_converted() {
        let image = image::RgbImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                image::Rgb([200, 60, 40])
            } else {
                image::Rgb([40, 120, 220])
            }
        });
        let mut untagged = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut untagged, 95)
            .encode_image(&image)
            .expect("fixture encodes");
        let profiles = Path::new("/System/Library/ColorSync/Profiles");
        let srgb = with_icc(
            &untagged,
            &fs::read(profiles.join("sRGB Profile.icc")).expect("sRGB profile"),
        );
        let p3 = with_icc(
            &untagged,
            &fs::read(profiles.join("Display P3.icc")).expect("P3 profile"),
        );

        let dir = std::env::temp_dir().join(format!(
            "lumina_icc_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let _guard = TempDir(dir.clone());
        let decode = |name: &str, bytes: &[u8]| {
            let path = dir.join(name);
            fs::write(&path, bytes).expect("fixture writes");
            DecoderPipeline::decode_media(MediaId(1), MediaType::Image, &path, 64)
                .expect("fixture decodes")
        };
        let plain = decode("untagged.jpg", &untagged);
        let tagged = decode("srgb.jpg", &srgb);
        let wide = decode("p3.jpg", &p3);

        // BGRA at the centre of the left half.
        let left = |f: &DecodedFrame| {
            let i = (16 * f.width as usize + 16) * 4;
            [f.data[i + 2], f.data[i + 1], f.data[i]]
        };
        let near = |a: [u8; 3], b: [u8; 3]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 3);
        assert!(
            near(left(&plain), [200, 60, 40]),
            "an untagged JPEG keeps its sRGB values: {:?}",
            left(&plain)
        );
        assert_eq!(
            tagged.data, plain.data,
            "an sRGB-tagged JPEG decodes exactly like the untagged one"
        );
        assert!(
            !near(left(&wide), left(&plain)),
            "the same values tagged Display P3 are converted to sRGB: {:?}",
            left(&wide)
        );
    }

    /// Regression: the image-crate fallback must keep image's 512 MiB output
    /// buffer guard. A tiny WebP whose header declares 16383x16383 (about
    /// 768 MiB of RGB) is refused before any buffer is allocated, rather
    /// than allocated (an abort when the allocation fails).
    #[test]
    fn huge_declared_image_is_refused_before_allocating() {
        // VP8L header: signature 0x2f, then 14-bit width-1 and height-1
        // (both 16382, so 16383 px), no alpha, version 0; a few bytes of
        // bitstream.
        let size_bits: u32 = 16382 | (16382 << 14);
        let mut vp8l = vec![0x2f];
        vp8l.extend_from_slice(&size_bits.to_le_bytes());
        vp8l.extend_from_slice(&[0; 11]);
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&((4 + 8 + vp8l.len()) as u32).to_le_bytes());
        webp.extend_from_slice(b"WEBPVP8L");
        webp.extend_from_slice(&(vp8l.len() as u32).to_le_bytes());
        webp.extend_from_slice(&vp8l);

        let path = std::env::temp_dir().join(format!(
            "lumina_huge_{}_{}.webp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::write(&path, &webp).expect("fixture writes");
        let _guard = TempFile(path.clone());
        let Err(err) = DecoderPipeline::decode_image(MediaId(1), &path, 256) else {
            panic!("a 16383x16383 image must not decode in process");
        };
        assert!(
            matches!(
                err.downcast_ref::<image::ImageError>(),
                Some(image::ImageError::Limits(_))
            ),
            "the output buffer guard refuses it, not a later decode error: {err:#}"
        );
    }

    /// The sRGB match that sips applies to every decode must leave sRGB and
    /// untagged pixels alone (no shift), checked against a lossless source.
    #[cfg(target_os = "macos")]
    #[test]
    fn sips_decode_does_not_shift_srgb_or_untagged_pixels() {
        use image::ImageEncoder;
        let image = image::RgbImage::from_fn(64, 32, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 8 + x) as u8, (255 - x * 3) as u8])
        });
        let encode = |icc: Option<Vec<u8>>| {
            let mut png = Vec::new();
            let mut encoder = image::codecs::png::PngEncoder::new(&mut png);
            if let Some(icc) = icc {
                encoder
                    .set_icc_profile(icc)
                    .expect("PNG takes an ICC profile");
            }
            encoder
                .write_image(&image, 64, 32, image::ExtendedColorType::Rgb8)
                .expect("fixture encodes");
            png
        };
        let srgb =
            fs::read("/System/Library/ColorSync/Profiles/sRGB Profile.icc").expect("sRGB profile");
        let dir = std::env::temp_dir().join(format!(
            "lumina_sips_shift_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let _guard = TempDir(dir.clone());
        for (name, png) in [
            ("untagged.png", encode(None)),
            ("srgb.png", encode(Some(srgb))),
        ] {
            let path = dir.join(name);
            fs::write(&path, png).expect("fixture writes");
            let frame = DecoderPipeline::decode_via_sips(MediaId(1), &path, 64)
                .expect("sips decodes the fixture");
            assert_eq!(
                (frame.width, frame.height),
                (64, 32),
                "{name}: not rescaled"
            );
            let worst = frame
                .data
                .as_chunks::<4>()
                .0
                .iter()
                .zip(image.pixels())
                .map(|(bgra, rgb)| {
                    (0..3)
                        .map(|c| bgra[2 - c].abs_diff(rgb[c]))
                        .max()
                        .unwrap_or(0)
                })
                .max()
                .unwrap_or(0);
            assert!(
                worst <= 2,
                "{name}: the sRGB match must not shift pixels, worst channel off by {worst}"
            );
        }
    }

    /// Regression: the sips path (large JPEGs at small sizes, HEIC, RAW)
    /// returned unrotated pixels while the in-process path applied EXIF
    /// orientation, so a crop picked a different region per decode size.
    #[cfg(target_os = "macos")]
    #[test]
    fn sips_and_in_process_decodes_agree_on_exif_orientation() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_orientation_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("rotated.jpg");
        orientation_six_jpeg(&path, 2800, 1400);

        // 320 goes through sips (over 2560 px), 4096 decodes in process.
        for size in [320, 4096] {
            let frame = DecoderPipeline::decode_media(MediaId(1), MediaType::Image, &path, size)
                .expect("the fixture decodes");
            assert!(
                frame.height > frame.width,
                "orientation 6 displays portrait at size {size}: {}x{}",
                frame.width,
                frame.height
            );
            let top = &frame.data[(frame.width as usize / 2) * 4..][..4];
            assert!(
                top[2] > 150 && top[0] < 100,
                "the left (red) half is on top at size {size}: BGRA {top:?}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
