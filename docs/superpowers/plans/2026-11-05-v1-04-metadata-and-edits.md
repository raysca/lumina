# Metadata, Keywords and Quick Edits (0.4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keywords can be organized as a hierarchy with aliases; creator/rights/contact templates apply to a selection; XMP sidecars round-trip date, location and rights without destroying other apps' sidecars; batch edits gain a date shift and every remaining edit path gets one grouped undo; photos get a small, non-destructive edit set (rotate, crop, exposure, auto-enhance) and a keyboard-reachable "Edit in…" handoff whose round trip refreshes the library.

**Architecture:** No change to `media_items` columns, `MediaItem`, or the shared media SELECT list: every new datum lives in a side table (`tag_aliases`, `media_rights`, `media_edits`), the pattern v12-v14 already use (`catalog/schema.rs:129-171`), so the `map_query_rows` SELECT lists (about a dozen; grep `is_trashed, m.title` / `is_trashed, title`) and the FTS triggers stay untouched. Hierarchy is encoded in the existing flat tag string (`places/europe/paris`), matched by an index range, so search, smart albums, FTS and sidecars keep one tag model. Edits are parameters in the catalog applied after the disk cache, exactly where rotation is applied today (`lib.rs:1425-1455`); originals and cached decodes are never modified, so `DiskCache::compute_key` (`cache.rs:68`) needs no edit component (only the in-memory GPU `texture_cache`, keyed by id, must be evicted on every edit change).

**Tech Stack:** Rust 2021, rusqlite 0.31 (bundled), image 0.25 (jpeg/png/webp; already a dependency), chrono 0.4, GPUI Kit 0.9 (`gpui-pre` 0.3.11). No new crates.

**Parts (approvable independently, in this order):** A Keywords (Tasks 1-5), B Sidecars and rights (Tasks 6-10), C Batch edit gaps and undo (Tasks 11-13), D Quick edits and handoff (Tasks 14-18). Each part bumps `SCHEMA_VERSION` at most once; if a part is dropped, renumber the later bumps.

## Global Constraints

