use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use xxhash_rust::xxh3::xxh3_64;

use crate::{DecodedFrame, MediaId, PixelFormat};

#[derive(Clone)]
pub struct DiskCache {
    cache_dir: PathBuf,
    max_bytes: u64,
    /// Bytes written since the last trim, shared by every clone so all decode
    /// workers count toward the same budget.
    written_since_trim: Arc<AtomicU64>,
    /// Set while a trim runs so overlapping trims don't each delete to budget.
    trimming: Arc<AtomicBool>,
}

/// Bump when the on-disk thumbnail encoding changes. v3: sips decodes apply
/// EXIF orientation, so v2 files of rotated photos are stale. v4: decodes
/// are color matched to sRGB, so v3 files of wide-gamut photos are stale.
const CACHE_FORMAT_VERSION: u32 = 4;

/// Default disk budget for thumbnails (1.5 GB).
pub const DEFAULT_MAX_CACHE_BYTES: u64 = 1_500_000_000;
/// Thumbnails older than this are removed on startup (30 days).
pub const DEFAULT_MAX_CACHE_AGE: std::time::Duration =
    std::time::Duration::from_secs(30 * 24 * 60 * 60);

/// Result of a cache cleanup pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CacheCleanupStats {
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub remaining_bytes: u64,
}

impl DiskCache {
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let cache_dir = base.join("lumina").join("thumbnails");
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create disk cache directory at {:?}", cache_dir))?;

