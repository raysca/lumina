# Design 10: States — Error — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Align the error surfaces with the design: library unreachable, import partly failed, original offline, playback failed, and kinded toasts. Each says what broke and offers the one recovery action.
**Design:** `lumina.pen` frame `NWldm` "States — Error" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation. Design 09 (States — Empty) provides the shared glyph, headline, body and button shell that this plan reuses for the centred states.
**App today:**
- Toast: `crates/lumina-ui-gpui/src/views/toast.rs:22-60` (state), `:103-150` (show, dismiss, undo) and `:152-264` (render; the file is 265 lines), tested in `src/tests/toasts.rs` and `src/tests/states.rs:11-76`.
- Error banner: `src/app/render/mod.rs:25-105`, with `set_error` and `set_error_with_retry` at `src/app/library.rs:451-470`, tested in `src/tests/states.rs:78-127`.
- Missing library at startup: `src/app/recents.rs:60-67`, `:84-116`, and the welcome notice at `src/views/onboarding.rs:230-252` (copy in `src/i18n.rs:292-295`).
- Missing original: the grid badge at `src/app/render/grid.rs:556-558`, the viewer pill at `src/views/viewer/viewport.rs:412-438`, and Locate… at `src/app/missing.rs:14-60`, tested in `src/tests/missing.rs`.
- Import failures: `src/app/import.rs:259-298` (a toast and then a banner).
- Video load failure: `src/app/video_actions.rs:177` and `:212-215`. **It fails silently**: there is no message, just an empty stage.

**Icons (fetched in 00 Task 1):** unplug (library unreachable), circle-alert (banners, danger toast), circle-check (positive toast), hard-drive (path pill), refresh-cw (Retry). Already in `resources/icons`: triangle-alert, folder, x, info, search.

**Export note:** `NWldm.html` and `l85pQN.html` are byte-identical (191,493 bytes) because each export holds both frames. "States — Error" is lines 1–1036 and "Settings — General" starts at line 1037. The PNGs differ and match their frames. The settings frame is therefore finished, not a copy of this one. The values here come from lines 1–1036.

