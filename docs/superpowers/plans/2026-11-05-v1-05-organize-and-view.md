# Organize and View (0.6) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lumina gets hand-picked albums, a keyboard-first culling flow (pick/reject, compare/survey), slideshow and an offline map, a review view for RAW+JPEG pairs and bursts, export presets and the macOS share sheet, a tested format/orientation/color matrix, and the cheap video items (loop range, audio tracks, chapters).

**Architecture:** Albums are two catalog tables (`albums`, `album_items`) with a new `AlbumSelection::Album(id)` that flows through the existing `ViewQuery` count/slice path. Flags are one `media_items.flag` column read as a map, like `rating`. Compare, slideshow and map are new view modes in the UI crate that reuse the viewer's decode path, the toast/undo stack and the modal key absorption. The map draws a bundled public-domain land outline with GPUI `PathBuilder` and clusters geotagged rows in core; it never touches the network. Stacks are computed groups (no schema). Share is a ~60-line ObjC bridge next to the player bridge.

**Tech Stack:** Rust 2021, rusqlite 0.31, image 0.25 (jpeg/png/webp), zune-jpeg 0.4, kamadak-exif 0.5, GPUI Kit 0.9 (`gpui-pre` 0.3.11), AppKit/AVFoundation via `cc`. **No new crates.** One new data asset (Natural Earth 110m land, public domain).

**Parts** (approve independently; each part leaves the tree shippable):
A. Curated albums and smart-album polish · B. Culling flags · C. Compare, slideshow, map · D. Stacks, export presets, share sheet · E. RAW/HEIC/orientation/color matrix · F. Video tracks, chapters, loop range.

## Global Constraints

- Schema: Part A bumps to the next `SCHEMA_VERSION` (currently 14, `catalog/mod.rs:28`) and Part B bumps once more. **Renumber after plan 4** (`2026-11-05-v1-04-metadata-and-edits.md` takes 15, 16 and 17 if all its parts land first). Tests assert `user_version == SCHEMA_VERSION`, never a literal. Migrations are `CREATE TABLE/INDEX IF NOT EXISTS` plus `pragma_table_info`-guarded `ALTER TABLE ADD COLUMN`, same style as the v14 `downloaded` column (`catalog/schema.rs:520-530`). **An index on a new column (`flag`) must be created after its guarded `ALTER`, never in the main `execute_batch` (`schema.rs:13`)**, which runs first on an old database and would fail with "no such column". Each migration gets an "open twice" idempotency test in core (pattern: `rotation_migration_is_idempotent_and_roundtrips`, `catalog/media.rs:1010`, which builds the previous layout by hand) and one extra write in the UI upgrade test `upgraded_legacy_catalog_accepts_every_everyday_write` (`tests/legacy_catalog.rs:51`; that fixture is **v7**, `tests/fixtures/legacy_catalog_v7.sql`, not v14).
- `MediaItem` gains no fields: flags and album membership are read as maps/sets, so the media column list and the ~10 `map_query_rows` callers stay untouched (the new album slice query is one more caller and uses the shared column list).
- **`merge_metadata_in_tx` (`catalog/duplicates.rs:292`) is shared with plan 4**, which also extends it (rights, edits). Album membership (A1) and flags (B1) are added there, each with a test through both callers: `merge_duplicate_group` and `fold_into` (`catalog/files.rs:192`). Rebase on plan 4's version; do not reorder its steps.
- No network anywhere in this plan. The map basemap is bundled; no tile fetch (see Decision 7).
- No catalog lock or file IO on the UI thread for new work: map points, stack grouping, export encoding, and compare decodes run on the background executor. New counts join the existing `refresh_album_counts` pass as **one** `GROUP BY` query (that pass is already synchronous on the UI thread, `app/library.rs:1089-1113`; moving it off-thread is pre-existing debt, out of scope).
- No `let _ =` on catalog writes; failures go to `set_error` and leave local state untouched, as `set_rating_selected` does (`app/navigation.rs:340-386`: it writes first and updates `item_ratings` only on success). Failure tests inject the error at a small result funnel, as `failed_tag_mutation_surfaces_in_the_error_banner_and_keeps_the_tag` does (`tests/tags_and_albums.rs:438`, `apply_tag_mutation`); the UI crate has no SQL handle to break.
- Every new user action: command palette entry (`app/command_palette.rs` `CommandId`), i18n key in `src/i18n.rs`, shortcut sheet row (`app/render/mod.rs:111+`), and a menu item when it is global (`menus.rs`). The menu bar today is Lumina / File / View / Help (`menus.rs:7-40`); there is **no Edit menu** and menu items carry no key equivalents (all shortcuts live in `app/keymap/`). This plan adds items to File and View only.
- **Key contract.** Before adding a key, check `app/keymap/mod.rs` (global handlers run first, before the viewer and grid handlers), `keymap/viewer.rs`, `keymap/grid.rs` and `keymap/modals.rs`. Verified free today: plain C, P, X, U, S and digits in the grid; ⌘0-⌘9 everywhere; ⌘N; ⌥X. Taken: plain digits 0/1/2/4 in the viewer (zoom presets, Decision 13), O in the viewer (open in default app, `viewer.rs:68`), I everywhere including ⌥I/⇧I (`mod.rs:184` matches `i` with any modifier), ⌘A also with ⇧ (`mod.rs:85` does not check shift), ⌘=/⌘- in the grid (grid zoom, `grid.rs:73-81`).
- New modules stay under ~500 lines. Do not grow `app/navigation.rs` (1624 lines) or `app/library.rs` (1546 lines): culling goes in `app/culling.rs`, albums in `app/albums.rs`, compare/slideshow in `app/present.rs`, map in `views/map.rs` + `app/map.rs`.
- `#[core::prelude::v1::test]` in modules that glob-import `gpui::*`.
- Release perf gates (`tests/benchmark_test.rs:296-376`) are unchanged; new queries get debug-budget asserts plus a `make bench` measurement recorded in the commit body.
- UI copy never names sips, QuickLook, AVFoundation or Natural Earth outside the About/acknowledgements (memory rule: hide mechanisms).

## Review Focus

1. Deleting media never orphans album rows, and duplicate merge / missing-file fold keeps album membership and flags on the keeper — Task A1 `album_items_cascade_on_media_delete`, `merge_duplicates_carries_album_membership`; Task B1 `merge_keeps_pick_over_reject`.
2. In an album view, ⌫ removes from the album and never trashes or deletes a file; ⌘⌫ still opens the Delete dialog — Task A3 `backspace_in_album_removes_membership_only`.
3. Flag keys inside text fields, the palette, or any modal do nothing to media — Task B2 `flag_keys_absorbed_by_modals_and_text`.
4. Compare/survey memory stays bounded (no 9×2048² textures) and closing it frees textures — Task C1 `survey_uses_reduced_decode_size`.
5. The map view makes no network request and works with zero geotagged items — Task C4 `map_empty_state` plus review of the diff for any `curl`/URL use; core clustering correctness — Task C3 `clusters_merge_by_cell_and_split_on_zoom`.
6. Export as JPEG bakes in catalog rotation and EXIF orientation, writes sRGB, strips location, and never overwrites — Task D3 `jpeg_export_applies_rotation_and_strips_gps`.
7. Large phone JPEGs (>2560 px, routed through sips at `decoder.rs:256-263`) and HEIC display upright — Task E1 `orientation_matrix` (this test decides whether E2 is needed).
8. `within:` and `is:pick` queries keep the 5 ms release structured-query gate — Task C3/B1 bench lines.
9. Restoring a deleted album or undoing an add never fails because a member was purged meanwhile — Task A1 `restore_album_skips_purged_members`.
10. Moving zoom presets to ⌘digits leaves no stale `0/1/2/4` hint in the tooltip, shortcut sheet or docs — Task B0 `cmd_digit_zoom_presets_in_viewer` plus grep in review.

---

## Current state (verified)

