# Design 13: Downloads — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Add a full Downloads screen in the main area (summary, filter chips, Active / Needs attention / Queued / Completed sections) on top of the existing download queue, plus the Pause action the frame assumes.
**Design:** `lumina.pen` frame `ttuOX` "Downloads" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::chip` = Chip `aisTo`, the **DownloadRow** widget = `agucN`, the Sidebar). Design 01 Task 1 (`page_header`, which this screen's top bar renders through). Design 15 decides the concurrency setting shown in the filter bar.
**Icons (fetched in 00 Task 1):** layers, list-ordered, check-check, settings-2, arrow-up, ellipsis, folder-open. Already in `resources/icons/`: arrow-down-to-line, pause, play, triangle-alert, check, rotate-ccw, x, film. No grip icon (decision 6 drops drag).
**App today:**
- No Downloads screen. The list exists only as a dropdown under the toolbar button: `crates/lumina-ui-gpui/src/views/downloads_panel.rs:220-356` (rows 85-218, status text 29-65).
- Sections: `crates/lumina-ui-gpui/src/app/downloads/panel.rs:12-18` (Downloading, Waiting, Paused, Failed, Done). Combined progress: `panel.rs:48-62`. Row main action: `panel.rs:124-142`. Clear finished: `panel.rs:164`.
- Queue: `crates/lumina-ui-gpui/src/app/downloads/mod.rs` (`MAX_RUNNING = 3` at :25, `pump_downloads` :315, cancel/remove :529, retry :570). Resume and Resume all: `crates/lumina-ui-gpui/src/app/downloads/lifecycle.rs:125-149`.
- Core rows: `crates/lumina-core/src/catalog/downloads.rs:10-66` (`DownloadState`, `DownloadRow`). The order is `added_epoch_ms, id` (:196).
- Main-area routing: `crates/lumina-ui-gpui/src/app/render/mod.rs:727-737` (Duplicates / Map / grid by `AlbumSelection`). Selection enum: `crates/lumina-ui-gpui/src/views/sidebar/mod.rs:13-24`.
- Sidebar: the "Downloaded" album (`views/sidebar/nav.rs:62-71`) appears only once something was downloaded. There is no "Downloads" item.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- Every catalog write stays off the UI thread and surfaces failures in the error banner (as `app/downloads/mod.rs` does today).

## Copy check (flagged)
No string names yt-dlp, ffmpeg, curl, brew or HLS. Some sample rows show things Lumina must not imply, so none of them may become fixed copy or a feature:
- `icloud.com` sources and "Family Album — Summer 2026 (48 photos) · JPEG + Live Photos". iCloud accounts are out of scope (AGENTS §5.4), and a link imports one video.
- "Converting · 62%" / "transcoding ProRes → HEVC, pass 2 of 2" / badge "ProRes → HEVC". The bundled tools only remux; they never re-encode (`docs/network-import.md`). The app's real last phase is `DownloadPhase::Finalizing`, shown as "Finishing…". That keeps the green status colour and the copy "Finishing…".
- "Failed · HTTP 403 Forbidden". Show the row's human message (`row.error`, for example "This video is private."), never a protocol code.
- "2 attempts". Attempts are not counted. Show "last tried HH:MM" from `finished_epoch_ms` only.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Route | dropdown only (`downloads_panel.rs:220`) | main area replaces the grid; sidebar "Downloads" selected, count = unfinished | new (`AlbumSelection::Downloads`, routed at `render/mod.rs:727`) |
| Top bar | n/a (01's `page_header`) | 67.5 h, padding 0 28, bottom border line; "Downloads" Inter Tight 20/600 −0.3; meta mono 10.5 t3 "3 downloading · 1 paused · 2 queued · 24.8 MB/s" | new, through `page_header` (`page-title` "Downloads", `page-meta`, buttons in `page-actions`); counts and combined speed from entries |
| Pause all | n/a (no pause exists) | 32 h, padding 0 12, gap 7, bg-2, r8, outline line, `pause` 14 + 12.5 t1 | new; becomes "Resume all" when nothing runs and some are paused (existing `resume_all_downloads`) |
| Clear completed | "Clear finished" link in the dropdown footer (`downloads_panel.rs:313-322`) | same button style, `check-check` icon, "Clear completed" | new button, existing action `clear_finished_downloads` |
| Add URL | n/a (⌥⌘U, File menu, palette) | 32 h, padding 0 13, accent, `link` icon, 12.5/600 accent-fg | new, calls `open_network_import_dialog` |
| Filter chips | n/a | bar 51.5 h, padding 0 28, bottom border; Chip 28 h, padding 0 11, gap 6; selected accent-soft + accent outline + t1, others bg-2 + line outline + t2; icons layers / arrow-down-to-line / list-ordered / pause / triangle-alert / check; "All 6", "Downloading 3", "Queued 2", "Paused 1", "Failed 1", "Completed 18" | new, Foundation Chip |
| Limits + settings | n/a | mono 10.5 t3 "3 concurrent · 25 MB/s cap"; 28×28 bg-2 r8 `settings-2` | new; text from real settings (Design 15). Without a speed cap it reads "3 at a time". The button opens Settings › Downloads |
| Status bar | toolbar ring only (`panel.rs:48-62`) | 70 h, padding 0 18, bg-1, r12, outline line; live dot 7 accent; headline Inter Tight 15/600 "Downloading 3 of 6 items"; detail mono 11 t2 "1.10 GB of 2.84 GB · 24.8 MB/s combined · about 1 min 12 s remaining"; right: throughput bars (20 bars, 3 w, gap 3, max 34 h, accent / accent 40 %), 1×30 divider, "41%" Inter Tight 21/600 + "OVERALL" 9.5/600 t3 | new; hidden when nothing runs |
| Section head | uppercase `section_label` (`downloads_panel.rs:246-251`) | label 10/600 tracking 0.9 t3 + count mono 10 t3; right action 11.5 t2 | restyle |
| Sections | Downloading / Waiting / Paused / Failed / Done (`panel.rs:12-18`) | ACTIVE (running) · NEEDS ATTENTION (paused + failed) · QUEUED (waiting) · COMPLETED TODAY | relayout (the screen gets its own grouping; the dropdown keeps `SECTIONS` unless Design 14 changes it) |
| Section actions | Resume all link (`downloads_panel.rs:323-335`) | ACTIVE "Pause all", NEEDS ATTENTION "Retry all", QUEUED "Clear queue", COMPLETED "View all" | new actions (Retry all, Clear queue); View all opens the Downloaded album (existing `downloads-show-album` behaviour) |
| Active / attention rows | 1-line title, grey status, 3 px bar, text buttons (`downloads_panel.rs:85-218`) | **DownloadRow** (Design 00): thumb 72×46, title 13.5/500, status mono 11 right of title, track 4 px, meta mono 11 t3, format badge, 30×30 buttons | new on this screen via DownloadRow |
| Row status colours | failure text `danger_text` only (139-143) | running accent, finishing positive, paused t2 with t3 fill, failed danger with danger fill and a 1 px danger outline on the card | restyle (passed to DownloadRow) |
| Row buttons | "Retry", "Resume", "Download Again", "✕" text (169-216) | pause / play (accent-soft) / Retry (danger-soft, `rotate-ccw` + "Retry" 12/500) / x | restyle + new Pause |
| Queued rows | same row style | 52 h, padding 0 16, gap 13, bg-1, r10, outline line; grip 13 (dropped, decision 6) + position mono 11 t3; thumb 54×34 r6; title 13/500 **t2**; meta mono 10.5 t3 "host · size · starts after <title>"; 28×28 `arrow-up` and `x` | new |
| Completed rows | "Added to library" / "Removed from library" / "Already in your library" (57-63) | 52 h; done badge 20 circle positive-soft `check` 11; thumb 54×34; title 13/500 t1; meta "Added to Road Trips 2026 · 512 MB · 4 min ago"; 28×28 `folder-open` and `ellipsis` | new; meta "Added to library · <size> · <ago>"; the Removed and Already variants keep their text |

## Not in the app yet (needs a product decision)
1. **Downloads screen + sidebar item.** Size **M**, UI only. **Build in this plan.** Foundation (00 Task 5) adds the sidebar "Downloads" row (`sidebar-nav-downloads`, count of unfinished downloads). This plan makes that row open the screen and show as active while it is open (Task 2). "Downloaded" stays as the album that "View all" opens.
2. **Pause (per row and Pause all).** Size **S–M**. No core schema change: `Paused` already exists. Pause cancels the job (the existing cancel flag) and stores `Paused` instead of removing the row. Resume starts over, as documented (no byte-range continue). **Recommend: build in this plan** (Design 14 uses it too). Docs: update "The download list" in `docs/network-import.md`.
3. **Thumbnails on rows.** Done rows have `media_id`, so they can use the existing thumbnail cache (**S**, build it). Running, waiting and failed rows have no image. A remote thumbnail is a new network request (see Design 12, decision 2). **Recommend:** a placeholder (bg-3 with a `film` icon) until that separate plan lands.
4. **Format badge ("2160p MP4").** Not known today. **Recommend:** show the container from the file extension when known (Done rows: the media item; others: hide the badge). Add resolution with the quality plan.
5. **Throughput bars.** A ring buffer of the last 20 combined-speed samples (one per second), in memory. **S. Recommend: build** (no IO). Drop it if review finds it noisy.
6. **Move up / drag to reorder queued rows.** Move up: **S–M**, no migration. Swap `added_epoch_ms` with the previous waiting row in one transaction (that column is the queue order, `catalog/downloads.rs:196`). Drag by grip: **M**. **Recommend:** build Move up with ⌥⌘↑ and drop drag and the grip glyph; the row keeps only its position number.
7. **"Completed today" vs all.** The app keeps Done rows until Clear finished. **Recommend:** the section shows every Done/Duplicate row, labelled "COMPLETED", newest first. The "Completed" chip filters to it. "View all" opens the Downloaded album.
8. **Completed row "…" menu.** **Recommend:** reuse the context-menu overlay with "Show in Library", "Download Again" (Duplicate rows) and "Remove from List". `folder-open` = Show in Library (`download_main_action`), not Finder.

## App has, design lacks
- Keyboard list navigation (↑/↓, ↵ main action, ⌘↵ Download Again, ⌫ remove, Esc): **keep and extend to the screen.** ↵ gains two cases (lead decision): on a running row it pauses, and on a paused row it resumes (already `download_main_action`, `panel.rs:130`).
- "Already in your downloads" highlight when a pasted link is already listed (`downloads_panel.rs:30-32`): **keep.** On the screen, scroll the row into view and highlight it.
- The "Downloads are still running" quit / library-switch prompt (`views/downloads_confirm.rs:10-122`, card 400 w at :66): not in any frame. **Keep.** Restyle it with Design 12's dialog shell (header, footer buttons) in Task 6.
- Wrapping failure reasons (up to 4 lines, `downloads_panel.rs:144-151`): **keep.** DownloadRow's meta line wraps for failed rows.

## Review Focus
- Empty queue: the screen shows an empty state ("No downloads yet", Add URL button). The sidebar row hides once the queue is empty and the screen is not selected. Selecting it from the palette with no rows still works.
- Unknown size or speed: the status bar drops the "of" and "remaining" parts rather than showing 0 B. A row with no total shows bytes only and an indeterminate track.
- 200+ completed rows: the body is one scroll view with stable keyboard focus. No per-frame catalog queries, and the counts come from `downloads.entries`.
- Long titles: one line with an ellipsis. The status text and right column never shrink (right column fixed at 176).
- Narrow main area (below 900 px): chips scroll horizontally. The Limits text hides before the chips do.

## Tasks
### Task 1: Pause and Pause all (core of the screen)
**Files:** Modify `crates/lumina-ui-gpui/src/app/downloads/mod.rs:529-640`, `crates/lumina-ui-gpui/src/app/downloads/lifecycle.rs:120-149`, `crates/lumina-ui-gpui/src/app/downloads/panel.rs:124-142`, `crates/lumina-ui-gpui/src/app/command_palette.rs:115-116,281-290,1040,1434`, `docs/network-import.md` / Test `crates/lumina-ui-gpui/src/tests/downloads.rs`
- [ ] Write failing tests. `pause_download_stops_job_and_keeps_row`: with a blocking fake job, pausing sets the cancel flag. The row becomes `Paused` in memory and in `list_downloads`, and the next waiting row starts. `pause_all_then_resume_all_round_trips`: every running and waiting row ends `Paused`, then all requeue in list order.
- [ ] Also write `enter_pauses_running_and_resumes_paused`: with a running row focused, ↵ pauses it; ↵ again requeues it. And `download_palette_entries`: "Pause All Downloads", "Resume All Downloads", "Retry Failed Downloads" and "Clear Finished Downloads" each act, and each is hidden when it has nothing to act on.
- [ ] Implement `pause_download(row_id)` and `pause_all_downloads()`; `download_main_action` (`panel.rs:124-142`) pauses a `Running` row. Add four `CommandId`s next to `ToggleDownloads` (`command_palette.rs:115-116`, entries near :281-290, availability near :1040, dispatch near :1434): Pause All Downloads, Resume All Downloads (`resume_all_downloads`), Retry Failed Downloads (`retry_all_downloads`, Task 5; land the palette entry with Task 5 if Task 1 ships first) and Clear Finished Downloads (`clear_finished_downloads`). A cancelled job whose row was marked paused finishes as Paused, not removed. A catalog write failure reverts the state and shows the error banner.
- [ ] Run `cargo test -p lumina-ui-gpui downloads`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(downloads): pause a download or all of them`, staging explicit paths.

