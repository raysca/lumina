# Design 15: Settings — Downloads — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Give Preferences a Downloads page in the frame's style (storage meter, grouped setting rows with segmented controls, selects and toggles), wired only to settings the importer can honour.
**Design:** `lumina.pen` frame `g52QWL` "Settings — Downloads" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::nav_item`; Task 3 splits `theme.rs` into `theme/{mod,controls,widgets}.rs`); Design 18 Task 1 (the Settings shell, category sidebar and top bar through 01's `page_header`) and Task 2 (the row, group, toggle, select and segmented widgets in `views/preferences/mod.rs`). Design 13 reads the concurrency setting.
**Icons (fetched in 00 Task 1):** hard-drive, eraser, chevron-down, wrench (the Library size row's rescan button, moved from Design 18). Already in `resources/icons/`: folder, rotate-ccw.
**App today:**
- Preferences is a centred modal, not a screen: `crates/lumina-ui-gpui/src/views/preferences.rs:11-178` (680 w card, an icon tab strip with General / Storage / About at 26-44, a body at 163-176).
- Pages: General `preferences.rs:180-231` (library folder, items indexed, Rescan, external editor), Storage `233-301` (thumbnail cache location, size, Clear Cache, config, logs), About `303-367`. Row helpers: `group` 383, `row` 401, `value` 420, `button` 445.
- State: `crates/lumina-ui-gpui/src/app/preferences.rs:10-16` (`PreferencesTab`), cache usage `31-57`, `clear_thumbnail_cache` 135. Config: `crates/lumina-ui-gpui/src/config.rs:337` (`AppConfig`), saved through `app/state.rs:710` `save_config` (no-op under test).
- No download settings exist. Concurrency is the constant `MAX_RUNNING = 3` (`app/downloads/mod.rs:25`), and downloads stage inside the library's `Imports/YYYY/YYYY-MM-DD/` (`docs/network-import.md`).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- Config writes go through `save_config`. A failure reverts the control and shows the error banner (AGENTS §2). Tests never persist config.

## Copy check (flagged)
No string names yt-dlp, ffmpeg, curl, brew or HLS. These strings promise things Lumina does not do or must not do:
- Top bar meta "How links and files are fetched, converted and filed away" → **"How videos from links are downloaded and filed away"**. Nothing is converted; the bundled tools only remux.
- TRANSFERS note "Applies to URL imports and shared-album syncs" → **"Applies to videos imported from a link"**. There are no shared-album syncs; two-way sync is out of scope (AGENTS §5.4).
- "Convert to HEVC after download" → **remove** (decision 6).
- "Save downloads to … ~/Pictures/Lumina/Downloads" / "Finished items are imported, then the temporary file is removed." → **"Downloads go to"** with the real `<library>/Imports` path. Desc: **"Each download is filed by date and added to the library."** (decision 8).
- Sidebar search field ("Search settings", shown twice): **removed**. Settings has no search field (lead decision).

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Container | modal card 680 w over a dim backdrop (`preferences.rs:78-125`) | full window: the settings sidebar replaces the library sidebar, and the page fills the main area | relayout, owned by Design 18 Task 1 (decision 1) |
| Settings sidebar | icon tab strip, 84×54 tabs (`preferences.rs:16-73`) | 252 w sidebar (Foundation); search field 34 h bg-0 r8; label "SETTINGS" 10/600 tracking 0.9; NavItems 32 h, padding 0 10, r8, icon 16 + Inter 13; selected bg-3, t1, 500; categories General, Library, Import, Downloads, Sync & Backup, Appearance, Shortcuts, Privacy, Advanced; Library size meter at the bottom | Design 18 Task 1 builds it; this plan adds Library, Downloads and Storage (decision 2). No search field. The Library size meter moves into the Library category (Task 4) |
| Top bar | 38 h title row with centred tab name + close (`preferences.rs:126-162`) | 67.5 h, padding 0 28, bottom border line; "Downloads" Inter Tight 20/600 −0.3; meta Inter 12 t2; "Reset to defaults" 32 h bg-2 r8 outline line, `rotate-ccw` 14 | rendered through 01's `page_header` (`page-title`, `page-meta`) by Design 18's shell; Reset is 18 Task 4's (if approved) |
| Close | `preferences-close` 20 px X (147-160) | none (sidebar navigation) | Esc closes and ⌘, toggles (Design 18) |
| Content | `px 26 py 22` scroll body (163-176) | padding 16 32, gap 14 | restyle |
| Library storage card | Storage page shows only the thumbnail cache (`preferences.rs:233-276`) | bg-1, r12, padding 16, gap 13; `hard-drive` 15 + "Library storage" 13.5/600; right "412.8 GB of 1 TB used" mono 11.5 t2; 10 px stacked bar bg-3 (accent photos, chart-blue videos, chart-violet downloads, t3 cache); legend dots 8 + label 11.5 t2 + size mono 11 t3; "Clear cache · 6.4 GB" 28 h bg-2 r8 `eraser` 12 | new (decision 3); Clear cache reuses `clear_thumbnail_cache` |
| Section head | `section_label` (376-380) | 10/600 tracking 0.9 t3; optional right note 11.5 t3 | restyle |
| Group | `group`: border line, `bg_app`, 44 px rows, 130 px label column (383-418) | bg-1, r12, padding 0 16, 1 px `line` outline; rows: label 13/500 t1 + optional desc 11.5 t2, padding 14 (with desc) or 11 (without), gap 24, control right-aligned; dividers line | use Design 18 Task 2's `group`/`row` |
| Segmented control | `theme::segmented` exists (`theme.rs:320`; after 00 Task 3, `theme/controls.rs`), 28 h, padding 2 | bg-0, r9, 1 px `line_2` outline, padding 3, gap 2; segments 28 h, padding 0 12, r6, 12.5; selected bg-3 t1 500, others t3 | Design 18 Task 2's `segmented(id, options, selected)`, which restyles the existing one; nothing new here |
| Select | none (General's editor uses buttons) | 34 h, bg-0, r9, padding 0 11, 1 px `line_2` outline, value 12.5 t1, `chevron-down` 13 | Design 18 Task 2's select; no select ships on this page under the recommendations below |
| Toggle | none | 40×22, r999, padding 2; on accent with knob 18 `#1A1206` (accent-fg); off bg-3 with a 1 px `line_2` outline and knob t3 | Design 18 Task 2's `toggle(id, on)`; no toggle ships on this page under the recommendations below |
| Library location (Library page) | "Library folder" + "Change…" opens another library (`preferences.rs:187-193`, `preferences-change-library`) | from frame `l85pQN` (18): "Library location", description, path pill (bg-0, r9, `line_2` outline, `folder`, mono 11.5 t2), "Choose…" (34 h, r9, bg-3, `line_2` outline, 12.5/500) | restyle; the button keeps today's behaviour (open another library) and is labelled "Open Another…". The design's description "Moving the library copies 412.8 GB…" is wrong: nothing is moved. Use "The folder this library lives in." |
| Library size (Library page) | "Items indexed: N" + "Rescan Library" (`preferences.rs:194-205`, `preferences-indexed`, `preferences-rescan`) | "Library size": mono 11.5 t2 "412.8 GB · 48,291 items" and a "Rebuild index" row button with `wrench` | restyle; real bytes (Task 4's `storage_summary`) and item count. The button keeps `rescan_library` and the label "Rescan Library": the app has no separate index rebuild |
| Path chip + Choose… | `value(path, mono)` + `button` (`preferences.rs:191-197`) | path 34 h bg-0 r9, `folder` 13, mono 11.5 t2; "Choose…" 34 h bg-3 r9 | restyle |
| Simultaneous downloads | constant 3 | segmented 1 / 2 / 3 / 5, desc "Extra items wait in the queue until a slot frees up." | new setting (decision 4) |
| Speed limit | none | select "25 MB/s" (120 w) + toggle | needs decision 5 |
| Resume interrupted downloads | interrupted rows come back Paused and wait for Resume (`docs/network-import.md`) | toggle | needs decision 5 |
| Retry failed downloads | manual Retry only | select "3 times, 30 s apart" (200 w) | needs decision 5 |
| Preferred video quality | always best H.264 | select "2160p (4K) when available" (260 w), desc "Falls back to the next best stream when the exact match is unavailable." | needs decision 6 |
| Container | MP4 (web video merged to MP4; direct links keep their file) | segmented MP4 / MKV / Original | needs decision 6 |
| Convert to HEVC | none | toggle | remove (decision 6) |
| Add to album | none | select "Ask each time" (180 w) | needs decision 7 |
| Keep source URL in metadata | always kept (`dc:source`, Downloaded album) | toggle | needs decision 7 |
| Watch the clipboard | none (`app/network_import.rs:38-39`: deliberately no clipboard reads) | toggle | needs decision 9 |
| Auto-import from the Downloads folder | none | toggle (off) | needs decision 9 |

## Not in the app yet (needs a product decision)
1. **Settings as a full-window screen.** **Decided:** full-window, in-window mode, owned by Design 18 Task 1 (both render roots, and a no-library window with its own category sidebar). Esc closes and ⌘, toggles. This plan only adds categories and their pages, and keeps the Downloads category.
2. **Categories.** Only categories with real content ship. Order (pinned in 18 Task 1): General, Library, Downloads, Storage, About (Advanced was dropped by 18 because it became empty). This plan owns **Library** (moved from 18's General page: the Library location row, the Library size row with its rescan button, and the library storage meter; today these are `preferences.rs:186-207`), **Downloads** (this frame) and **Storage** (today's Storage rows, `preferences.rs:233-301`, with their selectors). **Drop** Import, Sync & Backup (S3 backup is planned for 0.5 and not built; AGENTS §15), Appearance, Shortcuts and Privacy.
3. **Library storage meter.** Size **M**, core work: a query summing `file_size` by photo / video / downloaded (an indexed aggregate, measured against the release gates), and the cache from `CacheUsage`. Volume capacity goes through the existing native bridge, with no new crate: `build.rs:4-10` compiles the Objective-C files (`src/player/macos_bridge.m`, `src/share/macos_share.m`) with Foundation linked (:15), and Rust calls them through `extern "C"` (pattern: `src/share/mod.rs:19-25` → `macos_share.m:36`). Add `lumina_volume_capacity(path, *total, *available) -> bool` using `NSURLVolumeTotalCapacityKey` / `NSURLVolumeAvailableCapacityKey`, compiled by the same `cc::Build`. (`src/bridge.rs` only converts frames to GPUI images, so it is not the place.) Non-macOS returns `None`. Run off the UI thread and cache the result. **Recommend: build here.** The card and the "Library size" row sit in the Library category.
4. **Simultaneous downloads.** Size **S**. Replace `MAX_RUNNING` with a config value (1/2/3/5, default 3). Lowering it never cancels running jobs; it only stops new starts. **Recommend: build here.**
5. **Speed limit, auto-resume, auto-retry.** Each is **M** with core work in `net_import` (a rate-limit flag for each downloader; a retry timer in the scheduler). "Resume interrupted downloads" cannot mean continuing partial bytes (downloads start over). At most it could mean "start interrupted downloads when the library opens", and that conflicts with the documented rule that nothing downloads until the user resumes. **Recommend:** drop auto-resume. Speed limit and retry go to a separate plan if wanted; leave them out of this page until then.
6. **Quality, container, HEVC.** Quality selection is **M** core work (format choice in `net_import/helper.rs`), shared with Design 12, decision 4: separate plan. Container: MKV/Original can produce WebM or VP9 that the library refuses or macOS cannot play (`docs/network-import.md`), so **drop**. Convert to HEVC needs an encoder the copy-only bundled build does not have (licensing and AGENTS §5.4 pinning): **drop from design**.
7. **Add to album / keep source URL.** Add to album pairs with Design 12, decision 5: separate plan. Turning the source URL off breaks the Downloaded album, the "already downloaded" check and Get Info: **drop the toggle**. If shown at all, state it as a fact ("The source link is saved with each download").
8. **Save downloads to.** Downloads are staged and filed inside the library (`Imports/YYYY/YYYY-MM-DD/`), never in an outside folder. A chooser would change the filing model (**L**). **Recommend:** show the real path read-only with a "Show in Finder" button instead of "Choose…".
9. **Clipboard watching and Downloads-folder auto-import.** Both are background behaviour. Clipboard polling was deliberately avoided (macOS paste prompts, `app/network_import.rs:38-39`). A watcher on `~/Downloads` reaches outside the library. **Recommend: drop from design** (or a separate product decision). Remove the AUTOMATION section.

With these recommendations: the **Downloads** page has Transfers (Simultaneous downloads) and Saving (path, read-only). The **Library** page has the library rows, the storage card and the Library size row. The **Storage** page has today's cache, config and log rows. Reset to defaults (Design 18 Task 4, if approved) resets only settings that exist.

## App has, design lacks
- Esc to leave settings: **keep** (keyboard path; Design 18).
- Thumbnail cache location / Reveal Cache, Clear Cache with "Cleared", config file, log file rows (`preferences.rs:233-301`): **keep** in this plan's Storage category, with every existing selector.
- About page (version, licence, acknowledgements): **keep** (Design 18). Bundled third-party notices must stay reachable (AGENTS §15).

## Review Focus
- Config save fails: the segmented control snaps back to the previous value and the error banner shows (`SAVE_FAILS` test hook in `app/state.rs`).
- Changing concurrency while downloads run: lowering it starts nothing new until the running count drops. Raising it starts waiting rows at once.
- Storage meter on a library with 0 bytes or an unknown volume capacity: the bar shows the segments it has and hides "of N used". There is no division by zero, and segments under 1 px still show in the legend.
- Keyboard: ↑/↓ reach the categories (Design 18) and Tab walks every control. ←/→ change a focused segmented control.
- Narrow window: rows wrap the control under the label below about 640 px of content width. The sidebar stays 252.

## Tasks
### Task 1: Library, Downloads and Storage categories
**Files:** Modify `crates/lumina-ui-gpui/src/app/preferences.rs:10-16` (categories), the `views/preferences/` module Design 18 Task 1 creates / Test `crates/lumina-ui-gpui/src/tests/preferences.rs`
- [ ] Write failing test `settings_has_library_downloads_and_storage_categories`: after `open_preferences`, `settings-category-{library,downloads,storage}` exist in Design 18's `settings-sidebar`, in the pinned order. Clicking `settings-category-downloads` (and `down` from General) selects `Downloads`. `page-title` reads "Downloads". The palette entry "Settings: Downloads" opens it. The Library page shows `preferences-change-library` (label "Open Another…", clicking it calls the open-library picker through the existing test seam), `preferences-indexed` with "<N> items" for a seeded temp library (Task 4 extends the assertion to "<size> · <N> items"), and `preferences-rescan` (label "Rescan Library", with the `wrench` icon); clicking it starts `rescan_library`. The sidebar order is General, Library, Downloads, Storage, About. The Storage page holds the existing `preferences-reveal-cache`, `preferences-cache-size`, `preferences-clear-cache`, `preferences-reveal-config` and `preferences-reveal-logs`.
- [ ] Implement: add `PreferencesTab::{Library, Downloads, Storage}` to Design 18's shell. Move the library location and Library size rows (from Design 18's General page) into `views/preferences/library.rs` with the copy from the table (the storage meter joins them in Task 4) and the Storage rows into `views/preferences/storage.rs`. Each page's heading and meta go through `page_header`. Add the palette entry. Only categories with real content ship (decision 2).
- [ ] Run `cargo test -p lumina-ui-gpui preferences`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): settings library, downloads and storage categories`, staging explicit paths.

### Task 2: (removed) Widgets come from Design 18 Task 2
No new helpers here. This plan uses 18's `group`, `row`, `path_pill` and `segmented` from `views/preferences/mod.rs`. Do not add `theme::segmented`, `theme::toggle` or `theme::select`: `segmented` already exists (`theme.rs:320`, in `theme/controls.rs` after 00 Task 3), and 18 restyles it.

### Task 3: Simultaneous downloads (decision 4)
**Files:** Modify `crates/lumina-ui-gpui/src/config.rs:337`, `crates/lumina-ui-gpui/src/app/downloads/mod.rs:25,314-338`; Create `views/preferences/downloads.rs` / Test `tests/downloads.rs`, `tests/preferences.rs`
- [ ] Write failing tests. `concurrency_segmented_responds_to_keys`: a focused `settings-concurrency` moves 3→5 on → and 3→2 on ←. `concurrency_setting_limits_running_jobs`: with the setting at 1, only one of three queued rows runs. Raising it to 3 starts the other two. `concurrency_save_failure_reverts`: with `SAVE_FAILS`, the control returns to 3 and `error_message` is set.
- [ ] Implement `download_concurrency` in `AppConfig` (default 3, allowed 1/2/3/5). `pump_downloads` reads it. Replace the `MAX_RUNNING` uses in tests with the config default.
- [ ] Run `cargo test -p lumina-ui-gpui downloads preferences`; expect pass.
- [ ] Gate and commit `feat(downloads): choose how many downloads run at once`.

### Task 4: Library storage card (decision 3)
**Files:** Modify `crates/lumina-core/src/catalog/` (a `storage_summary()` query beside the media queries), `crates/lumina-ui-gpui/build.rs:4-10` (add the volume `.m` file to the `cc::Build` and a `rerun-if-changed`); Create `crates/lumina-ui-gpui/src/storage/macos_volume.m` and `src/storage/mod.rs` (`extern "C"` wrapper, same pattern as `src/share/mod.rs:19-25`); Modify `crates/lumina-ui-gpui/src/app/preferences.rs:31-57,131-162`, `views/preferences/library.rs` / Test `crates/lumina-core/tests/core_tests.rs`, `tests/preferences.rs`
- [ ] Write failing tests. Core `storage_summary_sums_by_kind`: a temp library with 2 photos, 1 video and 1 downloaded video returns the right byte sums, with the downloaded bytes counted once. Check `EXPLAIN QUERY PLAN` for no full sort. UI `storage_card_shows_segments`: `settings-storage-bar` (on the Library page) has four segments whose widths are proportional within 1 px. `settings-storage-clear-cache` clears the test cache dir (the Storage page keeps its own `preferences-clear-cache`). `volume_capacity_of_temp_dir_is_positive` (macOS only, `#[cfg(target_os = "macos")]`): total > available > 0.
- [ ] Implement off the UI thread (background executor) and cache the result in preferences state. The capacity comes from the native bridge call on the library folder; when it returns false (or on other platforms), hide "of N".
- [ ] Run `cargo test -p lumina-core storage` and `cargo test -p lumina-ui-gpui preferences`; expect pass. Run `make bench` if the query touches the media table's hot path.
- [ ] Gate and commit `feat(design): library storage breakdown in settings`.

### Task 5: Saving row (decision 8)
**Files:** Modify `views/preferences/downloads.rs` / Test `tests/preferences.rs`
- [ ] Write failing test `downloads_path_row_reveals_imports`: `settings-downloads-path` shows `<library>/Imports` in tilde form (`tilde`, `preferences.rs:369`), and `settings-downloads-reveal` calls the reveal action (test seam, no Finder).
- [ ] Implement with Design 18's `path_pill` (bg-0, r9, `folder` 13, mono 11.5 t2) and a "Show in Finder" button in the "Choose…" style, with the copy from the Copy check.
- [ ] Run, gate, commit `feat(design): show where downloads are filed`.

Manual checks against `g52QWL.png`: storage bar colours (accent / chart-blue / chart-violet / t3), group `line` outline, segmented `line_2` outline. GPUI headless tests cannot assert colours.
