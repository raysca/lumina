#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageDecoder};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

use crate::{DecodedFrame, MediaId, MediaType, PixelFormat};

#[cfg(target_os = "macos")]
mod imageio;

/// RAII guard that removes a file on drop.
#[cfg(any(target_os = "linux", test))]
struct TempFile(std::path::PathBuf);
#[cfg(any(target_os = "linux", test))]
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// RAII guard that removes a directory tree on drop, or at process exit
/// when its worker never gets to drop it (killed mid-decode by the exit).
#[cfg(any(target_os = "macos", test))]
struct TempDir(std::path::PathBuf);
#[cfg(any(target_os = "macos", test))]
impl TempDir {
    /// Guards `path`; create the directory after this, so no exit between
    /// the two can leave it behind.
    fn new(path: std::path::PathBuf) -> Self {
        live_children::add_dir(&path);
        Self(path)
    }
}
#[cfg(any(target_os = "macos", test))]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        live_children::remove_dir(&self.0);
    }
}

/// Whether `decode_image` tries the scaled native decoder (ImageIO on macOS,
/// ffmpeg on Linux) before the in-process Rust decoders, given the
/// lowercase extension and the file size.
///
/// TIFFs always go native: the image crate's TIFF ICC read can come back
/// empty (it did for a sips-written Display P3 TIFF), so its decode would
/// show a wide-gamut TIFF unconverted; ImageIO colour matches (and applies
/// the orientation). Large files (> 8 MB) go native so their full-resolution
/// pixels are never held in memory. On macOS a large JPEG stays with zune:
/// `decode_jpeg_simd` already hands oversized JPEGs to ImageIO. Linux has no
/// such cap in the JPEG path, so large JPEGs go native there too.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn uses_scaled_native_decode(ext: &str, len: u64) -> bool {
    let is_tiff = ext == "tif" || ext == "tiff";
    let large = len > 8 * 1024 * 1024;
    #[cfg(target_os = "macos")]
    let large = large && !(ext == "jpg" || ext == "jpeg");
    is_tiff || large
}

/// Most bytes of a non-TIFF file read to find its EXIF orientation.
#[cfg(any(target_os = "linux", test))]
const EXIF_READ_LIMIT: u64 = 8 * 1024 * 1024;

/// The Orientation tag (0x0112) of a classic TIFF's IFD0, reading only the
/// 8-byte header and IFD0's entries: 1 when IFD0 has none or cannot be
/// read. `None` when `file` is not a classic TIFF (BigTIFF included).
#[cfg(any(target_os = "linux", test))]
fn tiff_ifd0_orientation(file: &mut (impl std::io::Read + std::io::Seek)) -> Option<u32> {
    use std::io::SeekFrom;
    let mut header = [0u8; 8];
    file.read_exact(&mut header).ok()?;
    let big_endian = match &header[..4] {
        b"MM\0\x2a" => true,
        b"II\x2a\0" => false,
        _ => return None,
    };
    let u16_at = |b: &[u8]| {
        let pair = [b[0], b[1]];
        if big_endian {
            u16::from_be_bytes(pair)
        } else {
            u16::from_le_bytes(pair)
        }
    };
    let quad = [header[4], header[5], header[6], header[7]];
    let ifd0 = if big_endian {
        u32::from_be_bytes(quad)
    } else {
        u32::from_le_bytes(quad)
    };
    let mut read_entries = || {
        file.seek(SeekFrom::Start(ifd0.into())).ok()?;
        let mut count = [0u8; 2];
        file.read_exact(&mut count).ok()?;
        // At most 65535 entries of 12 bytes: under 800 KB.
        let mut entries = vec![0u8; usize::from(u16_at(&count)) * 12];
        file.read_exact(&mut entries).ok()?;
        Some(entries)
    };
    // Orientation is a SHORT (type 3) held in the entry's value field.
    let orientation = read_entries().and_then(|entries| {
        entries
            .as_chunks::<12>()
            .0
            .iter()
            .find(|entry| u16_at(&entry[0..2]) == 0x0112 && u16_at(&entry[2..4]) == 3)
            .map(|entry| u32::from(u16_at(&entry[8..10])))
    });
    Some(orientation.unwrap_or(1))
}

/// A temp path no other decode uses: `prefix`, this process and a counter.
/// The clock alone is not enough (macOS reads it in microseconds, and one
/// item can be decoded twice at once: its grid thumbnail and viewer frame).
fn temp_path(prefix: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{}_{n}", std::process::id()))
}

