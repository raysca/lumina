//! Import by copy: files and folders from anywhere are copied into
//! `<library>/Imports/YYYY/YYYY-MM-DD/`, by date taken. The originals are
//! never changed.
//!
//! [`LuminaEngine::plan_import`] walks, reads and duplicate-checks the
//! sources (nothing is written); [`LuminaEngine::run_import`] copies the plan
//! and catalogues each copy. Both block; callers run them off the UI thread.

use anyhow::Result;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::duplicates::compute_file_hash;
use crate::scanner::{is_supported_media, walk_files, Scanner};
use crate::{EngineEvent, LuminaEngine, MediaItem};

/// The folder inside the library that imports copy into.
pub const IMPORTS_DIR: &str = "Imports";
/// A copy in progress: hidden (the scanner and watcher skip dot-names) until
/// it is complete and linked under its real name.
pub(crate) const TEMP_SUFFIX: &str = ".lumina-import";
/// Copies catalogued per transaction, at most.
const CATALOG_BATCH: usize = 200;
/// Longest a copy waits to be catalogued (the watcher debounces 500 ms).
const CATALOG_EVERY: Duration = Duration::from_millis(250);
/// Most often a progress event is sent (one also follows a slow file).
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// One file to copy.
#[derive(Debug, Clone)]
pub struct PlannedCopy {
    pub source: PathBuf,
    pub dest_dir: PathBuf,
    /// The source as read by the scanner; catalogued at its new path.
    pub item: MediaItem,
}

/// What an import will do, for the confirm dialog.
#[derive(Debug, Clone, Default)]
pub struct ImportPlan {
    pub files: Vec<PlannedCopy>,
    /// Exact copies of library items, or of another file in this import.
    pub duplicates: usize,
    /// Files already inside the library folder.
    pub in_library: usize,
    /// Files Lumina does not catalogue (by extension).
    pub unsupported: usize,
    /// Supported files that could not be read.
    pub unreadable: usize,
    /// Total size of `files`.
    pub bytes: u64,
}

/// What [`LuminaEngine::run_import`] did. (`ImportReport` is XMP sidecar
/// import's.)
#[derive(Debug, Clone, Default)]
pub struct CopyImportReport {
    pub copied: usize,
    pub failed: Vec<(PathBuf, String)>,
    pub cancelled: bool,
}

