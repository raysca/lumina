# Import by copy, and missing files

Status: approved direction (2026-11-02), revised after an independent
code-grounded review. Copy-only import; referenced (outside-library) files are
out of scope.

## Why

Lumina indexes one library folder in place. To add photos today you copy them
into that folder with Finder. Two problems follow:

1. There is no import: no drag and drop, no picker, no duplicate check.
2. A file that disappears from the library folder (deleted, moved or renamed
   in Finder) has its catalog row deleted by the watcher
   (`watcher.rs` `handle_event` -> `remove_item_by_path`) or by the rescan
   (`lib.rs` `run_scan` -> `remove_items_not_in`). Its tags, rating, caption,
   colour label, title and edited date/place are lost silently. A Finder
   rename is a delete plus an add, so tidying folders wipes metadata. This
   breaks the core promise "user data is never silently lost".

Missing-file handling ships first: it is the data-loss fix, and import relies
on the watcher/scan paths it changes.

## Goals

- A file that vanishes keeps its catalog row and metadata, marked missing.
- A file moved or renamed inside the library is reconnected to its row
  automatically, keeping its metadata.
- Missing items are visible and fixable: a badge, a Missing view, Locate….
- Import copies files and folders into the library, skipping exact duplicates
  (using the duplicate detection already on main), with a confirm dialog.
- Drag and drop of files and folders onto the window; an Import… command.

## Non-goals

- Referencing files outside the library folder.
- Deleting or ejecting the source after import (e.g. wiping an SD card).
- Near-duplicate (visually similar) detection; duplicates are exact byte
  matches, as implemented.
- Undo of an import or of Locate.
- A "Last Import" view; free-space pre-check; post-copy hash verification.
- Case-only renames on case-insensitive APFS (`IMG.JPG` -> `img.jpg`): the old
  path still `exists()`, so today and after this change both spellings can be
  catalogued. Known limitation, unchanged.
- Recovering events FSEvents drops without a path (`MustScanSubDirs` is
  swallowed by notify-debouncer-mini). Rescan Library remains the fallback.

## Anti-goals (failure even if it "works")

- Any path that deletes a row with user metadata without the user asking.
- An unmounted/unavailable library folder or volume marking items missing.
- A half-copied file appearing in the library, or an import overwriting any
  existing file.
- A new file inheriting another photo's metadata.
- The UI thread waiting on copying, hashing, walking or the catalog lock.

## Part 1: Missing files

### Data

Schema v12 (`SCHEMA_VERSION` 11 -> 12). A side table, not new
`media_items` columns: `media_fts_au` fires on every `media_items` UPDATE and
rewrites the FTS row, so marking a large folder missing or backfilling inodes
through `media_items` would rewrite FTS for every row under the lock.

```sql
CREATE TABLE IF NOT EXISTS media_file_state (
    media_id INTEGER PRIMARY KEY,
    file_inode INTEGER,               -- NULL: unknown
    missing_since_epoch_ms INTEGER,   -- NULL: present
    FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_file_state_missing
    ON media_file_state(file_inode) WHERE missing_since_epoch_ms IS NOT NULL;
```

`CREATE ... IF NOT EXISTS` is the idempotent migration; existing catalogs get
an empty table (all present, inode unknown). A row without a state row means
present + unknown inode.

`MediaItem` does not gain fields. The UI learns which items are missing from
an id set, like ratings.

### Inode capture

`Catalog::insert_or_update_batch` stats each item's path *before* taking the
catalog lock (`MetadataExt::ino`, `#[cfg(unix)]`) and upserts
`media_file_state(media_id, file_inode)` with `missing_since = NULL` for every
item in the same transaction. Every ingest path (scan, watcher, import,
Locate) goes through it.

Backfill for existing rows happens in the rescan's pre-walk pass (below),
which already stats every present row.

### Marking missing (never deleting)

- `Catalog::mark_missing_under(path) -> Vec<MediaId>`: marks present rows
  whose `file_path = path` or lies under `path/`. "Under" is a range on the
  unique `file_path` autoindex: `file_path >= 'p/' AND file_path < 'p0'`
  (`'0'` follows `'/'`, BINARY collation), plus the equality term. No LIKE
  escaping.
- Watcher event filter: drop any event whose path is not strictly under the
  library root (FSEvents reports an unmount as a Remove at the mount point,
  an ancestor of the root). Before marking anything in a batch, if
  `library_root.is_dir()` is false, skip the whole batch.
- notify-debouncer-mini delivers paths without kinds, from a per-path map, in
  arbitrary order. Per batch: partition into gone paths and existing paths;
  mark gone paths first, then ingest existing ones, so a rename whose halves
  land in the same batch reconnects through the upsert.