/// Native helpers (QuickLook, AVFoundation, ffmpeg) must never hang a decode worker.
pub(crate) const NATIVE_TOOL_TIMEOUT: Duration = Duration::from_secs(10);

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<ExitStatus> {
    command.stdout(Stdio::null()).stderr(Stdio::null());
    wait_with_timeout(command.spawn()?, timeout)
}

/// Waits for `child`, killing it after `timeout`.
pub(crate) fn wait_with_timeout(
    child: std::process::Child,
    timeout: Duration,
) -> Result<ExitStatus> {
    wait_with_timeout_inner(child, timeout, None)
}

/// Waits for a child until it exits, times out, or the preview request is cancelled.
#[cfg(target_os = "linux")]
pub(crate) fn wait_with_timeout_or_cancel(
    child: std::process::Child,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicI32,
) -> Result<ExitStatus> {
    wait_with_timeout_inner(child, timeout, Some(cancel))
}

fn wait_with_timeout_inner(
    mut child: std::process::Child,
    timeout: Duration,
    cancel: Option<&std::sync::atomic::AtomicI32>,
) -> Result<ExitStatus> {
    let _reaper = live_children::track(child.id());
    let deadline = Instant::now() + timeout;
    loop {
        let polled = match child.try_wait() {
            Ok(polled) => polled,
            Err(err) => {
                // Not knowing whether it runs is no reason to leave it running.
                let _ = child.kill();
                let _ = child.wait();
                return Err(err.into());
            }
        };
        match polled {
            Some(status) => return Ok(status),
            None if cancel
                .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Acquire) != 0) =>
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!("the preview decode was cancelled"));
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!("native image tool timed out after {:?}", timeout));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

/// The pixel size of an image as macOS reads it (ImageIO), for a file
/// whose catalog size is unknown (an EXIF-less HEIC or RAW). `None` when
/// ImageIO cannot read it.
#[cfg(target_os = "macos")]
pub(crate) fn native_pixel_size(path: &Path) -> Option<(u32, u32)> {
    imageio::pixel_size(path)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn native_pixel_size(_path: &Path) -> Option<(u32, u32)> {
    None
}

/// Kills still-running native tools, and removes the temp dirs of unfinished
/// decodes, when the process exits.
///
/// The timeout above only fires while the waiting worker is alive. If the
/// process exits mid-wait (app quit during a scan, a test finishing), the child
/// is reparented to launchd and qlmanage, which never exits on a file it can't
/// read, would leak forever. An `atexit` hook kills whatever is still tracked
/// and marks the process as exiting: other threads still run during exit,
/// so a tool a worker starts after the hook is killed as it is tracked.
/// Network downloads (curl, yt-dlp) run in their own process group and are
/// tracked by group: the bundled yt-dlp is a PyInstaller launcher whose
/// Python child (and ffmpeg) would survive a kill of the launcher alone.
/// SIGKILL of the parent still leaks; macOS has no parent-death signal.
#[cfg(unix)]
pub(crate) mod live_children {
    use std::collections::HashSet;
    #[cfg(any(target_os = "macos", test))]
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::{Mutex, Once};

    #[derive(Default)]
    struct State {
        /// Positive: one process. Negative: a whole process group.
        pids: HashSet<i32>,
        /// Temp dirs of decodes still running.
        dirs: HashSet<PathBuf>,
        /// The exit hook ran.
        exiting: bool,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);
    static HOOK: Once = Once::new();

    fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        f(state.get_or_insert_with(State::default))
    }

    extern "C" {
        fn atexit(callback: extern "C" fn()) -> i32;
        fn kill(pid: i32, signal: i32) -> i32;
    }

    pub(super) extern "C" fn kill_tracked() {
        with_state(|state| {
            state.exiting = true;
            for &pid in &state.pids {
                // SAFETY: plain syscall on a pid we spawned and have not reaped.
                unsafe { kill(pid, 9) };
            }
            for dir in state.dirs.drain() {
                let _ = std::fs::remove_dir_all(dir);
            }
        });
    }

    pub(crate) struct Tracked(i32);

    impl Drop for Tracked {
        fn drop(&mut self) {
            with_state(|state| state.pids.remove(&self.0));
        }
    }

    fn hook() {
        // SAFETY: registers a function with no captured state.
        HOOK.call_once(|| unsafe {
            atexit(kill_tracked);
        });
    }

    fn insert(id: i32) -> Tracked {
        hook();
        with_state(|state| {
            state.pids.insert(id);
            if state.exiting {
                // SAFETY: as in `kill_tracked`.
                unsafe { kill(id, 9) };
            }
        });
        Tracked(id)
    }

    /// Removes `dir` at exit unless [`remove_dir`] is called first.
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn add_dir(dir: &Path) {
        hook();
        with_state(|state| state.dirs.insert(dir.to_path_buf()));
    }

    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn remove_dir(dir: &Path) {
        with_state(|state| state.dirs.remove(dir));
    }

    /// ponytail: a process that finishes exiting between `spawn` and this
    /// call leaves its tool running; tracking inside the spawn is the upgrade.
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
        with_state(|state| state.pids.contains(&(pid as i32)))
    }
}