impl LuminaEngine {
    /// Works out what importing `paths` (files and folders) would copy.
    /// When `cancel` is set the plan is incomplete; discard it.
    pub fn plan_import(&self, paths: &[PathBuf], cancel: &AtomicBool) -> Result<ImportPlan> {
        let root = self
            .library_root
            .canonicalize()
            .unwrap_or_else(|_| self.library_root.clone());
        let mut plan = ImportPlan::default();

        // 1. Expand folders; drop unsupported files and files in the library.
        // The walk itself can take a while on a large folder, so report the
        // files found even before a file total exists.
        let roots_total = paths.len();
        let mut files_found = 0usize;
        let mut walk_reported = Instant::now();
        let send_walk = |roots_done: usize, files_found: usize| {
            let _ = self.event_tx.send(EngineEvent::ImportWalk {
                roots_done,
                roots_total,
                files_found,
            });
        };
        send_walk(0, 0);
        let mut seen = HashSet::new();
        let mut sources = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            // Walk lazily: collecting the folder first would leave a large
            // library with no feedback until the whole tree had been read.
            let files: Box<dyn Iterator<Item = PathBuf> + '_> = if path.is_dir() {
                Box::new(walk_files(path))
            } else {
                Box::new(std::iter::once(path.clone()))
            };
            for file in files {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(plan);
                }
                files_found += 1;
                if walk_reported.elapsed() >= PROGRESS_EVERY {
                    walk_reported = Instant::now();
                    send_walk(index, files_found);
                }
                if !is_supported_media(&file) {
                    plan.unsupported += 1;
                    continue;
                }
                let canonical = file.canonicalize().unwrap_or_else(|_| file.clone());
                if canonical.starts_with(&root) {
                    plan.in_library += 1;
                } else if seen.insert(canonical) {
                    sources.push(file);
                }
            }
            send_walk(index + 1, files_found);
        }

        // 2. Read each file once: date taken and the item to catalogue.
        let scanner = Scanner::new(self.library_root.clone(), self.event_tx.clone());
        let total = sources.len();
        let checked = AtomicUsize::new(0);
        let reported = Mutex::new(Instant::now());
        let send_checked = |checked: usize| {
            let _ = self
                .event_tx
                .send(EngineEvent::ImportChecking { checked, total });
        };
        send_checked(0);
        let read: Vec<Option<MediaItem>> = sources
            .par_iter()
            .map(|path| {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                let item = scanner.inspect_media_file(path);
                let done = checked.fetch_add(1, Ordering::Relaxed) + 1;
                // try_lock: a worker never waits just to report.
                if let Some(mut last) = reported.try_lock() {
                    if last.elapsed() >= PROGRESS_EVERY {
                        *last = Instant::now();
                        send_checked(done);
                    }
                }
                item
            })
            .collect();
        // A cancelled plan's last count could land in a newer dialog.
        if !cancel.load(Ordering::Relaxed) {
            send_checked(total);
        }
        let mut items = Vec::with_capacity(read.len());
        for item in read {
            match item {
                Some(item) => items.push(item),
                None => plan.unreadable += 1,
            }
        }
        if cancel.load(Ordering::Relaxed) {
            return Ok(plan);
        }

        // 3. Exact copies of library items, then of each other.
        let item_paths: Vec<PathBuf> = items.iter().map(|item| item.path.clone()).collect();
        let in_library: HashSet<PathBuf> = self
            .check_import_duplicates(&item_paths, cancel)?
            .into_iter()
            .filter(|candidate| candidate.is_duplicate)
            .map(|candidate| candidate.path)
            .collect();
        let before = items.len();
        items.retain(|item| !in_library.contains(&item.path));
        let repeated = repeated_in_batch(&items, cancel);
        items.retain(|item| !repeated.contains(&item.path));
        plan.duplicates = before - items.len();

        // 4. Destination folder by date taken.
        let imports = self.library_root.join(IMPORTS_DIR);
        for item in items {
            plan.bytes += item.file_size;
            plan.files.push(PlannedCopy {
                source: item.path.clone(),
                dest_dir: imports.join(format!("{:04}", item.year)).join(format!(
                    "{:04}-{:02}-{:02}",
                    item.year, item.month, item.day
                )),
                item,
            });
        }
        Ok(plan)
    }

    /// Copies `plan` into the library and catalogues each copy, sending
    /// `ImportProgress` and `BatchDiscovered` as it goes. Stops after the
    /// current file once `cancel` is set, or when the disk is full; copies
    /// made so far stay.
    pub fn run_import(&self, plan: ImportPlan, cancel: &AtomicBool) -> CopyImportReport {
        let total = plan.files.len();
        let mut report = CopyImportReport::default();
        let mut batch = Vec::with_capacity(CATALOG_BATCH);
        let mut flushed = Instant::now();
        let mut reported = Instant::now();
        // A missing row still owns its path: a new file there would inherit
        // that photo's metadata.
        let taken = |path: &Path| {
            self.catalog
                .id_at_path(path)
                .map(|id| id.is_some())
                .map_err(|err| io::Error::other(format!("{err:#}")))
        };

        for (index, planned) in plan.files.into_iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            match copy_file(&planned, &taken) {
                Ok(dest) => {
                    let mut item = planned.item;
                    item.parent_dir = planned.dest_dir.to_string_lossy().to_string();
                    item.file_name = dest
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_default();
                    item.path = dest;
                    batch.push(item);
                    report.copied += 1;
                }
                Err(err) => {
                    let disk_full = matches!(
                        err.kind(),
                        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded
                    );
                    report.failed.push((planned.source, err.to_string()));
                    if disk_full {
                        break;
                    }
                }
            }
            // Soon after copying, so the watcher, whose events trail by its
            // debounce, mostly finds each copy catalogued already.
            if batch.len() >= CATALOG_BATCH || flushed.elapsed() >= CATALOG_EVERY {
                self.catalogue_copies(std::mem::take(&mut batch));
                flushed = Instant::now();
            }
            if reported.elapsed() >= PROGRESS_EVERY {
                reported = Instant::now();
                let _ = self.event_tx.send(EngineEvent::ImportProgress {
                    done: index + 1,
                    total,
                });
            }
        }
        self.catalogue_copies(batch);
        // Files copied or failed: short of `total` when stopped early.
        let done = report.copied + report.failed.len();
        let _ = self
            .event_tx
            .send(EngineEvent::ImportProgress { done, total });
        if let Err(err) = self.catalog.compute_timeline_buckets() {
            log::warn!("timeline refresh after import failed: {err:#}");
        }
        report
    }

    fn catalogue_copies(&self, items: Vec<MediaItem>) {
        if items.is_empty() {
            return;
        }
        match self.catalog.insert_or_update_batch(&items) {
            Ok(items) => {
                let _ = self.event_tx.send(EngineEvent::BatchDiscovered { items });
            }
            // The copies are on disk; the watcher or the next scan finds them.
            Err(err) => {
                log::error!("cataloguing {} imported files failed: {err:#}", items.len());
                let _ = self.event_tx.send(EngineEvent::Error(format!(
                    "Imported files were not added to the catalog: {err:#}. Rescan the library to add them."
                )));
            }
        }
    }
}