- A rename whose halves land in different batches (each path has its own
  timer) arrives as "new path ingested as a new row", then "old path gone".
  `mark_missing_under` therefore folds: for each row it marks, if a *present*
  row has the same inode and size and was added in the last 60 s
  (`added_epoch_ms`), the missing row takes over that row's path and the new
  row's metadata is merged in, then the new row is deleted (see Fold). The
  60 s window keeps hard-linked pairs (same inode, both present for a long
  time) from folding.
- Watcher: an existing *directory* path (folder moved or copied in) is walked
  with a new `Scanner::walk_media(dir)` helper (same rules as `walk`: hidden
  skipped, no symlinks, supported extensions) and ingested in batches, not
  one upsert per file.
- Watcher: skip the upsert (and the `BatchDiscovered` event) for a file whose
  catalogued size and mtime already match and whose row is present. Import
  catalogues its copies directly, so without this every copied file causes a
  second upsert and a UI count refresh.
- Watcher notify error (`Err` branch): log and call the engine's rescan, so
  overflowed event streams self-heal.
- Rescan (`run_scan`): if the library root is not an existing directory,
  abort with `EngineEvent::Error("Library folder is not available: …")` and
  mark nothing. Otherwise, before the walk, one pass over present rows: path
  gone -> mark missing; path present with unknown inode -> record inode
  (`Catalog::reconcile_files`). Replaces the post-walk `remove_items_not_in`.
  Pre-walk so files moved while the app was closed reconnect during the walk.
- `get_file_signatures` excludes missing rows, so a file restored at its old
  path is re-ingested (which clears missing) instead of skipped.
- `remove_item_by_path` and `remove_items_not_in` become unused: delete them;
  update their tests.
- New `EngineEvent::ItemsMissing(Vec<MediaId>)`: the UI refreshes counts and
  the missing set; selection is kept (order unchanged).

### Reconnect

In `insert_or_update_batch`, per item, in the same transaction, before the
upsert: if no row has this `file_path` and the inode is known, repoint the
newest missing row with the same inode and size:

```sql
UPDATE media_items SET file_path = ?, parent_dir = ?, file_name = ?
WHERE id = (SELECT s.media_id FROM media_file_state s
            JOIN media_items m ON m.id = s.media_id
            WHERE s.missing_since_epoch_ms IS NOT NULL
              AND s.file_inode = ? AND m.file_size = ?
            ORDER BY s.missing_since_epoch_ms DESC LIMIT 1);
```

The following upsert then hits the row by its new path and refreshes it; the
state upsert clears missing. FTS stays in sync via `media_fts_au` (path
columns really changed). Same inode + size guards against inode reuse (APFS
allocates inode numbers monotonically).

Path is identity otherwise: a different file placed at a missing row's exact
path restores that row (as an editor save does today). Import never writes to
a catalogued path (see Destination), so it cannot trigger this.

A rename onto an already-catalogued path (Finder "Replace"): the target row
keeps its row and metadata and is refreshed; the source row is marked missing
with its metadata intact. Nothing is deleted.

Not covered (stays missing + appears as new): copy out and back in (new
inode), moves across volumes. Locate… covers those.

### Fold

`Catalog::fold_into(keep_id, other_id)` in one transaction: merge `other`'s
user metadata into `keep` (tags union, max rating, favorite OR, caption /
colour label / title only where `keep` has none), then delete `other`, then
give `keep` `other`'s path and clear missing. Built by extracting the
metadata-merge steps of `merge_duplicate_group_in_tx`
(`catalog/duplicates.rs`) into a shared helper; the duplicates merge keeps
trashing as today. Emits `ItemRemoved(other)`.

Used only when the two rows are the same file: the watcher fold (same inode +
size, other added < 60 s ago) and Locate (same size + content hash).

### Duplicates

Missing rows are excluded from duplicate candidates, groups, and the import
pre-check: add `NOT EXISTS (missing state)` to
`find_unhashed_duplicate_candidates`, `query_duplicate_*`,
`find_items_by_hash` and `find_candidates_by_size`.

So an imported file whose content matches only a *missing* row is imported as
new; the missing row stays for the user to Locate or remove.

### Locate…

For one selected missing item. Palette command "Locate Missing File…" and a
button in the Missing view's header when exactly one missing item is
selected. Opens an rfd file picker in the item's old parent folder, or the
library root if that is gone. The chosen file must:

- be inside the library root (both sides canonicalized), else banner
  "Choose a file inside the library folder. To bring in a file from
  elsewhere, import it first.";
- have a supported extension.

