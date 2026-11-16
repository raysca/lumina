# Design 12: Import from URL — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle the Import Video from URL dialog to the frame's layout (header badge, URL field, link preview card, footer bar) without adding features the importer does not have.
**Design:** `lumina.pen` frame `D119th` "Import from URL" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::kbd`).
**Icons (fetched in 00 Task 1):** link, link-2, folder-down. Already in `resources/icons/`: x, check, film.
**App today:**
- `crates/lumina-ui-gpui/src/views/network_import_dialog.rs:28-268`: dialog, three stages (Input 61-155, Checking 156-181, AlreadyDownloaded 182-235), backdrop and card 238-267.
- `crates/lumina-ui-gpui/src/app/network_import.rs:10-90`: stage enum and open/close.
- `crates/lumina-ui-gpui/src/app/downloads/mod.rs:168-229` `submit_network_url`: checks the URL, looks up earlier downloads, then queues the link. The link is only inspected (title, duration, uploader, size) later, inside the job (`app/downloads/job.rs:72-79`).
- `crates/lumina-ui-gpui/src/views/import_dialog.rs:16-237`: the local file import dialog. It uses the same button closure and card as this one (21-56, 136-160).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- Importing from a link contacts only the site in the URL and the servers that site sends the video from (`docs/network-import.md`). Showing a remote thumbnail would be a new request, so it needs a decision (below).

## Copy check (flagged)
None of the frame's text names yt-dlp, ffmpeg, curl, brew or HLS. Three strings still have to change because they promise more than the importer does (`docs/network-import.md`: one video per link; photos, albums, live streams and WebM are refused):
- Title "Import from URL" → keep **"Import Video from URL"**. That matches File > Import Video from URL… and the palette entry.
- Sub "Paste a link to a video, photo or album — a YouTube video, a page on another site, or a direct file URL." → keep the app's sentence (`network_import_dialog.rs:78`): "Paste a link to a video: a YouTube video, a page on another video site, or a video file."
- Footer hint "Queues in Downloads · ~32s at 12 MB/s" → "Joins your downloads", plus the size when it is known ("Joins your downloads · 384 MB"). There is no honest time estimate before the download has started.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Scrim | `theme::modal_dim()`, `network_import_dialog.rs:243` | full window `#07070BD9` | restyle (use Foundation's scrim token; the hex is not a pen variable, so plan 00 maps it) |
| Card | 520 w, `p_5`, `bg_dialog`, `border_strong`, `RADIUS_LG`, `shadow_2xl` (254-261) | 644 w, bg-1, radius 16, outline 1 px line-2, shadow 0 24 64 #000000B3, no padding (sections pad themselves) | restyle |
| Header row | title only, `text_base` bold (68-73) | padding 24/26/0, gap 9; badge 30×30 accent-soft r9 with `link` icon 16 in accent; title Inter Tight 19/600, −0.3 tracking, t1 | relayout + new badge |
| Close button | none (Cancel and Esc close it) | 28×28, bg-2, r8, outline line, `x` icon 14, t2 | new (`net-import-close`, tooltip "Close (Esc)") |
| Subtitle | `text_xs`, `text_muted` (75-79) | Inter 13/400, line-height 20, t2 | restyle, copy kept (see Copy check) |
| URL field | 34 h, `bg_control`, border `focus_ring`/`danger_border`, `text_xs` (81-128) | 46 h, padding 0 13, gap 10, bg-0, r10, outline 1 px accent; `link-2` icon 15 t3; JetBrains Mono 12.5 t1 | restyle; red outline on error stays |
| Clear (✕) | text "✕", `px_1` (112-127) | 20×20 circle bg-3, `x` icon 11 t2 | restyle |
| Error line | `danger_text`, `text_xs` (129-134) | not in frame | keep, restyle to danger 12 |
| Link preview card | none: the dialog never inspects the link | padding 12, gap 14, bg-2, r12, outline line; thumb 152×86 r8 with duration chip (18 h, `#000000B8`, mono 10.5); title Inter 14/500 lh 19; meta: host mono 11 t2 · uploader 11.5 t2; badges 20 h bg-3 r5 mono 10.5 t2 ("2160p", "MP4 · H.264", "384 MB"); ok badge 24 circle positive-soft with `check` 13 | new, see decisions |
| QUALITY select | none | label 10/600 tracking 0.9 t3; control 38 h bg-0 r9 outline line-2, `gauge` icon, "2160p · MP4 (best available)" | not in app, see decisions |
| SAVE TO select | none | same control, `folder` icon, album name | not in app, see decisions |
| Three checkboxes | none | 16×16 r5 boxes, accent when on; labels 12.5 | not in app, see decisions |
| Footer bar | buttons right-aligned in the card body (135-154) | 61.5 h, padding 0 22, bg-2, top border line; hint left: `folder-down` 13 + mono 10.5 t3 | relayout + hint copy |
| Cancel | 30 h, `bg_control`, `text_xs`, Esc kbd (49-58) | 36 h, padding 0 12, gap 9, bg-3, r9, outline line-2, Inter 13/500 t1; kbd 22×20 bg-0 r5 mono 11 t3 "Esc" | restyle |
| Import | 30 h, accent, bold, "↵" kbd (143-153) | 36 h, accent, r9, Inter 13/600 `#1A1206` (accent fg); kbd bg `#1A120633` mono 11 "⏎" | restyle (keep "↵" glyph, matching the rest of the app) |
| Checking stage | title "Checking Link…" + "Checking host…" (156-181) | not in frame | keep; restyle as the Input layout with the field read-only and a spinner line where the preview card goes |
| Already Downloaded stage | title, date line, Cancel / Download Again ⌘↵ / Show in Library ↵ (182-235) | not in frame | keep; same header and footer shell |

