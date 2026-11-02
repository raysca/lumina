# Import and Missing Files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Files that vanish from the library keep their metadata (marked missing, reconnected on move/rename), and media can be imported by copy via drag and drop or Import….

**Architecture:** A `media_file_state` side table (inode, missing-since) is written by the one ingest path (`insert_or_update_batch`), which also reconnects moved files by inode + size. The watcher and rescan mark rows missing instead of deleting them. Import plans in the background (walk, inspect, duplicate check), shows an overlay dialog, then copies through hidden temp files + `hard_link` and catalogues copies directly.

**Tech Stack:** Rust 2021, rusqlite, notify-debouncer-mini 0.4, walkdir, rfd 0.15, GPUI 0.2.2. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-02-import-and-missing-files-design.md`

## Global Constraints

- `SCHEMA_VERSION` 11 -> 12; migration is `CREATE TABLE/INDEX IF NOT EXISTS`.
- No new crates. Inode via `std::os::unix::fs::MetadataExt::ino` under `#[cfg(unix)]`.
- No catalog-lock or file IO on the UI thread; hashing, walking, copying, stat run on background threads.
- No `let _ =` on catalog writes; background failures `log::warn!`/`error!`; user-facing failures via `EngineEvent::Error` / error banner.
- Never delete a row except via `fold_into` (same file proven) or the existing user delete flows.
- Destination `<library>/Imports/YYYY/YYYY-MM-DD/`; temp name `.<name>.lumina-import`; clash suffix `" 2"`, `" 3"`… before the extension.
- Copy: `ImportReport` is taken; the new report is `CopyImportReport`.
- debug_selectors: `grid-missing-badge-{id}`, `sidebar-missing`, `import-dialog`, `import-dialog-backdrop`, `import-cancel`, `import-confirm`, `missing-locate`.
- `#[core::prelude::v1::test]` in modules glob-importing `gpui::*`.
- Never commit `FIXES.md`; stage explicit paths. Never `git stash`.

## Review Focus

1. Library on an external volume that is ejected while Lumina is open: nothing is marked missing (event at mount point; root gone) — Task 3 test `unmount_event_at_ancestor_marks_nothing`.
2. Renaming a folder of many files in Finder: all reconnect, none duplicated — Task 3 test `folder_move_reconnects_all_files`.
3. Importing the same folder twice: second run reports everything as duplicates, copies nothing — Task 7 test `second_import_of_same_folder_copies_nothing`.
4. Dropping the library folder itself (or a subfolder) onto the window: counted "already in library", nothing copied — Task 7 test `files_inside_library_are_not_copied` (with `/var` vs `/private/var`).
5. A large import (≥1k files) keeps the UI responsive: the watcher does not re-upsert copied files — Task 3 test `unchanged_file_event_is_skipped`.

---

## Part 1: Missing files

**As built (differs from the tasks below):** the file-state code lives in
`catalog/files.rs`, not `trash.rs`/`duplicates.rs`. `reconcile_files()` takes
no closure; it stats paths itself with the catalog lock released.
`MissingChange.folded` is `Vec<(kept, removed)>`, so the watcher can re-send
the kept row. `get_file_signatures` excludes missing rows, and the watcher
reads `present_signatures` for the paths in a batch. The inode index covers
every row (`idx_file_state_inode`), not only missing ones, because a fold
looks up a present twin. A fold also carries rotation, edited date or place,
and hidden from the removed row. Locate is in the context menu as well as ⌘K,
and it merges twins only on equal size, dimensions and stored hash.
`check_import_duplicates` gains its `cancel` flag in Task 7.

### Task 1: File state table, inode capture, reconnect

**Files:**
- Modify: `crates/lumina-core/src/catalog/schema.rs` (table + index), `crates/lumina-core/src/catalog/mod.rs` (`SCHEMA_VERSION = 12`), `crates/lumina-core/src/catalog/media.rs` (`insert_or_update_batch`, `get_file_signatures`, new `get_missing_ids`)
- Modify: `crates/lumina-core/src/lib.rs` (`LuminaEngine::get_missing_ids`)
- Test: unit tests in `catalog/media.rs`

**Interfaces:**
- Produces: `Catalog::get_missing_ids(&self) -> Result<HashSet<MediaId>>`; `LuminaEngine::get_missing_ids(&self) -> HashSet<MediaId>` (logs and returns empty on error, like `get_ratings_map`); `pub(crate) fn file_inode(path: &Path) -> Option<u64>` in `catalog/media.rs`.
- `insert_or_update_batch` contract after this task: stats paths before locking; per item, if no row at the path and inode known, repoints the newest missing row with equal inode and `file_size` (SQL in spec "Reconnect"); then upserts; then `INSERT INTO media_file_state(media_id, file_inode, missing_since_epoch_ms) VALUES (?, ?, NULL) ON CONFLICT(media_id) DO UPDATE SET file_inode = excluded.file_inode, missing_since_epoch_ms = NULL`.

