# Design 04: Videos + PiP — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Make the Videos view show 16:9 captioned clip tiles under a video-specific header and
filter row, and restyle Picture-in-Picture to the design's player card.
**Design:** `lumina.pen` frame `DoXZy` "Videos + PiP" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::chip`, `theme::nav_item`, Sidebar); Design 01 Task 1 (`page_header`).
**Icons (fetched in 00 Task 1):** `audio-lines`. Already in `resources/icons/`: play, pause, skip-back, skip-forward, volume-2, volume-x, maximize-2, x, picture-in-picture-2.
It also depends on the Library frame plan (`t0zQL`). That plan owns the shared main-area top bar
(title + mono meta line, Mosaic/Grid/Map segmented control, filter and sort buttons, Import),
the filter-chip row with count and "Clear all", and the section header. This plan only
specialises them for Videos.
**App today:**
- Videos view: sidebar entry `views/sidebar/nav.rs:61-64` (`SystemAlbum::Videos`). It renders
  the shared grid.
- Toolbar title: `app/render/toolbar.rs:91-260` (title `toolbar-title` at `:164-174`, 15/600, no
  meta line).
- Grid cards: `app/render/grid.rs:219-633` (square well `photo_side`, no caption, filename only
  in the tooltip). Video duration badge at `:583-608`: bottom-right, play icon 9 + mono 11, black
  60%. Selection ring at `:613-632` (2 px accent, 2 px gap).
- Grid metrics: `views/grid.rs:92-116` (`row_height = photo_side + padding + gap`, square only).
- Month band: `app/render/grid.rs:674-712` (`grid-month-header`, 15/600 + "N items").
- PiP: a separate floating OS window. It opens in `app/windows.rs:60-127` (`WindowKind::Floating`,
  480×270 default for 16:9, centred) and renders in `views/floating_pip.rs:81-318`: 26 px title
  bar with the mono filename + Expand + Close, and a bottom pill with play, a 3 px segmented
  click-to-seek track and "m:ss / m:ss". Actions are in `app/pip.rs:18` (`play_in_pip`), `:73`
  (`expand_pip_to_viewer`) and `:114` (`close_pip`).
- Player backend: `player/backend.rs:19-59` (no buffered-range API).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data ("4,183 clips", "68 h 12 m", "Ferry crossing…"): render
  real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views. Scrims
  (`#000000B3`, `#000000A6`), white overlays and the PiP shadow `#000000D9` are not pen tokens:
  use 00's `media_scrim()` (badges, buttons), `media_fade()` (PiP gradients) and `bg_float()` (card surface); never `bg_viewer`/`bg_hud`. Also replace the existing literals
  `rgba(0x00000099)` / `rgb(0xffffff)` at `app/render/grid.rs:592,600-603`.
