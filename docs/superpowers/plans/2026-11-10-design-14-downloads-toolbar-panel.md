# Design 14: Downloads — Toolbar Panel — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle the toolbar Downloads button and its dropdown into the frame's 420 px popover (header with actions, active rows with progress, "Earlier today", "Show all downloads" footer).
**Design:** `lumina.pen` frame `eO3Hw` "Downloads — Toolbar Panel", popover `RXoOa` (420×550) and "Downloads Button" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts). Design 01 Task 1 owns the toolbar button's place and size in `page_header` (it keeps `downloads-toolbar-btn`). Design 13 provides Pause / Pause all, `download_display_order(DownloadSurface)` and the Downloads screen that "Show all downloads" opens.
**Ownership:** this plan owns the popover rows (the 420 px design). Design 00 drops its 320 px edits to `downloads_panel.rs`. DownloadRow (00) is used by 13's screen, not here.
**Icons (fetched in 00 Task 1):** arrow-down, arrow-right, settings-2. Already in `resources/icons/`: pause, play, x, check, folder, rotate-ccw, maximize-2.
**App today:**
- Button: `crates/lumina-ui-gpui/src/app/render/toolbar.rs:299-357` (`render_downloads_button`: `icon_button`, `ArrowDownToLine` icon, progress ring overlay, accent count badge 14 px, red failure dot 7 px, tooltip "Downloads (⌥⌘L)").
- Dropdown: `crates/lumina-ui-gpui/src/views/downloads_panel.rs:220-356` (backdrop 266-280; card at top 58, right 16, w 320, max_h 60 %, `bg_pane`, `RADIUS_LG` 281-300; sectioned body 235-260; footer links "Clear finished", "Resume all", "Show Downloaded" 301-353). Rows: 85-218.
- Sections and focus: `crates/lumina-ui-gpui/src/app/downloads/panel.rs:12-18, 65-122`. Toggle: ⌥⌘L (`app/keymap/mod.rs:214-218`), palette `ToggleDownloads` (`app/command_palette.rs:1434`), View menu item "Show Downloads" (`menus.rs:49`; its action handler is `app/render/mod.rs:560-562`).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.

