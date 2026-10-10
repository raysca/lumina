use anyhow::{anyhow, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use zune_jpeg::JpegDecoder;

use crate::{DecodedFrame, MediaId, MediaType, PixelFormat};

pub struct DecoderPipeline {
    pool: Arc<ThreadPool>,
}

impl DecoderPipeline {
    pub fn new(num_threads: usize) -> Result<Self> {
        let threads = if num_threads == 0 {
            num_cpus::get().max(2)
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
            if let Ok(frame) = Self::decode_jpeg_simd(id, &bytes, target_size) {
                return Ok(frame);
            }
        }

        // Fallback to image crate
        let img = image::load_from_memory(&bytes)?;
        Self::resize_to_frame(id, img, target_size)
    }

    fn decode_jpeg_simd(id: MediaId, bytes: &[u8], target_size: u32) -> Result<DecodedFrame> {
        let mut decoder = JpegDecoder::new(bytes);
        decoder.decode_headers()?;
        let info = decoder
            .info()
            .ok_or_else(|| anyhow!("Missing JPEG header info"))?;

        let orig_width = info.width as u32;
        let orig_height = info.height as u32;

        let pixels = decoder.decode()?;
        // zune-jpeg returns RGB
        let img = image::RgbImage::from_raw(orig_width, orig_height, pixels)
            .ok_or_else(|| anyhow!("Failed to create RgbImage from decoded JPEG bytes"))?;

        let dynamic = DynamicImage::ImageRgb8(img);
        Self::resize_to_frame(id, dynamic, target_size)
    }

    pub fn decode_video(id: MediaId, path: &Path, target_size: u32) -> Result<DecodedFrame> {
        // Use macOS quicklook thumbnail generator (instant native GPU/AVFoundation thumbnail)
        let temp_dir = std::env::temp_dir().join(format!("lumina_thumb_{}", id.0));
        let _ = fs::create_dir_all(&temp_dir);

        let size_arg = target_size.max(256).to_string();
        let status = Command::new("qlmanage")
            .arg("-t")
            .arg("-s")
            .arg(&size_arg)
            .arg("-o")
            .arg(&temp_dir)
            .arg(path)
            .output();

        let mut frame_res = Err(anyhow!("QuickLook thumbnail failed"));

        if let Ok(output) = status {
            if output.status.success() {
                // Find generated thumbnail image
                if let Ok(entries) = fs::read_dir(&temp_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if let Some(ext) = p.extension() {
                            if ext == "png" || ext == "jpg" {
                                if let Ok(bytes) = fs::read(&p) {
                                    if let Ok(img) = image::load_from_memory(&bytes) {
                                        frame_res = Self::resize_to_frame(id, img, target_size);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Clean up temporary directory
        let _ = fs::remove_dir_all(&temp_dir);

        frame_res
    }

    pub fn decode_media(
        id: MediaId,
        media_type: MediaType,
        path: &Path,
        target_size: u32,
    ) -> Result<DecodedFrame> {
        match media_type {
            MediaType::Image => Self::decode_image(id, path, target_size),
            MediaType::Video => Self::decode_video(id, path, target_size),
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
        let data = rgba.into_raw();

        Ok(DecodedFrame {
            id,
            width,
            height,
            format: PixelFormat::Rgba8,
            data,
        })
    }
}

mod num_cpus {
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}