#[cfg(not(unix))]
pub(crate) mod live_children {
    pub(crate) fn track(_pid: u32) {}
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn add_dir(_dir: &std::path::Path) {}
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn remove_dir(_dir: &std::path::Path) {}
    pub(crate) fn track_group(_pid: u32) {}
    pub(crate) fn stop_group(child: &mut std::process::Child) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Whether an ICC profile is sRGB, judged by its description ("sRGB
/// IEC61966-2.1", "sRGB built-in", ...). A profile whose description cannot
/// be read counts as not sRGB, so it is color matched rather than shown as
/// is. (A file whose profile the decoder cannot extract at all reports no
/// profile and is treated as untagged, that is, as sRGB.)
#[cfg(any(target_os = "macos", test))]
fn is_srgb_profile(icc: &[u8]) -> bool {
    icc_description(icc).is_some_and(|desc| desc.contains("sRGB"))
}

/// The `desc` tag of an ICC profile: ASCII in a v2 `desc` type, the first
/// UTF-16BE record of a v4 `mluc` type.
#[cfg(any(target_os = "macos", test))]
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

type Job = Box<dyn FnOnce() + Send>;

/// The decode pools. Three lanes share them:
///
/// - `spawn`: first in, first out, for one-off decodes (album covers,
///   batch thumbnails, cards of other surfaces).
/// - `spawn_newest_first`: grid tiles. A fast scroll queues hundreds of
///   tiles; the rows on screen now were asked for last, so they run first
///   instead of waiting behind every row scrolled past.
/// - `spawn_viewer`: the open item's full-size frame on a pool of its own,
///   so it never waits behind a screenful of grid decodes.
#[derive(Clone)]
pub struct DecoderPipeline {
    pool: Arc<ThreadPool>,
    viewer: Arc<ThreadPool>,
    /// Grid jobs, newest last. Each `spawn_newest_first` puts one runner on
    /// the pool, and each runner takes the newest job, so every job runs.
    newest: Arc<parking_lot::Mutex<Vec<Job>>>,
}

/// Threads for the viewer lane: one frame is shown at a time, a second
/// covers a compare pair or the frame after a step.
const VIEWER_THREADS: usize = 2;

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
        let viewer = ThreadPoolBuilder::new()
            .num_threads(VIEWER_THREADS)
            .thread_name(|i| format!("lumina-viewer-decode-{}", i))
            .build()?;

        Ok(Self {
            pool: Arc::new(pool),
            viewer: Arc::new(viewer),
            newest: Arc::default(),
        })
    }