- New user-facing strings go through `src/i18n.rs`.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Sidebar | Foundation Sidebar | Same as the Library frame, with "Videos" active | none (Foundation) |
| Title + meta | `toolbar-title` "Videos" 15/600, no meta (`toolbar.rs:164-174`) | "Videos" Inter Tight 20/600 −0.3. Meta JetBrains Mono 10.5 `t3`: "4,183 clips · 68 h 12 m · 211 GB" | copy: the meta line is `page-meta` in `page_header` (`views/page_header.rs`, built by 01 Task 1); Videos needs the clip count, total duration and total size (see decisions) |
| View mode / filter / sort / Import | Library plan | Segmented Mosaic·Grid·Map, 32 px icon buttons, amber Import | none here (Library plan) |
| Filter chips | Library plan's chip row | "All clips", "4K HDR", "Slow motion", "Live Photos", "+ Add filter". Right: mono count "4,183 clips" + "Clear all" (11.5 `accent`) | new video chip set (partly blocked, see decisions) |
| Section header | Month band "June 2025 · N items" (`grid.rs:674-712`) | "Recently captured" Inter Tight 15/600 −0.2 + mono 10 `t3` "128 clips"; right: "Play all ▷" 11.5 `accent` | copy ("clips" for Videos). "Play all" is new (see decisions) |
| Tile layout | Square wells, `grid_metrics` (`views/grid.rs:92-116`), column count from zoom | 4 columns at 1348 wide, column gap 14, row gap 22, thumb 16:9 (≈312×176), radius 5, cover-fit | relayout: a 16:9 tile variant for the Videos view |
| Tile caption | none (tooltip only, `grid.rs:281-284`) | Under thumb, gap 9: title Inter 12.5 `t1`, meta JetBrains Mono 10 `t3` "14 Jun 2025 · 4K · 1:18", gap 3 | new (Videos only); title from `display_title` (`inspector/body.rs:72`) |
| Duration badge | Bottom-right, h 18, black 60%, play 9 + mono 11 white (`grid.rs:583-608`) | Bottom-left 8/8, h 18, padding 0 6, radius 4, scrim `#000000B3`, gap 4, play 9 + mono 9.5 white. The clip now playing shows `audio-lines` + amber text | restyle + new "now playing" state |
| Selection | 2 px accent ring with a 2 px gap, square (`grid.rs:613-632`) | 2 px `accent` outline on the thumb itself, radius 5. Title turns `accent` | restyle (Videos tiles) |
| PiP surface | Separate floating OS window, 480×270, centred on screen (`windows.rs:60-127`) | Card 420×236, radius 10, 1 px outline `line-2`, shadow 0 18 50 `#000000D9`, bottom-right of the main area (≈56 px in) | restyle the window contents (see decision on in-window vs OS window) |
| PiP top | 26 px scrim bar: mono filename, Expand (Maximize 12), Close (X 12), 20 px buttons (`floating_pip.rs:163-220`) | 72 px top gradient scrim. Badge "PICTURE IN PICTURE" (pip icon 10, JetBrains Mono 8.5 `accent`, tracking 0.5, scrim bg, radius 4, h 20) at 12/12. Right: `maximize-2` and `x` buttons 24×24, radius 6, scrim bg, icons 12 | restyle + copy |
| PiP title | Filename in the title bar | Inter 13/500 white, display title, at bottom area (y 150) over a 116 px bottom gradient | relayout |
| PiP controls | Pill: play 20 px, 3 px track (40 click segments), "m:ss / m:ss" (`floating_pip.rs:222-317`) | Row at y 176, gap 10: skip-back 13, pause 16, skip-forward 13 (24 round hit areas), elapsed mono 10 white, track 4 px (white 20%), played `accent`, knob 10 white, total mono 10 white 70%, volume 13 | restyle + new buttons (skip ±, mute) |
| Buffered bar | none | white 30% segment | drop (local files have no buffering; see below) |

## Not in the app yet (decided 2026-11-11: all five recommendations accepted)
In Mosaic the Videos view keeps justified true-aspect rows; the 16:9 captioned tiles apply to Grid.
- **Video metadata chips: 4K HDR, Slow motion, Live Photos** (L, core). The catalog stores
  width, height and duration only (`lumina-core/src/lib.rs:108-151`). It has no frame rate, no
  HDR transfer function, and no Live Photo pairing (no match for those in `lumina-core/src`).
  Each chip needs new scanner fields, a migration plus `user_version` bump, and an FTS/query
  update. **Recommend: ship "All clips" and a "4K" chip (width ≥ 3840 from existing columns)
  now. Move HDR, slow motion and Live Photos to a separate core plan. Hide those chips until
  the data exists.**
- **Total duration and total size in the meta line** (S, core). It needs one aggregate query
  (`SUM(duration_ms)`, `SUM(file_size)`) for the current view, run off the UI thread and cached
  with the counts. **Recommend: build in this plan (Task 1), sharing the Library plan's meta-line
  plumbing.**
- **"Play all"** (M). There is no playlist or queue. The slideshow (`app/slideshow.rs`) steps
  stills on a timer and does not chain videos. **Recommend: separate plan. Omit the link until
  then.**
- **"Recently captured" section grouping** (M). The app has one sticky month band, not a
  header per section (`grid.rs:671-673` ponytail note). **Recommend: use the month band with
  the design's type and "clips" count. Keep per-section headers out of scope.**
- **PiP in-window card vs OS window** (decision). The frame draws PiP as a card inside the
  main window. Today it is a floating OS window, which stays visible over other apps and over
  a minimised library: that is the point of PiP on macOS. **Recommend: keep the OS window and
  restyle its contents to the card (radius 10, outline, 420×236 default for 16:9). Place it
  bottom-right of the main window instead of centred on screen.**