- **Albums are smart-only.** Tables: `smart_albums` (`catalog/schema.rs:88-95`: id, title, icon_name, match_mode, rules_json, created_at) — no pin/order/cover columns. `AlbumSelection` = AllLibrary, System, User(i64), Tag, Untagged, Duplicates, Trash (`views/sidebar/mod.rs:10-19`). Persisted as a `LastView` token (`config.rs:13-40`: `All`, `Favorites`, `Videos`, `Screenshots`, `Places`, `Trash`, `User(i64)`, `Tag`; `from_selection` at `:71`), so a curated album needs a new `LastView::Album(i64)` with its own token, distinct from the smart-album `User` id space. Sidebar sections: library, smart albums, tags (`views/sidebar/mod.rs:110-112`). `list_smart_albums` selects an explicit column list (`catalog/albums.rs:187-193`), so a new `smart_albums.pinned` column needs that SELECT, the mapper, `SmartAlbum`, and `restore_smart_album` updated. No smart-album rule field references albums (`smart_albums/rules.rs:14-38`), so deleting a curated album never needs a rule rewrite.
- **Deletes cascade.** `PRAGMA foreign_keys = ON` (`schema.rs:19`), and permanent delete is `DELETE FROM media_items` (`catalog/trash.rs:97`), so `ON DELETE CASCADE` membership rows vanish with the item. Trash is a flag (`is_trashed`), so a trashed member keeps its membership and returns with it on restore. `INSERT OR IGNORE` does **not** suppress foreign-key failures, so restoring membership for an id purged meanwhile must filter on `EXISTS (SELECT 1 FROM media_items ...)`.
- **Smart-album delete already confirms and undoes** (`views/smart_album_delete.rs`, `UndoAction::SmartAlbumDeleted` `app/undo.rs:77`, `Catalog::restore_smart_album` `catalog/albums.rs:87`). `docs/ux_audit.md` §7 ("deletion is immediate without confirmation/undo") is stale.
- **Pick/reject does not exist anywhere.** The ~9 grep hits for pick/reject are file pickers (`app/import.rs`, `views/onboarding.rs`), "rejected" rules/drafts (`app/album_builder_actions.rs:160`, tests), and the local helper `fn pick` in `app/undo.rs:217`. No flag column (`schema.rs:21-65`).
- **Ratings/colors exist but have no single-key shortcuts.** Rating is set only via palette `CommandId::Rate(u8)` (`command_palette.rs:45,352-384`, shortcut column empty), the context menu (`views/context_menu.rs:235,269`) and the inspector. Grid keys: F favorite, T tag, ⌫ delete, arrows (`app/keymap/grid.rs`). **Viewer digits are taken by zoom presets** 0/1/2/4 (`app/keymap/viewer.rs:48-60`); the hint also appears in the tooltip "Fit to window (0)" (`views/viewer/stage_controls.rs:30`), the shortcut sheet row `("0 / 1 / 2 / 4", …)` (`app/render/mod.rs:153`), doc comments (`viewer_state.rs:97,245`, `viewer_actions.rs:437`, `stage_controls.rs:13`, `keymap/viewer.rs:10,39`), `docs/ui_v3_plan.md:41,62`, `docs/ux_audit.md:142`, and tests `test_viewer_zoom_presets_hud_label_and_shortcuts` (`tests/viewer_and_video.rs:564-643`) and the navigator test that presses `2` (`viewer_and_video.rs:1518`). No ⌘digit binding exists anywhere. Plan 3 (`2026-11-05-v1-03-ux2-precision-browsing.md` D3) adds palette entries for the presets and a Fill preset; its palette shortcut column must show the ⌘ forms if this plan lands after it.
- **Quick filters.** `GridFilter` (`views/search_bar.rs:186-208`) dims non-matching cards. Plan 3 Task 9 deletes it and turns quick filters into query tokens (`CommandId::QuickFilter(&'static str)`, `toggle_query_token`), and its D11 moves the sidebar selection to Library whenever the query becomes non-empty. This plan builds on that, not on `GridFilter`.
- Rating clamps 0..=5 (`catalog/media.rs:383-386`); XMP rating parse is `u8` (`xmp.rs:148-151`), so a Bridge-style `xmp:Rating="-1"` reject reads as 0.
- **Duplicates = exact content hash only** (xxh3, `duplicates.rs:10`; groups in `catalog/duplicates.rs:180`). UI: `views/duplicates.rs` (merge one group with a fixed keeper, merge all). No stacks; "Bursts" is a filename `LIKE '%burst%'` system album (`smart_albums/system.rs`). `merge_metadata_in_tx` (`catalog/duplicates.rs:292`) is the single choke point used by merge and by `fold_into` (`catalog/files.rs:192-202`); it carries tags, caption, favorite, rating, color, title — **not** future album membership/flags. Plan 4 extends it too (rights, edits). Duplicate merge trashes the copies (`merge_duplicate_group`, `:387`); fold deletes `other`. Scanner timestamps are whole seconds (EXIF `DateTimeOriginal`, no `SubSecTime` read in `scanner.rs`), and rows without EXIF fall back to file mtime.
- **No compare, slideshow, or map code** exists (grep). `docs/ui_v3_plan.md:37` explicitly says "no map (no network)". Places is a system album (`system.rs`, lat/lon NOT NULL). Offline geocoder holds ~150 hard-coded cities with a 250 km cutoff (`geocoder.rs:59-98`), so many GPS rows have no city.
- Search has `city:`, `country:`, `has:location`, `rating:`, `color:`, `is:{favorite,video,image,raw,trashed}` (`search.rs:55-125`); no negation, no bounding box. Unknown *prefixes* become FTS terms (`:121-124`), but a known prefix with a bad value (`color:foo`, `is:bogus`) is silently dropped (`:86-93,104-117`).
- **Export:** `LuminaEngine::export_items` copies originals plus an XMP sidecar, never overwrites (`lib.rs:744-796`, `unique_destination` at `:63`); UI via `export_pick_folder`/`export_to_folder` (`app/library.rs:661,682`), reached from palette `ExportSelection` (no shortcut, `command_palette.rs:256`), the context menu "Export…" (`context_menu.rs:290-299`) and the batch dock (`render/overlays.rs:103`). The File menu has no Export item. No presets, no share. Decoded frames are **BGRA8** (`decoder.rs:447-461`), so a JPEG encoder needs a swizzle back to RGB. Context menu has no Share/Add to Album (`views/context_menu.rs:111-456`).
- **Decoder** (`decoder.rs`): JPEG via zune-jpeg with EXIF orientation applied (`:241-271`), but JPEGs with an edge >2560 px *and* larger than the requested size (so every thumbnail and the 2048 px viewer decode of a phone photo), plus any non-JPEG >8 MB, go through `sips -s format png -Z` (`:190-199,256-263,274-305`) **and that output is never passed through `apply_exif_orientation`** (verified by reading `decode_via_sips`: it loads the PNG and goes straight to `resize_to_frame`); HEIC/RAW go sips -> QuickLook (`:372-384`). Whether sips bakes orientation into the PNG is **not verified** (no fixture in the repo; E1 settles it). `tiff` is classified as an image (`scanner.rs:176`) but the `image` crate is built without the tiff feature, so small TIFFs always fall to QuickLook. **No ICC handling anywhere**: zune-jpeg exposes `icc_profile()` (verified in zune-jpeg 0.4.21 `decoder.rs:585`) but it is unused; Display P3 JPEGs are drawn as if sRGB. `sips -m <profile>` exists on this machine and `/System/Library/ColorSync/Profiles/sRGB Profile.icc` is present.
- **Scanner dimensions ignore EXIF orientation** for photos (`scanner.rs:207` `image::image_dimensions`, no swap; the EXIF `PixelXDimension` fallback at `:268-277` for HEIC/TIFF/RAW does not swap either; the only swap is `parse_tkhd_size` for video, `:609-611`), so portrait phone photos are stored landscape. Verified in code; the effect on the Panoramas/Screenshots aspect rules is inferred, and E1 confirms it with a fixture. Thumbnail cache keys include `CACHE_FORMAT_VERSION` (`cache.rs:68-80`), so a decode change is invalidated by bumping that constant.
- **Video:** loop is whole-file only (`player/macos_bridge.m:169,506-519`); the `VideoBackend` trait has no track/chapter API (`player/backend.rs:20-43`). Frames come from `AVPlayerItemVideoOutput`, which **does not render subtitles**, so subtitle tracks would need `AVPlayerItemLegibleOutput` plus drawing cues in GPUI. Resume is shipped (`app/video_actions.rs:10-47`).
- GPUI vector drawing works today (`PathBuilder::stroke` + `window.paint_path` in `app/render/toolbar.rs:258-280`).
- ObjC bridges compile through `crates/lumina-ui-gpui/build.rs` (one `cc::Build` with one `.file(...)`, AppKit already linked); a second `.m` is one more `.file` plus `rerun-if-changed`.
- Bundled binary assets are embedded with `include_bytes!` in `src/assets.rs:85-89` (fonts), not read from the app bundle at run time.

## Decisions