    pub fn spawn<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.pool.spawn(task);
    }

    /// Queues `task` ahead of every grid job still waiting.
    pub fn spawn_newest_first<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.newest.lock().push(Box::new(task));
        let newest = self.newest.clone();
        self.pool.spawn(move || {
            // Bind first: the lock must be released before the job runs.
            let job = newest.lock().pop();
            if let Some(job) = job {
                job();
            }
        });
    }

    /// Runs `task` on the viewer's own pool.
    pub fn spawn_viewer<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.viewer.spawn(task);
    }

    #[cfg(target_os = "linux")]
    fn decode_via_ffmpeg_image(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        let temp_out = temp_path(&format!("lumina_ffmpeg_image_{}", id.0)).with_extension("png");
        let _guard = TempFile(temp_out.clone());
        let status = run_with_timeout(
            // `-noautorotate`: whether ffmpeg honours EXIF orientation varies
            // by version, so it never rotates and the orientation read from
            // the source is applied below, as the in-process path does. Only
            // the file protocol: a preview never touches the network.
            Command::new("ffmpeg")
                .args(["-nostdin", "-v", "error", "-y", "-noautorotate"])
                .args(["-protocol_whitelist", "file", "-i"])
                .arg(path)
                .args(["-frames:v", "1", "-vf"])
                .arg(format!(
                    "scale='min({target_size},iw)':'min({target_size},ih)':force_original_aspect_ratio=decrease"
                ))
                .arg(&temp_out),
            NATIVE_TOOL_TIMEOUT,
        )?;
        if !status.success() {
            return Err(anyhow!("ffmpeg could not decode the image"));
        }
        let decoded = image::load_from_memory(&fs::read(temp_out)?)?;
        let orientation = Self::file_exif_orientation(path);
        let frame = Self::orient(decoded, orientation).to_rgba8();
        Ok(DecodedFrame {
            id,
            width: frame.width(),
            height: frame.height(),
            format: PixelFormat::Rgba8,
            data: frame.into_raw(),
        })
    }

    pub fn decode_image(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let is_jpeg = ext == "jpg" || ext == "jpeg";

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if uses_scaled_native_decode(&ext, fs::metadata(path).map_or(0, |m| m.len())) {
            #[cfg(target_os = "macos")]
            if let Ok(frame) = Self::decode_via_imageio(id, path, target_size) {
                return Ok(frame);
            }
            #[cfg(target_os = "linux")]
            if let Ok(frame) = Self::decode_via_ffmpeg_image(id, path, target_size) {
                return Ok(frame);
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
        if let Some(frame) = Self::color_match_natively(id, path, target_size, icc, width, height) {
            return Ok(frame);
        }
        // `into_decoder` skips the output-buffer guard that `decode()` has;
        // without it a tiny file declaring a huge size is allocated (and an
        // allocation failure aborts the process).
        image::Limits::default().reserve(decoder.total_bytes())?;
        let img = Self::apply_exif_orientation(DynamicImage::from_decoder(decoder)?, &bytes);
        Ok(Self::resize_to_frame(id, img, target_size))
    }

    /// The EXIF Orientation of the JPEG or TIFF in `reader`, 1 when absent.
    fn read_exif_orientation(reader: &mut (impl std::io::BufRead + std::io::Seek)) -> u32 {
        if let Ok(exif) = exif::Reader::new().read_from_container(reader) {
            if let Some(field) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
                if let Some(v) = field.value.get_uint(0) {
                    return v;
                }
            }
        }
        1
    }

    /// The EXIF Orientation of the image file at `path`; 1 when absent or
    /// unreadable.
    ///
    /// A TIFF's IFD0 can sit anywhere in the file, so its orientation is
    /// read by seeking to IFD0 ([`tiff_ifd0_orientation`]), never by
    /// reading the file up to it. Other files are read from at most their
    /// first [`EXIF_READ_LIMIT`] bytes (a JPEG's EXIF sits in its first
    /// 64 KB), since the EXIF reader follows offsets anywhere.
    #[cfg(any(target_os = "linux", test))]
    fn file_exif_orientation(path: &Path) -> u32 {
        use std::io::{Read, Seek};
        let Ok(mut file) = fs::File::open(path) else {
            return 1;
        };
        if let Some(orientation) = tiff_ifd0_orientation(&mut file) {
            return orientation;
        }
        let mut head = Vec::new();
        match file
            .rewind()
            .and_then(|()| file.take(EXIF_READ_LIMIT).read_to_end(&mut head))
        {
            Ok(_) => Self::read_exif_orientation(&mut std::io::Cursor::new(head)),
            Err(_) => 1,
        }
    }

    fn apply_exif_orientation(img: DynamicImage, bytes: &[u8]) -> DynamicImage {
        Self::orient(
            img,
            Self::read_exif_orientation(&mut std::io::Cursor::new(bytes)),
        )
    }

    /// `img` turned for display by an EXIF Orientation value.
    fn orient(img: DynamicImage, orientation: u32) -> DynamicImage {
        match orientation {
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
        // Strict, as zune 0.4 was by default: a damaged file fails over to the
        // image crate (or "No preview") instead of decoding half grey.
        let options =
            zune_jpeg::zune_core::options::DecoderOptions::default().set_strict_mode(true);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
        decoder.decode_headers()?;
        let info = decoder
            .info()
            .ok_or_else(|| anyhow!("Missing JPEG header info"))?;

        let orig_width = info.width as u32;
        let orig_height = info.height as u32;

        // Cap longest edge: over 2560 px, ImageIO decodes it scaled down, so
        // the full-resolution pixels are never held.
        #[cfg(target_os = "macos")]
        if (orig_width > 2560 || orig_height > 2560)
            && (orig_width > target_size || orig_height > target_size)
        {
            if let Ok(frame) = Self::decode_via_imageio(id, path, target_size) {
                return Ok(frame);
            }
        }

        let icc = decoder.icc_profile();
        if let Some(frame) =
            Self::color_match_natively(id, path, target_size, icc, orig_width, orig_height)
        {
            return Ok(frame);
        }

        let pixels = decoder.decode()?;
        // zune-jpeg returns RGB
        let img = image::RgbImage::from_raw(orig_width, orig_height, pixels)
            .ok_or_else(|| anyhow!("Failed to create RgbImage from decoded JPEG bytes"))?;

        let dynamic = Self::apply_exif_orientation(DynamicImage::ImageRgb8(img), bytes);
        Ok(Self::resize_to_frame(id, dynamic, target_size))
    }

    /// The in-process decoders (zune-jpeg, the image crate) ignore color
    /// profiles, so a wide-gamut file (Display P3 from an iPhone or a macOS
    /// screenshot, Adobe RGB from a camera) decoded as if it were sRGB looks
    /// dull. When `icc` is present and not sRGB, decode through ImageIO,
    /// which color matches; `None` (no profile, sRGB, or ImageIO failed)
    /// means decode with zune or the image crate as before.
    fn color_match_natively(
        id: MediaId,
        path: &Path,
        target_size: u32,
        icc: Option<Vec<u8>>,
        width: u32,
        height: u32,
    ) -> Option<DecodedFrame> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (id, path, target_size, icc, width, height);
            None
        }
        #[cfg(target_os = "macos")]
        {
            if icc.is_none_or(|icc| is_srgb_profile(&icc)) {
                return None;
            }
            Self::decode_via_imageio(id, path, target_size.min(width.max(height)))
                .map_err(|err| {
                    log::warn!(
                        "colour conversion failed for {}, showing it unconverted: {err:#}",
                        path.display()
                    );
                })
                .ok()
        }
    }

    /// Decodes through ImageIO, in process: scaled to fit `target_size`
    /// (never enlarged), turned by the EXIF orientation and color matched to
    /// sRGB (frames and the cache hold sRGB; untagged and sRGB sources come
    /// back unchanged).
    #[cfg(target_os = "macos")]
    pub fn decode_via_imageio(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        imageio::decode(id, path, target_size)
    }

    /// A video's thumbnail: a frame decoded in process by AVFoundation.
    pub fn decode_video(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        crate::video_frame::decode(id, path, target_size, NATIVE_TOOL_TIMEOUT)
    }

    /// QuickLook's thumbnail of `path`, for RAW files ImageIO cannot read.
    /// qlmanage hangs on a file it cannot read, so each failure costs the
    /// tool timeout; images and videos never come here.
    #[cfg(target_os = "macos")]
    fn decode_via_quicklook(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        let temp_dir = temp_path(&format!("lumina_thumb_{}", id.0));
        let _guard = TempDir::new(temp_dir.clone());
        fs::create_dir_all(&temp_dir)
            .with_context(|| format!("creating {}", temp_dir.display()))?;

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
                                        return Ok(Self::resize_to_frame(id, img, target_size));
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
            // Every scanned image format is one ImageIO reads, so it covers
            // what QuickLook would; and it fails fast on a broken file,
            // where qlmanage hangs until the tool timeout.
            MediaType::Image => {
                #[cfg(target_os = "linux")]
                if ext == "heic" || ext == "heif" {
                    return Err(anyhow!(
                        "HEIC/HEIF preview decoding is not available on Linux"
                    ));
                }
                #[cfg(target_os = "macos")]
                // No pure-Rust HEIC/HEIF decoder: ImageIO.
                if ext == "heic" || ext == "heif" {
                    return Self::decode_via_imageio(id, path, target_size);
                }
                #[cfg(target_os = "macos")]
                {
                    Self::decode_image(id, path, target_size)
                        .or_else(|_| Self::decode_via_imageio(id, path, target_size))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Self::decode_image(id, path, target_size)
                }
            }
            MediaType::Video => Self::decode_video(id, path, target_size),
            MediaType::Raw => {
                #[cfg(target_os = "linux")]
                return Err(anyhow!("RAW preview decoding is not available on Linux"));
                #[cfg(target_os = "macos")]
                {
                    // ImageIO's RAW rendering first. QuickLook stays as the
                    // fallback: RAW is unchecked on real files, and it may
                    // render cameras ImageIO does not.
                    Self::decode_via_imageio(id, path, target_size)
                        .or_else(|_| Self::decode_via_quicklook(id, path, target_size))
                }
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

    fn resize_to_frame(id: MediaId, img: DynamicImage, target_size: u32) -> DecodedFrame {
        let (w, h) = img.dimensions();
        let resized = if w > target_size || h > target_size {
            // Lanczos3, not Triangle: thumbnails are viewed at or above their
            // decoded size, so bilinear's softness is visible as blur.
            img.resize(target_size, target_size, FilterType::Lanczos3)
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

        DecodedFrame {
            id,
            width,
            height,
            format: PixelFormat::Bgra8,
            data,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