## App has, design lacks
- PiP status line "Loading video…" / "This video can't be played" (`floating_pip.rs:142-158`).
  Keep: it is error reporting.
- Window drag from anywhere on the video (`floating_pip.rs:138-140,166-168`). Keep.
- "Main library window closed" fallback (`floating_pip.rs:93`). Keep.
- Edited, flag and missing badges on cards (`grid.rs:577-582`). Keep on video tiles.
- Rating/colour-label overlays on cards. Keep wherever the Library plan keeps them.
- Grid zoom (column count). Keep for Videos, with 4 columns as the default at the design
  width.

## Review Focus
- **Long clip titles:** caption title and meta truncate to one line each, and the full name
  stays on the tooltip.
- **Zero videos / filter matches nothing:** the Library plan's empty state appears, with
  "clips" wording, and "Clear all" stays reachable.
- **Narrow window:** 16:9 tiles drop to 2 columns minimum. The caption height stays fixed so
  rows remain uniform (`uniform_list` needs equal row heights).
- **Portrait clips:** the thumb cover-fits into 16:9. The fit-mode toggle (`card_fit_mode`)
  still switches to contain.
- **PiP keyboard:** Space, J/L, ←/→ and M work while the PiP window is key. Expand and Close
  have tooltips, roles and labels. Closing PiP never leaves audio playing unseen
  (`windows.rs:116-123` behaviour stays).

## Tasks

### Task 1: Videos header copy and meta totals
**Files:** Modify `views/page_header.rs` (`page-meta`, created by 01 Task 1; not `toolbar.rs`),
`app/render/grid.rs:674-712`, `src/i18n.rs`, core aggregate in the catalog query module (find
the existing count query with `grep -rn "fn count" crates/lumina-core/src/catalog`) / Test
`crates/lumina-ui-gpui/src/tests/states.rs`, `crates/lumina-core/tests/core_tests.rs`
- [ ] Write failing tests. Core `view_totals_sum_duration_and_size` checks that the sums over a
      temp catalog with 3 videos equal the fixture totals. UI `videos_header_reads_clips` checks
      that in Videos `page-meta` contains "3 clips" and the month band count says "clips",
      not "items".
- [ ] Implement: an aggregate query run on the background executor and cached with the view
      counts. Format "N clips · H h M m · size", using `format_bytes`.