The frame is a catalogue: four 760×400 panels, each with a 45.5 px ContextBar, plus a row of three toasts. Map each panel to the app surface named in the table.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- AGENTS.md §2: catalog and config write failures go to the error banner and revert their optimistic state. A toast alone is not enough.
- **Off-token colours in this frame map to 00 tokens by name:** danger banner bg #1E100F → `danger_surface`, line #45211E → `danger_border`, text #F0C4C0 → `on_danger`; warn bar #1C1708 / #3A2E1A / #E8D7B0 → `warn_surface` / `warn_border` / `on_warn` (the `warn_*` set); toast bg #16161C and video box #0C0C10 → `bg_float`; accent-fg #1A1206 → `accent_fg`; danger glyph outline (#E8584F at 25–30 %) → `danger_outline`. Never `bg_hud` or `bg_viewer` (deleted by 00).

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| **Library unreachable** | Welcome screen notice: `danger_bg` box, triangle icon, one sentence "Lumina can't find the library “{name}” at {path}…" (`onboarding.rs:230-252`, `i18n.rs:292-295`). Recent rows mark the library unavailable (`recents.rs:103-113`) | Centred state. Glyph 54 circle, `danger_soft` fill, danger outline at 25 %, unplug icon. Headline "Can't reach your library" (Inter Tight 18/600). Path pill: h 30, r 6, px 11, `bg_1`, `line` outline, drive icon, mono 11 t2. Body 12.5/19 t2, width 450. Primary "Retry" (refresh icon), secondary "Choose another library…". Code line: mono 10 t3 "ENOENT · vol-4821 · Show details" | relayout + new actions |
| Unreachable: body copy | as above | "The volume isn't mounted, or another copy of Lumina has it open. Nothing has been changed." | copy: drop the "another copy" clause, because `instance_lock.rs` already keeps a second instance out |
| **Import partly failed: banner** | Toast "Imported N items", then the global banner "{n} files could not be imported: a: err; b: err; c: err and k more" (`import.rs:259-298`) | In-panel banner: h 43.5, px 16, gap 9, `danger_surface` and `danger_border`, alert-circle icon. Text "Imported 1,284 of 1,310. 26 files failed." (12.5/500). "Dismiss" as a text button (11.5 t3) | relayout (decision) |
| Import failed: rows | names inside the banner text | Rows: h 46, r 8, px 12, gap 11, `bg_1`, `line` outline. Kind icon. File name mono 11.5 t1. Reason 10.5 t3 ("Unsupported codec", "File is truncated", "Permission denied"). Per-row "Retry" in accent 11.5. Rows gap 8, list padding 16×24 | new |
| Import failed: more and footer | "and k more" | "+ 23 more" (11 t3). Primary "Retry all 26", secondary "Export report" (gap 9) | new |
| **Original offline: alert bar** | Viewer pill at the bottom: `bg_hud`, triangle icon in danger, "The original file is missing." (`viewport.rs:412-438`) | Warn bar under the ContextBar: h 39.5, px 16, `warn_surface` and `warn_border`, triangle icon in warn. Text 12 "“LaCie 4TB” is disconnected — you're looking at a cached preview." Action "Eject settings" in warn 11.5 | restyle + copy |
| Original offline: body | none beyond the pill; Locate… is only in the context menu and ⌘K (`context_menu.rs:307`, `missing.rs:14`) | Preview 220×150, r 6, `line_2` outline, badge "PREVIEW ONLY · 2048 px" (mono 8.5 warn on black at 80 %). Headline "Original not available". Body "Editing, export and full-size view need the original file. Reconnect the drive, or point Lumina at where the file lives now." Primary "Locate file…", secondary "Keep preview" | new (in the viewer) |
| Grid missing badge | `grid-missing-badge-{id}` (`render/grid.rs:556-558`) | not shown | keep |
| **Playback failed** | Nothing. `load()` failing only finishes the resume point (`video_actions.rs:212-215`) | VideoBox 460×224, r 8, `line_2` outline. Danger glyph 54. Headline "Can't play this clip" (Inter Tight 17/600). Body 12/18 t2. Primary "Create proxy", secondary "Download original". Code line mono 10 t3 "VideoToolbox · kVTCouldNotFindVideoDecoderErr (−12906)" | new (bug: silent failure) |
| **Toast container** | 360 wide, px 10, py 8, `bg_hud`, `border_strong`, `RADIUS_LG`, `shadow_lg` (`toast.rs:196-214`) | 480×62, r 10, `bg_float`, 1 px `line_2` outline, drop-shadow 0 6 12 black at 65 %, padding 0 14, gap 12 | restyle |
| Toast kind | always a Check icon in `success_text` (`toast.rs:215`) | 3 px left accent bar plus icon in the kind colour: danger (alert-circle), warn (triangle), positive (check-circle) | new `ToastKind` |
| Toast text | one message line, `text_xs` (`toast.rs:216-224`) | Title 12.5/500 t1, subtitle 11/400 t3, gap 3 | new subtitle |
| Toast action | "Undo ⌘Z" chip when `undo_available` (`toast.rs:159-192`) | Text action 12/500 in the kind colour: "Retry", "Resume", "View" | generalise |
| Toast dismiss | 26 px bordered × (`toast.rs:226-243`) | none | keep (see below) |
| **Error banner** (global) | `danger_bg`, bottom border, TriangleAlert 16, title `text_sm` semibold plus detail `text_xs`, Retry control, 26 px × (`render/mod.rs:35-105`) | Same anatomy as the import banner: alert-circle 16 danger, text 12.5/500 in `on_danger`, "Dismiss" as a text button (11.5 t3) | restyle |