1. **Album order follows the view's `SortSpec`, no manual order column.** Rejected: a `position` column with drag reordering; it adds a sort mode only albums support and a reorder UI with no keyboard design yet. Adding the column later is a trivial idempotent `ALTER`. (Open question 1.)
2. **Membership table with `ON DELETE CASCADE` on both sides**, matching `media_tags`. Rejected: storing ids in JSON on the album row (no FK cleanup, no index for "albums containing item").
3. **⌫ inside an album removes from the album** (undo toast); ⌘⌫ keeps opening the Delete dialog. Rejected: adding "Remove from album" to the Delete dialog (`views/delete_modal.rs` options are `DeletionTarget`s; mixing a non-destructive action into a destructive dialog invites mistakes).
4. **Flag is a column, not a side table**: `flag INTEGER NOT NULL DEFAULT 0` (-1 reject, 0 none, 1 pick), read with `get_flags_map` like ratings. Partial index only if the bench shows need. Rejected: a rating of -1 (Bridge convention) because it would collide with the 0..=5 rating clamp and every rating query.
5. **Flags stay in the catalog, not XMP, for now.** Lightroom does not round-trip picks through XMP; writing `xmp:Rating=-1` would overwrite stars. (Open question 3.)
6. **Compare and survey are one mode** whose layout follows the count (2 -> side by side with synced zoom/pan, 3-4 -> 2×2, 5-9 -> 3×3, decoded at 1024 px when >2). Rejected: separate compare and survey views.
7. **Map: bundled offline basemap (recommended).** Natural Earth 1:110m land polygons (public domain; no attribution required, credited in THIRD_PARTY_NOTICES anyway), converted as-is (110m is already generalized; no extra simplification step) to a compact binary embedded with `include_bytes!` (~50-80 KB target, measured in C4; a hard ceiling of 200 KB, checked by a unit test), drawn with `PathBuilder` in an equirectangular projection; pins are clusters computed in core. Rejected: an explicitly user-initiated tile fetch (OpenStreetMap tiles need a usage-policy-compliant User-Agent, caching, attribution, and a product approval under AGENTS.md §5.4; it also leaks coarse location of the user's photos to a third party). **Owner decision required** (Open question 2); this plan implements only the offline map.
8. **Cluster click filters by bounding box** with a new `within:lat1,lon1,lat2,lon2` operator, not by city: with ~150 geocoder cities many GPS rows have no city, so a city filter would miss them.
9. **Stacks are a review view, not grid collapsing.** RAW+JPEG pairs (same folder + stem) and bursts (same non-empty make+model, gaps <= 1 s; timestamps are whole seconds, so this means "same or next second"; rows with no make/model never stack, because their mtime-fallback timestamps cluster after any batch copy) are computed on demand like duplicate groups. Rejected: collapsing stacks in the grid; it changes every count/slice/position path in `app/active_view.rs` and selection remapping. (Open question 4.)
10. **Two fixed export presets**: "Originals" (today's behaviour) and "JPEG, 2048 px, no location" (decode through the existing pipeline, bake rotation, encode with the `image` crate's JPEG encoder, which writes no EXIF). Rejected: user-defined presets and a preset editor (no demonstrated need).
11. **Color: convert to sRGB at decode time**, not wide-gamut rendering. JPEGs whose ICC profile is not sRGB, and every sips decode, go through `sips -m "sRGB Profile.icc"`. Rejected: adding `lcms2`/`qcms` (new C/Rust dependency, license review) and P3 surfaces in GPUI (renderer change out of scope).
12. **Video: loop range and audio tracks + chapters now; subtitles and trim deferred.** Subtitles need a legible-output renderer (above); trim writes new media files, which ROADMAP §7 lists as "later, if it fits the non-editor scope".
13. **Digit keys rate, P/X/U flag, everywhere (grid, viewer, compare)**; viewer zoom presets move to ⌘0/⌘1/⌘2/⌘4 (verified unbound everywhere: no handler in `app/keymap/` checks a digit with ⌘, and menus carry no key equivalents). Plain 0 clears the rating, as `Rate(0)` does in the palette. Rejected: ⌥-digits for ratings in the viewer (inconsistent and undiscoverable). This changes an existing key contract. (Open question 5.)

---

## Part A: Curated albums and smart-album polish

### Task A1: Album tables, engine API, metadata carry-over  (M)

**Files:**
- Modify: `crates/lumina-core/src/catalog/schema.rs`, `catalog/mod.rs` (next `SCHEMA_VERSION`, currently 14; renumber after plan 4), `catalog/albums.rs` (`list_smart_albums` SELECT, mapper and `restore_smart_album` read/write `pinned`; `SmartAlbum` gains `pinned: bool`), `catalog/duplicates.rs` (`merge_metadata_in_tx`), `crates/lumina-core/src/lib.rs`
- Create: `crates/lumina-core/src/catalog/curated.rs` (keeps `albums.rs`, which is search/smart code, from mixing concerns)

**Interfaces:**
- Schema:
  ```sql
  CREATE TABLE IF NOT EXISTS albums (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      title TEXT NOT NULL,
      created_at_epoch_ms INTEGER NOT NULL,
      pinned INTEGER NOT NULL DEFAULT 0
  );
  CREATE TABLE IF NOT EXISTS album_items (
      album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
      media_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
      PRIMARY KEY (album_id, media_id)
  ) WITHOUT ROWID;
  CREATE INDEX IF NOT EXISTS idx_album_items_media ON album_items(media_id);
  -- guarded ALTER: smart_albums.pinned INTEGER NOT NULL DEFAULT 0
  ```
- `pub struct Album { pub id: i64, pub title: String, pub created_at_epoch_ms: i64, pub pinned: bool }`
- `Catalog::{create_album(&self, title: &str) -> Result<Album>, rename_album, delete_album(id) -> Result<Option<(Album, Vec<MediaId>)>>, restore_album(&Album, &[MediaId]) (re-inserts the row with its old id; members purged meanwhile are skipped with `INSERT OR IGNORE ... SELECT ?, id FROM media_items WHERE id = ?`, because `OR IGNORE` does not cover FK failures), list_albums_with_counts() -> Result<Vec<(Album, usize)>> (one `LEFT JOIN ... GROUP BY`, active members only, so the refresh pass adds one query, not two), add_to_album(id, &[MediaId]) -> Result<usize> (same purged-id filter, so undo of a remove cannot fail on a purged item), remove_from_album(id, &[MediaId]) -> Result<usize>, query_album_sorted(id, limit, offset, SortSpec) -> Result<Vec<MediaItem>>, query_album_count(id), set_album_pinned(id, bool), set_smart_album_pinned(id, bool)}`; thin `LuminaEngine` wrappers (`read(...)` for getters, as `get_ratings_map` does at `lib.rs:659`).
- Album SQL: `FROM media_items m JOIN album_items ai ON ai.media_id = m.id WHERE ai.album_id = ? AND m.is_trashed = 0 AND m.is_hidden = 0 ORDER BY {sort.order_by("m.")}`. Titles through `clean_text(.., MAX_NAME_CHARS)`.
- `merge_metadata_in_tx` gains `INSERT OR IGNORE INTO album_items(album_id, media_id) SELECT album_id, ?1 FROM album_items WHERE media_id = ?2` (and flags in B1). Duplicate merge only trashes the copies, so the copy keeps its own membership too; restoring it from Trash puts both back in the album, which is correct (they are two files).

- [ ] Failing tests (unit, `catalog/curated.rs`): `migration_creates_album_tables_idempotently` (previous-layout db built by hand, open twice, `user_version == SCHEMA_VERSION`, `smart_albums.pinned` added with existing rows kept and `pinned == 0`); `add_is_idempotent_and_counts_active_only` (trashed member not counted); `album_items_cascade_on_media_delete` (`permanently_delete_items(.., CatalogOnly)` -> no orphan rows); `delete_album_returns_members_and_restore_round_trips` (same id, same members); `restore_album_skips_purged_members` (delete album, purge one member, restore: `Ok`, other members back); `trashed_member_keeps_membership_and_returns_on_restore`; `smart_album_pinned_round_trips_through_list_and_restore`; `album_query_follows_sort_spec` (date desc/asc, name); `merge_duplicates_carries_album_membership` and `fold_into_carries_album_membership` (in `catalog/duplicates.rs` / `catalog/files.rs` tests; the fold case matters most, because fold *deletes* `other` and the cascade would drop its rows).
- [ ] Integration (`tests/core_tests.rs`): `album_query_plan_uses_membership_index` (`EXPLAIN QUERY PLAN` contains `album_items` PK or `idx_album_items_media`, no `SCAN media_items` for a 10k-row library).
- [ ] Bench: add album slice + count queries to `benchmark_scale_and_gates` under the existing `album_budget` (8 ms release) for a 500-member and a 20k-member album in 100k rows; the join cannot use `idx_media_timestamp` for the ORDER BY, so the big album measures the temp B-tree sort per page. If the 20k case misses the gate, investigate (e.g. seek by the last row's sort key instead of OFFSET) rather than relaxing it (§10). Record numbers in the commit.
- [ ] `cargo test -p lumina-core curated duplicates` FAIL -> implement -> PASS.

### Task A2: Sidebar section, album view, New Album  (M)

**Files:** `views/sidebar/mod.rs` (`AlbumSelection::Album(i64)`), `views/sidebar/sections.rs` (new `render_albums_section` above smart albums; `sections.rs` is 660 lines, so put it in new `views/sidebar/albums.rs`), `app/active_view.rs` (count/slice arms), `config.rs` (`LastView::Album(i64)`, token `album:{id}`; an unknown id falls back to All on load), `app/navigation.rs::resolve_navigation_selection` (deleted album -> AllLibrary), `app/library.rs::refresh_album_counts` (one `list_albums_with_counts()` call; it already runs after trash/restore/delete/watcher batches, so member counts follow those), new `app/albums.rs` (state struct `AlbumsState { list, counts, renaming: Option<i64> }` on `LuminaApp`), `app/command_palette.rs`, `menus.rs`, `i18n.rs`.
- debug_selectors: `sidebar-albums-header`, `new-album`, `album-row-{id}`, `album-rename-{id}`, `album-delete-{id}`.
- Keyboard/menu: File > New Album (⌘N, always empty: a single selected item is the normal grid state, so "from the selection when one exists" would silently fill most new albums) and "New Album with Selection" (palette, context menu, and the picker's "New Album…" row), plus palette "Rename Album…", "Delete Album…". A new album starts in rename mode. Rename reuses the sidebar tag-rename inline field pattern (`sections.rs:420-444`). Delete reuses the smart-album confirm modal pattern (`views/smart_album_delete.rs`) with `UndoAction::AlbumDeleted { album, members }`. Rename pushes `UndoAction::AlbumRenamed { id, before, after }` (ux_audit §7 acceptance: "album edits are reversible").
- Album rows get `id` + `.role(Role::ListItem)`/`aria_label` per AGENTS.md §11 (not claimed VoiceOver-verified).

- [ ] Failing UI tests (`src/tests/albums.rs`, new): `new_album_from_selection_shows_in_sidebar_with_count`; `cmd_n_creates_empty_album_in_rename_mode`; `album_view_lists_members_in_sort_order`; `album_count_follows_trash_and_restore` (trash a member: sidebar count drops; restore: back); `album_rename_undo`; `album_write_failure_shows_banner` (inject `Err` at the albums result funnel; sidebar unchanged); `delete_album_confirms_and_undo_restores_members`; `deleted_album_selection_falls_back_on_back_navigation`; `album_selection_persists_across_restart_token` (config round-trip without `save_config` writes, per §2).
- [ ] Implement; `cargo test -p lumina-ui-gpui albums` PASS.

### Task A3: Add/remove from selection, context menu, ⌫, undo  (M)

**Files:** `app/albums.rs`, new `views/album_picker.rs` (overlay: filter field + list + "New Album…" row; occluding card + backdrop per §6.3), `views/context_menu.rs` ("Add to Album…", and "Remove from Album" when the view is an album), `app/keymap/grid.rs` (⌫ in album view), `app/keymap/modals.rs` (absorb keys while the picker is open), `app/undo.rs` (`UndoAction::AlbumMembership { album: i64, ids: Vec<MediaId>, added: bool }`), palette + i18n.
- `LuminaApp::{open_album_picker, add_selection_to_album(id), remove_selection_from_album(cx)}`; toasts "Added N items to “{album}”" / "Removed N items from “{album}”" with Undo.
- Shortcut: ⇧⌘A opens the picker (↑/↓, type to filter, ↵ adds, Esc closes). **⌘A at `keymap/mod.rs:85` ignores Shift today**, so add `!shift` to Select All and handle ⇧⌘A before it.
- debug_selectors: `album-picker`, `album-picker-backdrop`, `album-picker-row-{id}`, `album-picker-new`, `context-add-to-album`, `context-remove-from-album`.

- [ ] Failing UI tests: `add_to_album_via_keyboard_picker` (⇧⌘A, type, ↵ -> DB rows); `context_menu_add_to_album` (right-click card, click item, click row); `backspace_in_album_removes_membership_only` (file still on disk, `is_trashed == 0`, no delete dialog); `cmd_backspace_in_album_opens_delete_dialog`; `album_membership_undo_redo`; `shift_cmd_a_opens_picker_not_select_all`; `album_picker_absorbs_delete_and_text` (⌫ in the filter field does not remove/trash).
- [ ] Implement; tests PASS.

### Task A4: Pin, duplicate, rule export/import  (S)

**Files:** `views/sidebar/albums.rs` + `sections.rs` (pinned albums and smart albums render first in a "Pinned" group), `app/album_builder_actions.rs` (duplicate; export/import via `rfd` save/open, the dialog already used for export), `lumina-core/src/smart_albums/rules.rs` (`pub fn to_portable_json(&SmartAlbum) -> String`, `pub fn from_portable_json(&str) -> Result<PortableSmartAlbum>` with `{"lumina_smart_album": 1, title, icon_name, match_mode, rules}`).
- Import validates with `SmartAlbum::build_sql()` before saving, the same check the builder uses (`album_builder_actions.rs:158-163`; the count helpers turn errors into 0, so counting is not a validation). Unknown rule fields (e.g. a file from a newer build with `RuleField::Flag`) are a parse error shown in the banner. `pinned` is not exported. The file read and write run off the UI thread (rfd's `AsyncFileDialog`, already a dependency: `library.rs:669`).
- Palette: "Pin Album", "Unpin Album", "Duplicate Smart Album", "Export Smart Album Rules…", "Import Smart Album Rules…". Tests stub the file dialogs with the thread-local pick counter pattern (`FOLDER_PICKS`, `app/library.rs:1543`; used in `tests/browser.rs:61`) and call the post-dialog function with a temp path.

- [ ] Failing tests: core `portable_json_round_trips_every_rule_field` and `portable_json_rejects_unknown_version`; UI `pinned_albums_render_first`, `duplicate_smart_album_copies_rules`, `import_rejected_rule_shows_banner`.
- [ ] Implement; PASS. Independent review of Part A; fix; gates (fmt, clippy, `cargo test --workspace`, `make bench`, `make ui-smoke`).

### Task A5 (optional): Cover thumbnails and drag to album  (S)

Sidebar rows show a 16 px cover (first item in the album's current sort, loaded with `request_thumbnail(.., 64, ..)`), and dragging a grid selection onto an album row adds it (GPUI typed `on_drag`/`on_drop`, distinct from the root file `on_drop`). Keyboard path is A3. Tests: `drop_selection_on_album_row_adds_members`. Skip if the owner does not need it for 0.6. The cover id comes from the same `list_albums_with_counts` query (`MIN`/first by sort is a correlated subquery per album; measure it) — never a per-frame query.

**Docs (Part A):** README Features, ROADMAP §5 (move curated albums/pin/duplicate/rule JSON to baseline) and the backlog row (`ROADMAP.md:531`), `docs/ux_audit.md` §7 (`:232-233`: the smart-album delete line is stale, and pin/duplicate/rule export now ship), CHANGELOG, shortcut sheet.

---

## Part B: Culling

### Task B0: Move viewer zoom presets to ⌘digits  (S, own commit: key-contract change)

Lands first so the contract change is reviewable on its own and digits are free for B2. Needs the owner's yes on Open question 5.

**Files:** `app/keymap/viewer.rs:48-60` (match `cmd_or_ctrl` instead of `!cmd_or_ctrl`; plain digits fall through unhandled until B2), `views/viewer/stage_controls.rs:13,30` (tooltip "Fit to window (⌘0)"), `app/render/mod.rs:153` (sheet row "⌘0 / ⌘1 / ⌘2 / ⌘4"), doc comments at `viewer_state.rs:97,245`, `viewer_actions.rs:437`, `keymap/viewer.rs:10,39`; if plan 3 D3 (zoom palette entries) landed, those entries' shortcut column; `docs/ui_v3_plan.md:41,62`, `docs/ux_audit.md:142`.
- [ ] Failing test: rewrite `test_viewer_zoom_presets_hud_label_and_shortcuts` (`tests/viewer_and_video.rs:564-643`) as `cmd_digit_zoom_presets_in_viewer`: ⌘1/⌘2/⌘4/⌘0 set the presets, and plain `2` leaves `zoom_scale` at Fit. Update the navigator test's `keystrokes(vcx, "2")` (`viewer_and_video.rs:1518`) to `cmd-2`.
- [ ] Implement; `grep -rn '0/1/2/4\|0 / 1 / 2 / 4\|(0)' crates docs` returns nothing stale; PASS.

### Task B1: Flag column, search, merge  (S)

**Files:** `catalog/schema.rs` (guarded `ALTER TABLE media_items ADD COLUMN flag INTEGER NOT NULL DEFAULT 0`), `catalog/mod.rs` (one more `SCHEMA_VERSION` bump after Part A's; renumber after plan 4), `catalog/media.rs` (`set_flags(&self, &[(MediaId, Flag)])` through the existing one-transaction `update_each` helper (`media.rs:392`), `get_flags_map() -> HashMap<MediaId, Flag>` with `WHERE flag != 0`), `catalog/duplicates.rs` (`merge_metadata_in_tx` step 3 reads `flag` with the other columns: keeper takes the max, so pick beats unflagged beats reject), `search.rs` (`is:pick`, `is:reject`, `is:unflagged` -> `m.flag = 1/-1/0`; new arms in the `is` match at `search.rs:86-93`), `lib.rs` (`pub enum Flag { Reject = -1, Unflagged = 0, Pick = 1 }` — not `None`, which would shadow `Option::None` in glob imports; engine wrappers).

- [ ] Failing tests: `migration_adds_flag_idempotently` (Part A's layout -> `SCHEMA_VERSION`, open twice, existing rows `flag == 0`); `set_flags_round_trips_and_map_omits_unflagged` (an `UPDATE` on an unknown id affects 0 rows and is not an error, so there is no "atomic on unknown id" case to test); `search_is_pick_and_reject`; `merge_keeps_pick_over_reject` and `fold_into_keeps_pick` (both `merge_metadata_in_tx` callers); bench line for `is:reject` under the 5 ms structured-query gate (add `CREATE INDEX ... ON media_items(flag, timestamp_epoch_ms DESC, id DESC) WHERE flag != 0 AND is_trashed = 0 AND is_hidden = 0` only if the measurement misses the gate; it must be created after the guarded `ALTER`, see Global Constraints). Add an `EXPLAIN QUERY PLAN` assertion in `core_tests.rs` for `is:pick` sorted by date (no `USE TEMP B-TREE` if the index is added).

### Task B2: Keyboard-first culling in grid and viewer  (M)

**Files:** new `app/culling.rs` (`set_flag_selected(flag, cx)` mirroring `set_rating_selected`: toggle-off when all equal, one write, `UndoAction::Flag { changes }`, sidecar sync not needed), `app/state.rs` (`item_flags: HashMap<MediaId, Flag>` loaded beside `item_ratings` at `library.rs:1112`), `app/keymap/grid.rs` + `keymap/viewer.rs` (P pick, X reject, U unflag, 0-5 rate, all without ⌘/⌥ so ⌥P PiP and ⌥⌘U stay; color labels stay on the palette/context menu), `app/render/grid.rs` (flag badge; rejected cards drawn at a dedicated `REJECTED_OPACITY` constant, not the `GridFilter` dimming that plan 3 Task 9 deletes), palette quick filters "Show Picks" / "Show Rejected" as plan 3 `CommandId::QuickFilter("is:pick")` / `("is:reject")` tokens (if plan 3 Task 9 has not landed, this plan waits for it rather than extending `GridFilter`), palette (`Flag(Flag)` commands with shortcuts P/X/U; fill the empty `Rate` shortcut column with 0-5), `views/context_menu.rs` (Pick/Reject row), shortcut sheet.
- Auto-advance: in the viewer, a flag or rating **key** on a single item advances to the next item (Lightroom's "auto advance"); the context menu and palette never advance; in the grid nothing moves the selection. At the last item it stays put.
- After a flag write, call `refresh_counts_if_rules_use(RuleField::Flag)` once B3 adds the field (as ratings do, `navigation.rs:377`).
- debug_selectors: `grid-flag-badge-{id}`, `context-flag-pick`, `context-flag-reject`. (No filter-chip selectors: quick filters are palette tokens after plan 3.)

- [ ] Failing UI tests (`src/tests/culling.rs`, new): `p_x_u_flag_selection_and_undo_groups_batch` (asserts DB rows via `get_flags_map`, not only `item_flags`); `digit_keys_rate_selection_in_grid_and_viewer` (0 clears); `viewer_flag_auto_advances` (and stays on the last item); `alt_p_still_toggles_pip`; `show_picks_palette_filter_adds_is_pick_token`; `flag_keys_absorbed_by_modals_and_text` (palette open, tag field focused, delete dialog open, album picker open -> no flag/rating written); `flag_write_failure_shows_banner_and_keeps_state` (inject `Err` at the culling result funnel, the `apply_tag_mutation` pattern at `tests/tags_and_albums.rs:438`; `with_lock_held` only blocks, it cannot make a write fail).
- [ ] Implement; PASS.

### Task B3: Finish a cull: smart rule and "Delete Rejected…"  (S)

**Files:** `smart_albums/rules.rs` (`RuleField::Flag` with is/is-not), `app/smart_draft.rs` (field option), `app/culling.rs` (`delete_rejected(cx)`: resolves the rejected ids of the current view in core — one query ANDing `m.flag = -1` with the active view's WHERE (library, album, smart album, tag, year, search) — selects them and opens the existing Delete dialog, so trash/undo/permanent choices stay where they are; it must **not** write `is:reject` into the search box, because plan 3 D11 would move the sidebar to Library and widen the set), palette "Delete Rejected…", File menu (there is no Edit menu). Zero rejects -> toast "No rejected items here", no dialog.
- [ ] Failing tests: core `flag_rule_matches_picks`; UI `delete_rejected_opens_dialog_with_only_rejects` (inside an album with a reject outside it: dialog count is the album's rejects only; Esc deletes nothing); `delete_rejected_with_none_shows_toast`; `smart_album_with_flag_rule_recounts_after_flag_key`.
- [ ] Implement; PASS. Independent review of Part B; gates.

**Docs (Part B):** README, ROADMAP §5/§6 and backlog row (`ROADMAP.md:532`), CHANGELOG (call out the ⌘digit zoom change from B0), shortcut sheet, `docs/search.md` (`is:pick`, `is:reject`, `is:unflagged`), `docs/accessibility.md` (keyboard culling flow).

---

## Part C: Compare, slideshow, map

### Task C1: Compare / survey mode  (L)

**Files:** new `app/present.rs` (`CompareState { ids: Vec<MediaId>, focus: usize, textures: HashMap<MediaId, Arc<RenderImage>>, view: ViewerCanvas }`), new `views/compare.rs`, `app/render/mod.rs` (render instead of grid/viewer when active), `app/keymap/modals.rs` (compare absorbs keys), palette/menu (View > Compare, shortcut C in the grid with 2-9 selected; C is unbound today).
- Closing compare (or dropping a pane) frees each texture with `cx.drop_image(image, Some(window))`, as the viewer does (`viewer_actions.rs:92`); dropping the `Arc` alone leaves it in the atlas.
- Layout per Decision 6. Decode size `VIEWER_IMAGE_SIZE` (2048, `viewer_actions.rs:47`) for 2 items, 1024 for more. 2-up shares one `ViewerCanvas` zoom/pan so both panes move together.
- Keys: ←/→ (and Tab) move focus, P/X/U and 0-5 act on the focused item (B2 handlers with an explicit id), ⌫ drops the focused item from the compare set (never deletes), ↵ opens it in the viewer and Esc there returns to compare (not the grid), Esc in compare closes it. ⌘Z/⇧⌘Z pass through (flag/rating undo); zoom uses +/−, Z and the B0 ⌘digit presets; every other key, including ⌫'s global paths and ⌘A, is absorbed. 10+ selected -> toast "Compare up to 9 items", no mode. Requires B0 and B2.
- debug_selectors: `compare-view`, `compare-pane-{id}`, `compare-pane-focused`.

- [ ] Failing UI tests (`src/tests/compare.rs`): `c_opens_compare_with_selection_and_esc_closes`; `compare_flag_acts_on_focused_pane`; `compare_backspace_removes_pane_not_media`; `two_up_zoom_is_synchronized`; `survey_uses_reduced_decode_size` (assert requested size 1024 for 4 items via a test counter on requests); `compare_needs_two_items` (1 selected -> toast, no mode); `compare_close_drops_textures` (atlas/texture count back to the pre-open value, or the drop counter in `state.rs:500`'s pattern); `compare_enter_then_esc_returns_to_compare`; `compare_absorbs_cmd_a_and_delete_dialog`.

### Task C2: Slideshow  (M)

**Files:** `app/present.rs` (`SlideshowState { interval: Duration, paused: bool, task: Option<Task<()>> }`), reuse the immersive fullscreen viewer (`state.rs:342-344`) and `navigate_selection(1)`; `config.rs` (`slideshow_interval_secs: u8`, 3/5/10, default 5), palette/menu (View > Slideshow, ⌥⌘S: verified unbound — no handler in `app/keymap/` checks `s`), `config.rs` `slideshow_loop: bool` (default false; palette "Slideshow: Loop" toggles it), shortcut sheet.
- Plays the current view from the selection (or the start); the slideshow key handler runs **before** `handle_viewer_key_down` so Space pauses instead of playing a video and ←/→ step instead of seeking; Space pauses, ←/→ step, Esc exits (one press: it leaves the slideshow and the immersive viewer together); modals and the palette are not reachable while it runs (keys other than these, ⌘Q and ⌘, are absorbed); videos show their poster for the interval (no autoplay; open question 6); stops at the end unless Loop is on; no transition animation when `motion::reduce_motion()`.
- [ ] Failing UI tests (`src/tests/slideshow.rs`): `slideshow_advances_on_clock` (`executor().advance_clock`); `space_pauses_and_arrows_step`; `esc_exits_to_previous_selection`; `slideshow_stops_at_end_without_loop`; `slideshow_loop_wraps_to_first`; `space_on_video_pauses_slideshow_not_video`; `slideshow_reduce_motion_has_no_transition` (no fade animation started when `reduce_motion()` is forced on in the test).

### Task C3: Geo points, clustering, `within:`  (M)

**Files:** `catalog/media.rs` (`geo_points() -> Result<Vec<GeoPoint>>`, `pub struct GeoPoint { pub id: MediaId, pub lat: f32, pub lon: f32 }`, active rows with lat/lon only), new `crates/lumina-core/src/geo_cluster.rs` (pure: `pub fn cluster(points: &[GeoPoint], cell_deg: f32) -> Vec<Cluster>`, `pub struct Cluster { pub lat: f32, pub lon: f32, pub count: usize, pub bounds: [f32; 4], pub sample: MediaId }`, grid bucketing O(n)), `search.rs` (`within:lat1,lon1,lat2,lon2`, inclusive; antimeridian: lon1 > lon2 means wrap), `catalog/schema.rs` (index `idx_media_geo ON media_items(latitude, longitude) WHERE latitude IS NOT NULL AND is_trashed = 0 AND is_hidden = 0`, only if the bench below misses the gate; the columns already exist, so it can go in the main `CREATE INDEX IF NOT EXISTS` batch and needs no `SCHEMA_VERSION` bump — an index-only change on existing columns is idempotent and old builds ignore it).
- [ ] Failing tests: `clusters_merge_by_cell_and_split_on_zoom`; `cluster_bounds_contain_members`; `within_filters_bbox_and_wraps_antimeridian`; `within_ignores_malformed_value` (`within:1,2,3` or non-numeric -> no geo filter, exactly as a bad `color:` value is dropped today, `search.rs:104-117`; a known prefix never becomes an FTS term); `within_query_plan` (`EXPLAIN QUERY PLAN` in `core_tests.rs` for a city-sized and a world-sized box sorted by date; record which index the planner picks). Bench: `within:` for both boxes in `benchmark_scale_and_gates` under the existing 5 ms structured-query release gate (Review Focus 8); `geo_points` for 100k rows and `cluster` for 100k points measured with a debug budget assert only (they are not queries the gate covers; owner sets a release gate after the number exists — do not invent one).

### Task C4: Offline map view  (M)

**Files:**
- Create: `scripts/build_world_basemap.py` (maintainer-run, documented: Python 3 standard library only; reads a locally downloaded Natural Earth `ne_110m_land` GeoJSON whose release version and SHA-256 are pinned in the script, like `scripts/fetch_*.sh` pin theirs; converts without simplifying; writes `resources/map/land_110m.bin` as `u32 ring count, [u32 len, (f32 lon, f32 lat)*]`; never run by the build), `resources/map/land_110m.bin` (committed, embedded with `include_bytes!` next to the fonts in `src/assets.rs`, so tests and the bundle read the same bytes), `views/map.rs`, `app/map.rs` (`MapState { zoom: u8, center: (f32, f32), clusters: Vec<Cluster>, focus: Option<usize> }`, points loaded off-thread and cached until the data revision changes).
- Modify: `views/sidebar/sections.rs` (Places row gets a Grid/Map toggle; selection stays `System(Places)`), palette "Show Map" / "Show Grid" and View > Show Map (keyboard path to the toggle; it selects Places first), `app/render/mod.rs`, `app/keymap/` (the map handler runs before `handle_grid_key_down`, or ⌘=/⌘- would zoom the hidden grid, `grid.rs:73-81`), `THIRD_PARTY_NOTICES.md` (Natural Earth, public domain), `README.md` "Third-party assets".
- Rendering: equirectangular, land as filled `PathBuilder` paths in `theme` colors; clusters as circles with counts; zoom 1-6 (cell size halves per step), drag to pan, ⌘=/⌘- zoom.
- Keyboard: Tab/arrow keys move cluster focus (sorted west->east), ↵ applies `within:` for that cluster and returns to the grid (with plan 3 D11 this moves the sidebar to Library; Back returns to Places in map mode), Esc returns to the grid. Zoom keeps the focused cluster centred, so arrow-key panning is not needed. Empty state when no geotagged items ("No photos with a location yet").
- debug_selectors: `map-view`, `map-toggle`, `map-cluster-{index}`, `map-empty`.

- [ ] Failing UI tests (`src/tests/map.rs`): `map_empty_state`; `map_shows_cluster_counts_for_seeded_gps`; `enter_on_focused_cluster_filters_grid` (search query begins `within:` and the count matches); `basemap_asset_parses` (unit, ring count > 0, all coords in range, asset ≤ 200 KB); `cmd_plus_zooms_map_not_grid`; `map_toggle_reachable_from_palette`. Review checks the diff has no `curl`, `http`, `URLSession` or socket use (Review Focus 5); a test cannot prove absence of network.
- [ ] `make ui-smoke` screenshot of the map (artifact under `target/`, not committed). Independent review of Part C; gates.

**Docs (Part C):** README, ROADMAP §8 and backlog row (`ROADMAP.md:533`), CHANGELOG, shortcut sheet, `docs/search.md` (`within:`), `docs/ui_v3_plan.md:37` (map now offline).

---

## Part D: Stacks, export presets, share sheet

### Task D1: Stack grouping in core  (S)

**Files:** new `crates/lumina-core/src/stacks.rs`: `pub enum StackKind { RawJpeg, Burst }`, `pub struct Stack { pub kind: StackKind, pub items: Vec<MediaId> }`, `LuminaEngine::find_stacks(&self) -> Result<Vec<Stack>>` (one ordered read of `(id, parent_dir, file_name, make, model, timestamp_epoch_ms, media_type)` for active rows; stem computed in Rust, compared case-insensitively; RAW+JPEG = same dir and stem, one Raw + one Image; Burst = same non-empty make **and** model, consecutive gaps <= 1000 ms (whole-second timestamps, see Current state), >= 3 items; an item already in a RAW+JPEG pair can also be in a burst, and the pair counts once). Run on the background executor when the tab opens, not in `refresh_album_counts`. Exact duplicates stay in `duplicates.rs`; stacks never touch `media_hashes`.
- [ ] Failing tests: `raw_jpeg_pair_grouped_by_stem`; `burst_needs_three_within_one_second`; `different_cameras_never_stack`; `rows_without_camera_never_burst` (screenshots copied together share an mtime second); `trashed_items_never_stack`; bench measurement for 100k rows (recorded, debug budget only).

### Task D2: Stacks review view  (M)

**Files:** extend the Duplicates destination rather than adding a sidebar row: `views/duplicates.rs` gets a segmented "Exact copies | Similar shots" header (file is 356 lines; put the new list in `views/stacks.rs`), per stack "Keep picks, reject the rest" (requires B2) and "Reject all but the first". Nothing is deleted here; rejects flow into B3.
- debug_selectors: `stacks-tab`, `stack-{index}`, `stack-keep-picks-{index}`.
- [ ] Failing UI tests: `stacks_tab_lists_raw_jpeg_pair`; `keep_picks_rejects_others_with_one_undo`; `stacks_tab_empty_state` (ROADMAP §8 acceptance: duplicate views have empty and error states); keyboard: ↑/↓ move between stacks and ↵ runs "Keep picks" on the focused one (`stacks_keyboard_keep_picks`).

### Task D3: Export presets  (M)

**Files:** `crates/lumina-core/src/export.rs` (new; first a **separate no-behaviour commit** that moves `export_items` + `unique_destination` out of `lib.rs`, 1701 lines; then the preset), `pub enum ExportPreset { Originals, Jpeg2048 }`, `LuminaEngine::export_items_with(&self, ids, dest, preset, cancel: &AtomicBool) -> Result<ExportReport>`. Jpeg2048: `DecoderPipeline::decode_media(.., 2048)` (EXIF orientation + sRGB from Part E), `rotate_frame(frame, rotation)` for catalog rotation (`decoder.rs:394`), swizzle the BGRA8 frame to RGB8, `image::codecs::jpeg::JpegEncoder::new_with_quality(.., 85)` (writes JFIF only, no EXIF/XMP/ICC), name `{stem}.jpg` with the existing collision suffixing; no sidecar (the preset's point is a clean file). Decode bypasses the thumbnail disk cache. Videos under this preset are copied as originals and counted in the report ("2 videos exported as originals"). If plan 4 Part D (quick edits) has landed, apply its edits with the same function the viewer uses, and test it. Runs on the background executor with the existing export progress/toast. Correct orientation for >2560 px sources depends on E2, so Part E lands before D3 or D3's tests use ≤2560 px fixtures only and E1 adds an export cell.
- UI: `app/library.rs` `export_pick_folder` takes a preset; palette "Export Originals…" (the existing `ExportSelection`, `command_palette.rs:256`, renamed; it has no shortcut today) and "Export as JPEG…", both also in the File menu (new items; it has none today); context menu and batch dock "Export…" stay Originals; config remembers nothing (YAGNI).
- [ ] Failing core tests (`tests/core_tests.rs`): `jpeg_export_applies_rotation_and_strips_gps` (fixture with GPS EXIF; output has no EXIF APP1; dimensions rotated); `jpeg_export_never_overwrites`; `export_cancel_stops_between_files`; `missing_source_reported_not_fatal`; `jpeg_export_copies_videos_as_originals`; `jpeg_export_pixels_are_rgb_not_bgr` (red fixture stays red).
- [ ] Failing UI test: `export_as_jpeg_uses_picked_folder` (stubbed picker).

### Task D4: Share sheet  (S)

**Files:** create `crates/lumina-ui-gpui/src/share/mod.rs` + `share/macos_share.m` (`void lumina_share_files(const char *const *paths, int count, double x, double y)`: builds `NSURL`s, shows `NSSharingServicePicker` relative to `[NSApp keyWindow].contentView` at the flipped point; main thread only, asserted with `[NSThread isMainThread]`; the picker is kept in a static strong reference until its delegate reports dismissal, so it is not released while showing; no window -> returns without showing), `build.rs` (one more `.file("src/share/macos_share.m")` and `rerun-if-changed`), `views/context_menu.rs` ("Share…" at the clicked card), palette "Share…" (anchors at the window center), batch-dock button with tooltip and `aria_label` (no new toolbar button: the context menu and palette already cover single items).
- Shares originals only. Lumina makes no network call itself; the services are the system's (ROADMAP "Out of scope", `ROADMAP.md:557`, keeps the native share sheet in scope). Non-macOS builds compile the module out and hide the commands. Missing files are filtered out before the call; an all-missing selection shows a toast instead of an empty picker.
- Tests: `SHARE_REQUESTS` thread-local counter under `cfg(test)` (same pattern as `IMPORT_PICKS`, `app/import.rs:269-271`); `share_from_palette_requests_selected_paths`; `share_hidden_for_missing_files` (a missing item is not offered). The real picker is checked by `make ui-smoke`/manual run only, and reported as such.
- [ ] Independent review of Part D; gates.

**Docs (Part D):** README, ROADMAP §8/backlog rows, THIRD_PARTY_NOTICES unchanged.

---

## Part E: RAW/HEIC validation and color

### Task E1: Format matrix tests (decides E2/E3 scope)  (M)

**Files:** `crates/lumina-core/tests/format_matrix.rs` (new, `#![cfg(target_os = "macos")]`), small fixture builder in the test: a 64×32 PNG with a red top-left quadrant, converted at test time with `sips` to JPEG/HEIC/TIFF and to a 3000×1500 JPEG (forces the sips path), plus an EXIF APP1 orientation-6 segment spliced into the JPEGs by hand (no new crate). P3 fixture: `sips -m "Display P3.icc"` + embed.

Matrix (each cell asserts pixels, not "no panic"):

| Format | Path | Upright (orientation 1/6) | Color (sRGB vs P3) | Scanner dims |
| --- | --- | --- | --- | --- |
| JPEG ≤2560 | zune | assert | assert | assert swapped for 6 |
| JPEG >2560 | sips | assert (unknown today) | assert | assert |
| PNG / WebP | image | n/a | sRGB only | assert |
| HEIC | sips -> QL | assert (sips-converted orientation) | assert | assert |
| TIFF | QuickLook | assert | — | assert |
| RAW (CR3/NEF/ARW/DNG) | sips -> QL | `#[ignore]` test `raw_fixture_matrix` over `$LUMINA_RAW_FIXTURES`; owner-supplied files, never committed; reported as skipped when unset | — | assert |
| Video | AVFoundation | covered by `video_frame` tests | — | covered |

- [ ] Write the tests; record which cells fail on current `main` in the commit body. Expected failures (unverified): JPEG >2560 with orientation 6, P3 color, scanner dims for orientation 6.

### Task E2: Orientation fixes  (M, only for cells E1 shows failing)

**Files:** `decoder.rs` (read orientation once from the source with kamadak-exif, apply after any sips/QuickLook decode unless E1 shows sips already rotates), `scanner.rs:207` (swap width/height for orientations 5-8), `lib.rs` `run_scan` (one-time background backfill: for image/raw rows, re-read EXIF orientation and fix dims, guarded by an `app_config` key `dims_oriented_v1`, logs with `log::warn!`, never blocks the UI), bump `CACHE_FORMAT_VERSION` (part of every key, `cache.rs:68-80`) so stale wrong-orientation thumbnails are regenerated; old files age out through the existing size trim. The backfill writes dims in one transaction per batch and refreshes counts once at the end (Panoramas/Screenshots counts change).
- [ ] Tests: E1 cells go green; `scanner_swaps_dims_for_orientation_6` (unit, fixture from E1's builder); `backfill_runs_once_and_fixes_portrait_dims` (second open does not re-read files: guard key present); `panorama_rule_uses_oriented_dims`; `orientation_applied_once_not_twice` (zune path still upright after the change, so a sips-rotated output is not rotated again).

### Task E3: Convert to sRGB at decode  (S)

**Files:** `decoder.rs`: in `decode_jpeg_simd`, if `decoder.icc_profile()` is present and its `desc` tag is not an sRGB description (a ~30-line parser of the `desc`/`mluc` tag; an unparseable profile counts as non-sRGB), decode via sips with `-m "/System/Library/ColorSync/Profiles/sRGB Profile.icc"`; add the same `-m` to `decode_via_sips` for every format. Bump `CACHE_FORMAT_VERSION` again only if E2 has already shipped in a release; otherwise one bump covers both. Non-macOS: unchanged (documented limitation).
- [ ] Tests: E1 color cells green; thumbnail throughput measured with `make bench` before/after (the extra color match cost on sips decodes is the risk). Independent review of Part E; gates.

**Docs (Part E):** README "Supported formats" with the matrix, ROADMAP backlog row, AGENTS.md §15 (wide gamut is clipped to sRGB; RAW is the embedded/system rendering, not a develop pipeline).

---

## Part F: Video (brief)

### Task F1: Loop range  (S)

In/out points per open video (⌥I and ⌥O while a video is focused in the viewer; ⌥X clears; not persisted — open question 7). Plain I and O are taken (I toggles the inspector globally at `keymap/mod.rs:184`; O opens in the default app in the viewer, `viewer.rs:68`), and the global I branch matches `i` with **any** modifier, so it gains `&& !keystroke.modifiers.alt` and `viewer.rs:68` likewise, the way ⌥P is told apart from P today. The range is enforced in Rust on the frame tick (`app/video_actions.rs`: `position >= out` -> `seek(in)`), drawn on the scrubber (`views/viewer/transport.rs`). Tests in `src/tests/video_player.rs` with the dummy backend (`player/dummy.rs`): `loop_range_seeks_back_to_in_point`, `clear_loop_range_restores_full_loop`, `out_before_in_is_refused` (toast, range unchanged), `alt_i_sets_in_point_without_toggling_inspector`, `alt_o_does_not_open_default_app`. Palette entries "Set Loop Start", "Set Loop End", "Clear Loop Range" and shortcut sheet rows.

### Task F2: Audio tracks and chapters  (M)

Extend `VideoBackend` (`player/backend.rs`) with `audio_tracks() -> Vec<String>`, `select_audio_track(usize)`, `chapters() -> Vec<(f64, String)>`; bridge via `AVMediaSelectionGroup` for `AVMediaCharacteristicAudible` and `chapterMetadataGroupsBestMatchingPreferredLanguages:` in `macos_bridge.m`; dummy backend returns fixtures. UI: a transport menu "Audio" (hidden with one track; keyboard path via palette "Audio Track: {name}" entries while a video is open) and chapter ticks + `⌥←/⌥→` previous/next chapter (verified unbound: `viewer.rs` checks `left`/`right` without modifiers, so the ⌥ check must come first). Tests with the dummy backend (`audio_menu_hidden_with_one_track`, `select_audio_track_calls_backend`, `alt_right_jumps_to_next_chapter`, `no_chapters_no_ticks`); real-file check is manual (state it). Remembering choices: per-session only; ROADMAP §7 asks to "remember last choices", which is deferred with Open question 7.

**Deferred with reasons:** subtitles (frames come from `AVPlayerItemVideoOutput`, which never draws captions; needs `AVPlayerItemLegibleOutput` and a GPUI caption renderer — M/L of its own), trim/export clip (creates new media files; ROADMAP §7 marks it "later").

---

## Open questions for the owner

1. Albums: is order-by-sort enough for 0.6, or do you want manual drag/keyboard reordering (adds a `position` column and a sort mode)?
2. **Map network policy:** approve the offline bundled basemap only (recommended), or also an explicit, opt-in "Load detailed map" tile fetch? The latter needs product approval under AGENTS.md §5.4, a tile provider choice, and a privacy note.
3. Should picks/rejects travel in XMP sidecars (a custom `lumina:` property), or stay catalog-only like Lightroom?
4. Stacks: is a review tab enough, or must the grid collapse stacks (much larger change to counts/selection)?
5. Key contract: OK to move viewer zoom presets from 0/1/2/4 to ⌘0/⌘1/⌘2/⌘4 so digits rate everywhere?
6. Slideshow and videos: show the poster for the interval (planned), or play each video to its end?
7. Loop range and audio-track choice: per-session only (planned), or persisted per video like resume (ROADMAP §7 says "remember last choices")?
8. Part A5 (cover thumbnails, drag to album): in 0.6 or later? ROADMAP §5 lists both cover thumbnails and drag add as targets, so "later" means updating that section.
9. Order with plan 3: Part B uses plan 3 Task 9's query-token quick filters and assumes its D11 (search moves the sidebar to Library). If plan 3 Task 9 is not approved, should "Show Picks" extend `GridFilter` instead?

## Owner answers (2026-11-07)

The owner accepted the recommended defaults:

1. Albums follow the sort order in 0.6; no manual reordering, no `position` column.
2. Map: the offline bundled basemap only. No tile fetch.
3. Picks and rejects stay catalog-only; they are not written to XMP sidecars.
4. Stacks: a review tab only; the grid does not collapse stacks.
5. Yes: viewer zoom presets move to ⌘0/⌘1/⌘2/⌘4, so digits rate everywhere (Task B0).
6. Slideshow: videos show their poster for the interval.
7. Loop range and audio-track choice are per session only.
8. Part A5 (cover thumbnails, drag to album) is later, not 0.6. ROADMAP §5 is updated to say so.
9. Plan 3 Task 9 has landed (`dbab4cb`), so "Show Picks" builds on its query-token quick filters.

## Out of scope

- Subtitles, trim/export clip (see Part F), wide-gamut (P3) rendering, a RAW develop pipeline.
- Network map tiles, reverse-geocoding beyond the bundled city list, place search.
- Nested album folders, album descriptions/icons/colors, user-defined export presets, perceptual ("similar image") duplicate detection.
- Moving `refresh_album_counts` off the UI thread (pre-existing; worth its own task).
- A smart-album rule "is in album X". No rule field references curated albums today, so album delete needs no rule rewrite; adding one later must also rewrite or flag rules on album delete.
- Smart-album empty-state explanation ("why did this item match") and richer rules (ROADMAP §5): plan 3 owns query explanation; not repeated here.
- Album reorder by hand and nested folders (Decision 1; Open question 1).
- Writing flags to XMP (Decision 5; Open question 3).
- Windows/Linux equivalents of the share sheet, sips color conversion, and format matrix.

## Review notes (2026-11-05)

Independent review against the code at `5110780` plus the working tree. Changes made:

1. Schema: dropped hard-coded 15/16; versions are "next `SCHEMA_VERSION` (currently 14), renumber after plan 4"; tests assert `== SCHEMA_VERSION`. Added the rule that an index on the new `flag` column goes after its guarded `ALTER`, not in the main batch (it would fail on old databases). The geo index needs no bump.
2. The legacy UI harness is a **v7** fixture, not v14. Upgrade tests now use the core hand-built-layout pattern (`media.rs:1010`) plus one write each in the existing v7 UI test.
3. `merge_metadata_in_tx` is shared with plan 4. Membership and flags get a test through **both** callers. `fold_into` deletes `other`, so without the carry the cascade silently drops its album rows.
4. `restore_album` / undo: `INSERT OR IGNORE` does not cover FK failures, so purged members are filtered. Added `restore_album_skips_purged_members`.
5. Key contract, checked against `app/keymap/*` and `menus.rs`: ⌘0-9 are unbound and menus carry no key equivalents, so the ⌘digit move collides with nothing. Split it into its own Task B0 and listed every stale `0/1/2/4` hint (tooltip, sheet, docs, two tests). Also found ⌘A ignoring Shift (it would swallow ⇧⌘A), I matching any modifier (it would swallow ⌥I), O taken in the viewer (F1 now uses ⌥I/⌥O and adds guards), ⌘=/⌘- reaching grid zoom under the map, and ⌥←/→ reaching video seek. ⌥⌘S is free, so the open choice is closed.
6. Plan 3 conflict: B2 extended `GridFilter` and its dimming, which plan 3 Task 9 deletes. It now uses `QuickFilter` tokens. "Delete Rejected…" no longer goes through the search box, because plan 3 D11 would widen the set to the whole library. Added Open question 9.
7. There is no Edit menu. B3 goes to File, and export presets get new File items. The export command has no shortcut today, so that "verify" is closed.
8. Wrong claims fixed. `within:` malformed input is dropped like `color:foo`, not turned into FTS. A set_flags "atomic on unknown id" test cannot fail (an UPDATE on an unknown id is a no-op), so it was replaced. `with_lock_held` cannot force a write error, so tests use the result-funnel pattern (`tests/tags_and_albums.rs:438`). Smart-rule import is validated with `build_sql()`, not a count. The file-dialog stub is `FOLDER_PICKS` (`library.rs:1543`). Export line ranges were corrected.
9. Decoder/scanner, read in code: the sips path (`decode_via_sips`) never applies EXIF orientation, and it covers every thumbnail of a >2560 px JPEG. Scanner dims are never swapped for photos, including the EXIF fallback. Cache invalidation goes through `CACHE_FORMAT_VERSION`. Decoded frames are BGRA, so the JPEG export swizzles.
10. Added missing tests and paths: album count after trash/restore, album rename undo, write-failure banners, compare texture freeing (`cx.drop_image`), compare ↵/Esc round trip, slideshow loop and Space-on-video, palette/menu path to the map, the stacks empty state and keyboard, burst rows with no camera, JPEG export of videos and pixel order, a 20k-member album bench, `within:` query plan and gate, and a basemap size ceiling.
11. YAGNI: dropped basemap simplification (110m is already generalized), the share toolbar button, and ⌘N "from selection when one exists" (now always empty, plus an explicit "New Album with Selection"). Recorded the out-of-scope items: album-referencing smart rules, smart-album match explanation, reorder/nesting, and XMP flags.

Not verified (needs a build or real files): whether sips bakes orientation into its PNG (E1 decides), the size of `ne_110m_land` once converted, `NSSharingServicePicker` lifetime behaviour, whether `chapterMetadataGroupsBestMatchingPreferredLanguages:` returns chapters for MP4/M4V as well as MOV, and whether GPUI delivers ⌘digit keystrokes to `on_key_down` on macOS without a menu equivalent (B0's first test settles it). No builds or tests were run.