- Schema: each of Parts A, B and D takes the **next `SCHEMA_VERSION` (currently 14, `catalog/mod.rs:28`)** at the time it lands. Plan 5 (`2026-11-05-v1-05-organize-and-view.md`) also bumps the schema twice; whichever plan lands second renumbers. Tests assert `user_version == SCHEMA_VERSION`, never a literal (the existing literal `assert_eq!(SCHEMA_VERSION, 14)` in `catalog/sources.rs:198` must be updated by whichever bump lands first). Every migration is `CREATE TABLE/INDEX IF NOT EXISTS` in the main `execute_batch` (`schema.rs:13`) plus the version stamp (`schema.rs:530-534`); no `ALTER TABLE` on `media_items`. The only FTS rebuild is Part A's one-time repair (Task 1), gated on the stored `user_version` being below Part A's version.
- Each new per-media side table has `FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE` (`PRAGMA foreign_keys = ON`, `schema.rs:19`) and is carried by `merge_metadata_in_tx` (`catalog/duplicates.rs:292-384`), which both duplicate merge (`duplicates.rs:279`) and missing-file fold (`files.rs:202`) use. Plan 5 also extends `merge_metadata_in_tx` (album memberships, flags): the second plan to land rebases onto the first and keeps both sets of copies; neither rewrites the function wholesale.
- Tag path rules: segments separated by `/`, each segment `trim()`med, empty segments dropped, whole path cleaned by `clean_text(_, MAX_NAME_CHARS)` and lowercased (as `add_tags_batch` already does). `MAX_NAME_CHARS` is 100 (`text.rs:6`) for the **whole path**; a path longer than that is refused with an error, never truncated (truncation would cut a segment). One function, `keywords::normalize_path`, used by every write path.
- Threading: short single-transaction catalog writes keep today's synchronous pattern (e.g. `apply_batch_date`, `navigation.rs:1084-1138`). File IO (sidecar writes, Edit a Copy's file copy, reading existing sidecars) and pixel work (edit preview) run on the background executor; sidecar writes stay in `sync_metadata_sidecars` (`library.rs:876-891`).
- No `let _ =` on catalog writes. UI writes: on `Err`, revert optimistic state and `set_error`; background sidecar failures `log::warn!` (existing pattern).
- Sidecars: Lumina never overwrites a sidecar another app wrote (Task 6). Edits are never written to XMP in 0.4.
- UI copy never names tools or formats beyond "XMP sidecar" (memory rule: no tool names).
- `#[core::prelude::v1::test]` in modules that glob-import `gpui::*`.
- Every new command: `CommandId` + i18n key in `src/i18n.rs` + shortcut sheet entry where a shortcut exists; menu items need a GPUI action in `src/actions.rs` and an entry in `src/menus.rs` (both at the crate root, not under `app/`); data-changing UI tests assert the catalog row, not just state.
- Shortcuts are hard-coded matches in `app/keymap/{mod,grid,viewer,modals}.rs`. `⌘[` is Back (`keymap/mod.rs:77`) and plan 2 adds `⌘]` Forward; `⌘R` rescan and `⇧⌘R` reveal (`mod.rs:153-178`) match any extra modifier; plan 5 takes digits and P/X/U. Before adding a key, grep the four keymap files and the other 2026-11-05 plans.
- Sidecar sync: every catalog write that changes a sidecar field (including global tag rename/merge/delete, template apply, rights edits, date/location) calls `sync_metadata_sidecars` for the affected ids; undo already syncs `affected_ids` (`undo.rs:268,288`).
- Gates per task: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`; `make bench` for Tasks 2, 3 and 15; Task 1 also reports how long the one-time index rebuild takes on the bench library (it runs at catalog open); `make ui-smoke` for Tasks 4, 16a, 16b, 17, 18. Report skips honestly.
- Stage explicit paths; never `git stash`; one task = one commit (`feat:`/`fix:`/`test:`).

## Review Focus

1. A Darktable/digiKam sidecar (`photo.jpg.xmp` with `darktable:history`) next to an item is never rewritten when the user rates or tags it — Task 6 test `foreign_sidecar_is_not_overwritten`.
2. A sidecar holding only a creator or only an edited date is not deleted as "empty" — Task 7 test `rights_only_sidecar_is_kept` and Task 8 `date_only_sidecar_is_kept` (`SidecarMetadata::is_empty`, `xmp.rs:28-37`, deletes the file today when its known fields are blank).
3. Tag filters match descendants without losing index use: `tag:places` finds `places/europe/paris`, the timeline seek and structured-query gates stay under 1 ms / 5 ms — Task 2 tests `tag_filter_matches_descendants_not_siblings` (`places` must not match `placesx` or `places-old`) and `tag_filter_plan_uses_index`, plus `make bench`. Exact-row helpers (`ids_with_tag`, add/remove) stay exact: tag undo depends on them.
4. Renaming a parent keyword moves every descendant, merges collisions, rewrites smart-album rules, retargets aliases, rewrites the affected sidecars, and one undo restores the exact rows — Task 3 `rename_parent_moves_subtree_and_merges` and Task 12 `undo_merge_restores_rows_and_rules`.
5. The search index matches the catalog after tag edits and merges — Task 1 `caption_word_shared_with_tag_survives_tag_removal` and `merge_keeps_search_index_in_sync` (confirmed mismatch, see Current state), plus the one-time index rebuild for existing libraries.
6. Duplicate merge / missing-file fold keeps rights and edits — Task 7 `merge_carries_rights`, Task 14 `fold_carries_edits`.
7. A crop never reaches the original or the disk cache: cache files hold the unedited decode and originals are byte-identical — Task 15 `edits_never_touch_original_or_cache`. Rotating a cropped photo keeps the same region — Task 15 `rotation_after_crop_keeps_region`.
8. After an in-place save in an external editor the grid and viewer show the new pixels and the item keeps its id and metadata — Task 18 `external_save_refreshes_texture_and_keeps_metadata`.
9. Lumina's own sidecars, from every released render version, stay "owned" (a caption containing `a:b=` must not flip one to foreign) — Task 6 `legacy_lumina_sidecars_are_owned`, `caption_text_with_colon_equals_stays_owned`.

---

## Current state (verified)

Verified by reading the code on 2026-11-05 (HEAD `5110780`, plus uncommitted net_import changes not touched by this plan). Roadmap §3/§4/§6/§11 and ux_audit §8 are partly stale; corrections are marked **stale**.

**Tags and Untagged**
- Tags are flat strings in `media_tags(media_id, tag)` with `PRIMARY KEY (media_id, tag)` and `idx_media_tags_tag` (`schema.rs:97-105`). Writes lowercase and clean (`catalog/tags.rs:55-57`).
- Global rename/merge/delete exist (`catalog/tags.rs:244-365`); `merge_tags` is `rename_tag`; rename rewrites every `RuleField::Tag` smart-album rule (any operator, including `Contains`) whose text value equals the old name exactly (`tags.rs:282-318`). Delete leaves rules alone. The UI's bulk delete loops `delete_tag_globally` per tag, one transaction each (`app/tags.rs:510-530`). None of rename/merge/delete calls `sync_metadata_sidecars` (`app/tags.rs:185-275, 395-470`), so sidecars keep the old names and a later "Import XMP Metadata" re-adds them. **Gap not listed in the roadmap.**
- `ids_with_tag` (`catalog/tags.rs:175`) is used by tag add/remove undo (`app/tags.rs:68,113`) to record exactly which items changed; it must stay an exact match. `get_items_by_tag` (`tags.rs:423`) joins `media_tags`, so a subtree match there would return an item once per matching tag. The sidebar Tag view goes through the search tag filter (`active_view.rs:60,91`).
- **Stale (roadmap §4 "no first-class Untagged view"):** an Untagged sidebar row, count, view and empty state ship: `AlbumSelection::Untagged` (`app/active_view.rs:61,94`), `untagged_count` (`app/state.rs:410`), palette entry (`app/command_palette.rs:545`), UI test `ui_automation_untagged_sidebar_row_selection_and_empty_state` (`tests/ui_automation.rs:2953`). Gap: it is not restored on relaunch (`config.rs:86-88` maps it to `LastView::All`).
- Autocomplete exists: prefix match on library tags, most used first, max six (`views/inspector/tabs/tags.rs:29-44`). No aliases, no segment match.
- Tag filters are exact: search `tag = ?` (`search.rs:253-257`), smart album `Equals`/`NotEquals` (`smart_albums/rules.rs:220-235`).
- **Confirmed mismatch (by reading; not run):** the tag-mutation FTS helpers `delete_fts_for_items`/`insert_fts_for_items` (`catalog/tags.rs:10-52`) omit the `caption` column that every trigger includes (`schema.rs:179-232`). The table is contentless (`content=''`, `schema.rs:191`), where a `'delete'` must pass exactly the indexed values or the index goes inconsistent. The effect is subtler than "captions stop matching": the old caption tokens are never deleted, so a plain caption word usually still matches, but a word that is in both the caption and a tag loses its caption entry when the tag is added, and stops matching once the tag is removed (caption "kyoto trip" + tag `kyoto`, add then remove the tag: `kyoto` no longer finds the item). A simple "caption still matches after tagging" test would pass before the fix. Related: `merge_metadata_in_tx` copies tags and the caption onto the keeper with no FTS refresh, then its `UPDATE media_items` fires `media_fts_au`, whose `'delete'` half reads the *new* tags and caption (`schema.rs:215-222`), again not the indexed values. Existing libraries already carry these inconsistencies, so the fix needs a one-time rebuild.

**XMP**
- `SidecarMetadata` carries title, caption, rating, color label, tags, source URL, credit only (`xmp.rs:12-22`). **No date, GPS, creator, rights, or contact** — confirms the brief and roadmap §11.
- `write_sidecar` renders a fresh packet and renames it over any existing file, and deletes the file when Lumina's known fields are blank (`xmp.rs:193-213`); there is no check that Lumina wrote the existing file. Every released render version (git `4a94c34`, `73480ad`, `c76f15e`, `0147095`, HEAD) writes only `x:xmpmeta`, `rdf:RDF/Description/Alt/Bag/li`, `rdf:about`, `xml:lang`, `xmlns:{x,rdf,dc,xmp,photoshop}`, `xmp:Rating`, `xmp:Label`, `dc:title/description/subject/source`, `photoshop:Credit`, plus the `<?xpacket` processing instructions: a subset of today's names, so a name allow-list classifies all of them as Lumina's. Lumina's `file.ext.xmp` naming (`xmp.rs:40-44`) is the same convention darktable and digiKam use, so their sidecars are overwritten on the next metadata edit. **Data-safety gap not listed in the roadmap.**
- Sidecars are written after title/caption/tag/rating edits via `sync_metadata_sidecars` (`app/library.rs:876-891`); `apply_batch_date`/`apply_batch_location` do not call it (`app/navigation.rs:1083-1200`), consistent with dates not being in sidecars.
- Import merges present values, never clears (`lib.rs:826-923`).
- Parsing is string-based (`between`, `xmp.rs:126-131`); there is no XML crate.

**Batch edit and undo**
- **Stale (roadmap §6, ux_audit §8 "no batch title/date/location/caption, mixed values not shown, undo per item"):** the batch inspector has an EDIT ALL section with title, caption, date and location rows, a `Mixed` marker (`views/inspector/batch.rs:47-90, 410-490`, selectors `batch-title-input`, `batch-*-mixed`), and each apply writes one grouped undo entry: `BatchMetadata`, `BatchDate`, `BatchLocation` (`app/undo.rs:63-76`; `navigation.rs:1132`). Catalog writes are one transaction (`catalog/media.rs:543-616`).
- Gaps: no date **shift/offset** (batch date sets an absolute value, keeping time of day, `navigation.rs:1083-1108`); global tag rename/merge/delete are outside undo (`undo.rs:6`; `app/tags.rs:185-275, 395-530` has no `push_undo`); rotation has no undo (`viewer_actions.rs:457-510`).

**Quick edits and handoff**
- Rotation exists: `media_items.rotation_quarters` (`schema.rs:61`), `LuminaEngine::set_rotation(ids, quarters)` (one value for all ids, `lib.rs:677-693`), applied after the disk cache by `DecoderPipeline::rotate_frame` (`lib.rs:1425-1455`, `decoder.rs:394`). Context menu only (grid and viewer), single item, images only (`views/context_menu.rs:390-420`, `viewer_actions.rs:457-516`); **no palette command, shortcut or menu item** (AGENTS §11 gap). The rotate action evicts `texture_cache` and the `in_flight` marker by hand (`viewer_actions.rs:485-495`).
- External editor exists: preference + picker (`config.rs:377-390`, `app/preferences.rs:215-262`), `open -a`/`-b` launcher (`viewer_actions.rs:56-80, 235-266`), context menu "Edit in External Editor" in grid and viewer (`context_menu.rs:375-389`), tests in `tests/editor_handoff.rs`. **No palette/menu/shortcut path.** `⌘E` and `⌥⌘E` are unused in the keymap.
- Round trip: an in-place save reaches the watcher, which only re-reads files whose signature changed (`watcher.rs:322-336`) and upserts them (path is `UNIQUE`), but the UI's `BatchDiscovered` handler never evicts `texture_cache` for those items (`app/library.rs:213-237`; only `ItemRemoved` does, `:278`). Expected result: stale thumbnail until LRU eviction. Not run; Task 18 starts with the failing test.
- The disk cache stores the unedited decode keyed by path/size/mtime/target size (`cache.rs:68-81`), so post-cache edits need no cache-key change. An edit that changes the decode size (crop, Task 15) produces a new key and a second cache file at that size; the cache's 1.5 GB budget trims it (`cache.rs:25`).
- Export copies originals with `fs::copy` (`lib.rs:771`); edits would not be included.

---

## Decisions

1. **Hierarchy encoded in the tag string (`a/b/c`), not a `keywords(id, parent_id)` table.** A keyword table would rewrite every `media_tags` reader (FTS triggers, search, smart albums, sidecars, duplicates merge, tag manager). The path keeps one model; FTS already tokenizes `/` as a separator, so `europe` finds `places/europe/paris` with no FTS change. Descendant match is one range plus a filter: `tag >= ?p AND tag < ?p || '0' AND (tag = ?p OR substr(tag, length(?p) + 1, 1) = '/')`. The range alone also covers `places-old`, `places.x`, `places x` (every byte between `'\0'` and `'0'`, `'/'` is 0x2F and `'0'` 0x30), so the filter is required; one range (not an `OR` of two) keeps a single index seek on `(media_id, tag)` or `idx_media_tags_tag`. Only filters (search `tag:`, smart-album `Tag` rules, the sidebar tag view, `get_items_by_tag`) match subtrees; add/remove and `ids_with_tag` stay exact.
2. **Separator `/`** (digiKam's tag-path syntax, typeable, no conversion layer). Rejected `|` (Lightroom's export separator, but users never type it). Sidecars convert to `|` for `lr:hierarchicalSubject`. Existing tags containing `/` would become hierarchies; see Open question 1.
3. **Assigning a child does not add ancestor rows.** Filters match descendants instead. Rejected auto-ancestor rows (doubles rows, makes "remove parent" ambiguous).
4. **Aliases in a catalog table `tag_aliases(alias PRIMARY KEY, tag)`, resolved on input.** Typing an alias applies its canonical keyword; the alias itself is never stored on media. Rejected storing aliases in user config (aliases describe this library's vocabulary and must follow the catalog backup).
5. **Rights (creator, copyright, usage terms, contact email, contact URL) in a side table `media_rights`**, not FTS-indexed and not a smart-album field in 0.4 (Open question 3). Contact phone/address deferred.
6. **Templates in user config, not the catalog.** A creator/copyright template belongs to the person, across libraries. Stored as `Vec<MetadataTemplate>` in `AppConfig` with the struct in a new `app/templates.rs` (config.rs is already 1003 lines). Template file import/export deferred: config is already JSON.
7. **Template apply overwrites only the fields the template sets** and adds its keywords. Rejected "fill empty only" as a second mode until asked for.
8. **Sidecar ownership check instead of an XML merge.** Lumina rewrites a sidecar only if every qualified element/attribute name in it is one Lumina writes (`owned_by_lumina`); otherwise it skips, logs, and counts the skip. Rejected a string-surgery merge (no parser, corruption risk) and adding `quick-xml` (MIT; possible later, Open question 2). Pre-existing Lumina sidecars pass the check without a marker because the check is a name allow-list.
9. **Date/GPS written only when the user edited them** (`date_edited`/`place_edited`, `schema.rs:63-64`); file EXIF stays the source otherwise. Date is written as naive wall time (`exif:DateTimeOriginal`, `photoshop:DateCreated`) because the catalog stores wall time as UTC (`scanner.rs:554`, `panels.rs:779-791`); imported offsets are dropped to match.
10. **Edits in the catalog (`media_edits` side table), not XMP.** Lightroom's `crs:` crop/exposure semantics differ (crop relative to orientation, process-version-dependent tone), so writing a partial `crs:` set could mis-render in Lightroom. Rejected `tiff:Orientation` for rotation for the same reason (Lumina's quarter turns are relative to the EXIF-corrected image). The daily catalog backup carries edits; sidecar portability for edits is out of scope for 0.4.
11. **v1 edit set: rotate (exists), crop with aspect presets, exposure (±2 EV in 1/3 steps), auto-enhance toggle (luminance-percentile levels stretch).** All are a crop plus one 256-entry LUT on the existing RGBA frame using `image::imageops::crop_imm` and std. **The crop is stored in normalized coordinates of the unrotated (decoded, EXIF-corrected) frame and applied before rotation**, so changing rotation from any path (R key, context menu, fold, duplicate merge) never needs to remap the crop; the crop overlay converts from on-screen (rotated) coordinates once on Done. Rejected storing it in rotated coordinates (every rotation write would have to transform the rect, and `fold_in_tx` carries rotation separately from `merge_metadata_in_tx`). Deferred: straighten (needs arbitrary-angle resampling: new code or `imageproc`), white balance, contrast, filters/presets, red-eye, per-item version history, RAW develop. One current edit state per item plus undo plus "Revert to Original" replaces version history for 0.4.
12. **Edits applied post-cache in `process_thumbnail_request`**, same place as rotation. To keep crop sharp, the decode size is `target_size / max(crop_w, crop_h)` capped at 4096. Rejected caching edited frames (edit changes would need cache-key versioning and invalidation).
13. **External editing: keep "Edit in External Editor" (in place) and add "Edit a Copy"** that duplicates the file next to the original (`name (edited).ext`), catalogues it with copied metadata, and opens the copy. No version stack (stacks are 0.6). Lumina's own edits are not baked into what the editor opens, so the copy starts with **no** Lumina edits or rotation (it would otherwise be cropped twice once the editor saves); documented.
14. **Grouped undo for global tag ops by row snapshot**, not inverse operations (a merge has no exact inverse). The engine returns the removed/added `(media_id, tag)` rows, alias rows before/after, and smart-album JSON before/after; undo replays them in one transaction. A smart album whose rules changed since the tag op is left alone on undo (compare with `albums_after`), so undo never discards a later rule edit.
15. **Rotate keys: `R` (right) and `⇧R` (left), no modifier, in grid and viewer.** `⌘[` is Back and plan 2 makes `⌘]` Forward, so the Photos-style `⌘[`/`⌘]` is not available; `⌘R`/`⌥⌘R` are taken by rescan (the `⌘R` match ignores ⌥). Plain `R` is unused in all four keymap files and in plans 2, 3, 5 and 6 (plan 5 takes digits and P/X/U). Edit mode reuses the same keys. Rejected: palette/menu only (rotation is frequent; AGENTS §11 prefers a key).

---

## Part A: Hierarchical keywords and aliases

### Task 1: Search index stays in sync after tag edits and merges (S)

**Files:** Modify `crates/lumina-core/src/catalog/tags.rs` (`delete_fts_for_items`, `insert_fts_for_items`, made `pub(crate)`), `catalog/duplicates.rs` (`merge_metadata_in_tx`), `catalog/schema.rs` (one-time rebuild), `catalog/mod.rs` (next `SCHEMA_VERSION`, Part A's bump; Task 3 adds `tag_aliases` under the same version when both ship in one release, else takes its own). Test: `crates/lumina-core/tests/core_tests.rs`, migration test in the `catalog/media.rs` tests module.

**Interfaces:** No signature change. Both helpers list `caption` and select `COALESCE((SELECT caption FROM media_captions WHERE media_id = m.id), '')`, matching `schema.rs:195-232`. `merge_metadata_in_tx` runs its `UPDATE media_items` (step 3) **first**, so `media_fts_au` deletes the values that are actually indexed, then wraps the tag and caption copies in `delete_fts_for_items(&[keep])` / `insert_fts_for_items(&[keep])`. Repair: when the stored `user_version` is below Part A's version, rebuild the index with the existing `'delete-all'` + reinsert statement (`schema.rs:401-409`; reuse it, don't copy it) in one transaction.

- [ ] Failing test `caption_word_shared_with_tag_survives_tag_removal`: caption "kyoto trip", add tag `kyoto`, remove tag `kyoto`, search `kyoto` -> item found. (A plain "caption word still matches after tagging" test passes before the fix, because the stale caption tokens are never deleted; do not use it as the regression test.)
- [ ] Failing test `caption_change_after_tagging_drops_old_words`: caption "golden hour", add a tag, change caption to "blue", `golden` -> not found, `blue` -> found.
- [ ] Failing test `merge_keeps_search_index_in_sync`: trash item has tag `paris` and caption "sunset"; merge into keeper; `paris` and `sunset` find the keeper; then remove `paris` from the keeper -> `paris` no longer finds it.
- [ ] Test `fts_repair_migration_runs_once` (seed a deliberately mismatched row via the old helper SQL, set `user_version` back to 14, reopen: search matches the catalog and `user_version == SCHEMA_VERSION`; reopen again: still correct, rows kept).
- [ ] Do not drop the fix if a test happens to pass first: the helpers violate the contentless-FTS5 `'delete'` contract regardless.
- [ ] Docs: CHANGELOG fix line.

### Task 2: Keyword paths, descendant matching (M)

**Files:** Create `crates/lumina-core/src/catalog/keywords.rs` (path helpers; tags.rs is 775 lines). Modify `catalog/tags.rs` (`add_tags_batch` and `remove_tags_batch` call `normalize_path` but stay exact-row; `get_items_by_tag` switches its `JOIN` to an `EXISTS` subtree filter so an item with two matching tags appears once; `ids_with_tag` is **not** changed, tag undo relies on exact rows), `search.rs:253-257`, `smart_albums/rules.rs:220-235`, `catalog/mod.rs` (mod decl). Tests in `keywords.rs`, `search.rs`, `core_tests.rs`.

**Interfaces:**
- `pub fn normalize_path(raw: &str) -> Result<String>` (segments trimmed, empties dropped, `clean_text` + lowercase; `bail!` when the result is empty-after-cleaning or longer than `MAX_NAME_CHARS`, instead of truncating).
- `pub(crate) fn subtree_clause(column: &str) -> String` returning `({c} >= ? AND {c} < ? || '0' AND ({c} = ? OR substr({c}, length(?) + 1, 1) = '/'))` with anonymous `?` placeholders (search.rs pushes positional values in order, `search.rs:253-257`; numbered `?1` would collide), and `pub(crate) fn subtree_params(path: &str) -> [rusqlite::types::Value; 4]`. One definition, used by search, smart albums and `get_items_by_tag`.
- `pub fn parent_of(path: &str) -> Option<&str>`, `pub fn leaf(path: &str) -> &str`.
- Smart album `Tag Equals p` matches p and descendants; `NotEquals p` excludes both; `Contains` unchanged. Rule JSON unchanged.

- [ ] Tests: `normalize_path_trims_and_drops_empty_segments` (` Places // Europe/ ` -> `places/europe`); `normalize_path_refuses_over_long_paths` (101 chars -> `Err`, nothing written); `tag_filter_matches_descendants_not_siblings` (`places` matches `places`, `places/europe/paris`; not `placesx`, `placesx/a`, `places-old`, `places.x`, `other/places`); `items_by_parent_tag_are_not_duplicated` (item with `a/x` and `a/y`, `get_items_by_tag("a")` returns it once); `smart_album_tag_equals_includes_children`; `smart_album_tag_not_equals_excludes_children`; `tag_filter_plan_uses_index` (`explain_query_plan`, `catalog/mod.rs:243`, contains `USING` an index on `media_tags`, no `SCAN media_tags`); `tag_undo_ids_stay_exact` (`ids_with_tag(ids, "a")` ignores items that only have `a/x`).
- [ ] `make bench` before and after; record numbers in the commit body.
- [ ] Docs: README search syntax (`tag:places` includes sub-keywords).