## Not in the app yet (needs a product decision)
- **Retry for a library at startup.** Re-runs the folder check and opens the library; Choose (`pick_folder_dialog`) already exists. Size S–M. **Required (lead decision): Retry runs off the UI thread.** Today `open_recent_library` runs `is_dir` (`recents.rs:99-104`) and then the synchronous `LuminaEngine::open` (`library.rs:515`) on the UI thread, which can stall on an unmounted network volume. Task 3 moves both to the background executor, with a "Checking…" state on the Retry button, and hands the opened engine back to the UI.
- **Error code line / "Show details".** The app keeps no errno; the missing state comes from `is_dir()` returning false. Recommendation: drop the code line and keep "Show details" only where a real error string exists (the playback and open failures). Size S.
- **Import failure panel (rows, per-file Retry, Retry all, Export report).** The app has `CopyImportReport.failed: Vec<(PathBuf, String)>` (`lumina-core/src/import.rs:63-67`). Friendly reasons need a small error-kind enum in core, because the strings are raw today (S, core). Retry means re-importing those paths with the existing copy import (S–M). Export report means writing a text file through a save dialog (S). Where it lives (the import dialog, a sheet, or the banner expanded) is a decision. Recommendation: separate plan, unless the import screen plan covers it. Here, only restyle the banner text to "Imported X of Y. N files failed."
- **Drive name in the offline bar.** A path under `/Volumes/<name>` whose volume is not mounted can say "“<name>” is disconnected". That is a UI-only string check. Size S. Recommendation: build it.
- **"Eject settings".** Unclear: it is neither a Lumina action nor a system pane the app opens. Recommendation: drop it from the design.
- **"Keep preview".** It closes the card and leaves the cached image. Size S. Recommendation: build it as a dismiss for the session.
- **"Create proxy" for clips that won't play.** This is a transcode. The bundled ffmpeg is approved only for URL import (AGENTS.md §5.4), and a proxy cache is new storage. Size L. Recommendation: separate plan with product approval. Until then, use primary "Try Again" and secondary "Reveal in Finder" (⇧⌘R exists, `app/keymap/mod.rs:249`).
- **"Download original".** This is a cloud-library concept and is out of scope (AGENTS.md §5.4). Recommendation: drop it.
- **Decoder code line.** `player.load` returns only "Failed to load video in AVFoundation" (`player/macos.rs:133`); it has no OSStatus. Recommendation: show the error string under "Show details" and pass the status through the bridge later if wanted (M, platform code).
- **Toast "Drive ejected mid-import · Resume".** Import can only be cancelled; there is no resumable import. Size M, core. Recommendation: separate plan. Until then a warn toast "Import stopped · N of M copied" can offer "View".
- **Error toasts such as "Couldn't delete 3 items · Retry".** AGENTS.md §2 requires write failures in the banner, and an 8 s toast could hide one. Recommendation: keep failures in the banner. Use the danger toast only for transient failures that change no data, or have AGENTS.md updated first.

## App has, design lacks
- The toast's dismiss button and its Esc dismissal (`src/tests/toasts.rs:255-336`). Keep both: they are the keyboard and pointer path to close it. Restyle the button as a 20 px ghost × on the right.
- The Undo ⌘Z action and hover-to-hold. Keep both: Undo becomes the positive toast's action.
- The error banner's ⌘K "Retry Failed Action" path (tooltip at `render/mod.rs:78`). Keep it.
- The grid missing badge and the Missing sidebar view (`views/sidebar/nav.rs:100`). Keep both.
- **Bug, outside the design:** a failed duplicate merge shows a success-style toast with a check icon (`app/library.rs:1174`, `:1213`). Route it through `set_error`, or at least the danger kind, in Task 4.