If no row has the chosen path: repoint the missing row, clear missing, then
re-inspect and upsert so size, mtime, EXIF and inode are fresh. If a row
already has it (the copy-back case): fold only when size and content hash
match the missing row's stored hash, or — when the missing row was never
hashed — refuse with "That file is already in your library as another item.
Delete one of them, or merge them from Duplicates." Locate never deletes a
row with different content.

Hashing runs off the UI thread.

### Removing missing items

No new action. Existing delete flows already guard `path.exists()`, so
⌫ / Move to Recently Deleted and Empty Trash work on missing items. In the
Missing view, ⌘A then ⌫ removes them all.

### UI

- `SystemAlbum::Missing`: title "Missing", icon `triangle-alert`
  (`icons.rs`), where clause
  `EXISTS (SELECT 1 FROM media_file_state s WHERE s.media_id = m.id AND s.missing_since_epoch_ms IS NOT NULL) AND m.is_trashed = 0 AND m.is_hidden = 0`
  (same hidden rule as every other system album). Counts come from the
  existing `refresh_album_counts` loop. Update the smart-album perf gate's
  hard-coded `/ 9.0` (`core_tests.rs:1056`) to `SystemAlbum::all().len()`.
- Sidebar: a "Missing" row after "Duplicates", rendered only when the count
  is > 0.
- Missing items stay in All Photos, albums and search (Lightroom
  convention). Their thumbnails come from the cache (path/size/mtime on the
  row are unchanged, so keys still match); otherwise the existing placeholder.
- Grid card badge: `triangle-alert`, bottom-left, tooltip "File is missing",
  debug_selector `grid-missing-badge-{id}`. Driven by
  `missing_items: HashSet<MediaId>` from `engine.get_missing_ids()`, refreshed
  in `refresh_album_counts` like `item_ratings`.