### Task 3: Subtree rename/move/delete and aliases (M)

**Files:** Modify `catalog/keywords.rs`, `catalog/tags.rs` (`rename_tag` -> subtree, `delete_tag_globally` -> subtree, new `delete_tags_globally(&[String])` in one tx for bulk delete), `catalog/schema.rs` (table), `catalog/mod.rs` (Part A's `SCHEMA_VERSION`, see Task 1), `lib.rs` (engine wrappers), `app/tags.rs` (rename/merge/delete/bulk delete call `sync_metadata_sidecars` for every id in `TagChange`; bulk delete calls the one-tx variant). Tests: unit in `keywords.rs`, migration in `catalog/media.rs` tests module next to `rotation_migration_is_idempotent_and_roundtrips` (`media.rs:1010`), UI sidecar check in `tests/tag_manager.rs`.

**Schema (Part A version):**
```sql
CREATE TABLE IF NOT EXISTS tag_aliases (
    alias TEXT PRIMARY KEY,   -- normalized, never also a keyword path in use
    tag   TEXT NOT NULL       -- canonical normalized path
);
CREATE INDEX IF NOT EXISTS idx_tag_aliases_tag ON tag_aliases(tag);
```
No `media_items`/FTS change; aliases are not on media, so FTS needs nothing.

**Interfaces:**
- `pub struct TagChange { pub removed: Vec<(MediaId, String)>, pub added: Vec<(MediaId, String)>, pub aliases_before: Vec<(String, String)>, pub aliases_after: Vec<(String, String)>, pub albums_before: Vec<(i64, String)>, pub albums_after: Vec<(i64, String)> }` (only the alias and album rows the op touched) returned by `rename_tag`, `merge_tags`, `delete_tag_globally`, `delete_tags_globally` (replaces `usize`; callers use `removed.len()`; `fn affected_ids(&self) -> Vec<MediaId>` for sidecar sync). Used by Task 12 for undo.
- `rename_tag(old, new)` rewrites `old` and every `old/…` to `new/…` in one tx (`INSERT OR IGNORE ... SELECT media_id, ?new || substr(tag, length(?old) + 1)` over the subtree range, then `DELETE` the old subtree), merges collisions, refreshes FTS for the affected ids with the Task 1 helpers, rewrites `Tag` rules (any operator, as today) whose value is `old` or in its subtree, retargets aliases. Refuses `new` inside `old`'s subtree (`bail!("A keyword cannot move inside itself")`) and `new` equal to an existing alias.
- `delete_tag_globally(tag)` deletes the subtree; aliases pointing into it are kept (typing one re-creates the keyword, which is what an alias means) and smart-album rules are left as today.
- `Catalog::set_alias(alias, tag) -> Result<()>` (errors if alias is an existing keyword path), `remove_alias(alias)`, `aliases() -> Result<Vec<(String, String)>>`, `resolve_alias(&str) -> Result<String>`; `add_tags_batch` resolves aliases before insert.
- `Catalog::subtree_item_count(path) -> Result<usize>` (distinct media, indexed range).

- [ ] Tests: `migration_creates_tag_aliases_idempotently` (open twice, `user_version == SCHEMA_VERSION`, rows kept); `rename_parent_moves_subtree_and_merges` (`a/x` on item 1, `b/x` on items 1,2; rename `a`->`b`: item 1 has one `b/x`, no `a/*` rows, FTS finds `b`, and `a` no longer finds item 1); `rename_into_own_subtree_is_refused`; `rename_onto_alias_is_refused`; `rename_rewrites_smart_album_rules_in_subtree`; `delete_parent_removes_subtree_only` (`places` delete leaves `placesx` and `places-old`); `bulk_delete_returns_one_change` (two tags with children deleted in one call; one `TagChange` lists every removed row); `alias_resolves_on_add` (`nyc` -> `places/usa/new york`); `alias_cannot_shadow_keyword`; `rename_retargets_aliases`; `subtree_item_count_is_distinct` (item with `a/x` and `a/y` counts once); UI `rename_keyword_rewrites_sidecars` (item with a Lumina sidecar; rename in the tag manager; after the background pass the sidecar parses to the new path).
- [ ] Docs: none yet (Task 4).

### Task 4: Tree in the tag manager, sidebar, autocomplete; Untagged restore (M)

**Files:** Modify `views/tag_manager/list.rs`, `views/tag_manager/detail.rs` (subtree count, aliases editor, "Move to…"), new `app/keywords.rs` (alias and move actions; `app/tags.rs` is already 583 lines), `views/inspector/tabs/tags.rs:29-44` (`tag_suggestions`), `views/sidebar/sections.rs` (indented path rows), `config.rs:86-88` (`LastView::Untagged`), `app/command_palette.rs`, `i18n.rs`. Test: `crates/lumina-ui-gpui/src/tests/keywords.rs` (new; add to `tests/mod.rs`; existing tag-manager tests stay in `tests/tag_manager.rs`).

**Interfaces:**
- `tag_suggestions` matches the typed prefix against the whole path, any segment, and aliases; returns `Suggestion { path: &str, via_alias: Option<&str> }`; label shows `places › usa › new york`. Aliases are loaded into app state with the library tag counts (refreshed after alias edits and tag ops), never queried while rendering.
- The rename and "Move to…" prompts show how many items the change touches (`subtree_item_count`, roadmap §4 "bulk rename with a preview of affected items").
- Rows sort by path, indent by depth (16 px per level), synthesize ancestor rows with no direct items (count = `subtree_item_count`, computed for the focused row only, not per frame). No expand/collapse state in 0.4.
- Detail panel: Aliases list (add/remove), "Move to…" (rename with a parent path prompt; reuses modal rename).
- `LastView::Untagged` token `"untagged"`; restore maps back.
- debug_selectors: `tag-row-{path}`, `tag-row-indent-{depth}`, `tag-detail-alias-input`, `tag-detail-alias-{alias}`, `tag-detail-move`, `tag-suggestion-{path}`.
- Commands: `CommandId::AddKeywordAlias` ("Add Alias for Keyword…"), `MoveKeyword` ("Move Keyword…"), both act on the focused tag-manager row.

- [ ] UI tests: `keyword_tree_rows_indent_by_depth` (seed `a`, `a/b`, `a/b/c`; selectors with depth 0,1,2 render in order); `typing_alias_suggests_canonical_and_applies_it` (keyboard only: `t`, type `nyc`, Down, Enter -> catalog row `places/usa/new york`, no `nyc` row); `segment_prefix_suggests_child` (`par` suggests `places/europe/paris`); `move_keyword_by_keyboard_updates_catalog`; `untagged_view_restores_after_relaunch` (config round trip in test mode, no disk write).
- [ ] Keyboard: tag manager arrows/Enter (existing `step_tag_manager_focus`, `tags.rs:346`); palette commands above.
- [ ] Docs: README keywords section; `docs/accessibility.md` keyboard paths.

### Task 5: Keyword hierarchy in sidecars (S)

**Files:** Modify `crates/lumina-core/src/xmp.rs` (or `xmp/` after Task 6). Tests in the xmp module.

**Interfaces:** Render: `dc:subject` = unique set of every path segment (what Lightroom/Bridge show as flat keywords); `lr:hierarchicalSubject` (`xmlns:lr="http://ns.adobe.com/lightroom/1.0/"`) = every tag, flat ones included as one-segment entries, with `/` written as `|`. A tag containing `|` is written to `dc:subject` only (it cannot be expressed in Lightroom's syntax). Parse: when `lr:hierarchicalSubject` exists, tags = its entries (`|` -> `/`) plus `dc:subject` values that are not a segment of any entry; else `dc:subject` as flat tags. Writing flat tags into the hierarchy too is what keeps a flat `europe` next to `places/europe` from being dropped on re-import. Task 6's allow-list gains `xmlns:lr` and `lr:hierarchicalSubject` (Task 5 lands first, so Task 6 must include them).

- [ ] Tests: `hierarchy_roundtrips_through_xmp`; `flat_tag_equal_to_a_segment_roundtrips` (`europe` + `places/europe` -> both back); `flat_subject_without_hierarchy_imports_flat`; `lightroom_style_sidecar_imports_paths` (fixture string with `lr:hierarchicalSubject` and duplicated `dc:subject` leaf names -> no duplicate flat tags); `pipe_in_tag_stays_flat`.
- [ ] Docs: README XMP field table.

---

## Part B: Sidecars that round-trip date, location and rights

### Task 6: Never overwrite another app's sidecar (M)

**Files:** Split `crates/lumina-core/src/xmp.rs` into `xmp/mod.rs` (types, read/write), `xmp/render.rs`, `xmp/parse.rs`, `xmp/owner.rs` (the file passes 500 lines with Part B). Modify `lib.rs` (`write_metadata_sidecars` returns `SidecarWriteReport { written, skipped_foreign: Vec<PathBuf> }`), `app/library.rs:876-891` (log once per path; the export report already lists failures). Tests in `xmp/owner.rs`, `core_tests.rs`.

**Interfaces:**
- `pub(crate) fn owned_by_lumina(xml: &str) -> bool`: scans qualified names **only inside markup** (between `<` and `>`: element names after `<` or `</`, and attribute names before `=`), never element text, because Lumina escapes `<` but not `:` or `=` in values (`xmp.rs:48-57`), so a caption like `ratio a:b=2` must not count; skips `<?…?>` processing instructions. True iff every name is in `KNOWN_NAMES` (the names listed in Current state for every released version, extended by Tasks 5, 7, 8).
- `write_sidecar` gains the check: an existing file that is not owned, cannot be read, or is not UTF-8 (XMP allows UTF-16) -> `Ok(SidecarOutcome::SkippedForeign)` (enum replaces `Option<PathBuf>`: `Written(PathBuf) | Removed | Unchanged | SkippedForeign`; `Unchanged` = no file and empty metadata, so nothing to do). The check also guards the empty-metadata delete. Export's destination is fresh (`unique_destination`, `lib.rs:63`), and the same check covers it.
- Skips are logged with `log::warn!` per pass (path count plus first path), not per field edit; no UI banner (the catalog holds the edit, so nothing is lost).
- Import is unchanged: it still reads foreign sidecars.

- [ ] Tests: `foreign_sidecar_is_not_overwritten` (darktable-style fixture with `darktable:history`; set rating via engine + `write_metadata_sidecars` -> bytes identical, `skipped_foreign` has the path); `legacy_lumina_sidecars_are_owned` (one fixture string per released render version, copied from `git show <rev>:crates/lumina-core/src/xmp.rs` output for `4a94c34`, `c76f15e`, `0147095`, plus today's `render_xmp` for a full `SidecarMetadata`, all pass); `caption_text_with_colon_equals_stays_owned`; `foreign_sidecar_with_empty_metadata_is_not_deleted`; `unknown_attribute_makes_sidecar_foreign` (Adobe's `x:xmptk` on `x:xmpmeta`); `unreadable_sidecar_is_skipped` (invalid UTF-8 bytes -> unchanged).
- [ ] Docs: README "Sidecars" note: sidecars written by other apps are read, never rewritten.

### Task 7: Rights fields and `media_rights` (M)

**Files:** Create `crates/lumina-core/src/catalog/rights.rs`. Modify `catalog/schema.rs`, `catalog/mod.rs` (Part B's `SCHEMA_VERSION`: next after Part A and any plan-5 bump), `catalog/duplicates.rs` (`merge_metadata_in_tx` copies rights when keep has none: `INSERT OR IGNORE INTO media_rights SELECT ?keep, creator, … FROM media_rights WHERE media_id = ?trash`, like the caption copy at `duplicates.rs:310-321`), `xmp/*` (fields), `lib.rs` (`sidecar_metadata_for` takes a rights map read once per pass with `get_rights`, like the caption/rating/label maps at `lib.rs:799-812`; `import_sidecars`; engine wrappers). Tests: `rights.rs`, `xmp/*`, `core_tests.rs`.

**Schema (Part B version):**
```sql
CREATE TABLE IF NOT EXISTS media_rights (
    media_id INTEGER PRIMARY KEY,
    creator TEXT, copyright TEXT, usage_terms TEXT,
    contact_email TEXT, contact_url TEXT,
    FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
);
```
Not in FTS, so no trigger change. All-NULL rows are deleted on write.

**Interfaces:**
- `#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)] #[serde(default)] pub struct Rights { pub creator, copyright, usage_terms, contact_email, contact_url: Option<String> }` (cleaned with `clean_text(_, MAX_TEXT_CHARS)`).
- `Catalog::get_rights(&[MediaId]) -> Result<HashMap<MediaId, Rights>>`, `Catalog::set_rights(&[(MediaId, Rights)]) -> Result<()>` (one tx).
- `SidecarMetadata.rights: Rights`; `is_empty` includes it. XMP: `dc:creator` (rdf:Seq), `dc:rights` (rdf:Alt x-default), `xmpRights:UsageTerms` (rdf:Alt), `Iptc4xmpCore:CreatorContactInfo rdf:parseType="Resource"` with `Iptc4xmpCore:CiEmailWork`, `Iptc4xmpCore:CiUrlWork`. Parse both element and attribute forms of the contact struct (Lightroom writes attributes).
- Import applies only present fields (existing merge rule).
- Info tab shows non-empty rights rows read-only (`views/inspector/tabs/info.rs`), selector `info-rights-{field}`.

- [ ] Tests: `migration_creates_media_rights_idempotently` (`user_version == SCHEMA_VERSION`); `rights_roundtrip_through_xmp`; `rights_only_sidecar_is_kept`; `lightroom_contact_attributes_parse`; `merge_carries_rights`; `import_never_clears_rights`; `cascade_deletes_rights`.
- [ ] Docs: README XMP field table.

### Task 8: Date and GPS in sidecars (M)

**Files:** Modify `xmp/render.rs`, `xmp/parse.rs`, `xmp/mod.rs` (`SidecarMetadata.date_taken: Option<NaiveDateTime>`, `location: Option<(f64, f64)>`), `catalog/media.rs` (`edited_flags(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, (bool, bool)>>`), `lib.rs` (`sidecar_metadata_for`, `import_sidecars`), `app/navigation.rs` (`apply_batch_date`/`apply_batch_location` call `sync_metadata_sidecars`), the single-item date/location apply paths in `app/panels.rs`.

**Interfaces:**
- Render `exif:DateTimeOriginal` and `photoshop:DateCreated` (`YYYY-MM-DDTHH:MM:SS`, no offset) only when `date_edited`; `exif:GPSLatitude="48,51.400000N"` / `exif:GPSLongitude` (XMP `DDD,MM.mmmmmmK`) only when `place_edited` and coordinates exist.
- Parse `exif:DateTimeOriginal` first, then `photoshop:DateCreated`; accept attribute or element form; accept `Z`/`±HH:MM` and drop the offset (wall time); accept date-only. GPS accepts `DDD,MM.mmK` and `DDD,MM,SSK`.
- Import sets date via `set_timestamps` and location via `set_locations` when they differ (GPS tolerance 1e-5°); both mark the edited flags, so a rescan keeps them (`catalog/media.rs:90-100`, the upsert's `CASE WHEN date_edited`; fold keeps them via `files.rs:205-230`).

- [ ] Tests: `edited_date_roundtrips_through_xmp`; `unedited_date_is_not_written`; `date_only_sidecar_is_kept`; `offset_dates_import_as_wall_time` (`2024-05-01T10:00:00+02:00` -> 10:00); `gps_roundtrip_within_tolerance` (south/west signs); `import_sets_edited_flags_so_rescan_keeps_date` (core test with real temp file + rescan); `batch_date_rewrites_sidecars` (UI test: apply batch date, wait for background, sidecar parses to new date).
- [ ] Docs: README XMP table; ROADMAP §11 "Current" line.

### Task 9: Metadata templates (M)

**Files:** Create `crates/lumina-ui-gpui/src/app/templates.rs` (struct, apply, undo push), `crates/lumina-ui-gpui/src/views/template_editor.rs` (modal: name + five rights fields + keywords). Modify `config.rs` (`templates: Vec<MetadataTemplate>`, serde default), `lib.rs` (`LuminaEngine::apply_template(ids, &Rights, &[String]) -> Result<TemplateChange>`), `app/undo.rs` (`UndoAction::Template`), new `views/inspector/batch_template.rs` (TEMPLATE section: picker + Apply; `batch.rs` is already 532 lines, so it only mounts the section), `command_palette.rs`, `src/actions.rs`, `src/menus.rs`, `i18n.rs`. Apply and undo call `sync_metadata_sidecars` for the affected ids. Tests: `crates/lumina-ui-gpui/src/tests/templates.rs`, core test for `apply_template`.

**Interfaces:**
- `#[derive(Serialize, Deserialize, Clone, PartialEq)] pub struct MetadataTemplate { pub name: String, pub rights: Rights, pub keywords: Vec<String> }`.
- Engine `apply_template` in one tx: merges set fields over each item's rights, adds keywords; returns `TemplateChange { rights: Vec<(MediaId, Rights, Rights)>, added_tags: Vec<(MediaId, String)> }` (only tags not already present). Undo restores rights and removes exactly `added_tags`.
- `UndoAction::Template { change: TemplateChange }`, label "template", `affected_ids` = rights ids.
- Commands: `ApplyTemplate` ("Apply Metadata Template…", opens a palette sub-list of template names), `ManageTemplates` ("Metadata Templates…"); menu: File > Metadata Templates…; shortcut none (Open question 4).
- debug_selectors: `template-editor`, `template-editor-backdrop`, `template-field-{field}`, `template-save`, `template-cancel`, `batch-template-picker`, `batch-template-apply`, `palette-template-{name}`.
- Config persistence goes through `save_config` (test early return already exists).

- [ ] Tests: `apply_template_overwrites_only_set_fields` (core); `apply_template_is_one_transaction` (ids = one real item + one id that does not exist; the `media_rights` foreign key fails the tx -> the real item has no rights row and no new tags); `template_apply_and_single_undo` (UI: select 3, apply via palette keyboard only, catalog rights + tags on all 3, Cmd+Z restores all 3 including pre-existing tags kept); `template_editor_escape_discards` (modal key absorption: Escape closes, background shortcuts do not fire); `template_apply_write_failure_reverts_and_banners`; `template_keywords_resolve_aliases_and_paths` (core: template keyword ` NYC ` with alias `nyc` -> canonical path row).
- [ ] Docs: README templates section; shortcut sheet; `docs/accessibility.md`.

### Task 10: Rights in the single-item inspector (S, optional)

Editable rights rows in the Info tab reusing `render_batch_metadata_row` text machinery; batch inspector gets the same rows with Mixed display and `UndoAction::Template` reuse. Tests `edit_creator_single_item_persists_and_undoes`, `batch_creator_shows_mixed`. Only if Open question 5 says yes; templates alone already cover "apply to a selection".

---

## Part C: Batch edit gaps and grouped undo

### Task 11: Date shift (S)

**Files:** Modify `app/panels.rs` (`parse_date_shift`), `app/navigation.rs` (`apply_batch_date` accepts a shift), single-item date apply. Tests: unit in `panels.rs`; UI test in `tests/ui_automation.rs` or a new `tests/batch_edit.rs` (preferred; ui_automation.rs is large).

**Interfaces:** `pub(crate) fn parse_date_shift(input: &str) -> Option<i64>` accepting `+2h`, `-1d`, `+1d 3h 15m`, `-0:30` (signed, units d/h/m/s, `H:MM`). A leading sign selects shift mode in the same DATE field; each item gets `ts + delta` (its own time kept relative), written by `set_timestamps`, one `UndoAction::BatchDate`. Placeholder text in the field: "2024-05-01, or +2h to shift". Error text: "Enter a date as YYYY-MM-DD, or a shift like +2h or -1d".

- [ ] Tests: `parse_date_shift_units_and_signs`; `parse_date_shift_rejects_garbage` (`+`, `2h`, `+2x`); `batch_date_shift_moves_each_item_by_delta` (UI, keyboard only; three items at different times keep their spacing; one Cmd+Z restores all); `date_shift_crossing_month_rebuilds_buckets` (core: timeline bucket counts after shift).
- [ ] Docs: README batch editing; help text.

### Task 12: Undo for global tag operations (M)

**Files:** Modify `lib.rs` (`LuminaEngine::apply_tag_rows(removed, added, albums) -> Result<()>`), `catalog/keywords.rs` (`replace_tag_rows` in one tx with FTS refresh and rule JSON restore), `app/undo.rs` (`UndoAction::TagRows { label: &'static str, change: TagChange }`; update the module doc at `undo.rs:1-6`), `app/tags.rs` (rename, merge, delete, bulk delete push undo + undo toast). Tests: core unit; UI `tests/keywords.rs`.

**Interfaces:** Undo applies `remove(added) + insert(removed)`, restores `aliases_before`, and restores `albums_before` only for albums whose current `rules_json` still equals `albums_after` (others are skipped and counted in the toast); redo the reverse. Inserts use `INSERT OR IGNORE … WHERE EXISTS (SELECT 1 FROM media_items WHERE id = ?)` so rows for purged media are skipped rather than failing the foreign key. Bulk delete of N tags is one entry (the single `TagChange` from `delete_tags_globally`). `affected_ids` = every media id in `removed`/`added`, so undo's existing sidecar sync (`undo.rs:268,288`) covers it.

- [ ] Tests: `replace_tag_rows_is_atomic_and_reindexes_fts`; `undo_merge_restores_rows_and_rules` (UI: merge `cat` into `cats` where one item had both; Cmd+Z: both items exactly as before, smart album rule back to `cat`, counts correct); `undo_rename_restores_alias_targets`; `undo_rename_skips_album_edited_since` (rename, then edit the album's rules, Cmd+Z: tag rows restored, album keeps the later edit); `undo_subtree_delete_restores_children`; `bulk_delete_is_one_undo_step`; `undo_after_item_purged_skips_missing_ids`.
- [ ] Docs: ROADMAP §11 bullet; `undo.rs` module doc.

### Task 13: Batch rotation with undo (S)

**Files:** Modify `app/viewer_actions.rs:449-516` (`rotate_viewer_selected` becomes `rotate_selection(direction)`, acting on the grid selection or viewer item, skipping videos; the context menu calls it), `app/undo.rs` (`UndoAction::Rotation { changes: Vec<(MediaId, u8, u8)> }`), `catalog/media.rs` + `lib.rs` (`get_rotations(&[MediaId]) -> Result<HashMap<MediaId, u8>>` and `set_rotations(&[(MediaId, u8)])`, one tx, per-item values; the existing `set_rotation(ids, quarters)` sets one value for all), `command_palette.rs`, `src/actions.rs`, `src/menus.rs` (new Image menu), `app/keymap/grid.rs`, `app/keymap/viewer.rs`, `i18n.rs`. Tests: `tests/viewer_and_video.rs`.

**Interfaces:** Commands `RotateLeft` ("Rotate Left", `⇧R`), `RotateRight` ("Rotate Right", `R`), see Decision 15; Image menu holds them plus Part D items. Texture eviction moves to `fn evict_rendered(&mut self, ids: &[MediaId])`: pops `texture_cache` and `in_flight`, retires the image, re-requests the viewer frame when the open item is in `ids` (today's inline code at `viewer_actions.rs:485-511`; also used by Tasks 16a and 18).

- [ ] Tests: `rotate_selection_by_shortcut_persists_all_and_undoes_once` (grid, 3 images + 1 video, press `R`: images rotated in catalog, video untouched, one Cmd+Z restores); `shift_r_rotates_left_in_viewer`; `rotate_from_palette_in_viewer`; `r_in_text_field_does_not_rotate` (search field focused, typing `r` edits the query, catalog unchanged).
- [ ] Docs: shortcut sheet, README.

---

## Part D: Quick edits and external editor handoff

### Task 14: `media_edits` storage (S)

**Files:** Create `crates/lumina-core/src/catalog/edits.rs`. Modify `catalog/schema.rs`, `catalog/mod.rs` (Part D's `SCHEMA_VERSION`, the next one at landing time), `catalog/duplicates.rs` (`merge_metadata_in_tx` copies the `media_edits` row when keep has none; rotation stays carried only by `fold_in_tx`, `files.rs:205`, which is safe because the crop is in unrotated coordinates, Decision 11), `lib.rs` (wrappers; `set_edits` drops nothing from the disk cache because the cache holds unedited decodes).

**Schema (Part D version):**
```sql
CREATE TABLE IF NOT EXISTS media_edits (
    media_id INTEGER PRIMARY KEY,
    -- Crop in normalized [0,1] coordinates of the unrotated decoded frame; NULL = none.
    crop_x REAL, crop_y REAL, crop_w REAL, crop_h REAL,
    exposure_ev REAL NOT NULL DEFAULT 0,
    auto_enhance INTEGER NOT NULL DEFAULT 0,
    updated_epoch_ms INTEGER NOT NULL,
    FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
);
```
Rotation stays in `media_items.rotation_quarters`.

**Interfaces:** `#[derive(Debug, Clone, Copy, PartialEq, Default)] pub struct Edits { pub rotation: u8, pub crop: Option<CropRect>, pub exposure_ev: f32, pub auto_enhance: bool }` with `CropRect { x, y, w, h: f32 }` validated (`0 <= x, x+w <= 1`, `w,h >= 0.02`). `Catalog::get_edits(&[MediaId]) -> Result<HashMap<MediaId, Edits>>`, `Catalog::set_edits(&[(MediaId, Edits)]) -> Result<()>` (one tx writes both rotation column and side row; default `Edits` deletes the row). `Catalog::edited_ids() -> Result<HashSet<MediaId>>` for the grid badge (read once per view load and after edit writes, cached in app state; an item counts as edited if it has a `media_edits` row or non-zero rotation).

- [ ] Tests: `migration_creates_media_edits_idempotently` (`user_version == SCHEMA_VERSION`); `edits_roundtrip_and_default_deletes_row`; `invalid_crop_is_rejected` (also NaN and negative values); `fold_carries_edits` (missing-file fold, `Catalog::fold_into`, `files.rs:192`); `duplicate_merge_carries_edits_when_keeper_has_none`; `cascade_deletes_edits`.

### Task 15: Render edits in thumbnails and viewer (M)

**Files:** Create `crates/lumina-core/src/edits.rs` (pixel ops; decoder.rs stays decode-only). Modify `lib.rs:1415-1460` (`process_thumbnail_request` reads `Edits` once in place of `get_rotation`, decodes at `edit_decode_size`, then `apply_edits`; add `request_unedited_frame(id, size, cb)` for edit mode's preview source, Task 16a).

**Interfaces:**
- `pub fn apply_edits(frame: DecodedFrame, edits: &Edits) -> DecodedFrame`: crop (`image::imageops::crop_imm` on an `RgbaImage` view, unrotated coordinates), then rotate (existing `rotate_frame`), then one LUT: `lut[v] = encode(decode(v) * 2^ev)` with sRGB approximation, composed with auto-enhance stretch (0.5 % / 99.5 % luminance percentiles from a 256-bin histogram; identity if the range is < 16 levels).
- `pub fn edit_decode_size(target: u32, crop: Option<CropRect>) -> u32` = `ceil(target / max(w, h))` capped at 4096. Above `target * 2` the decode is not written to the disk cache (a 4096² RGBA frame is ~64 MB; one cropped viewer item must not evict hundreds of thumbnails); it is decoded per request instead.
- Videos: edits ignored (rotation already is).

- [ ] Tests: `crop_selects_expected_pixels` (4x4 fixture); `rotation_after_crop_keeps_region` (crop the top-left quadrant, rotate 90°: the frame shows the same pixels turned, not a different quadrant); `exposure_plus_one_ev_brightens_midtones_and_clips_white`; `auto_enhance_stretches_low_contrast` and `auto_enhance_leaves_flat_image_unchanged`; `edits_never_touch_original_or_cache` (engine, real JPEG: set crop, request thumbnail -> frame is cropped; original bytes hash unchanged; cache file decodes to the uncropped size); `edit_decode_size_caps`; `oversize_edit_decode_is_not_cached`.
- [ ] `make bench`: thumbnail throughput before/after with no edits (must be unchanged: the rotation read becomes one `LEFT JOIN media_edits` read, not an extra query).
- [ ] Manual check, reported in the commit body: one HEIC and one RAW (decoded through the native fallback) with crop + exposure render with the right orientation in grid and viewer (roadmap §3 "Validate RAW/HEIC orientation"). Fixtures are not committed.

### Task 16a: Edit mode in the viewer: adjustments, Done/Cancel, undo, badge (M)

**Files:** Create `crates/lumina-ui-gpui/src/app/quick_edit.rs` (state + actions), `crates/lumina-ui-gpui/src/views/viewer/edit_bar.rs`. Modify `views/viewer/mod.rs` (mount), `app/keymap/modals.rs` (edit-mode keys, see below), `app/keymap/viewer.rs` (`E`), `app/undo.rs` (`UndoAction::Edits { changes: Vec<(MediaId, Edits, Edits)> }`), `app/render/grid.rs` (edited badge), `command_palette.rs`, `src/actions.rs`, `src/menus.rs`, `i18n.rs`. Test: `crates/lumina-ui-gpui/src/tests/quick_edit.rs` (new; add to `tests/mod.rs`).

**Interfaces:**
- `pub struct QuickEditState { pub item: MediaId, pub draft: Edits, pub original: Edits, pub source: Option<Arc<RenderImage>>, pub crop_aspect: CropAspect }` grouped in one `Option<QuickEditState>` field on `LuminaApp` (not flat fields). The preview source is the **unedited** frame from `request_unedited_frame` (the normal viewer frame already has the saved edits applied); each draft change re-runs `apply_edits` on the background executor and replaces the preview texture. Never per-frame SQLite.
- Enter with `E` in the viewer (images only; `E` is unused in the viewer keymap) or palette "Edit Photo". Edit bar: Rotate L/R, Exposure −/+ (1/3 EV), Auto (toggle), Revert to Original, Cancel, Done. Crop arrives in Task 16b.
- Keys in edit mode are handled in `handle_modal_key_down` (`keymap/modals.rs`), because `handle_key_down` handles Enter (closes the viewer, `keymap/mod.rs:195-206`) and the viewer's `-`/`=` zoom before the viewer handler runs: `R`/`⇧R` rotate, `-`/`=` exposure, `A` auto, Enter = Done, Escape = Cancel; every other key is absorbed (AGENTS §6.3). Done writes `set_edits` once, pushes `UndoAction::Edits`, calls `evict_rendered`, refreshes the edited-ids cache, shows the undo toast; on `Err` keeps the draft open and `set_error`.
- Commands: `EditPhoto` ("Edit Photo", `E` in viewer), `RevertToOriginal` ("Revert to Original", acts on the selection, one undo entry). Image menu entries for both.
- Grid shows a small edited badge `grid-edited-badge-{id}` (icon-only, tooltip "Edited") from the cached `edited_ids`; role + label per §11.
- debug_selectors: `edit-bar`, `edit-rotate-left`, `edit-rotate-right`, `edit-exposure-down`, `edit-exposure-up`, `edit-exposure-value`, `edit-auto`, `edit-revert`, `edit-cancel`, `edit-done`.

- [ ] Tests: `keyboard_only_exposure_persists` (E, `=` twice, Enter -> catalog exposure within 1e-3 of 0.667, viewer still open); `escape_cancels_without_writing` (no `media_edits` row, viewer still open); `edit_mode_absorbs_delete_shortcut` (Cmd+Backspace in edit mode does not trash); `edit_mode_enter_does_not_close_viewer`; `done_then_undo_restores_original_once`; `revert_selection_is_one_undo_step`; `edit_write_failure_keeps_draft_and_banners`; `edited_badge_shows_only_for_edited`; `e_on_video_does_nothing`.
- [ ] `make ui-smoke`.
- [ ] Docs: README quick edits (what is and is not edited, export note), shortcut sheet, `docs/accessibility.md`.

### Task 16b: Crop overlay and aspect presets (M)

**Files:** Create `views/viewer/crop_overlay.rs`. Modify `app/quick_edit.rs`, `views/viewer/edit_bar.rs` (aspect control), `app/keymap/modals.rs`. Test: `tests/quick_edit.rs`.

**Interfaces:**
- While cropping, the preview shows the whole (uncropped, rotated) frame with the rectangle over it; the crop is applied to the preview on Done or when leaving crop. The overlay works in on-screen coordinates and converts to unrotated normalized coordinates once (Decision 11).
- Aspect presets: Original, Square, 4:3, 3:2, 16:9, Free; a preset centers the largest rect of that aspect.
- Mouse: four corner handles (drag). Keyboard: `C` cycles aspect, arrows move 1 % (Shift 10 %), `⌥` + arrows resize.
- debug_selectors: `edit-aspect-{name}`, `crop-overlay`, `crop-handle-{tl,tr,bl,br}`.

- [ ] Tests: `keyboard_only_crop_persists` (E, `C` to Square, Enter -> catalog crop square in pixel terms, viewer frame dimensions square); `crop_on_rotated_photo_stores_unrotated_coordinates` (rotation 1, crop the visible left half -> stored rect is the unrotated top half); `crop_handle_drag_resizes` (drag `crop-handle-br` by rendered bounds); `crop_arrow_keys_stay_inside_frame`.
- [ ] `make ui-smoke`.
- [ ] Docs: README crop paragraph; shortcut sheet.

### Task 17: Keyboard path and "Edit a Copy" for the external editor (M)

**Files:** Modify `app/viewer_actions.rs:235-266` (`edit_selected_externally(copy: bool)`), `app/keymap/mod.rs` (`⌘E` / `⌥⌘E` before the viewer handler, so they work in grid and viewer), `lib.rs` (`LuminaEngine::duplicate_for_edit(id) -> Result<MediaItem>`: picks `name (edited).ext` with `unique_destination` (`lib.rs:63`, same folder), copies into a file opened with `OpenOptions::create_new(true)` so a file that appears in between is never overwritten, catalogues it with `insert_or_update_batch`, then copies user metadata with `merge_metadata_in_tx(new, &[original])` in one tx (tags, caption, favorite, rating, label, title, plus rights and anything plans 4/5 add) and deletes the copy's `media_edits` row (Decision 13); on a catalog failure removes the copied file; rejects missing originals), `views/context_menu.rs` (add "Edit a Copy in External Editor"), `command_palette.rs`, `src/actions.rs`, `src/menus.rs`, `i18n.rs`. The copy runs on the background executor (a video can be gigabytes); the UI selects the new item and launches the editor when it finishes. Tests: core `duplicate_for_edit_*`; UI in `tests/editor_handoff.rs` (existing handoff tests and helpers).

**Interfaces:** Commands `EditInExternalEditor` (`⌘E`) and `EditCopyInExternalEditor` (`⌥⌘E`), enabled for one selected item (both unused today; verified in the four keymap files); with no editor configured they open Preferences on the editor row instead of only showing the banner. The copy gets a sidecar sync like any metadata write. Tests never launch an app (existing `#[cfg(test)] drop(command)`, `viewer_actions.rs:254`).

- [ ] Tests: `duplicate_for_edit_copies_file_and_metadata` (bytes equal, new id, tags and rights copied, no `media_edits` row and rotation 0 on the copy, original row untouched); `duplicate_for_edit_never_overwrites_existing_name`; `duplicate_for_edit_catalog_failure_removes_copy`; `edit_in_external_editor_by_shortcut_builds_command` (assert the built command args, not launch); `edit_copy_selects_new_item`; `unset_editor_shortcut_opens_preferences`; `cmd_e_in_text_field_is_ignored`.
- [ ] Docs: README "Edit in…" round trip; `docs/accessibility.md`.

### Task 18: Round trip refreshes the library (S)

**Files:** Modify `app/library.rs:213-237` (`BatchDiscovered`, outside an active scan: call `evict_rendered` for every id in the batch that is in `texture_cache`; the watcher only sends files whose signature changed, `watcher.rs:322-336`, so no old/new comparison is needed and the UI does not hold the old `MediaItem` anyway; `evict_rendered` already reloads the open viewer item). Test: `tests/editor_handoff.rs`.

- [ ] Failing test first: `external_save_refreshes_texture_and_keeps_metadata` (seed item with tag + crop edit, render grid, overwrite the file with a different image via temp+rename, deliver the watcher batch -> texture cache entry gone and re-requested, same id, tag and edits kept). If it passes before the change, record that and keep only the test.
- [ ] Test `scan_batches_do_not_evict_textures` (during `is_scanning`, a batch leaves textures alone: a full rescan must not flush the grid).
- [ ] `make ui-smoke`.
- [ ] Docs: README round-trip paragraph: in-place saves update the item; "Save As" files appear as new items; Lumina edits stay applied on top (use Revert to Original if the editor already cropped).

---

## Open questions for the owner

1. **Separator `/` and existing tags.** Any existing tag with `/` (e.g. `24/7`) becomes a parent/child pair. Run `SELECT tag, COUNT(*) FROM media_tags WHERE tag LIKE '%/%' GROUP BY tag;` on a real library. Accept `/`, or choose `|` / ` > ` with a display mapping?
2. **Foreign sidecars.** Task 6 skips writing them (safe, but Lumina edits then don't reach that sidecar). Is skip-and-report acceptable for 0.4, or do you want a real merge (needs an XML crate such as `quick-xml`, MIT, plus a review of its CI/licence)?
3. **Rights searchable?** Should creator/copyright be FTS-indexed and/or a smart-album field? Either needs an FTS rebuild migration; this plan leaves both out.
4. **Shortcuts.** Proposed: `R`/`⇧R` rotate (Decision 15; `⌘[`/`⌘]` are Back/Forward, `⌘R` is rescan), `E` edit (viewer), `⌘E`/`⌥⌘E` external editor, no shortcut for templates. OK?
5. **Task 10 (editable rights per item)** — needed in 0.4, or are templates enough?
6. **Export of edited photos.** Export copies originals today (`lib.rs:771`), so crop/exposure are not in exported files. Ship 0.4 with a note in the export report ("edits are not included"), or add "Export Edited JPEG" (full-resolution decode + `image` JPEG encoder; HEIC/RAW decode at full size needs checking)? Plan 5 proposes "Export as JPEG…"; if it lands, it should run `apply_edits` so that path exports what the user sees.
7. **Edit a Copy vs in place as the default** for `⌘E`. The plan makes in-place the default (current behaviour) and the copy `⌥⌘E`.
8. **Roadmap §3 acceptance: "a selection can receive keywords and a preset without opening each item."** This plan reads "preset" as a metadata template (Task 9). If it means an edit preset (e.g. Auto-Enhance or an exposure applied to a selection), that is a small extra task on `set_edits` + `UndoAction::Edits`; say so.

## Out of scope

- Straighten, white balance, contrast, filters/presets, red-eye, RAW develop, per-item version history and version stacks (stacks: 0.6).
- Writing edits or rotation to XMP (`crs:`, `tiff:Orientation`); edit portability beyond the catalog backup.
- Controlled-vocabulary import/export (keyword list files), recently-used tag lists, tag colors/pins, drag-to-reparent, expand/collapse tree state.
- Template file import/export; contact phone/address fields; `xmpRights:Marked`.
- Batch metadata copy/paste between items; location by place-name search; batch rename/move.
- Title in FTS (not indexed today; separate issue).
- Near-duplicate tag cleanup report (roadmap §4): needs a similarity rule the owner has not chosen; the subtree merge in Task 3 is the tool it would feed.
- A one-time normalization of existing tags (e.g. `a / b` stored before cleaning) into canonical paths: decide after Open question 1's query shows real counts.
- Tag right-click menu and drag-tags-onto-selection (ux_audit §7): UX-2 / other plans.
- Rubber-band selection and other §6 items outside metadata.
- VoiceOver claims for the new controls beyond setting roles/labels (unverified with VoiceOver, AGENTS §15).

## Review notes (2026-11-05)

Independent review against HEAD `5110780`; code read only (no build, no tests, no catalog opened). Changes made:

1. **Task 1 rewritten.** The FTS mismatch is real, but its effect is different from the original claim: on the contentless table the old caption tokens are never deleted, so the planned regression test (`golden` still matches after tagging) would pass before the fix and the plan said to then drop the fix. New tests use a word shared by caption and tag. Added the `merge_metadata_in_tx` FTS mismatch (tags/caption copied without a refresh, then `media_fts_au` deletes the wrong values) and a one-time index rebuild for existing libraries, which takes Part A's schema bump.
2. **Schema numbering made relative** ("next `SCHEMA_VERSION`, currently 14"); renumber against plan 5; noted the literal `assert_eq!(SCHEMA_VERSION, 14)` in `catalog/sources.rs:198`; migration tests assert `SCHEMA_VERSION`, not literals.
3. **Subtree SQL fixed.** The original range `[p/, p0)` was right, but the clause returned `&'static str` from a formatted column and used `?1` inside search.rs's anonymous-parameter builder. Replaced with one range + segment filter and explicit params; added `places-old` / `places.x` sibling cases.
4. **`ids_with_tag` kept exact** (the original plan made it subtree; tag undo depends on exact rows). `get_items_by_tag` gets `EXISTS` to avoid duplicate rows.
5. **Rotate shortcut moved off `⌘[`/`⌘]`** (Back / plan-2 Forward) to `R`/`⇧R`, verified unused in all four keymap files and in plans 2, 3, 5, 6; `⌘E`/`⌥⌘E` verified free. Edit-mode keys moved to `handle_modal_key_down`, since global Enter closes the viewer and the viewer's `-`/`=` zoom run first.
6. **Crop stored in unrotated coordinates, applied before rotation**, so rotating a cropped photo (R key, context menu, fold) needs no remap; test added.
7. **Sidecar ownership check scans markup only** (Lumina does not escape `:`/`=` in text, so a caption could flip its own sidecar to foreign); unreadable/UTF-16 sidecars are skipped; legacy fixtures from each released render version (names verified via `git show`).
8. **Sidecar sync gaps:** global tag rename/merge/delete never synced sidecars (a later Import XMP re-adds old names); template apply had no sync. Added to Global Constraints and Tasks 3, 9.
9. **Undo safety:** `TagChange` carries alias rows; undo leaves smart albums edited since the op alone; FK-safe inserts; bulk delete is one transaction.
10. **XMP hierarchy:** flat tags also go into `lr:hierarchicalSubject`, otherwise a flat `europe` beside `places/europe` was dropped on re-import; `|` in tags stays flat.
11. **Task 16 (L) split** into 16a (adjustments, undo, badge) and 16b (crop). Preview now starts from an unedited frame (the viewer frame already has the edits).
12. **Edit a Copy:** reuses `unique_destination` + `merge_metadata_in_tx`, never-overwrite create, runs off the UI thread, starts with no Lumina edits; tests moved to the existing `tests/editor_handoff.rs`.
13. **Task 18 simplified:** the watcher only emits changed files, so evict every batch id outside a scan (the UI has no old item to compare); added a rescan-does-not-flush test.
14. Smaller fixes: the threading constraint now matches the codebase (short catalog writes stay synchronous); `menus.rs`/`actions.rs` paths; the `MAX_NAME_CHARS` (100) whole-path limit refuses instead of truncating; oversize edit decodes skip the disk cache; `batch.rs` (532 lines) and `app/tags.rs` (583) get sibling files instead of growing; corrected citations (rescan keeps edited dates at `media.rs:90-100`, not `files.rs`; rule rewrite covers every operator; SELECT-list count; `merge_metadata_in_tx` at `duplicates.rs:292-384`); added Open question 8 and three out-of-scope items from roadmap §4 / ux_audit §7.

Since observed (sqlite3 CLI, contentless FTS5 `tags, caption`, row with tags `beach` and caption `sunset beach`, then a tag edit done the way `tags.rs` does it, with no caption column): `MATCH 'sunset'` still finds the row (a stale token that can never be deleted), and `MATCH 'beach'` finds nothing although the caption still says beach. So Task 1's shared-word test fails before the fix, as written. Not verified: whether `leaves_key_for_text_input` already blocks plain `R`/`E` in every text field (tests added); the index plan for the new subtree clause (Task 2's `EXPLAIN QUERY PLAN` test checks it); plan 5's final merge order for `merge_metadata_in_tx`.