## Review Focus
- Several failures at once (import errors plus a delete error): one banner. The newest wins and the import detail stays reachable, so it is not silently replaced.
- A very long path in the unreachable pill or a long drive name: truncate in the middle (mono) at `max_w 100%` of the 450 column. Never wrap the buttons off screen.
- Keyboard: Esc dismisses the toast first (`app/keymap/modals.rs:538`), then the banner (`app/keymap/mod.rs:89`). The new centred-state buttons are reachable with Tab and ⌘K (Retry Failed Action, Locate…, Open Library…).
- A failed video load in PiP (`views/floating_pip.rs`): show a compact version of the message, never an empty black window.
- 10,000 failed imports: the list is virtualised or capped ("+ N more") and the banner count is exact.

## Tasks
### Task 1: Error banner restyle
**Files:** Modify `src/app/render/mod.rs:25-105` / Test `src/tests/states.rs:78-127`
- [ ] Extend `error_banner_splits_title_retries_and_esc_dismisses`: `error-banner-dismiss` shows the text "Dismiss" and still dismisses. The banner height is 43.5 ±1 px when there is no detail. All existing selectors stay.
- [ ] Implement: alert-circle icon, 12.5/500 text, a text-button Dismiss with tooltip "Dismiss (Esc)", `danger_surface` fill, `danger_border` line and `on_danger` text.
- [ ] Run `cargo test -p lumina-ui-gpui error_banner`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): error banner restyle`, staging explicit paths.

### Task 2: Kinded toasts
**Files:** Modify `src/views/toast.rs` / Test `src/tests/toasts.rs`, `src/tests/states.rs:11-76`
- [ ] Write a failing test `toast_kinds_show_accent_title_subtitle_and_action`. `show_toast_kind(Positive, "Imported 3 items", Some("2.1 MB"), Some(View))` renders `toast-accent`, `toast-message`, `toast-subtitle` and `toast-action` ("View"). The toast is 480 wide and 62 tall. A replaced toast keeps the hover hold.
- [ ] Implement: a `ToastKind { Positive, Warning, Danger }` field, `subtitle: Option<String>`, and a `ToastAction` enum that replaces `undo_available`, each naming its handler: `Undo` → `undo_from_toast` (`toast.rs:123`); `View(AlbumSelection)` → `select_album(selection, cx)` (import success uses `AlbumSelection::AllLibrary`, or Recently Added once that plan lands); `Retry(ErrorRetry)` → runs the same `fn(&mut LuminaApp, &mut Context)` type the banner uses (`app/library.rs:51`), then dismisses the toast. Each action has a selector `toast-action` and a ⌘K path only where the banner already has one (Retry Failed Action). Keep `show_toast` and `show_undo_toast` as Positive wrappers so the call sites don't change.
- [ ] Update `toast_sits_centred_over_the_grid_and_waits_while_hovered`: the dismiss button is now 20 px, not 26 px. Say in the commit that this changes the test contract.
- [ ] Run `cargo test -p lumina-ui-gpui toast`; expect pass.
- [ ] Gate and commit `feat(design): toast kinds`.

### Task 3: Library unreachable state
**Files:** Modify `src/views/onboarding.rs:230-252`, `src/app/recents.rs:84-116`, `src/app/library.rs:503-530` (split the open into a background part and a UI-thread part), `src/i18n.rs:292-295` / Test `src/tests/welcome.rs`
- [ ] Write a failing test `missing_library_state_retries_and_chooses`. With a missing `last_library_path`, the screen shows `missing-library-headline`, `missing-library-path` (the path text) and `missing-library-retry`. Retry after creating the folder opens the library once the executor runs (`run_until_parked`), and while it runs `missing-library-retry` shows the checking state and the UI thread is not blocked (assert the state before parking). `missing-library-choose` counts one folder pick (the `NEW_LIBRARY_PICKS`-style counter). `welcome-missing-library` keeps `Role::Alert`.
- [ ] Implement: the centred state using Design 09's shell plus the path pill, with the copy from the table (no "another copy" clause). Tab reaches the buttons Retry runs `is_dir` and `LuminaEngine::open` on `cx.background_executor()` and applies the result on the UI thread; a failure keeps the state and shows the reason under "Show details". Keep the test-only `open_with_paths` branch so tests never touch real config.
- [ ] Run `cargo test -p lumina-ui-gpui missing_library`; expect pass.
- [ ] Gate and commit `feat(design): library unreachable state`.

### Task 4: Import result banner and merge-failure routing
**Files:** Modify `src/app/import.rs:259-298`, `src/app/library.rs:1174`, `:1213` / Test `src/tests/import.rs`, `src/tests/duplicates.rs`
- [ ] Write a failing test `partial_import_banner_reads_imported_x_of_y`. With one failed path, the banner title is "Imported 2 of 3. 1 file failed." and the first failed name appears in the detail.
- [ ] Write a failing test `failed_merge_uses_the_error_banner`. Force the error with a removed item id: `error_message` is set and the toast is not Positive.
- [ ] Implement both. The failed-file panel stays out of scope (decision).
- [ ] Run `cargo test -p lumina-ui-gpui import duplicates`; expect pass.
- [ ] Gate and commit `fix(design): import result copy and merge failure banner`.

### Task 5: Original offline in the viewer
**Files:** Modify `src/views/viewer/viewport.rs:412-438`, `src/app/missing.rs` / Test `src/tests/missing.rs:74-90`
- [ ] Extend `viewer_says_the_original_is_missing`. `viewer-missing-bar` shows "you're looking at a cached preview". `viewer-missing-locate` calls `locate_missing_selected` (test-thread counter). `viewer-missing-keep` hides the card but keeps the bar. For a path under `/Volumes/X` that is not mounted, the bar names “X”.
- [ ] Implement: the warn bar at the top of the stage, and a card with the headline, body, "Locate file…" and "Keep preview". The cached image stays visible. Keep the ⌘K Locate path.
- [ ] Run `cargo test -p lumina-ui-gpui missing`; expect pass.
- [ ] Gate and commit `feat(design): original offline state`.

### Task 6: Playback failed
**Files:** Modify `src/app/video_actions.rs:165-216`, `src/views/viewer/mod.rs` / Test `src/tests/video_player.rs`
- [ ] Write a failing regression test `video_load_failure_says_so`. Make the test player fail the load: check how `player/dummy.rs` behaves and add a failing path if it has none. `viewer-playback-failed` renders with the headline "Can't play this clip". `playback-retry` reloads, and `playback-reveal` reveals the file (test counter). The test must fail before the fix, because the failure is silent today.
- [ ] Implement: store the load error in the video state (`video.load_error: Option<String>`) and render the VideoBox with Design 09's shell (VideoBox 460×224, headline 17/600, body 12/18). Put the error string behind "Show details" (mono 10 t3). Clear it on the next item.
- [ ] Run `cargo test -p lumina-ui-gpui video_load_failure`; expect pass.
- [ ] Gate and commit `fix(design): show playback failures`.

Manual visual checks: the banner and warn bar colours, the toast shadow and the 3 px accent bar, and the danger glyph alpha. Compare them against `NWldm.png`.

## Decisions (2026-11-13, "go with your recommendations")
- Retry for an unreachable library runs `is_dir` and `LuminaEngine::open` off the UI thread with a "Checking…" state (Task 3).
- No errno code line; "Show details" only where a real error string exists (open and playback failures).
- Import failure panel (rows, per-file Retry, Retry all, Export report) is a separate plan; here only the banner text "Imported X of Y. N files failed."
- Offline bar names the drive for unmounted `/Volumes/<name>` paths; "Eject settings" dropped; "Keep preview" built as a session dismiss.
- Playback failed: primary "Try Again", secondary "Reveal in Finder"; "Create proxy" and "Download original" dropped.
- No resumable import toast; write failures stay in the banner (AGENTS.md §2); danger toasts only for transient failures that change no data.
- Toast keeps its dismiss button (20 px ghost ×) and Esc; Undo becomes the positive toast's action.
- Failed duplicate merge goes through `set_error` (Task 4).