- [ ] Write failing tests: `migration_creates_file_state_idempotently` (open twice, `user_version == 12`, table exists, rows kept); `upsert_records_inode` (real temp file; state row inode == `metadata.ino()`); `upsert_reconnects_missing_row_by_inode` (insert at `a.jpg`, set missing via SQL, rename file to `b.jpg`, upsert `b.jpg` -> same id, path `b.jpg`, tag kept, not in `get_missing_ids`); `upsert_ignores_inode_match_with_different_size`; `signatures_exclude_missing_rows`.
- [ ] Run `cargo test -p lumina-core --lib catalog::media` — expect FAIL (no table / fn).
- [ ] Implement as per Interfaces.
- [ ] Run again — PASS.

### Task 2: Mark missing, fold, rescan reconcile

**Files:**
- Modify: `crates/lumina-core/src/catalog/trash.rs` (delete `remove_item_by_path`, `remove_items_not_in`; add `mark_missing_under`, `reconcile_files`), `crates/lumina-core/src/catalog/duplicates.rs` (extract merge-metadata helper; add `fold_into`), `crates/lumina-core/src/lib.rs` (`EngineEvent::ItemsMissing`, `run_scan`)
- Test: `crates/lumina-core/tests/core_tests.rs`; update tests using the deleted fns

**Interfaces:**
- Produces:
  - `Catalog::mark_missing_under(&self, path: &Path) -> Result<MissingChange>`, where `pub struct MissingChange { pub missing: Vec<MediaId>, pub folded: Vec<MediaId> }` (`folded` = rows deleted by fold, emit `ItemRemoved`). Marks with the range predicate; for each marked row, folds a present row with same inode + size and `added_epoch_ms >= now - 60_000`.
  - `Catalog::reconcile_files(&self, exists: impl Fn(&Path) -> Option<Option<u64>>) -> Result<Vec<MediaId>>`: reads present rows (lock released), calls `exists` (None = gone, Some(ino)), then in one tx marks gone rows missing and fills NULL inodes. Returns newly missing ids.
  - `Catalog::fold_into(&self, keep: MediaId, other: MediaId) -> Result<()>` (spec "Fold"); shared `fn merge_metadata_in_tx(tx, keep, others: &[MediaId])` used by it and `merge_duplicate_group_in_tx`.
  - `EngineEvent::ItemsMissing(Vec<MediaId>)`.
- `run_scan`: if `!library_root.is_dir()` send `Error(format!("Library folder is not available: {}", root.display()))` + `ScanComplete` and return; else `reconcile_files` before the walk (send `ItemsMissing` if non-empty); remove the post-walk removal.

- [ ] Failing tests: `mark_missing_under_folder_spares_siblings` (`dir/a.jpg`, `dir/b.jpg`, `dir 2/c.jpg`, `dir.old/d.jpg`; mark `dir` -> exactly a, b); `mark_missing_keeps_metadata`; `split_rename_folds_new_row` (row A tagged, row B same inode/size added now at new path; mark A's path -> B deleted, A at B's path, present, tag kept, `folded == [B]`); `old_present_twin_is_not_folded` (B `added_epoch_ms` 2 min ago -> no fold); `rescan_marks_deleted_and_reconnects_renamed`; `rescan_with_root_gone_marks_nothing`; `rescan_restored_file_clears_missing`; `reconcile_backfills_inodes`.
- [ ] Update `core_tests.rs` FTS cascade tests to delete via `permanently_delete_items(…, DeletionTarget::CatalogOnly)`.
- [ ] Run `cargo test -p lumina-core` — FAIL; implement; PASS.

### Task 3: Watcher

**Files:**
- Modify: `crates/lumina-core/src/watcher.rs`, `crates/lumina-core/src/scanner.rs` (`pub fn walk_media(dir: &Path) -> impl Iterator<Item = PathBuf>`; `pub fn is_supported_media(path: &Path) -> bool` wrapping `classify_media_extension`), `crates/lumina-core/src/lib.rs` (watcher gets a rescan callback)
- Test: `crates/lumina-core/tests/watcher_test.rs`

**Interfaces:**
- Consumes: Task 1 upsert, Task 2 `mark_missing_under`, `ItemsMissing`.
- Batch handler: normalize paths; drop paths not strictly under root; if `!root.is_dir()` drop batch; partition gone/existing; mark gone (send `ItemsMissing` / `ItemRemoved` for folded); for existing: directories -> `walk_media`, files -> collect; skip files whose catalog signature (size, mtime) matches and row is present (one `get_signature_and_present(&[PathBuf])` read); upsert the rest in one batch; send `BatchDiscovered` if non-empty. `Err` from notify -> `log::warn!` + rescan callback.

