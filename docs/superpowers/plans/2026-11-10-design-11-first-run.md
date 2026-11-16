# Design 11: Empty — First Run — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Replace the stand-alone welcome column with the frame's first-run view (the app shell with a dormant sidebar, the "All Media / No media yet" header, a centred empty state and three hint cards), keeping every welcome action and shortcut the app has today and changing copy that promises features Lumina lacks.
**Design:** `lumina.pen` frame `DiFtb` "Empty — First Run" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, Sidebar). Design 01 Library Task 1 (`page_header`). Task 4 depends on Design 09 Task 1 (`render_empty_state` with its `Hero` size variant).
**Icons (fetched in 00 Task 1):** folder-open, folder-sync, file-image, shield-check, aperture (the glyph and brand mark, if 00 has not already listed it). folder, camera, search, images, image, heart and x already exist in `resources/icons/`.
**App today:**
- `crates/lumina-ui-gpui/src/app/render/mod.rs:424-453`: with no library (`grid_state` is None) the root paints only `OnboardingView` (no sidebar, no header), plus the error banner, toast and Preferences.
- `crates/lumina-ui-gpui/src/views/onboarding.rs:7-196`: 520 px left-aligned column 120 px from the top. 60 px amber logo with a `Sparkles` icon (21-32), "Welcome to Lumina" `text_3xl` bold + intro (95-109), three feature rows (folder / check / database, 110-131, 379-410), recent libraries (132-134, 257-375), two equal 38 h buttons "Open Existing Library…" ↵ and "Import into New Library…" ⇧⌘I (135-161, 413-451), links "Explore a Sample Library" ⌥↵ and "Use Pictures Folder" ⇧↵ (162-187), footer about ⌘O (188-193), missing-library notice (232-255).
- `crates/lumina-ui-gpui/src/app/recents.rs:212-254`: `welcome_props`: Open → `pick_folder_dialog` (`app/library.rs:635`), Import new → `pick_new_library_for_import` (`app/import.rs:121`), Pictures → `open_pictures_library`, Sample → `open_sample_library` (`app/sample.rs:13`).
- `crates/lumina-ui-gpui/src/i18n.rs:259-293`: `welcome.*` strings.
- `crates/lumina-ui-gpui/src/app/render/mod.rs:455-470` + `crates/lumina-ui-gpui/src/views/first_scan.rs:16-249`: "Building your library" screen during the first scan of an empty catalog. Not in this frame.
- `crates/lumina-ui-gpui/src/app/render/grid.rs:870-910`: in-library empty states. "Scanning…" with the path, "No photos or videos found" + Choose Another Folder… / Rescan, "Your library is empty" + Open Library…, all in a 44 px icon tile with a 440 px column (929-1040).
- Tests: `crates/lumina-ui-gpui/src/tests/welcome.rs` (layout 12-78, recents 321-500, palette 512-585, keys 639-786).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- Copy must not promise unbuilt features: no cameras, no Apple Photos, no faces (AGENTS §15).
- Visual properties (glow, ring, colours, type) are a manual comparison with `DiFtb.png`. Tests cover selectors, bounds, enabled state and actions.

