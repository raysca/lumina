use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use xxhash_rust::xxh3::xxh3_64;

use crate::{DecodedFrame, MediaId};

mod codec;

/// Cache entries end in this; `.bin` files are raw entries from format v5
/// and earlier, removed by the next cleanup.
const ENTRY_EXTENSION: &str = "thumb";
const LEGACY_EXTENSION: &str = "bin";

#[derive(Clone)]
pub struct DiskCache {
    /// The thumbnail root: every library's folder lives under it. Cleanup and
    /// item eviction walk this, so the byte budget is global.
    root_dir: PathBuf,
    /// This library's folder; reads and writes stay here.
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
/// v5: the key is `{item_hash}_{size}` so every decoded size of one item can
/// be evicted together (v4 hashed the size in and could not be enumerated).
/// v6: entries are compressed (`codec`) and named `.thumb`.
const CACHE_FORMAT_VERSION: u32 = 6;

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
    /// Where thumbnails live for a normal install, without creating it.
    pub fn default_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("lumina")
            .join("thumbnails")
    }

    /// The folder name for a library, derived from its catalog file. A
    /// non-UTF-8 stem is hashed, so two such catalogs cannot share a folder.
    pub fn library_id(catalog_db: &Path) -> String {
        match catalog_db.file_stem().and_then(|stem| stem.to_str()) {
            Some(stem) if !stem.is_empty() => stem.to_string(),
            _ => {
                let bytes = catalog_db
                    .file_stem()
                    .map(|stem| stem.as_encoded_bytes())
                    .unwrap_or_default();
                format!("id-{:016x}", xxh3_64(bytes))
            }
        }
    }

    /// A library's own cache folder under `root`, named by its catalog file
    /// (`catalog_<hash>`). Deleting the catalog therefore names exactly the
    /// thumbnails to remove, without opening the database.
    pub fn for_library(root: impl AsRef<Path>, library_id: &str) -> Result<Self> {
        debug_assert!(
            !library_id.contains(['/', '\\']) && library_id != "." && library_id != "..",
            "a library id must be one path component: {library_id}"
        );
        let root_dir = root.as_ref().to_path_buf();
        let cache_dir = root_dir.join(library_id);
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("Failed to create disk cache directory at {:?}", cache_dir))?;
        Ok(Self::at(root_dir, cache_dir, DEFAULT_MAX_CACHE_BYTES))
    }

    /// A single flat cache folder, for tests and tools.
    /// Test-support entry point: pins the cache to a temp directory.
    #[doc(hidden)]
    pub fn with_directory<P: AsRef<Path>>(dir: P) -> Result<Self> {
        Self::with_budget(dir, DEFAULT_MAX_CACHE_BYTES)
    }

    /// A cache that trims itself back under `max_bytes` while it is written.
    pub fn with_budget<P: AsRef<Path>>(dir: P, max_bytes: u64) -> Result<Self> {
        let cache_dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&cache_dir)?;
        Ok(Self::at(cache_dir.clone(), cache_dir, max_bytes))
    }

    fn at(root_dir: PathBuf, cache_dir: PathBuf, max_bytes: u64) -> Self {
        Self {
            root_dir,
            cache_dir,
            max_bytes,
            written_since_trim: Arc::new(AtomicU64::new(0)),
            trimming: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The stable hash of one item independent of decoded size. Every cached
    /// size of the item shares this prefix, which is what lets a delete evict
    /// the standard thumbnails *and* the crop-dependent edit decodes together.
    fn item_hash(path: &Path, file_size: u64, mtime_epoch_ms: i64) -> String {
        // Encoded, not lossy: two non-UTF-8 names that look the same when
        // lossily converted must not share thumbnails.
        let path_str = crate::catalog::paths::encode(path);
        let payload = format!(
            "v{}:{}:{}:{}",
            CACHE_FORMAT_VERSION, path_str, file_size, mtime_epoch_ms
        );
        format!("{:016x}", xxh3_64(payload.as_bytes()))
    }

    pub fn compute_key(
        path: &Path,
        file_size: u64,
        mtime_epoch_ms: i64,
        target_size: u32,
    ) -> String {
        format!(
            "{}_{}",
            Self::item_hash(path, file_size, mtime_epoch_ms),
            target_size
        )
    }

    pub fn cache_file_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{key}.{ENTRY_EXTENSION}"))
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

        let bytes = fs::read(&file_path).ok()?;
        let frame = codec::decode(id, &bytes)?;
        // Eviction removes the least recently modified entries first, so a
        // read marks the entry as used. Best-effort: a failed touch only
        // makes the entry look older.
        if let Err(err) = fs::File::options()
            .write(true)
            .open(&file_path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()))
        {
            log::debug!("could not mark {} as used: {err}", file_path.display());
        }
        Some(frame)
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
        // Clear Cache removes the whole root (and with it this library's
        // folder) while the app keeps running; recreate it lazily.
        fs::create_dir_all(&self.cache_dir).with_context(|| {
            format!(
                "creating the thumbnail cache at {}",
                self.cache_dir.display()
            )
        })?;

        let out = codec::encode(frame)?;

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

    /// Removes every cached decode of each item in `items` in one directory
    /// pass, so deleting a large selection does not scan the cache per item.
    /// Best-effort: a cache that cannot be read is left to the age/budget
    /// sweep rather than failing the delete that requested the eviction.
    pub fn remove_cached_items<'a, I>(&self, items: I)
    where
        I: IntoIterator<Item = (&'a Path, u64, i64)>,
    {
        self.remove_cached_items_counted(items);
    }

    /// [`Self::remove_cached_items`], reporting how many cache files went
    /// away: deleting a library tells the user what it removed.
    pub fn remove_cached_items_counted<'a, I>(&self, items: I) -> usize
    where
        I: IntoIterator<Item = (&'a Path, u64, i64)>,
    {
        let prefixes: std::collections::HashSet<String> = items
            .into_iter()
            .map(|(path, file_size, mtime_epoch_ms)| {
                Self::item_hash(path, file_size, mtime_epoch_ms)
            })
            .collect();
        if prefixes.is_empty() {
            return 0;
        }
        let mut removed = 0;
        for path in files_under(&self.root_dir) {
            let name = path.file_name().and_then(|name| name.to_str());
            let Some(name) = name else {
                continue;
            };
            let Some((prefix, rest)) = name.split_once('_') else {
                continue;
            };
            let is_entry = Path::new(rest)
                .extension()
                .is_some_and(|ext| ext == ENTRY_EXTENSION || ext == LEGACY_EXTENSION);
            if is_entry && prefixes.contains(prefix) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        removed
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// The thumbnail root shared by every library.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Removes stale/oversized thumbnails. Entries older than `max_age` are
    /// deleted, then the oldest remaining entries are evicted until the cache is
    /// under `max_bytes`.
    pub fn cleanup(
        &self,
        max_bytes: u64,
        max_age: std::time::Duration,
    ) -> Result<CacheCleanupStats> {
        if !self.root_dir.exists() {
            return Ok(CacheCleanupStats::default());
        }

        let now = std::time::SystemTime::now();
        let mut files = Vec::new();
        let mut stats = CacheCleanupStats::default();

        for path in files_under(&self.root_dir) {
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_temp = name.contains(".tmp.");
            let extension = path.extension().and_then(|ext| ext.to_str());
            let is_thumbnail = !is_temp && extension == Some(ENTRY_EXTENSION);
            let is_legacy = !is_temp && extension == Some(LEGACY_EXTENSION);
            if !is_thumbnail && !is_temp && !is_legacy {
                continue;
            }

            let age = metadata
                .modified()
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();

            // Temp files older than an hour are leftovers from a crash.
            // A raw entry from an older format is never read again.
            let stale = is_legacy
                || age > max_age
                || (is_temp && age > std::time::Duration::from_secs(3600));
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

    /// Removes every library's folder under the root (the app's Clear Cache
    /// removes the root itself). Used by tests and tools.
    pub fn clear(&self) -> Result<()> {
        if self.root_dir.exists() {
            for entry in fs::read_dir(&self.root_dir)? {
                let entry = entry?;
                let path = entry.path();
                if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    let _ = fs::remove_dir_all(path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
        }
        Ok(())
    }
}

/// Every regular file under `root`, recursively. Symlinks are skipped so a
/// cache entry can never point outside the thumbnail directory.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelFormat;

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
        // Noise, so compression cannot shrink the entries below the budget.
        let big = DecodedFrame {
            width: 16,
            height: 16,
            data: (0..16 * 16 * 4u64)
                .map(|i| (xxh3_64(&i.to_le_bytes()) % 255) as u8)
                .collect(),
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
        let entry = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().len())
            .max()
            .unwrap();
        let written = 64 * entry;
        assert!(
            written > 2 * budget,
            "control: the entries overflow the budget ({written} of {budget})"
        );
        assert!(
            on_disk <= budget + budget / 8 + entry,
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
        let cold = cache.cache_dir().join("cold.thumb");
        let warm = cache.cache_dir().join("warm.thumb");
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

    /// Regression: a delete only removed five hardcoded sizes, so a
    /// crop-dependent edit decode (e.g. 640 px) of the same item survived.
    #[test]
    fn remove_cached_item_evicts_every_decoded_size_of_that_item() {
        let cache = temp_cache("remove_all_sizes");
        let item = std::path::Path::new("/photos/a.jpg");
        let other = std::path::Path::new("/photos/b.jpg");
        // 256 is a standard thumbnail; 640 is a plausible edit-decode size.
        for size in [256, 640] {
            cache.put_cached(item, 10, 1000, size, &frame()).unwrap();
        }
        cache.put_cached(other, 10, 1000, 256, &frame()).unwrap();
        assert_eq!(std::fs::read_dir(cache.cache_dir()).unwrap().count(), 3);

        cache.remove_cached_items([(item, 10, 1000)]);

        let left: Vec<String> = std::fs::read_dir(cache.cache_dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            left.len(),
            1,
            "every size of the removed item must go, the other item stays: {left:?}"
        );
        assert!(
            left[0].starts_with(&DiskCache::item_hash(other, 10, 1000)),
            "the surviving file must belong to the other item: {left:?}"
        );
    }

    #[test]
    fn a_write_after_clear_recreates_the_library_folder() {
        let root = std::env::temp_dir().join(format!(
            "lumina_cache_recreate_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = DiskCache::for_library(&root, "catalog_recreate").unwrap();
        let item = std::path::Path::new("/photos/a.jpg");
        cache.put_cached(item, 10, 1000, 256, &frame()).unwrap();
        assert!(cache.cache_dir().is_dir());

        cache.clear().unwrap();
        assert!(
            !cache.cache_dir().exists(),
            "clear removes whole library folders"
        );

        cache.put_cached(item, 10, 1000, 256, &frame()).unwrap();
        assert!(
            cache
                .cache_file_path(&DiskCache::compute_key(item, 10, 1000, 256))
                .exists(),
            "a decode after Clear Cache recreates the folder and caches again"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn per_library_folders_share_the_budget_and_clear() {
        let root = std::env::temp_dir().join(format!(
            "lumina_cache_libraries_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let one = DiskCache::for_library(&root, "catalog_a").unwrap();
        let two = DiskCache::for_library(&root, "catalog_b").unwrap();
        assert_ne!(one.cache_dir(), two.cache_dir());
        for (index, cache) in [&one, &two].into_iter().enumerate() {
            let item = std::path::Path::new("/photos/a.jpg");
            let _ = index;
            cache.put_cached(item, 10, 1000, 256, &frame()).unwrap();
        }

        // The budget covers both libraries, not each folder separately.
        let stats = one.cleanup(1, DEFAULT_MAX_CACHE_AGE).unwrap();
        assert_eq!(
            stats.removed_files, 2,
            "a tiny budget evicts both libraries' thumbnails"
        );
        assert_eq!(std::fs::read_dir(one.cache_dir()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(two.cache_dir()).unwrap().count(), 0);

        two.put_cached(
            std::path::Path::new("/photos/b.jpg"),
            10,
            1000,
            256,
            &frame(),
        )
        .unwrap();
        one.clear().unwrap();
        assert_eq!(
            std::fs::read_dir(&root).unwrap().count(),
            0,
            "clear removes every library folder under the root"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A photo-like opaque frame: smooth gradients with fine detail.
    fn photo_frame(width: u32, height: u32) -> DecodedFrame {
        let data = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    let grain = (xxh3_64(&[x as u8, y as u8]) % 24) as u8;
                    [
                        (x * 255 / width) as u8 / 2 + grain,
                        (y * 255 / height) as u8 / 2 + grain,
                        ((x + y) % 200) as u8,
                        255,
                    ]
                })
            })
            .collect();
        DecodedFrame {
            id: MediaId(1),
            width,
            height,
            format: PixelFormat::Bgra8,
            data,
        }
    }

    /// Entries are stored compressed: a grid thumbnail costs a fraction of
    /// its raw 4 bytes per pixel, and reads back at the same size and format.
    #[test]
    fn entries_are_compressed_and_read_back() {
        let cache = temp_cache("compressed");
        let frame = photo_frame(512, 384);
        let path = std::path::Path::new("/photos/compressed.jpg");
        cache.put_cached(path, 10, 1000, 512, &frame).unwrap();
        let file = cache.cache_file_path(&DiskCache::compute_key(path, 10, 1000, 512));
        let on_disk = std::fs::metadata(&file).unwrap().len();
        let raw = frame.data.len() as u64;
        assert!(
            on_disk * 4 < raw,
            "a cached thumbnail must be compressed: {on_disk} bytes on disk for {raw} raw"
        );
        let read = cache
            .get_cached(MediaId(7), path, 10, 1000, 512)
            .expect("the entry reads back");
        assert_eq!((read.id, read.width, read.height), (MediaId(7), 512, 384));
        assert_eq!(read.format, PixelFormat::Bgra8);
    }

    /// Eviction follows use: reading an entry marks it, so the budget trim
    /// removes the entry nobody looked at, not the one written first.
    #[test]
    fn eviction_removes_the_least_recently_read_entry() {
        let cache = temp_cache("by_use");
        let (first, second) = (
            std::path::Path::new("/photos/first.jpg"),
            std::path::Path::new("/photos/second.jpg"),
        );
        let frame = photo_frame(64, 64);
        for (path, hours) in [(first, 3), (second, 2)] {
            cache.put_cached(path, 10, 1000, 256, &frame).unwrap();
            let file = cache.cache_file_path(&DiskCache::compute_key(path, 10, 1000, 256));
            std::fs::File::options()
                .write(true)
                .open(&file)
                .unwrap()
                .set_modified(
                    std::time::SystemTime::now() - std::time::Duration::from_secs(hours * 3600),
                )
                .unwrap();
        }
        assert!(cache.get_cached(MediaId(1), first, 10, 1000, 256).is_some());
        let one =
            std::fs::metadata(cache.cache_file_path(&DiskCache::compute_key(first, 10, 1000, 256)))
                .unwrap()
                .len();
        cache.cleanup(one, DEFAULT_MAX_CACHE_AGE).unwrap();
        assert!(
            cache.get_cached(MediaId(1), first, 10, 1000, 256).is_some(),
            "the entry just read survives the trim although it was written first"
        );
        assert!(
            cache
                .get_cached(MediaId(1), second, 10, 1000, 256)
                .is_none(),
            "the entry not read since it was written is evicted"
        );
    }

    /// Raw `.bin` entries from format v5 and earlier are never read again
    /// (the key carries the format version) and the next cleanup removes
    /// them, however fresh, instead of leaving them to fill the budget.
    #[test]
    fn the_format_bump_drops_raw_entries_from_older_versions() {
        let cache = temp_cache("legacy");
        let path = std::path::Path::new("/photos/legacy.jpg");
        let key = DiskCache::compute_key(path, 10, 1000, 256);
        let legacy = cache.cache_dir().join(format!("{key}.bin"));
        let mut raw = Vec::new();
        for field in [2u32, 2, 1] {
            raw.extend_from_slice(&field.to_le_bytes());
        }
        raw.extend_from_slice(&[0u8; 16]);
        std::fs::write(&legacy, raw).unwrap();
        assert!(
            cache.get_cached(MediaId(1), path, 10, 1000, 256).is_none(),
            "a raw entry is not read as a v6 entry"
        );
        cache.put_cached(path, 10, 1000, 256, &frame()).unwrap();
        let stats = cache.cleanup(u64::MAX, DEFAULT_MAX_CACHE_AGE).unwrap();
        assert_eq!(stats.removed_files, 1, "only the legacy entry goes");
        assert!(!legacy.exists());
        assert!(cache.get_cached(MediaId(1), path, 10, 1000, 256).is_some());
    }

    /// A damaged entry reads as a miss, so the item is decoded again.
    #[test]
    fn a_corrupt_entry_is_a_miss() {
        let cache = temp_cache("corrupt");
        let path = std::path::Path::new("/photos/corrupt.jpg");
        cache
            .put_cached(path, 10, 1000, 256, &photo_frame(32, 32))
            .unwrap();
        let file = cache.cache_file_path(&DiskCache::compute_key(path, 10, 1000, 256));
        let mut bytes = std::fs::read(&file).unwrap();
        bytes.truncate(bytes.len() / 2);
        std::fs::write(&file, &bytes).unwrap();
        assert!(cache.get_cached(MediaId(1), path, 10, 1000, 256).is_none());
        std::fs::write(&file, &bytes[..10]).unwrap();
        assert!(cache.get_cached(MediaId(1), path, 10, 1000, 256).is_none());
    }
}
