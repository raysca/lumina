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
        for chunk in data.chunks_exact_mut(4) {
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
}