- [ ] Update existing watcher test: delete -> `ItemsMissing` containing the id; `total_count` stays 2; id in `get_missing_ids()`.
- [ ] Failing tests: `rename_reconnects_same_id`; `folder_move_reconnects_all_files` (move `media/a/` with 2 files to `media/b/`; same ids, no new rows); `unmount_event_at_ancestor_marks_nothing` (call the batch handler fn directly with the root's parent path -> nothing missing); `unchanged_file_event_is_skipped` (touch-free event on catalogued file -> no `BatchDiscovered` within timeout).
- [ ] Run `cargo test -p lumina-core --test watcher_test` — FAIL; implement; PASS.

### Task 4: Duplicates ignore missing rows

**Files:** `crates/lumina-core/src/catalog/duplicates.rs`, `crates/lumina-core/src/lib.rs` (`check_import_duplicates(&self, paths: &[PathBuf], cancel: &AtomicBool)`)

- [ ] Failing tests: `missing_rows_are_not_duplicates` (two identical files, mark one missing -> no groups, count 0); `import_check_ignores_missing_match` (external file identical to a missing row -> not duplicate). Update existing `check_import_duplicates` call with a fresh `AtomicBool`.
- [ ] Add `AND NOT EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL)` to the four queries named in the spec; cancel checked per file (returns results so far).
- [ ] Run `cargo test -p lumina-core duplicate` — PASS.

### Task 5: Missing in the UI

**Files:**
- Modify: `crates/lumina-core/src/smart_albums/system.rs` (`Missing` variant), `crates/lumina-core/tests/core_tests.rs` (perf gate `/ SystemAlbum::all().len() as f64`)
- Modify: `crates/lumina-ui-gpui/src/app/state.rs` (`missing_items: HashSet<MediaId>`), `app/library.rs` (refresh in `refresh_album_counts`; handle `ItemsMissing`; refresh `selected_item` by id on `BatchDiscovered`), `views/sidebar/sections.rs` (row after Duplicates when count > 0, selector `sidebar-missing`), `app/render/grid.rs` (badge), viewer load-failure text
- Test: `crates/lumina-ui-gpui/src/tests/missing.rs` (new, add to `tests/mod.rs`)

- [ ] Failing UI tests: `missing_item_shows_badge_and_view` (seed library, mark one item missing via catalog, refresh -> `grid-missing-badge-{id}` renders only for it; `sidebar-missing` renders; click -> grid shows 1 item); `missing_row_hidden_when_none`.
- [ ] Implement; run `cargo test -p lumina-ui-gpui missing` — PASS.
- [ ] Run full gates (fmt, clippy `-D warnings`, `cargo test --workspace`, `make bench`, `make ui-smoke`).

### Task 6: Locate…

**Files:**
- Modify: `crates/lumina-core/src/lib.rs` (`LuminaEngine::relink(&self, id: MediaId, new_path: &Path) -> Result<RelinkOutcome>`), `crates/lumina-ui-gpui/src/app/library.rs` (`locate_missing_selected`, `apply_locate(id, path)`), `app/command_palette.rs` (`CommandId::LocateMissing`, key `command.locate_missing`), `i18n.rs` ("Locate Missing File…"), Missing view header button `missing-locate`
- Test: `core_tests.rs`, `tests/missing.rs`

**Interfaces:**
- `pub enum RelinkOutcome { Relinked, Folded(MediaId) }`. Errors (as `anyhow` messages shown in the banner): outside root -> "Choose a file inside the library folder. To bring in a file from elsewhere, import it first."; unsupported extension -> "That file type is not supported."; different-content twin or unhashed missing row with a twin -> "That file is already in your library as another item. Delete one of them, or merge them from Duplicates."
- `apply_locate` runs `relink` on the background executor; on success refresh counts; `Folded(id)` handled like `ItemRemoved`.

- [ ] Failing core tests: `relink_repoints_and_clears_missing`; `relink_rejects_outside_library`; `relink_folds_identical_twin`; `relink_refuses_different_twin`.
- [ ] Failing UI test: `locate_outside_library_shows_banner` (call `apply_locate` with a temp path outside root; error banner text matches).
- [ ] Implement; run tests — PASS.
- [ ] Independent review of Part 1 (reviewer does not edit); fix findings; re-check; gates.
- [ ] Commit Part 1:

```bash
git add docs/superpowers/specs/2026-11-02-import-and-missing-files-design.md docs/superpowers/plans/2026-11-02-import-and-missing-files.md crates/
git commit -m "feat: keep missing files and reconnect moved ones"
```

## Part 2: Import

### Task 7: Import engine

**Files:**
- Create: `crates/lumina-core/src/import.rs` (plan + copy; keep under ~500 lines)
- Modify: `crates/lumina-core/src/lib.rs` (`pub mod import;`, engine methods, events)
- Test: `crates/lumina-core/tests/import_test.rs` (new)

**Interfaces:**
- `pub struct PlannedCopy { pub source: PathBuf, pub dest_dir: PathBuf, pub item: MediaItem }`
- `pub struct ImportPlan { pub files: Vec<PlannedCopy>, pub duplicates: usize, pub in_library: usize, pub unsupported: usize, pub unreadable: usize, pub bytes: u64 }`
- `pub struct CopyImportReport { pub copied: usize, pub failed: Vec<(PathBuf, String)>, pub cancelled: bool }`
- `LuminaEngine::plan_import(&self, paths: &[PathBuf], cancel: &AtomicBool) -> Result<ImportPlan>` (blocking; caller runs it off the UI thread)
- `LuminaEngine::run_import(&self, plan: ImportPlan, cancel: Arc<AtomicBool>) -> CopyImportReport` (blocking; sends `ImportProgress`/`BatchDiscovered`; recomputes buckets at end)
- `EngineEvent::ImportProgress { copied: usize, total: usize }`
- `fn claim_destination(dir: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> io::Result<PathBuf>` loop with `hard_link` (in `import.rs`); `taken` = catalog has a row at the path.
- `Catalog::path_is_catalogued(&self, path: &Path) -> Result<bool>`

- [ ] Failing tests: `plan_walks_folders_with_scanner_rules` (hidden dir, symlink, `.txt` -> counted unsupported); `files_inside_library_are_not_copied`; `plan_skips_library_duplicates`; `plan_skips_in_batch_duplicates`; `dest_dir_is_by_capture_date` (`Imports/2024/2024-05-06`); `copy_preserves_mtime_and_leaves_no_temp`; `clash_with_file_and_missing_row_get_suffix` (expect `IMG 2.jpg`, `IMG 3.jpg`); `copied_items_are_catalogued`; `failed_copy_is_reported_and_temp_removed` (unreadable source via removed-after-plan file); `cancel_stops_copying`; `second_import_of_same_folder_copies_nothing`; `stale_temp_files_are_cleaned`.
- [ ] Run `cargo test -p lumina-core --test import_test` — FAIL; implement; PASS.

### Task 8: Import UI

**Files:**
- Create: `crates/lumina-ui-gpui/src/views/import_dialog.rs`, `crates/lumina-ui-gpui/src/app/import.rs` (state + actions; `ImportState` struct on `LuminaApp`)
- Modify: `app/render/mod.rs` (overlay + root `on_drop`/`drag_over`), `app/keymap/modals.rs` (absorb keys), `app/keymap/mod.rs` (⇧⌘I before ⌘I), `app/text_input.rs` (`text_target` None while dialog open), `app/library.rs` (`open_library_inner` clears import dialog; handle `ImportProgress`), `app/render/status_bar.rs` ("Importing… {copied} of {total}" priority), `menus.rs` + `actions` (`ImportMedia`), `app/command_palette.rs` (`ImportMedia`, `CancelImport`), `i18n.rs`
- Test: `crates/lumina-ui-gpui/src/tests/import.rs` (new)

**Interfaces:**
- `pub(crate) enum ImportDialog { Planning { count: usize }, Ready(ImportPlan), Empty(ImportPlan) }`; `pub(crate) struct ImportState { dialog: Option<ImportDialog>, plan_cancel: Option<Arc<AtomicBool>>, running: Option<Arc<AtomicBool>>, progress: Option<(usize, usize)> }`.
- `LuminaApp::begin_import(&mut self, paths: Vec<PathBuf>, cx)`, `confirm_import`, `cancel_import_dialog`, `cancel_running_import`, `pick_import_folders`.
- Copy: dialog strings exactly as the spec "Dialog"; toast `"Imported {n} items"`; banner `"{n} files could not be imported: …"`; drop highlight "Drop to import"; no-library toast "Open a library before importing"; running toast "An import is already running".

- [ ] Failing tests: `import_dialog_flow` (begin_import with a temp folder of 2 jpgs -> Planning then Ready; Esc closes, nothing copied; begin again, ↵ -> files under `Imports/`, catalogued, toast); `import_dialog_backdrop_and_cancel_close`; `import_dialog_absorbs_keys_and_text` (⌫ does not trash; typing does not reach an active tag input); `shift_cmd_i_opens_import_and_cmd_i_get_info`; `library_switch_clears_import_dialog`; `nothing_new_shows_ok_only`.
- [ ] Implement; run `cargo test -p lumina-ui-gpui import` — PASS.
- [ ] Full gates incl. `make bench`, `make ui-smoke`.
- [ ] Independent review of Part 2; fix; re-check.
- [ ] Commit:

```bash
git add crates/ docs/superpowers/plans/2026-11-02-import-and-missing-files.md
git commit -m "feat: import media by copying, with drag and drop"
```