## Copy check (flagged)
No string names yt-dlp, ffmpeg, curl, brew or HLS. Sample rows that must not become features or fixed copy:
- "icloud.com · ProRes → HEVC, pass 2/2" (green fill). There are no iCloud sources (AGENTS §5.4) and no re-encoding. The real finishing phase reads "Finishing…" with the positive fill.
- "Family Album — Summer 2026 · 48 photos". A link imports one video.
- "paused at 61/255 MB". Resume starts over (`docs/network-import.md`), so a paused row reads "Paused · resumes from the start" with no bytes.
- "Road Trips 2026 · 512 MB · 4 min ago" / "Unsorted …". There is no album assignment yet (Design 12, decision 5). The meta reads "Added to library · 512 MB · 4 min ago".

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Button | `icon_button`, `ArrowDownToLine`, ring overlay (`toolbar.rs:309-321`) | 34×34, bg-2, r9; 24 px ring (track bg-3, progress accent); `arrow-down` 11 centred | Design 01 Task 1 (place, size, ring); not this plan |
| Count badge | accent pill 14 px top-right (`toolbar.rs:322-341`) | none (the popover header says "3 active") | keep, see App has |
| Failure dot | 7 px danger (`toolbar.rs:342-353`) | none | keep |
| Popover card | top 58, right 16, w 320, max_h 60 %, `bg_pane`, border, `RADIUS_LG`, `shadow_2xl` (`downloads_panel.rs:286-295`) | left 1152 → right 28, top 78, w 420, max h 550, bg `#14141AF5` (bg-1 at 96 %) + 11 px backdrop blur, r14, outline 1 px line-2, shadow 0 18 48 #000000A6 | restyle; opaque bg-1 (GPUI has no backdrop blur; manual check) |
| Header | none | 43.5 h, padding 0 13, bottom border line; "Downloads" Inter Tight 14/600 −0.2; pill 18 h accent-soft, mono 10 accent "3 active"; right: three 25×25 r7 icon buttons `pause`, `settings-2`, `maximize-2` | new |
| Active list | sections "DOWNLOADING", "WAITING", "PAUSED", "FAILED" (`panel.rs:12-18`) | one list, padding 8, gap 4; rows padding 10, gap 9, r9; focused row bg-2 | relayout |
| Active row | title `text_xs` + grey status + 3 px bar (`downloads_panel.rs:122-167`) | thumb 56×34 r6; title 12.5/500 t1; meta mono 10 t3 "host · 148/384 MB · 12.4 MB/s"; track 3 px bg-3 with fill accent (running), positive (finishing), t3 (paused); buttons 25×25 bg-3 r7 `pause`/`x`, paused `play` on accent-soft | restyle + new Pause (Design 13) |
| Failed row | danger text + "Retry" text (139-178) | not drawn | keep in the active list: danger fill, danger meta (the human message, up to 3 lines), `rotate-ccw` 25 px button on danger-soft |
| Waiting row | "Next up" / "Waiting" (57-58) | not drawn | keep in the active list: no fill, meta "Next up" / "Waiting" |
| Divider | none | 1 px line | new |
| Earlier section | "DONE" section with "Added to library" etc. (`panel.rs:17`, `downloads_panel.rs:61-63`) | padding 10/8/8, gap 2; head "EARLIER TODAY" 9.5/600 tracking 0.9 t3 + "Clear" 11 t3; rows 44 h, padding 0 10, gap 10, r8: done badge 18 circle positive-soft `check`, title 12.5/400, meta mono 10 t3, reveal 24×24 r7 `folder` | restyle; label "DONE" → "EARLIER" (Review Focus: today-only is not how the list works) |
| Footer | links "Clear finished", "Resume all", "Show Downloaded" (301-353) | 39.5 h, bg-1, top border; "Show all downloads" 12/500 t2 + `arrow-right` | relayout: Clear → section head; Resume all → header button; Show Downloaded → the Downloads screen's View all (Design 13) |

## Not in the app yet (needs a product decision)
1. **Pause / Pause all in the popover.** Built in Design 13, Task 1. The header `pause` becomes `play` ("Resume all") when nothing runs and some rows are paused. **Recommend: build here** once 13 lands.
2. **Header "settings" button.** It opens Settings › Downloads (Design 15). **Recommend: build** after 15. Until then, hide it.
3. **"Show all downloads" / maximize.** Both open the Design 13 screen and close the popover. **Recommend: build here** (same action twice; the footer is the labelled one).
4. **Row thumbnails.** Same decision as Design 13, decision 3: Done rows use the cache thumbnail, the rest a placeholder. Done rows in this frame show a check badge, not a thumb. Follow the frame.

## App has, design lacks
- Count badge and red failure dot on the button: **keep.** The failure dot is the only signal that something failed while the list was closed (`docs/network-import.md`). The count is cheap and accessible.
- Waiting and failed rows: **keep** (the frame shows only running/paused rows).
- Duplicate ("Already in your library") rows with Download Again ⌘↵: **keep** in Earlier, with the reveal button replaced by `download-again-{id}`.
- Keyboard: ↑/↓, ↵, ⌘↵, ⌫, Esc and the highlight of a re-pasted link: **keep.**

## Review Focus
- `scroll_to_item` counts the body's direct children (`downloads_panel.rs:298-299`). The new header, divider and Earlier head must stay outside that scroll body, or the focus scroll math has to count them. A test must cover ↓ past the visible area.
- Many rows: the card caps at 550 h and only the body scrolls. Header and footer stay put.
- "Earlier today" vs the real list: Done rows stay until cleared, so rows from earlier days must still show. Label the section "EARLIER" (or "DONE"), not "EARLIER TODAY". Times older than a day read as a date.
- Narrow window: the card keeps 420 w but is limited to window width − 32. The toolbar button anchors the card's right edge.
- Popover open + import dialog: pasting a link already listed opens the popover with the row highlighted (existing `submit_network_url:185-190`). That must still work with the new layout.