- On `BatchDiscovered`, refresh `selected_item` (and the viewer's item) by id
  when it is in the batch, so a reconnected item's new path is used.
- Viewer: when the shown item is missing, the existing load-failure state
  with the text "The original file is missing."
- Export already reports "source file is missing" per item.

## Part 2: Import by copy

### Entry points

- Drop files and/or folders anywhere on the main window while a library is
  open (`on_drop::<ExternalPaths>` on the root; `drag_over::<ExternalPaths>`
  shows a full-window highlight with "Drop to import"). No library: toast
  "Open a library before importing".
- "Import…" — File menu (after Open Library…), command palette, ⇧⌘I.
  The keymap's ⌘I branch (`keymap/mod.rs`) must check shift first. Opens an
  rfd folder picker allowing multiple selection (rfd cannot offer files and
  folders in one panel; cards and phone exports are folders; loose files can
  be dropped).
- One import at a time; another start toasts "An import is already running".

### Destination

`<library>/Imports/YYYY/YYYY-MM-DD/<file name>`, by capture date
(`timestamp_epoch_ms` from inspection, which falls back to mtime). One
top-level folder leaves the user's own layout untouched. Source folder
structure is not kept.

A destination name is free only if nothing exists on disk there *and* no
catalog row has that path (a missing row still owns its path; reusing it
would hand the new file that photo's metadata). Clashes take " 2", " 3", …
before the extension. No pid fallback (the export helper's `.export`
fallback would make the copy unsupported).

### Planning (background, cancellable)

`LuminaEngine::plan_import(paths, cancel) -> ImportPlan`:

1. Clean up stale `.*.lumina-import` temp files under `<library>/Imports`.
2. Expand: files as given; folders via `walk_media`. Count unsupported files.
   Files inside the library root (both canonicalized) are counted as "already
   in library" and dropped.
3. Inspect each candidate (`inspect_media_file`): gives the capture date and
   the `MediaItem` used to catalogue the copy (parsed once). Unreadable ->
   counted.
4. Duplicates against the library: `check_import_duplicates`, which gains a
   cancel flag checked per file. Duplicates within the batch: among
   candidates sharing a size, hash and keep the first.
5. Compute destination folders (names are claimed at copy time).

`ImportPlan { files: Vec<PlannedCopy { source, dest_dir, item }>,
duplicates, in_library, unsupported, unreadable: usize, bytes: u64 }`.

### Dialog

Overlay card following `views/smart_album_delete.rs` (occluding backdrop and
card, Esc cancels, ↵ confirms, absorbed keys in `keymap/modals.rs`,
`text_target` returns None while open, state cleared in
`open_library_inner`). Selectors: `import-dialog`, `import-dialog-backdrop`,
`import-cancel`, `import-confirm`.

- Planning: "Checking N items…" with Cancel (cancels planning).
- Ready: title "Import 232 items" with size ("4.1 GB"). Lines, each only when
  non-zero: "14 already in your library will be skipped", "2 unsupported files
  will be skipped", "1 file could not be read". Footer: "Copied into Imports
  in your library folder, sorted by date taken. The originals are not
  changed." Buttons Cancel / Import.
- Nothing to import: "Nothing new to import", the skip lines, a single OK.

### Copying (background thread)

Per file: create `dest_dir`; copy to the hidden temp
`.<name>.lumina-import` in `dest_dir` (scanner and watcher skip hidden
names); set its mtime to the source's (`File::set_modified`); then
`fs::hard_link(temp, candidate)` for the first free candidate name, retrying
the next name on `AlreadyExists` (never overwrites, unlike `rename`); remove
the temp. On error remove the temp and record `(source, error)`. Stop the
import on `ErrorKind::StorageFull`.

Catalogue copies directly: clone the planned `MediaItem` with the
destination path/name/parent, `insert_or_update_batch` every 200 files, send
`BatchDiscovered`. The watcher's own events for these paths are skipped by
its unchanged-file check.

Progress: `EngineEvent::ImportProgress { copied, total }`; the status bar
shows "Importing… 120 of 232", taking priority over scan text. Palette
"Cancel Import" stops after the current file; copies so far stay catalogued.

Finish: `EngineEvent::ImportFinished(CopyImportReport { copied, failed:
Vec<(PathBuf, String)>, cancelled })` (`ImportReport` is taken by XMP
sidecar import). Toast "Imported 232 items"; failures go to the error banner
with details, like Export. Timeline buckets are recomputed once at the end.

## Testing (lowest layer that proves it)

Core (temp dirs):

- Migration v11 -> v12 creates `media_file_state`, is idempotent, keeps data.
- Watcher: deleting a file marks it missing; tags/rating survive.
- Watcher: renaming a file reconnects it (same id, metadata kept, new path),
  both when halves arrive in one batch and in separate batches (fold).
- Watcher: moving a folder with two files reconnects both.
- Watcher: deleting a folder marks its files missing; siblings `dir 2` and
  `dir.old` are untouched.
- Watcher: an event at an ancestor of the root (unmount) marks nothing; a
  batch when the root is gone marks nothing.
- Watcher: an unchanged present file produces no upsert/event.
- Rescan: a file renamed while no watcher runs reconnects; a deleted one is
  marked missing; a file restored at its old path with the same mtime clears
  missing; unknown inodes get backfilled.
- Rescan with the library root gone marks nothing and reports an error.
- Duplicates (groups and import pre-check) exclude missing rows.
- Fold merges metadata and deletes the other row; Locate refuses a different
  file at a catalogued path.
- `plan_import`: walk rules, in-library files (with `/var` vs
  `/private/var`), library duplicates, in-batch duplicates, unsupported,
  destination by capture date, cancel.
- Import copy: mtime preserved, no temp files left, clash with a file on disk
  and with a missing row's path both get " 2", items catalogued, failure
  recorded and temp removed, cancel stops.
- Update: FTS cascade tests (`core_tests.rs` ~122-127, ~340-350) use
  `permanently_delete_items`; `watcher_test.rs` ~172-180 expects
  `ItemsMissing` and a kept row.

UI (in-process):

- Missing badge on a missing item only; Missing row only with a non-zero
  count, and it opens the view.
- Import dialog: planning and ready states, Esc / backdrop / Cancel close
  without copying, ↵ / Import copies, keys absorbed, typing does not reach a
  hidden text field, library switch clears it.
- ⇧⌘I opens Import (not Get Info); ⌘I still opens Get Info.
- Locate: an outside-library choice shows the banner (handler called with a
  path; the picker is bypassed).
- Drop: `ExternalPaths` cannot be built outside GPUI, so the drop handler is
  a one-line call to `begin_import(paths)`, which tests call directly.

Gates: fmt, clippy, full tests, `make ui-smoke`, `make bench` (ingest and
system-album SQL change).

## Decisions made without asking (check these)

- Destination `Imports/YYYY/YYYY-MM-DD/`, by capture date.
- Import… picks folders only; loose files via drag and drop.
- Duplicates are always skipped.
- Missing items stay in All Photos with a badge; the Missing view hides
  hidden items like every other system album.
- Path is identity: a different file at a missing row's exact path restores
  that row. Import never does this.
- An imported file matching only a missing row is imported as new.
- No hash fallback for automatic reconnect; Locate… instead. Locate merges
  only identical content and never deletes a different photo.
- A watcher error triggers a rescan.
- No dedicated "Remove all missing" button; ⌘A ⌫ in the Missing view.