        Ok(Self::at(cache_dir, DEFAULT_MAX_CACHE_BYTES))
    }

    pub fn with_directory<P: AsRef<Path>>(dir: P) -> Result<Self> {
        Self::with_budget(dir, DEFAULT_MAX_CACHE_BYTES)
    }

    /// A cache that trims itself back under `max_bytes` while it is written.
    pub fn with_budget<P: AsRef<Path>>(dir: P, max_bytes: u64) -> Result<Self> {
        let cache_dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        Ok(Self::at(cache_dir, max_bytes))
    }

    fn at(cache_dir: PathBuf, max_bytes: u64) -> Self {
        Self {
            cache_dir,
            max_bytes,
            written_since_trim: Arc::new(AtomicU64::new(0)),
            trimming: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn compute_key(
        path: &Path,
        file_size: u64,
        mtime_epoch_ms: i64,
        target_size: u32,
    ) -> String {
        let path_str = path.to_string_lossy();
        let payload = format!(
            "v{}:{}:{}:{}:{}",
            CACHE_FORMAT_VERSION, path_str, file_size, mtime_epoch_ms, target_size
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

        let mut bytes = fs::read(&file_path).ok()?;
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

        let expected_len = width as usize * height as usize * 4;
        if bytes.len() - 12 != expected_len {
            return None;
        }

        let data = bytes.split_off(12);

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
        self.trim_if_due(out.len() as u64);
        Ok(())
    }

    /// Startup cleanup alone let the cache grow without bound during a long
    /// session (each 2048px viewer frame is ~12 MB). Once an eighth of the
    /// budget has been written, trim back under it on the writing (decode)
    /// thread. Exactly one writer wins the reset and runs the trim.
    // ponytail: each trim stats the whole cache dir (~every 190 MB written);
    // keep a running size index if that shows up in profiles.
    fn trim_if_due(&self, written: u64) {
        let total = self
            .written_since_trim
            .fetch_add(written, Ordering::Relaxed)
            + written;
        let trim_every = (self.max_bytes / 8).max(1);
        if total < trim_every
            || self
                .written_since_trim
                .compare_exchange(total, 0, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
        {
            return;
        }
        if self.trimming.swap(true, Ordering::Acquire) {
            return;
        }
        if let Err(err) = self.cleanup(self.max_bytes, DEFAULT_MAX_CACHE_AGE) {
            log::warn!("thumbnail cache trim failed: {err:?}");
        }
        self.trimming.store(false, Ordering::Release);
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

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Removes stale/oversized thumbnails. Entries older than `max_age` are
    /// deleted, then the oldest remaining entries are evicted until the cache is
    /// under `max_bytes`.
    pub fn cleanup(
        &self,
        max_bytes: u64,
        max_age: std::time::Duration,
    ) -> Result<CacheCleanupStats> {
        if !self.cache_dir.exists() {
            return Ok(CacheCleanupStats::default());
        }

        let now = std::time::SystemTime::now();
        let mut files = Vec::new();
        let mut stats = CacheCleanupStats::default();

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_thumbnail = name.ends_with(".bin");
            let is_temp = name.contains(".tmp.");
            if !is_thumbnail && !is_temp {
                continue;
            }

            let age = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();

            // Temp files older than an hour are leftovers from a crash.
            let stale = age > max_age || (is_temp && age > std::time::Duration::from_secs(3600));
            if stale {
                if fs::remove_file(&path).is_ok() {
                    stats.removed_files += 1;
                    stats.removed_bytes += metadata.len();
                }
            } else if is_thumbnail {
                files.push((metadata.modified().ok(), metadata.len(), path));
            }
        }

        files.sort_by_key(|(modified, _, _)| *modified);
        let mut total: u64 = files.iter().map(|(_, len, _)| *len).sum();
        for (_, len, path) in files {
            if total <= max_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                stats.removed_files += 1;
                stats.removed_bytes += len;
                total = total.saturating_sub(len);
            }
        }
        stats.remaining_bytes = total;
        Ok(stats)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_cache(name: &str) -> DiskCache {
        let dir = std::env::temp_dir().join(format!(
            "lumina_cache_test_{}_{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        DiskCache::with_directory(dir).unwrap()
    }

    fn frame() -> DecodedFrame {
        DecodedFrame {
            id: MediaId(1),
            width: 2,
            height: 2,
            format: PixelFormat::Rgba8,
            data: vec![0u8; 16],
        }
    }

    /// Regression: the budget was only enforced at startup, so paging the
    /// viewer grew the cache by ~6 MB per photo for the whole session.
    #[test]
    fn writes_keep_the_cache_within_budget_during_a_session() {
        let budget = 8 * 1024;
        let dir = temp_cache("budget").cache_dir().to_path_buf();
        let cache = DiskCache::with_budget(&dir, budget).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(dir.clone());
        let big = DecodedFrame {
            width: 16,
            height: 16,
            data: vec![0u8; 16 * 16 * 4],
            ..frame()
        };
        for i in 0..64 {
            cache
                .put_cached(
                    std::path::Path::new(&format!("/photos/{i}.jpg")),
                    10,
                    1000,
                    2048,
                    &big,
                )
                .unwrap();
        }
        let on_disk: u64 = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .sum();
        let written = 64 * (12 + big.data.len() as u64);
        assert!(
            on_disk <= budget + budget / 8 + 12 + big.data.len() as u64,
            "cache must be trimmed while written: {on_disk} bytes on disk of {written} written, budget {budget}"
        );
    }

    #[test]
    fn cleanup_evicts_to_size_limit() {
        let cache = temp_cache("size");
        for i in 0..3 {
            cache
                .put_cached(
                    std::path::Path::new(&format!("/photos/{i}.jpg")),
                    10,
                    1000,
                    256,
                    &frame(),
                )
                .unwrap();
        }
        let stats = cache
            .cleanup(0, std::time::Duration::from_secs(3600))
            .unwrap();
        assert_eq!(stats.removed_files, 3);
        assert_eq!(stats.remaining_bytes, 0);
        assert!(std::fs::read_dir(cache.cache_dir())
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn cleanup_removes_stale_entries_but_keeps_fresh_ones() {
        let cache = temp_cache("age");
        let cold = cache.cache_dir().join("cold.bin");
        let warm = cache.cache_dir().join("warm.bin");
        std::fs::write(&cold, vec![0u8; 8]).unwrap();
        std::fs::write(&warm, vec![0u8; 8]).unwrap();

        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        std::fs::File::options()
            .write(true)
            .open(&cold)
            .unwrap()
            .set_modified(old)
            .unwrap();

        let stats = cache
            .cleanup(1024 * 1024, std::time::Duration::from_secs(60))
            .unwrap();
        assert_eq!(stats.removed_files, 1);
        assert!(!cold.exists());
        assert!(warm.exists());
        assert_eq!(stats.remaining_bytes, 8);
    }
}
