//! Duplicate detection, merging and relinking for the engine.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::*;

impl LuminaEngine {
    /// Scans candidate media items with duplicate file sizes, computes their
    /// content hashes in parallel, stores them in `media_hashes`, and returns
    /// a report of all duplicate groups.
    pub fn scan_duplicates(&self) -> Result<DuplicateReport> {
        let candidates = self.catalog.find_unhashed_duplicate_candidates()?;
        let total_scanned = candidates.len();

        if !candidates.is_empty() {
            use rayon::prelude::*;
            let computed: Vec<(MediaId, u64, String)> = candidates
                .into_par_iter()
                .filter_map(|(id, path, size)| match compute_file_hash(&path) {
                    Ok(hash) => Some((id, size, hash)),
                    Err(err) => {
                        log::warn!("failed to compute hash for {}: {err:#}", path.display());
                        None
                    }
                })
                .collect();

            self.catalog.batch_insert_hashes(&computed)?;
        }

        let groups = self.catalog.query_duplicate_groups()?;
        let total_duplicate_items: usize = groups.iter().map(|g| g.items.len()).sum();
        let potential_reclaimable_bytes: u64 =
            groups.iter().map(|g| g.potential_reclaimable_bytes).sum();

        Ok(DuplicateReport {
            total_scanned,
            groups,
            total_duplicate_items,
            potential_reclaimable_bytes,
        })
    }

    /// Queries all duplicate groups. Ensures unhashed candidate items with
    /// duplicate file sizes are hashed first.
    pub fn query_duplicate_groups(&self) -> Vec<DuplicateGroup> {
        match self.scan_duplicates() {
            Ok(report) => report.groups,
            Err(err) => {
                log::warn!("scan_duplicates encountered an error: {err:#}");
                self.read(
                    "query duplicate groups fallback",
                    self.catalog.query_duplicate_groups(),
                )
            }
        }
    }

    /// Fast, non-blocking read of already-computed duplicate groups from the catalog
    /// without scanning or hashing unhashed files.
    pub fn get_cached_duplicate_groups(&self) -> Vec<DuplicateGroup> {
        self.read(
            "get cached duplicate groups",
            self.catalog.query_duplicate_groups(),
        )
    }

    /// Returns the total count of duplicate items across all duplicate groups.
    pub fn query_duplicate_count(&self) -> usize {
        self.read(
            "query duplicate items count",
            self.catalog.query_duplicate_items_count(),
        )
    }

    /// Returns a slice of items in duplicate groups for pagination and grid viewing.
    pub fn query_duplicate_items_slice(&self, offset: usize, limit: usize) -> Vec<MediaItem> {
        self.read(
            "query duplicate items slice",
            self.catalog.query_duplicate_items_slice(offset, limit),
        )
    }

    /// Merges duplicates into `keep_id` and moves `trash_ids` to Recently Deleted.
    /// The keeper's pre-merge metadata (for undo) is dropped here; use
    /// [`Catalog::merge_duplicate_group`](crate::Catalog::merge_duplicate_group)
    /// through `catalog()` to keep it.
    pub fn merge_duplicate_group(&self, keep_id: MediaId, trash_ids: &[MediaId]) -> Result<()> {
        self.catalog
            .merge_duplicate_group(keep_id, trash_ids)
            .map(|_| ())
    }

    /// Merges all duplicate groups in the catalog in a single atomic transaction,
    /// each keeping its entry in `keepers` (by content hash) or else its
    /// heuristic pick, and leaves the groups in `skipped` alone. Returns the
    /// IDs of all newly trashed items.
    pub fn merge_all_duplicates(
        &self,
        keepers: &std::collections::HashMap<String, MediaId>,
        skipped: &std::collections::HashSet<String>,
    ) -> Result<Vec<MediaId>> {
        self.catalog
            .merge_all_duplicates(keepers, skipped)
            .map(|merge| merge.trashed)
    }