## Not in the app yet (needs a product decision)
1. **Link preview card (title, uploader, duration, size, format).** Size **M**, UI only: `lumina_core::net_import::inspect_url` (`net_import/mod.rs:205`) already returns `title`, `duration_secs`, `uploader`, `expected_bytes` and `kind`. The work: run it in the Checking stage instead of only in the job, show the card, then pass the result to the job so the link is not inspected twice. The job type `JobRequest` (`job.rs:15`) gets an optional `NetworkMediaInfo`. Rows restored after a restart have none and inspect again. The job keeps its own `helper_missing` check (`job.rs:62-64`) for those rows. The cost is that pressing Import no longer closes the dialog straight away: inspecting can take seconds (up to `INSPECT_TIMEOUT`). **Recommend: build in this plan**, behind the existing Checking stage, with Cancel stopping the inspection through its `AtomicBool`.
2. **Thumbnail in the preview.** Size **M**, core work. It means fetching an image URL from the site's metadata, which is a new network request plus decode, and needs a line in `docs/network-import.md`. **Recommend: separate plan.** Until then the thumb slot shows a bg-3 placeholder with a `film` icon and the duration chip.
3. **Format badges ("2160p", "MP4 · H.264").** Resolution and codec are not in `NetworkMediaInfo`. **Recommend:** show only the container (`info.extension`, upper-case) and the size badge. Add the rest with item 4.
4. **QUALITY select.** Size **M**, core work: format selection in `net_import/helper.rs`. Today downloads always prefer the best H.264 version. **Recommend: separate plan,** together with Settings › Preferred video quality (Design 15). Drop it from this dialog for now.
5. **SAVE TO album select.** Size **M**: add the catalogued item to an album after ingest, with error surfacing and undo. **Recommend: separate plan** (it pairs with Design 15 "Add to album"). Drop it for now.
6. **"Download subtitles (English)".** Subtitle files would not be catalogued. Embedding them needs a subtitle encoder, which the copy-only bundled build does not have. **Recommend: drop from design.**
7. **"Keep original audio track".** Downloads already merge the one audio track. There is no multi-track selection. **Recommend: drop from design.**
8. **"Delete source link after import".** The source URL drives the Downloaded album, the "already downloaded" check and Get Info's source (`docs/network-import.md`). Removing it breaks all three. **Recommend: drop from design.**

## App has, design lacks
- Checking and Already Downloaded stages, and the ⌘↵ Download Again path: **keep** (they are documented flows with keyboard paths).
- Inline error line under the field: **keep**.
- Backdrop click to close: **keep** (`network-import-backdrop`).

## Review Focus
- Very long URL: the field stays one line, clipped on the left of the clear button, never wrapping. The value is mono 12.5.
- Very long title or no uploader in the preview: the title clamps to 2 lines. With no uploader, the meta shows only the host, with no dangling "·".
- Inspection fails or times out: back to Input with the human error under the field. Nothing is queued.
- Esc while inspecting cancels the inspection and closes. ↵ while inspecting does nothing (no double submit, as `submit_network_url:170-175` already guards).
- Narrow window (below 700 px): the card shrinks to the window width minus 32 and keeps its 644 max width.