## Copy check (flagged)
- Body "Point Lumina at a folder, a drive, or a camera. Nothing is copied or moved — Lumina reads your files where they already live and leaves them there." → **"Point Lumina at a folder or a drive. Nothing is copied or moved: Lumina reads your files where they already live and leaves them there."** Lumina has no camera import. This copy is true for Choose a folder (open in place). Import into New Library copies, and keeps its own label.
- "Connect a camera" and "or import from Apple Photos" → not shown (decisions 1, 2).
- Hint "On-device — Faces and places never leave your Mac" → **"On-device — Your catalog and places stay on this Mac"**. There is no face recognition. Geocoding is offline.
- Hint "RAW and video — RAF · CR3 · HEIC · ProRes" → **"RAW · HEIC · MOV · MP4"**. These formats come from the scanner's list (`lumina-core/src/scanner.rs:176-180`). ProRes playback depends on the system player and has not been checked.
- Hint "Watch folders — Stays in sync as you shoot" → keep. The watcher exists (`lumina-core/src/watcher.rs`).
- Sidebar placeholders "No albums yet", "Save a search to create one", "Tags appear as you import", search "Nothing to search yet" → keep.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Shell | welcome column only, no sidebar or header (`render/mod.rs:424-453`) | Sidebar 252 + main column; header 67.5 h; timeline rail at right | relayout; no rail (with no library there are no buckets, and 01's rail has no empty form) |
| Sidebar search | n/a | 34 h, bg-0, outline line, r8, opacity 0.5; "Nothing to search yet" Inter 12.5 t3; no shortcut | new (dormant variant, see Task 1) |
| Sidebar nav rows | n/a | All Media, Photos, Videos, Favourites only; opacity 0.45; label t3 13; count "0" mono 10 t3 | new (dormant variant). The "Photos" row ships only if 00 decision 2 (a Photos row) is approved; otherwise three rows |
| Sidebar section hints | n/a | ALBUMS / SMART ALBUMS / TAGS: label 10/600 tracking 0.9 t3; hint box 34 h, r7, outline line, padding 0 10, Inter 11 `#4E4E58` | new (`#4E4E58` is not a pen variable: Foundation maps it, or use t3) |
| Sidebar storage | n/a | "Library size" Inter 11 t3, "0 GB / 1 TB" mono 10 t3, empty 4 h track bg-3 | per Foundation; with no library open, show no meter (see Review Focus) |
| Header | none | "All Media" Inter Tight 20/600 −0.3; meta "No media yet" mono 10.5 t3; no actions | new (Design 01 `page_header`, actions hidden) |
| Timeline rail | none | 83.5 w, bg-1, left border line, padding 8 10; "TIMELINE" Inter 8/600 tracking 1 `#3A3A44`; centred "—" mono 12 `#3A3A44` | not built (lead decision: no empty rail form) |
| Glyph | 60 px amber square with a `Sparkles` icon, r14 (`onboarding.rs:21-32`) | 150 box: radial glow accent 20 % → 0; ring 84 circle bg-1, outline 1 px accent 35 %; `aperture` 36 accent | restyle (aperture is the brand mark) |
| Headline | "Welcome to Lumina" `text_3xl` bold, left (`onboarding.rs:99-103`) | "Your library is empty", Inter Tight 28/600 −0.6, t1, centred, 20 below the glyph | restyle + copy |
| Body | intro `text_base` muted (104-108) | Inter 13.5, line height 21, t2, 470 w, centred, 12 below | restyle + copy (see Copy check) |
| Primary button | "Open Existing Library…" 38 h, flex-1, accent, ↵ kbd (139-148, 413-451) | "Choose a folder…" 40 h, padding 0 18, gap 8, accent, r9, `folder-open` 15, Inter 13/600 accent-fg; 26 below the body | restyle + copy; the ↵ hint stays |
| Secondary button | "Import into New Library…" ⇧⌘I (150-159) | "Connect a camera", 40 h, bg-2, outline line-2, r9, `camera` 15, Inter 13 t1 | restyle; keep the Import label and action (decision 1) |
| Text link | "Explore a Sample Library" ⌥↵, "Use Pictures Folder" ⇧↵ (162-187) | one accent link "or import from Apple Photos" Inter 12, 14 below the buttons | restyle: the links take this slot and style (decision 2) |
| Hint cards | three feature rows with 32 px icon tiles (110-131, 379-410) | 3 equal cards gap 12, 46 below the link; bg-1, outline line, r9, padding 14, gap 7; icon 16 accent (folder-sync, file-image, shield-check); title Inter 12.5/500 t1; sub Inter 11 lh 16 t3 | restyle + copy |
| Recent libraries | list with open/remove, ↑/↓, ⌫ (`onboarding.rs:257-375`) | none | keep (see App has) |
| Missing-library notice | banner above the title (232-255) | none | keep, above the glyph |
| Footer ⌘O hint | (188-193) | none | remove; ⌘O stays in the File menu and the palette |

## Not in the app yet (needs a product decision)
1. **Connect a camera.** Size **L**, platform work (ImageCaptureCore on macOS: device discovery, a download-to-folder import, permissions). Nothing exists. **Recommend: drop from design.** The secondary button stays "Import into New Library…" (⇧⌘I) with the frame's secondary style and a `folder` icon.
2. **Import from Apple Photos.** Size **L**. Reading a `.photoslibrary` (originals, edits, albums) is a new importer, and AGENTS §5.4 rules out iCloud Photos integration. **Recommend: drop from design.** The link slot holds "Explore a Sample Library" (⌥↵) and, when there is a Pictures folder, "Use Pictures Folder" (⇧↵), both in the frame's accent link style.
3. **Which state the frame replaces.** The frame shows a window shell with 0 counts. The app has two "empty" moments: no library open (welcome) and a library open with 0 items (`grid.rs:870-910`). **Recommend:** the frame replaces the **welcome** screen, with the sidebar dormant because there is no catalog. The in-library empty states keep their copy and actions but adopt this glyph, headline and button styling (Task 4). Their sidebar is the live one.

## App has, design lacks
- **Recent libraries** with open, remove, ↑/↓ focus, ⌫ remove and the "Not available" state. Keep, as a "RECENT LIBRARIES" list (label style as the sidebar section labels) under the hint cards, 520 wide. Returning users depend on it.
- **Sample library and Pictures folder links** with their shortcuts. Keep (decision 2).
- **Import into New Library…** (⇧⌘I). Keep (decision 1).
- **Missing-library notice.** Keep.
- **Keyboard on the welcome:** ↵ open, ⇧⌘I, ⌥↵, ⇧↵, ⌘O, ⌘, (Preferences), Esc, and ⌘K not opening a palette that cannot paint (`tests/welcome.rs:639`). All stay. The dormant sidebar must not take focus or keys.
- **First-scan screen** ("Building your library"). Keep, not in this frame.

## Review Focus
1. **Small window (900×600):** the column scrolls vertically. Hint cards wrap to one per row below ~640 px content width. The glyph and buttons never overlap the header.
2. **Dormant sidebar is inert:** clicks on its rows do nothing (no panic without `grid_state`), each row reads "unavailable" to VoiceOver (role + `aria_disabled`), and Tab skips it. The storage meter is hidden with no library: a "0 GB / 1 TB" figure would be invented.
3. **Sample library preparing:** the link reads "Preparing the Sample Library…" and is disabled. A failure shows the existing banner.
4. **Long recent paths, and 10 recents:** paths truncate in the middle with a tooltip carrying the full path. The list scrolls inside the column.
5. **Keyboard focus visible:** a focused recent row and the buttons show the Foundation focus ring. ↵ with a focused recent opens that recent, not the folder picker, as today (`tests/welcome.rs:321`).

## Tasks
### Task 1: Shell for the no-library state with a dormant sidebar
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/mod.rs:424-453`, Foundation's sidebar (`crates/lumina-ui-gpui/src/views/sidebar/mod.rs`: a dormant variant that needs no `grid_state`) / Test `crates/lumina-ui-gpui/src/tests/welcome.rs`
- [ ] Write failing test `first_run_has_sidebar_and_header`: with no library, `sidebar` width is 252, `page-title` reads "All Media", `page-meta` reads "No media yet", and `page-actions` has no buttons. `sidebar-nav-all`, `sidebar-nav-videos` and `sidebar-nav-favorites` have aria disabled (plus `sidebar-nav-photos` if 00 decision 2 is approved), and clicking `sidebar-nav-all` leaves `grid_state` None.
- [ ] Implement: the no-library root renders the shell (dormant sidebar, `page_header` with no actions, no rail) around the empty-state body. Dormant sidebar: search placeholder at opacity 0.5, the nav rows at 0.45 with count 0 (four with the Photos row, three without), three section hint boxes, no storage meter, no focus targets.
- [ ] Run `cargo test -p lumina-ui-gpui welcome`; expect the new test to pass. Update `welcome_is_a_left_aligned_column_with_two_equal_buttons` in Task 2.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): first-run shell`.

### Task 2: Empty-state body
**Files:** Modify `crates/lumina-ui-gpui/src/views/onboarding.rs:7-196, 379-451`, `crates/lumina-ui-gpui/src/i18n.rs:259-293`, `crates/lumina-ui-gpui/src/icons.rs` (folder-open, folder-sync, file-image, shield-check, aperture if Foundation has not added them) / Test `crates/lumina-ui-gpui/src/tests/welcome.rs:12-78`
- [ ] Rewrite test `welcome_is_a_left_aligned_column_with_two_equal_buttons` as `first_run_empty_state_is_centred_with_folder_and_import_actions`: `welcome-glyph` is 150×150 and horizontally centred in the main column (right of the sidebar, ±1). `welcome-title` reads "Your library is empty". `welcome-open-btn` (40 h) sits left of `welcome-import-new-btn`. Three `welcome-hint-{0..2}` cards have equal widths. `welcome-sample-btn` sits below the buttons.
- [ ] Implement: values from the table. New copy in `i18n.rs` (`welcome.empty_title`, `welcome.empty_body`, `welcome.choose_folder` = "Choose a Folder…", hint keys). Remove the old `welcome.title`, `welcome.intro`, feature and footer keys once unused. The ↵ / ⇧⌘I kbd hints stay inside the buttons. The glow is a radial-gradient canvas or a soft accent shadow, compared by eye against the PNG.
- [ ] Run `cargo test -p lumina-ui-gpui welcome`; expect pass (the key, recents and palette tests unchanged).
- [ ] Gate and commit `feat(design): first-run empty state`.

### Task 3: Recent libraries and notice in the new layout
**Files:** Modify `crates/lumina-ui-gpui/src/views/onboarding.rs:232-375` / Test `crates/lumina-ui-gpui/src/tests/welcome.rs:321-500`
- [ ] Write failing test `recents_list_sits_under_the_hints_and_keeps_keys`: with two recents, `welcome-recent-0` is below `welcome-hint-0`. ↓ ↵ opens recent 1. ⌫ removes the focused row. The missing notice renders above `welcome-glyph`.
- [ ] Implement: 520 px list, section label style, rows restyled to Foundation row tokens (32 h, r8, name t1 13, path mono 10 t3 truncated in the middle).
- [ ] Run `cargo test -p lumina-ui-gpui welcome`; expect pass.
- [ ] Gate and commit `feat(design): recent libraries on first run`.

### Task 4: In-library empty states adopt the style
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/grid.rs:764-1063` (call sites only; 09 Task 1 owns the shell) / Test `crates/lumina-ui-gpui/src/tests/palette_and_empty.rs`. Depends on Design 09 Task 1.
- [ ] Write failing test `empty_library_state_uses_first_run_glyph_and_buttons`: a library with 0 items, after the scan, renders `render_empty_state` in its `Hero` variant: `empty-state-glyph` is 150×150, `empty-state-headline` reads "No photos or videos found", and `empty-primary` is 40 h. A tag view with no items still shows the default (54 px) glyph.
- [ ] Implement: the zero-item library states ("Scanning…", "No photos or videos found", "Your library is empty") pass `Hero` to 09's `render_empty_state`. The first-run body (Task 2) uses the same Hero glyph and button helpers rather than its own copy of them. Copy and actions per state stay as they are. Other states (Favourites, tag, search) keep 09's default size: a 150 px glow for "no favourites" is too loud.
- [ ] Run `cargo test -p lumina-ui-gpui empty`; expect pass.
- [ ] Gate (including `make ui-smoke` screenshot of the welcome and of an empty library) and commit `feat(design): empty-library styling`.

## Decisions (2026-11-13, "go with your recommendations")
1. Connect a camera: dropped. The secondary button stays "Import into New Library…" (⇧⌘I), frame secondary style, `folder` icon.
2. Import from Apple Photos: dropped. The link slot holds "Explore a Sample Library" (⌥↵) and, when there is a Pictures folder, "Use Pictures Folder" (⇧↵), accent link style.
3. The frame replaces the welcome screen (no library open), with a dormant sidebar. In-library zero-item states adopt the Hero glyph and buttons (Task 4).
4. No Photos row: the live sidebar has none (00 D5 kept All Media, Videos, US "Favorites"), so the dormant sidebar shows three rows, in the live sidebar's order: All Media, Favorites, Videos.
5. No timeline rail and no storage meter with no library (no invented "0 GB / 1 TB").
6. Copy per the Copy check section; "Watch folders" hint kept.
7. Hint cards are `flex_1` with a 180 px minimum in a wrapping row, not a forced one-per-row switch: the 960 px minimum window and ≤320 px sidebar keep the content ≥576 px, so all three always fit in one row.
8. Task 3: there is no small missing-library notice any more (Design 10's full unreachable-library state replaced it), so the notice-above-glyph assertion is dropped.
9. Task 4: in a library with zero items, every view that falls back to the library-empty copy (Recently Added, Downloaded, Missing, Untagged as well as All Media) shows it at Hero size: the library has nothing at all, so the big state fits.
