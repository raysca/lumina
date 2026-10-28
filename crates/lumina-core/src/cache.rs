use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

use crate::{DecodedFrame, MediaId, PixelFormat};

#[derive(Clone)]
pub struct DiskCache {
    cache_dir: PathBuf,
}

impl DiskCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let cache_dir = base.join("lumina").join("thumbnails");
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create disk cache directory at {:?}", cache_dir))?;

        Ok(Self { cache_dir })
    }

    pub fn with_directory<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let cache_dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }

    pub fn compute_key(
        path: &Path,
        file_size: u64,
        mtime_epoch_ms: i64,
        target_size: u32,
    ) -> String {
        let path_str = path.to_string_lossy();
        let payload = format!(
            "{}:{}:{}:{}",
            path_str, file_size, mtime_epoch_ms, target_size
        );
        let hash = xxh3_64(payload.as_bytes());
        format!("{:016x}", hash)
    }

    pub fn cache_file_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.bin", key))
    }

    pub fn get_cached(
        &self,
        id: MediaId,
        path: &Path,
        file_size: u64,
        mtime_epoch_ms: i64,
        target_size: u32,
    ) -> Option<DecodedFrame> {
        let key = Self::compute_key(path, file_size, mtime_epoch_ms, target_size);
        let file_path = self.cache_file_path(&key);

        if !file_path.exists() {
            return None;
        }

        let bytes = fs::read(&file_path).ok()?;
        if bytes.len() < 12 {
            return None;
        }

        // Binary header: [u32 width, u32 height, u32 format_tag, raw_rgba_bytes...]
        let width = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        let height = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let format_tag = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let format = if format_tag == 1 {
            PixelFormat::Bgra8
        } else {
            PixelFormat::Rgba8
        };

        let data = bytes[12..].to_vec();
        if data.len() != (width as usize * height as usize * 4) {
            return None;
        }

        Some(DecodedFrame {
            id,
            width,
            height,
            format,
            data,
        })
    }

    pub fn put_cached(
        &self,
        path: &Path,
        file_size: u64,
        mtime_epoch_ms: i64,
        target_size: u32,
        frame: &DecodedFrame,
    ) -> Result<()> {
        let key = Self::compute_key(path, file_size, mtime_epoch_ms, target_size);
        let file_path = self.cache_file_path(&key);

        let mut out = Vec::with_capacity(12 + frame.data.len());
        out.extend_from_slice(&frame.width.to_le_bytes());
        out.extend_from_slice(&frame.height.to_le_bytes());
        let format_tag: u32 = match frame.format {
            PixelFormat::Rgba8 => 0,
            PixelFormat::Bgra8 => 1,
        };
        out.extend_from_slice(&format_tag.to_le_bytes());
        out.extend_from_slice(&frame.data);

        // Atomic write via temp file
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_path =
            self.cache_dir
                .join(format!("{}.tmp.{}.{}", key, std::process::id(), nanos));
        fs::write(&temp_path, &out)?;
        if fs::rename(&temp_path, &file_path).is_err() {
            // rename can fail across filesystem boundaries; fall back to copy + remove
            if let Err(e) = fs::copy(&temp_path, &file_path) {
                let _ = fs::remove_file(&temp_path);
                return Err(e.into());
            }
            let _ = fs::remove_file(&temp_path);
        }
        Ok(())
    }

    pub fn remove_cached(
        &self,
        path: &Path,
        file_size: u64,
        mtime_epoch_ms: i64,
        target_size: u32,
    ) -> bool {
        let key = Self::compute_key(path, file_size, mtime_epoch_ms, target_size);
        let file_path = self.cache_file_path(&key);
        if file_path.exists() {
            fs::remove_file(file_path).is_ok()
        } else {
            false
        }
    }

    pub fn remove_cached_item(&self, path: &Path, file_size: u64, mtime_epoch_ms: i64) {
        for size in [256, 320, 512, 1024, 2048] {
            self.remove_cached(path, file_size, mtime_epoch_ms, size);
        }
    }

    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}