- [ ] Run `cargo test -p lumina-ui-gpui states` and `cargo test -p lumina-core view_totals`;
      expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): videos header totals` staging explicit paths.

### Task 2: 16:9 captioned video tiles
**Files:** Modify `views/grid.rs:92-116` (`grid_metrics` gains a tile-shape input),
`app/render/grid.rs:219-633` / Test `tests/grid_zoom.rs`, `tests/viewer_and_video.rs`
- [ ] Write failing test `videos_view_uses_wide_captioned_tiles`. In Videos at 1600×1000 with 4
      columns: `media-card-photo-<id>` width/height ≈ 16/9 (±1 px) and
      `media-card-caption-<id>` exists below the photo. In All Media the well stays square.
      Extend the unit test `metrics_derive_expected_squares_at_default_window` with a wide case.
- [ ] Implement: when `selection == SystemAlbum::Videos`, set thumb height = width × 9/16, radius
      5, column gap 14, row gap 22, plus a fixed caption block (title 12.5 `t1`, meta mono 10
      `t3`: date · resolution label · duration). Put the selection outline on the thumb, with an
      `accent` title.
- [ ] Run `cargo test -p lumina-ui-gpui grid`; expect pass.
- [ ] Gate and commit `feat(design): captioned 16:9 tiles in Videos`.

### Task 3: Duration badge restyle and now-playing state
**Files:** Modify `app/render/grid.rs:583-608` / Test `tests/pip.rs`
- [ ] Write failing test `video_badge_marks_the_clip_playing_in_pip`. While PiP plays item X,
      `media-card-playing-<X>` exists and no other card has it. The badge sits at the thumb's
      bottom-left (badge left = thumb left + 8).
- [ ] Implement: bottom-left 8/8, h 18, padding 0 6, radius 4, `media_scrim()`, play 9 + mono 9.5
      white. When `video.in_pip && video.item.id == item.id`, show `audio-lines` and `accent`
      text (add the lucide `audio-lines` icon to `icons.rs`/`assets.rs`).
- [ ] Run `cargo test -p lumina-ui-gpui pip`; expect pass.
- [ ] Gate and commit `feat(design): video duration badge`.

### Task 4: Video filter chips (available data only)
**Files:** Modify `lumina-core/src/search/parse.rs`, `search/mod.rs`, `search/explain.rs`, `docs/search.md`, the Library plan's chip row, `src/i18n.rs` / Test `tests/browser.rs`, `lumina-core/src/search/mod.rs` tests
- [ ] Write failing test `videos_4k_chip_filters_by_width`. In Videos with a 3840-wide and a
      1920-wide clip, clicking `filter-chip-4k` leaves one item. "Clear all" restores both.
      `filter-chip-hdr`, `-slowmo` and `-live` are absent.
- [ ] Write failing core tests in `lumina-core/src/search/mod.rs` tests:
      `is_4k_matches_long_edge_3840` (`StructuredQuery::parse("is:4k")` matches 3840×2160 and
      2160×3840, not 1920×1080) and `explain` reads "4K" (`search/explain.rs`).
- [ ] Implement: add an `is:4k` arm next to the other `is:` arms (`lumina-core/src/search/parse.rs:82-90`)
      that sets a new `min_long_edge: Option<u32>` (3840) on the query, with SQL
      `MAX(width, height) >= ?` in `build_sql`. The columns exist: `width INTEGER NOT NULL
      DEFAULT 0`, `height INTEGER NOT NULL DEFAULT 0` (`lumina-core/src/catalog/schema.rs:45-46`).
      Add the label to `search/explain.rs` and document `is:4k` in `docs/search.md`. Check
      `EXPLAIN QUERY PLAN` (an expression filter, not an ORDER BY change; no index needed unless
      the bench says so).
- [ ] Implement the chips "All clips" and "4K" through `theme::chip`; "4K" toggles the `is:4k`
      token. Add a palette entry "Filter: 4K Videos".
- [ ] Run `cargo test -p lumina-ui-gpui browser` and `cargo test -p lumina-core is_4k`; expect pass.
- [ ] Gate (+ `make bench` if the query path changed) and commit
      `feat(design): video filter chips`.

### Task 5: PiP card restyle
**Files:** Modify `views/floating_pip.rs:35-318`, `app/windows.rs:66-90` (default 420×236,
bottom-right placement), `src/i18n.rs` / Test `tests/pip.rs`
- [ ] Write failing test `pip_card_has_design_controls`. The PiP window view contains
      `pip-badge`, `pip-expand`, `pip-close`, `pip-title` (display title), `pip-skip-back`,
      `pip-play-pause`, `pip-skip-forward`, `pip-scrub-track`, `pip-mute`. Clicking
      `pip-skip-forward` does what ⌥→ does (`app/keymap/viewer.rs:165-172`: `jump_chapter(true)`, which seeks 5 s when there are no chapters, `app/video_tracks.rs:124`); `pip-skip-back` matches ⌥←.
      Clicking `pip-mute` toggles `video.is_muted`. Keep the existing `pip-expand`/`pip-close`
      assertions (`tests/pip.rs:476,506`).
- [ ] Implement: `media_fade()` gradient scrims 72 top / 116 bottom. Badge "Picture in Picture" (uppercase
      mono 8.5 `accent`), 24 px `media_scrim()` buttons with radius 6. Title Inter 13/500. Controls row
      gap 10. Replace the 40-segment hit row with a continuous 4 px track + 10 px knob, reusing
      the viewer's `scrub_fraction`/`drag_tracker` helpers (`views/viewer/transport.rs:15-55`;
      make them `pub(crate)`). No buffered bar. Default size 420×236 for 16:9. Initial
      position: bottom-right of the main window's content, ≈56 px inset.
- [ ] Run `cargo test -p lumina-ui-gpui pip`; expect pass.
- [ ] Gate (include `make ui-smoke`) and commit `feat(design): restyle picture-in-picture`.

Manual visual checks (compare with `DoXZy.png`): scrim gradients, PiP shadow and radius on the
OS window (a transparent titlebar may need the window background cleared), badge colours, tile
radius.