## Tasks
### Task 1: Count badge and failure dot
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/toolbar.rs:322-353` (or wherever 01 Task 1 moves `render_downloads_button`) / Test `crates/lumina-ui-gpui/src/tests/downloads_panel.rs`
- [ ] Write failing test `downloads_badge_and_dot_sit_on_the_34px_button`: `downloads-badge` shows the unfinished count and its bounds overlap the top-right corner of `downloads-toolbar-btn`. `downloads-failure-dot` appears after a failure while the popover is closed, overlaps the bottom-right corner, and clears when the popover opens.
- [ ] Implement: badge 14 h accent with accent-fg 9/700, dot 7 danger, both repositioned for 01's 34×34 button with Foundation tokens. The button's size, ring and icon are 01's.
- [ ] Run `cargo test -p lumina-ui-gpui downloads_panel`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): downloads badge and failure dot`, staging explicit paths.

### Task 2: Popover shell, header and footer
**Files:** Modify `crates/lumina-ui-gpui/src/views/downloads_panel.rs:220-356` / Test `tests/downloads_panel.rs`
- [ ] Write failing test `downloads_popover_shell`: `downloads-panel` is 420 wide, at most 550 tall, and its right edge is 28 from the window's right edge. `downloads-header` shows "N active" (running + waiting). `downloads-header-pause` pauses all (Design 13 action). `downloads-show-all` closes the popover and selects the Downloads screen. `downloads-clear-finished` lives in the Earlier head.
- [ ] Implement with the table values. Remove the old footer links once each has its new home (Resume all → header `play`, Show Downloaded → screen).
- [ ] Run `cargo test -p lumina-ui-gpui downloads_panel downloads`; expect pass.
- [ ] Gate and commit `feat(design): downloads popover header and footer`.

### Task 3: Rows
**Files:** Modify `downloads_panel.rs:14-260` (render rows from `download_display_order(DownloadSurface::Panel)`, Design 13 Task 5: Active = Running, Paused, Waiting, Failed; Earlier = Done, Duplicate) / Test `tests/downloads_panel.rs`
- [ ] Write failing tests. `popover_groups_active_and_earlier`: row order follows the two groups and equals `download_display_order(DownloadSurface::Panel)`, so ↓ visits rows in visual order. `download-pause-{id}` exists on running rows, `download-resume-{id}` on paused ones, `download-retry-{id}` on failed ones, and `download-reveal-{id}` on Done rows. `popover_keyboard_scrolls_focused_row`: ↓ to the last of 30 rows keeps it inside the body's bounds.
- [ ] Implement the active row (thumb 56×34, 12.5/500 title, mono 10 meta, 3 px track with state colour, 25 px buttons) and the earlier row (44 h, check badge, reveal = `download_main_action`), using the copy fixes above.
- [ ] Run `cargo test -p lumina-ui-gpui downloads_panel`; expect pass.
- [ ] Gate and commit `feat(design): downloads popover rows`.

Manual checks against `eO3Hw.png`: popover translucency (rendered opaque), shadow, ring colours, focused-row bg-2. GPUI headless tests cannot assert colours.

## Decisions (2026-11-16, "go with your recommendations")
1. Pause / Resume all in the popover header: build here, using Design 13's `pause_all_downloads` / requeue. The header button shows `pause` while anything runs, `play` ("Resume all") when nothing runs and some rows are paused, and is hidden otherwise.
2. Header settings button: hidden until Design 15 lands (Design 15 adds it with the Downloads tab).
3. "Show all downloads" footer and the header maximize button: build both; each opens the Downloads screen and closes the popover.
4. Thumbnails: follow the frame. Active rows get a 56×34 thumbnail (cache when catalogued, otherwise a bg-3 placeholder); Earlier rows get the check badge, not a thumbnail.
5. Keep the count badge, failure dot, waiting/failed rows, duplicate rows with Download Again, and the existing keyboard paths. The Earlier section reads "EARLIER"; times older than a day read as a date.