### Task 2: Route, sidebar row and top bar
**Files:** Modify `crates/lumina-ui-gpui/src/views/sidebar/mod.rs:13-24`, `crates/lumina-ui-gpui/src/views/sidebar/nav.rs` (the Foundation `downloads` row's selection), `crates/lumina-ui-gpui/src/app/render/mod.rs:727-737`, `crates/lumina-ui-gpui/src/app/command_palette.rs:115-116,281-290,1040,1434`, `crates/lumina-ui-gpui/src/menus.rs:49` (add "Show All Downloads" after "Show Downloads"), `crates/lumina-ui-gpui/src/config.rs:19-60,626` (`LastView::Downloads`, token `downloads`, mapped in `restored_session`); Create `crates/lumina-ui-gpui/src/views/downloads_screen.rs` / Test `crates/lumina-ui-gpui/src/tests/downloads_screen.rs`
- [ ] Write failing test `downloads_screen_replaces_grid`: selecting `AlbumSelection::Downloads` shows `downloads-screen` and no grid. `downloads-pause-all`, `downloads-clear-completed` and `downloads-add-url` exist. Clicking Add URL opens the import dialog. The meta reads "2 downloading · 1 queued" for a fixture with 2 running and 1 waiting. The palette command "Show All Downloads" selects the screen. `downloads_sidebar_row_opens_screen`: clicking `sidebar-nav-downloads` shows `downloads-screen`, and the row is the active one while it shows. `last_view_restores_downloads`: `LastView::from_token("downloads")` round-trips, and `restored_session` maps it to `AlbumSelection::Downloads` (falling back to All when the queue is empty).
- [ ] Implement the top bar through `page_header` with the table values. Add the palette entry and the View menu item "Show All Downloads" (`menus.rs:49`). Map the screen to `LastView::Downloads`. Keep ⌥⌘L for the popover (Design 14).
- [ ] Run `cargo test -p lumina-ui-gpui downloads_screen`; expect pass.
- [ ] Gate and commit `feat(design): Downloads screen and top bar`.

### Task 3: Filter chips and limits
**Files:** Modify `views/downloads_screen.rs`, `crates/lumina-ui-gpui/src/app/downloads/panel.rs` (add `DownloadFilter`) / Test `tests/downloads_screen.rs`
- [ ] Write failing test `download_filter_chips_count_and_filter`: chips `downloads-filter-{all,downloading,queued,paused,failed,completed}` show the counts. Clicking Failed leaves only failed rows. Keyboard: with the screen focused, ←/→ move between chips.
- [ ] Implement with Foundation `theme::chip`. The limits text comes from the live concurrency (and the speed cap once Design 15 adds it). The settings button opens Settings › Downloads.
- [ ] Run `cargo test -p lumina-ui-gpui downloads_screen`; expect pass.
- [ ] Gate and commit `feat(design): filter downloads by state`.

### Task 4: Status bar
**Files:** Modify `views/downloads_screen.rs`, `crates/lumina-ui-gpui/src/app/downloads/panel.rs:48-62` (reuse `download_progress`; add a 20-sample speed history to `DownloadQueue` in `mod.rs:67-91`) / Test `tests/downloads_screen.rs`
- [ ] Write failing test `downloads_status_bar_sums_running_rows`: two running rows at 100/400 MB and 50/100 MB show "30%" in `downloads-overall` and "150 MB of 500 MB" in the detail. With no running rows there is no `downloads-status-bar`.
- [ ] Implement the headline, detail, throughput bars and overall %, with the values from the table.
- [ ] Run, gate, commit `feat(design): downloads summary bar`.

### Task 5: Sections and rows
**Files:** Modify `views/downloads_screen.rs`, `crates/lumina-ui-gpui/src/app/downloads/panel.rs:11-46` (`SECTIONS` → `download_sections(surface)` / `download_display_order(surface: DownloadSurface)`: `Panel` = two groups, Active and Earlier, consumed by Design 14; `Screen` = the four sections; `move_download_focus` takes the surface that is showing), `crates/lumina-ui-gpui/src/app/downloads/mod.rs` (add `move_download_up`, `retry_all_downloads`, `clear_queue`), `crates/lumina-core/src/catalog/downloads.rs` (add `swap_download_order(a, b)` in a transaction) / Test `tests/downloads_screen.rs`, `crates/lumina-core/tests/core_tests.rs`
- [ ] Write failing tests. `downloads_screen_groups_rows`: sections `downloads-section-{active,attention,queued,completed}` hold the right rows. Running rows show `download-pause-{id}` (the same selector as Design 14), paused rows `download-resume-{id}`, failed rows `download-retry-{id}`. `screen_keyboard_order_matches_visual_order`: ↓ from the top visits rows in `download_display_order(DownloadSurface::Screen)` order, which equals the on-screen order. `move_queued_download_up_persists`: after Move up, `list_downloads` returns the swapped order (core test for `swap_download_order`). `retry_all_and_clear_queue`: Retry all requeues only `can_retry` failures. Clear queue removes only waiting rows.
- [ ] Implement: Active and Attention rows use Foundation **DownloadRow** with status, colours and buttons per the table (Pause, Resume, Retry, ✕). Queued and Completed rows are the compact 52 h rows. Done rows use thumbnails from the cache. Section actions. ↑/↓/↵/⌘↵/⌫/Esc work on the screen through `move_download_focus` (with `DownloadSurface::Screen`) / `download_main_action`, and ⌥⌘↑ moves a queued row up.
- [ ] Run `cargo test -p lumina-ui-gpui downloads` and `cargo test -p lumina-core downloads`; expect pass.
- [ ] Gate and commit `feat(design): downloads screen sections and rows`.

### Task 6: Restyle the running-downloads prompt
**Files:** Modify `crates/lumina-ui-gpui/src/views/downloads_confirm.rs:10-122` / Test `tests/downloads.rs`
- [ ] Write failing test `downloads_confirm_buttons_in_footer`: the quit prompt's buttons sit inside `downloads-confirm-footer`. The existing keyboard tests keep passing.
- [ ] Implement with Design 12's dialog footer helper and Foundation tokens.
- [ ] Run, gate, commit `feat(design): restyle the running-downloads prompt`.

Manual checks against `ttuOX.png`: chip selected colour, failed row outline, status colours, throughput bar opacity. GPUI headless tests cannot assert colours.

## Decisions (2026-11-14, "go with your recommendations")
- Copy: no iCloud sources, no "Converting/transcoding" (the last phase is "Finishing…" in positive), failures show the row's human message (never a protocol code), "last tried HH:MM" instead of attempt counts.
- 1 Downloads screen + sidebar row: build here; "Downloaded" stays the album "View all" opens.
- 2 Pause / Pause all: build here (cancel the job, store `Paused`; Resume starts over). Update docs/network-import.md.
- 3 Thumbnails: Done rows from the cache; others a bg-3 `film` placeholder (no remote image).
- 4 Format badge: container from the extension when known, else hidden.
- 5 Throughput bars: 20-sample in-memory ring buffer; drop if review finds it noisy.
- 6 Move up (⌥⌘↑, swap `added_epoch_ms` in one transaction); no drag, no grip glyph.
- 7 Section "COMPLETED" shows every Done/Duplicate row, newest first; "View all" opens the Downloaded album.
- 8 Completed row "…" menu reuses the context-menu overlay: Show in Library, Download Again (Duplicate rows), Remove from List. `folder-open` = Show in Library.
- Keep keyboard list navigation (↵ pauses running, resumes paused), the "already in your downloads" highlight, wrapping failure reasons, and restyle the running-downloads prompt (Task 6).