    /// Points the missing item `id` at `chosen`, a file inside the library
    /// ("Locate…"). When `chosen` is already catalogued as another item, the
    /// two merge only if they can be the same file: equal size and
    /// dimensions, and equal content hash when one was stored. A different photo is never merged
    /// away. Blocking: reads and may hash the file.
    pub fn relink(&self, id: MediaId, chosen: &Path) -> Result<RelinkOutcome> {
        const OUTSIDE: &str = "Choose a file inside the library folder. To bring in a file from elsewhere, import it first.";
        let root = self.library_root.canonicalize().context("library folder")?;
        let chosen = chosen.canonicalize().context("chosen file")?;
        let Ok(relative) = chosen.strip_prefix(&root) else {
            anyhow::bail!(OUTSIDE);
        };
        // Catalog paths use the library root as it was opened.
        let path = self.library_root.join(relative);
        if !scanner::is_supported_media(&path) {
            anyhow::bail!("That file type is not supported.");
        }
        let missing = self
            .catalog
            .get_item_by_id(id)?
            .context("the missing item no longer exists")?;
        let item = Scanner::new(self.library_root.clone(), self.event_tx.clone())
            .inspect_media_file(&path)
            .context("the chosen file cannot be read")?;

        let outcome = match self.catalog.id_at_path(&path)? {
            Some(twin) if twin != id => {
                // Fixed-size formats make size alone weak; dimensions too.
                // A quarter-turned photo that went missing before sizes were
                // stored oriented holds its unrotated size. Only then does a
                // transposed size match: two uncompressed TIFFs of 6000x4000
                // and 4000x6000 share a byte size.
                let (w, h) = (missing.width, missing.height);
                let turned = (5..=8).contains(&dims::read_exif_size(&path).orientation);
                let same_file = item.file_size == missing.file_size
                    && ((item.width, item.height) == (w, h)
                        || (turned && (item.width, item.height) == (h, w)));
                let same_content = match self.catalog.stored_hash(id)? {
                    Some(hash) => compute_file_hash(&path)? == hash,
                    None => true,
                };
                if !(same_file && same_content) {
                    anyhow::bail!("That file is already in your library as another item. Delete one of them, or merge them from Duplicates.");
                }
                self.catalog.fold_into(id, twin)?;
                let _ = self.event_tx.send(EngineEvent::ItemRemoved(twin));
                RelinkOutcome::Folded(twin)
            }
            Some(_) => RelinkOutcome::Relinked,
            None => {
                self.catalog.relink_path(id, &path)?;
                RelinkOutcome::Relinked
            }
        };
        // Fresh size, dates and inode for the file now behind the row.
        self.catalog.insert_or_update_batch(&[item])?;
        Ok(outcome)
    }

    /// Ids of items whose file is missing from disk.
    pub fn get_missing_ids(&self) -> std::collections::HashSet<MediaId> {
        self.read("get missing ids", self.catalog.get_missing_ids())
    }

    /// Evaluates candidate files before importing them into the library.
    /// Once `cancel` is set, the remaining files are reported as not
    /// duplicates without being hashed; the caller discards the result.
    pub fn check_import_duplicates(
        &self,
        paths: &[PathBuf],
        cancel: &AtomicBool,
    ) -> Result<Vec<DuplicateCandidate>> {
        use rayon::prelude::*;
        let newly_computed: Arc<Mutex<Vec<(MediaId, u64, String)>>> =
            Arc::new(Mutex::new(Vec::new()));

        let results: Vec<DuplicateCandidate> = paths
            .par_iter()
            .map(|path| {
                let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                if file_size == 0 || cancel.load(Ordering::Relaxed) {
                    return DuplicateCandidate {
                        path: path.clone(),
                        file_size: 0,
                        content_hash: None,
                        matched_item_id: None,
                        matched_item_path: None,
                        is_duplicate: false,
                    };
                }

                let candidates = self
                    .catalog
                    .find_candidates_by_size(file_size)
                    .unwrap_or_default();
                if candidates.is_empty() {
                    return DuplicateCandidate {
                        path: path.clone(),
                        file_size,
                        content_hash: None,
                        matched_item_id: None,
                        matched_item_path: None,
                        is_duplicate: false,
                    };
                }

                let content_hash = match compute_file_hash(path) {
                    Ok(h) => h,
                    Err(_) => {
                        return DuplicateCandidate {
                            path: path.clone(),
                            file_size,
                            content_hash: None,
                            matched_item_id: None,
                            matched_item_path: None,
                            is_duplicate: false,
                        };
                    }
                };

                let matched = self
                    .catalog
                    .find_items_by_hash(&content_hash)
                    .unwrap_or_default();
                if let Some(existing) = matched.first() {
                    return DuplicateCandidate {
                        path: path.clone(),
                        file_size,
                        content_hash: Some(content_hash),
                        matched_item_id: Some(existing.id),
                        matched_item_path: Some(existing.path.clone()),
                        is_duplicate: true,
                    };
                }

                for (cand_id, cand_path) in candidates {
                    if let Ok(cand_hash) = compute_file_hash(&cand_path) {
                        newly_computed
                            .lock()
                            .push((cand_id, file_size, cand_hash.clone()));
                        if cand_hash == content_hash {
                            return DuplicateCandidate {
                                path: path.clone(),
                                file_size,
                                content_hash: Some(content_hash),
                                matched_item_id: Some(cand_id),
                                matched_item_path: Some(cand_path),
                                is_duplicate: true,
                            };
                        }
                    }
                }

                DuplicateCandidate {
                    path: path.clone(),
                    file_size,
                    content_hash: Some(content_hash),
                    matched_item_id: None,
                    matched_item_path: None,
                    is_duplicate: false,
                }
            })
            .collect();

        // Batch insert newly computed hashes in a single transaction
        let to_insert = newly_computed.lock().clone();
        if !to_insert.is_empty() {
            if let Err(err) = self.catalog.batch_insert_hashes(&to_insert) {
                log::warn!("failed to cache computed import hashes: {err:#}");
            }
        }

        Ok(results)
    }
}