/// Paths of items that repeat an earlier item's content in this batch.
fn repeated_in_batch(items: &[MediaItem], cancel: &AtomicBool) -> HashSet<PathBuf> {
    let mut by_size: HashMap<u64, Vec<&MediaItem>> = HashMap::new();
    for item in items {
        by_size.entry(item.file_size).or_default().push(item);
    }
    let mut repeated = HashSet::new();
    for group in by_size.values().filter(|group| group.len() > 1) {
        let mut hashes = HashSet::new();
        for item in group {
            if cancel.load(Ordering::Relaxed) {
                return repeated;
            }
            match compute_file_hash(&item.path) {
                Ok(hash) => {
                    if !hashes.insert(hash) {
                        repeated.insert(item.path.clone());
                    }
                }
                Err(err) => log::warn!("import could not hash {}: {err:#}", item.path.display()),
            }
        }
    }
    repeated
}

/// Copies one file to a free name in its folder, keeping its modified time
/// (dates of files without EXIF come from it). Never overwrites.
fn copy_file(
    planned: &PlannedCopy,
    taken: &dyn Fn(&Path) -> io::Result<bool>,
) -> io::Result<PathBuf> {
    let name = planned
        .source
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    fs::create_dir_all(&planned.dest_dir)?;
    let temp = planned.dest_dir.join(format!(".{name}{TEMP_SUFFIX}"));
    let result = (|| {
        fs::copy(&planned.source, &temp)?;
        let modified = fs::metadata(&planned.source)?.modified()?;
        fs::File::options()
            .write(true)
            .open(&temp)?
            .set_modified(modified)?;
        claim_name(&temp, &planned.dest_dir, &name, taken)
    })();
    if let Err(err) = fs::remove_file(&temp) {
        if err.kind() != io::ErrorKind::NotFound {
            log::warn!("could not remove {}: {err}", temp.display());
        }
    }
    result
}

/// Links `temp` under `name` in `dir`, or "name 2", "name 3"… when a file or
/// a catalog row already has the name.
pub(crate) fn claim_name(
    temp: &Path,
    dir: &Path,
    name: &str,
    taken: &dyn Fn(&Path) -> io::Result<bool>,
) -> io::Result<PathBuf> {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
        _ => (name, String::new()),
    };
    for n in 1.. {
        let candidate = if n == 1 {
            dir.join(name)
        } else {
            dir.join(format!("{stem} {n}{ext}"))
        };
        if candidate.exists() || taken(&candidate)? {
            continue;
        }
        // A hard link fails if the name appeared meanwhile, where a rename
        // would replace that file.
        match fs::hard_link(temp, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            // ponytail: volumes without hard links (exFAT, SMB) fall back to
            // rename, which can replace a file that appears between the check
            // and the rename.
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
                ) =>
            {
                fs::rename(temp, &candidate)?;
                return Ok(candidate);
            }
            Err(err) => return Err(err),
        }
    }
    unreachable!("names run out only after usize::MAX tries")
}

/// Removes copies and downloads a crash or quit left half-done. Runs only
/// when a library opens: imports and downloads run in parallel after that.
pub(crate) fn remove_stale_temps(imports: &Path) {
    if !imports.is_dir() {
        return;
    }
    for entry in walkdir::WalkDir::new(imports)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let name = entry.file_name().to_string_lossy();
        if !(name.starts_with('.') && name.ends_with(TEMP_SUFFIX)) {
            continue;
        }
        // Network downloads stage in a temp folder (yt-dlp picks its own
        // file names inside it); copies stage in a temp file.
        let removed = if entry.file_type().is_dir() {
            fs::remove_dir_all(entry.path())
        } else if entry.file_type().is_file() {
            fs::remove_file(entry.path())
        } else {
            continue;
        };
        if let Err(err) = removed {
            log::warn!("could not remove {}: {err}", entry.path().display());
        }
    }
}