## Tasks
### Task 1: Shared dialog shell
**Files:** Modify `crates/lumina-ui-gpui/src/views/network_import_dialog.rs:28-58,238-267`, `crates/lumina-ui-gpui/src/views/import_dialog.rs:21-56,119-160` / Test `crates/lumina-ui-gpui/src/tests/network_import.rs`
- [ ] Write failing test `net_import_dialog_has_header_close_and_footer`: after opening, `network-import-card` is 644 wide. `net-import-close` exists and clicking it clears `network_import.stage`. `net-import-footer` exists and sits below the URL field.
- [ ] Implement: one small `dialog_footer_button(id, label, kbd, primary)` helper, used by both dialogs in place of their duplicate `button` closures (Cancel bg-3 with line-2 outline, primary accent, 36 h, r9, kbd 22×20 r5). Card: 644 max width, bg-1, r16, line-2 outline. Header: badge 30 accent-soft r9 + title Inter Tight 19/600 + close 28 bg-2 r8. Footer: 61.5 h, bg-2, top border line, padding 0 22. `import_dialog` keeps its 420 width and gets only the button helper.
- [ ] Run `cargo test -p lumina-ui-gpui network_import import`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): import dialogs share the new header and footer`, staging explicit paths.

### Task 2: URL field restyle
**Files:** Modify `network_import_dialog.rs:81-134` / Test `tests/network_import.rs`
- [ ] Write failing test `net_import_url_field_is_46_tall_with_clear_button`: `net-import-url-input` bounds height 46. After typing, `net-import-clear-btn` is 20×20 and clicking it empties the URL.
- [ ] Implement: values from the table (bg-0, r10, accent outline, danger outline on error, `link-2` icon, mono 12.5, clear button 20 circle bg-3). Keep the error line.
- [ ] Run `cargo test -p lumina-ui-gpui network_import`; expect pass.
- [ ] Gate and commit `feat(design): restyle the import URL field`.

### Task 3: Inspect in the dialog and show the preview card (decision 1)
**Files:** Modify `crates/lumina-ui-gpui/src/app/network_import.rs:10-19`, `crates/lumina-ui-gpui/src/app/downloads/mod.rs:168-229`, `crates/lumina-ui-gpui/src/app/downloads/job.rs:15-21,60-80`, `network_import_dialog.rs:156-181` / Test `tests/network_import.rs`, `tests/downloads.rs`
- [ ] Write failing tests. `net_import_shows_preview_before_queueing`: a fake inspector returns title, uploader, 252 s and 384 MB. After submit, the stage is `Ready`, `net-import-preview` shows the title, "04:12" and "384 MB", and nothing is queued. Pressing Import queues one row whose job gets the info and does not inspect again (a counter on the fake). `net_import_cancel_stops_inspection`: Esc during `Checking` sets the cancel flag and queues nothing.
- [ ] Also assert, unchanged: `download_job_reports_a_missing_helper_without_inspecting` (`tests/network_import.rs:235`) still passes, and a new `net_import_dialog_reports_missing_helper_without_inspecting` shows `net_import::MISSING_PART` (`net_import/mod.rs:35`) as the dialog's error with the fake inspector's counter at 0. Pasting a link already in the list still closes the dialog and opens the downloads list with that row highlighted (existing path, `downloads/mod.rs:185-190`), and nothing is inspected.
- [ ] Implement: keep the order in `submit_network_url`: empty/`classify_url` checks, then the "already in your downloads" highlight (`downloads/mod.rs:185-190`), then `helper_missing(kind)` (as `job.rs:62-64` does, off the UI thread; on true, show `MISSING_PART` as the Input error and do not inspect), and only then inspect. Add a `Ready { url, info }` stage. Checking runs the prior-download lookup and `inspect_url` on the background executor, with the same test seam as `JobFn` (an injectable inspector) so no test touches the network. `JobRequest` gets `info: Option<NetworkMediaInfo>`. The card uses the table values, with a placeholder thumb (decision 2), the container and size badges (decision 3), and the green ok badge. The footer hint reads "Joins your downloads · <size>".
- [ ] Run `cargo test -p lumina-ui-gpui network_import downloads`; expect pass.
- [ ] Gate and commit `feat(design): preview a link before importing it`.

### Task 4: Restyle Checking and Already Downloaded
**Files:** Modify `network_import_dialog.rs:156-235` / Test `tests/network_import.rs`
- [ ] Write failing test `already_downloaded_uses_dialog_shell`: in the AlreadyDownloaded stage, `net-import-close` sits in the header (above `net-import-already-downloaded`), `net-import-cancel`, `net-import-download-again` and `net-import-show-in-library` sit inside `net-import-footer`, and ↵ / ⌘↵ still act (keep the existing keyboard assertions).
- [ ] Implement: the same header (title "Already Downloaded" / "Checking Link…"), the body text in Inter 13 t2, and the footer buttons through the Task 1 helper.
- [ ] Run `cargo test -p lumina-ui-gpui network_import`; expect pass.
- [ ] Gate and commit `feat(design): restyle the import dialog's other stages`.

Manual checks against `D119th.png`: scrim colour, accent outline on the field, badge and kbd colours, shadow. GPUI headless tests cannot assert colours.

## Decisions (2026-11-13, "go with your recommendations")
- Copy: keep "Import Video from URL", keep the app's subtitle sentence, footer hint "Joins your downloads" plus " · <size>" when known. No time estimate.
- 1 Link preview card: build in this plan (Task 3), inspecting in the Checking stage; Cancel/Esc stops inspection; the job reuses the info.
- 2 Remote thumbnail: separate plan. Placeholder thumb (bg-3, `film` icon) with the duration chip.
- 3 Format badges: container (extension, upper-case) and size only.
- 4 QUALITY select, 5 SAVE TO album: separate plans (with Design 15). Not in this dialog.
- 6 subtitles, 7 original audio track, 8 delete source link: dropped from the design.
- Keep the Checking and Already Downloaded stages, ⌘↵ Download Again, the inline error line and backdrop click to close.
