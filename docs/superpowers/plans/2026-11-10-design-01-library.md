# Design 01: Library — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Bring the browser's main area (page header, filter chips, grid tiles, timeline rail, zoom control) to the Library frame, building the small missing pieces now and sending the two large ones (justified mosaic with day sections, semantic Years/Months/Days/Moments zoom) to a product decision.
**Design:** `lumina.pen` frame `t0zQL` "Library" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::chip`, `media_scrim()`, Sidebar 252 px, and the shell relayout in 00 Task 5: full-height sidebar, main column, traffic-light inset). Task 1 lands after Design 03 Task 1, which moves the search field from the toolbar into the sidebar slot.
**Icons (fetched in 00 Task 1):** layout-dashboard, grid-3x3, map, sliders-horizontal, arrow-up-narrow-wide, import, calendar, zoom-in, zoom-out. None is in `resources/icons/` today (only `sliders-vertical`). heart, play, plus, star, camera, map-pin, chevron-right, check and x already exist.
**App today:**
- `crates/lumina-ui-gpui/src/app/render/mod.rs:668-744`: window shell. A 52 px toolbar spans the full width **above** sidebar + center + inspector (677); center column = trash note, grid/map/duplicates, year rail, dock, toast (711-742), 28 px status bar (744).
- `crates/lumina-ui-gpui/src/app/render/toolbar.rs:91-264`: toolbar. Sidebar toggle (134-147), 15 px semibold title or the active year (153-190), drag area (192-207), search field (208), Trash bulk actions (210-234), downloads button (235, 302-358), sort button with `Sliders` icon (236-244), tags button (245-253), inspector toggle (254-262), client window controls (263).
- `crates/lumina-ui-gpui/src/app/render/grid.rs:42-470`: square-well grid in a `uniform_list` (128-460); one sticky month band `render_month_header` (674-711); card badges: meta pill bottom-left with filled heart, stars, colour dot (508-550), missing badge top-right (553-575), flag badge (578, `views/marks.rs:153-177`), edited badge top-left (580-582, 1095-1112), video duration bottom-right (584-606), 2 px selection ring (611-628); empty states (764-1040).
- `crates/lumina-ui-gpui/src/views/grid.rs:12-116`: layout constants (row padding 16, gap 10, card padding 8, `YEAR_RAIL_WIDTH` 44) and `grid_metrics`, with unit tests that hard-code 44 (455-532).
- `crates/lumina-ui-gpui/src/app/render/rail.rs:8-76`: year rail, `'26`-style labels; a click **filters** to that year (`set_active_year`, `app/library.rs:871-886`).
- `crates/lumina-ui-gpui/src/views/search_bar.rs:214-229`: thumbnail-size ladder `SIZE_COLUMNS = [8, 6, 4, 3, 2]`; ⌘= / ⌘- (`app/keymap/grid.rs:98-107`), ⌘-wheel and pinch (`app/panels.rs:170-379`), palette Zoom In/Out (`app/command_palette.rs:1385-1386`).
- `crates/lumina-ui-gpui/src/views/sort_menu.rs:9-134`: sort popover (field + direction), anchored top 58 / right 16.
- `crates/lumina-ui-gpui/src/views/sidebar/sections.rs:497-537`: the only Map switch, a "Map" pill on the Places row; `app/map.rs:318-366` `map_shown` requires Places.
- Quick filters exist only as palette commands (`app/command_palette.rs:1388-1389`, `app/search.rs:212-250`); no chip row is rendered anywhere.
- `lumina_core::LuminaEngine::get_timeline_buckets("year"|"month"|"day")` (`crates/lumina-core/src/lib.rs:180-183, 499`): year and month counts are already cached (`app/library.rs:1057-1072`).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- The header row is shared by every main-area frame (First Run uses it too). This plan owns it as `page_header` (`views/page_header.rs`, Task 1); later screen plans call it.
- Colours are not checkable headlessly. Tests cover selectors, bounds, state and behaviour; colour, radius and type are a manual comparison against `t0zQL.png` at 1600×1000.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Shell | toolbar spans the window above the sidebar (`render/mod.rs:677`) | sidebar full height; the header lives in the main column only | done by 00 Task 5; this plan fills the header only |
| Header bar | 52 h, `bg_app`, bottom border, gap 8 (`toolbar.rs:113-132`) | 67.5 h, padding 0 28, bottom border line, title block left, actions right (gap 10) | relayout |
| Title | 15 px semibold, active year replaces it (`toolbar.rs:153-175`) | Inter Tight 20/600, −0.3 tracking, t1 | restyle |
| Meta line | none (only the Trash count, `toolbar.rs:176-189`) | JetBrains Mono 10.5, t3, gap 3 under the title: "48,291 items · Jun 2018 — Nov 2026" | new (count + date range of the view) |
| View mode control | none; Map is a pill on the Places row (`sections.rs:497-537`) | bg-2 r9 outline line, padding 3, gap 2; segments 26 h, padding 0 10, gap 6, icon 13 + Inter 12; on: bg-3, t1; off: t3. "Mosaic" (layout-dashboard), "Grid" (grid-3x3), "Map" (map) | new (Grid + Map now; Mosaic needs a decision) |
| Filter button | none | 32×32 bg-2 r8 outline line, `sliders-horizontal` 15 | new (opens the Add filter menu) |
| Sort button | 28×28 ghost, `Sliders` icon, tooltip "Sort: …" (`toolbar.rs:236-244`) | 32×32 bg-2 r8 outline line, `arrow-up-narrow-wide` 15 | restyle + icon |
| Import button | none in toolbar (palette `ImportMedia`, `command_palette.rs:1432`) | accent, r8, 32 h, padding 0 13, gap 7, `import` 14, Inter 12.5/600 `#1A1206` (accent fg) | new (runs `pick_import_folders`) |
| Search field | 300 px in the toolbar (`toolbar.rs:208`) | not in the header (sidebar, Design 03) | remove from header after 03 Task 1 |
| Filter chip row | none | 51.5 h, padding 0 28, bottom border line; chips gap 8 (Foundation Chip: 28 h, bg-2, outline line, r999, padding 0 11, icon 13, Inter 12 t1); "Add filter" chip transparent, outline line-2, `plus`, t3; right: "1,284 matches" mono 10.5 t3, gap 12, "Clear all" Inter 11.5 accent | new; shown only while the query has filter tokens |
| Month band | one sticky band, 36 h, 15 px semibold month + count (`grid.rs:674-711`) | a header per day: date Inter Tight 15/600 −0.2, `map-pin` 11, place Inter 12 t2, count mono 10 t3 (gap 9); right "Select" Inter 11.5 t3 + `chevron-right` 12 | needs decision (variable-height list); interim restyle of the band |
| Grid body | square wells in a card with 8 px padding, row padding 16, gap 10, no radius (`views/grid.rs:12-30`) | padding 22 20 0 28; tiles r4, gap 6; justified rows 158–224 h; section gap 26, header-to-row 10 | Grid mode: restyle (padding 28/20, gap 6, r4); Mosaic: needs decision |
| RAW badge | none (`MediaType::Raw` exists, `lumina-core/src/lib.rs:101-105`) | bottom-right, 18 h, padding 0 6, `#000000A6`, r4; "RAW" JetBrains Mono 9.5 accent | new |
| Video duration | bottom-right, 18 h, `rgba(0,0,0,.6)`, mono 11, play 9 (`grid.rs:584-606`) | **bottom-left** 8 px inset, 18 h, padding 0 6, gap 4, `#000000A6`, r4; play 9 + mono 9.5 white | relayout + restyle |
| Favourite | filled heart inside the bottom-left meta pill (`grid.rs:508-550`) | outline `heart` 14, top-right 8 px inset, no backing | relayout; meta pill keeps stars and colour label (see App has) |
| Flag / missing / edited badges | top-right / top-right / top-left (`grid.rs:553-582`, `marks.rs:153-177`) | not in frame | keep; flag moves left of the heart so they never overlap |
| Selection ring | 2 px accent ring with 2 px gap (`grid.rs:611-628`) | not in frame | keep |
| Selection check circles + dim | none | while a selection exists: selected tile gets a 22 px accent circle with `check` 13 top-left; unselected tiles get a #00000059 circle and 0.62 opacity (from Design 07 `w4zpqh`, owned here) | new (Task 5) |
| Timeline rail | 44 w, border `#222326`, `'26` labels gap 18, click filters to a year (`rail.rs:8-76`) | 83.5 w, bg-1, left border line; "TIMELINE" Inter 8/600 tracking 1 t3 at 10,8; year labels mono 9.5 t3 (current year t1) at x 10; one 4 h r2 bar per month right-aligned to x 70, width ∝ month count (max ≈ 36), `#32323A`, current year `#4A4A56`, visible months accent; "2020 ↓" `#3A3A44` for older years | relayout + new bars (month counts already cached) |
| Rail lens | none | 52×29 accent-soft, outline accent, r5, over the visible months | new |
| Rail hover bubble | none | bg-3 r6 outline line-2, shadow 0 2 10 `#000000AA`, 23 h, padding 0 9, left of the rail; "MAY — JUL 2025" mono 9.5 accent | new |
| Zoom control | none on screen; ⌘=/⌘-, ⌘-wheel, pinch, palette (`panels.rs:170-379`) | floating pill bottom-centre of the content (top 812 in frame, ~146 above the bottom), `#16161CF2`, outline line-2, r999, 42 h, padding 0 6, gap 3; `zoom-out` / `zoom-in` 30×30 icon 15; segments 30 h padding 0 14 Inter 11.5 t2; on: accent bg, `#1A1206` 600 | new (labels need a decision, see 2) |
| Status bar | 28 h "N of M selected", scan/import progress, Cancel import (`status_bar.rs:41-122`) | none | keep (see App has) |
| Sidebar / tags / inspector toggles, downloads | toolbar buttons (`toolbar.rs:134-147, 235, 245-262`) | none | keep, see App has |

## Not in the app yet (needs a product decision)
1. **Mosaic: justified rows grouped by day.** Size **L**, no engine work beyond item 4. The grid is a `uniform_list` of equal rows (`grid.rs:128`), and three things depend on that: the zoom anchor math (`views/grid.rs:154-310`), arrow-key row moves (`app/keymap/grid.rs`), and marquee hit-testing. A mosaic needs GPUI's variable-height `list` with a `ListState`, rows computed from each item's aspect (width/height are on `MediaItem`), day header rows, and new keyboard geometry. **Recommend: separate plan (01b).** Until then the Mosaic segment is not rendered, and Grid stays the default.
2. **Years / Months / Days / Moments zoom.** Size **L**. The frame's pill picks a grouping level. The app's ladder picks a thumbnail size (8/6/4/3/2 columns). "Moments" (clusters by time and place) does not exist. **Recommend:** in this plan, ship the pill with `zoom-out`/`zoom-in` buttons that walk the existing ladder, and with **no** labelled segments. Grouping levels go in 01b with the mosaic, since both need section headers.
3. **Day headers with place and "Select >".** Size **M**, core work: a per-day dominant place (most common `city, country` among the day's items) and a "select range" action. Only meaningful once item 1 exists. **Recommend: 01b.** The interim band keeps showing the month and gets the frame's type.
4. **Header date range.** Size **S**. For the unfiltered library it comes from the cached month counts (`app/library.rs:1057-1064`): oldest and newest non-zero month. For an album, filter or search it needs a `MIN/MAX(timestamp_epoch_ms)` (`catalog/schema.rs:49`, NOT NULL, so 0 counts as undated) over the view query (one engine call on the background executor, cached with the view count). **Recommend: build in this plan.** When the view is undated, show only the count.
5. **Timeline scrubbing.** Size **M**. The frame's rail is a scroll scrubber with a lens over the visible range. Today a click filters to a year. In date order (All Media, date sort, no filter), the month counts give an item offset per month, so the rail can scroll to `offset / columns`. **Recommend: build in this plan for date-sorted, unfiltered views.** The year filter stays reachable through the palette's year targets (`PaletteTarget::Year`, `command_palette.rs:221`). In other views the rail is hidden, as today. Flag: this changes what a rail click does.
6. **Map segment outside Places.** The map only plots Places (`map.rs:318-321`). **Recommend:** the Map segment runs `show_map` (it switches to Places, as ⌘K Show Map does today), and the sidebar pill is removed. A map scoped to the current view (an album, a filter) needs its own plan.
7. **Filter button vs "Add filter".** The frame has both. **Recommend:** both open the same Add filter menu: the quick-filter families from `search.md`, plus Camera…, Place…, Date… rows that put the matching `camera:`, `city:`, `date:` token into the search field.

## App has, design lacks
- **Status bar** (selection count, scan/import progress, Cancel import, last action). Keep. It holds the only on-screen Cancel for a running import.
- **Sidebar toggle, Tags, Info (inspector) and Downloads buttons** in the toolbar. Keep them, as 32×32 Foundation icon buttons left of the filter button, in this order: Downloads (only when it has rows), Tags, Info. The sidebar toggle stays at the left of the header, before the title, with its ⌃⌘S tooltip.
- **Trash title count and Restore All / Empty Trash…** (`toolbar.rs:176-234`). Keep them in the header on Recently Deleted. The count moves into the meta line.
- **Meta pill with stars and colour label** (`grid.rs:508-550`). Keep it bottom-left. The duration badge sits above it when both show. Ratings and labels are culling data, so removing them is not a restyle.
- **Missing, flag, edited badges and the selection ring.** Keep. They carry state.
- **Year filter by click.** Moves to the palette (decision 5). Keep the active-year title override.

## Review Focus
1. **Narrow window (1024 px, inspector open):** the header's actions must not push the title off. The title truncates first, then the meta line hides, then segment labels collapse to icons (tooltips keep the names). The chip row scrolls horizontally rather than wrapping.
2. **Huge counts:** "1,284,503 items", and a filter matching 0 items. With 0 matches, the chip row stays, shows "0 matches" and keeps Clear all, and the grid shows the existing search empty state (`grid.rs:786-792`).
3. **macOS traffic lights:** with the sidebar full height they overlap the sidebar brand row. Foundation's sidebar reserves the 84 px inset (today in `toolbar.rs:118-122`). Verify by `make ui-smoke` screenshot, not only headless.
4. **Keyboard:** every new control (segments, filter, sort, Import, chips with their ✕, Clear all, zoom ± buttons, rail years) has a debug_selector, a tooltip, a role and a label, and a palette or menu path. Rail scrubbing has an equivalent through Home/End and the palette year targets.
5. **Undated items / a library spanning 1970–2026:** the rail's "older ↓" row collapses years past what fits. The header date range ignores year 0 (`month_title`, `grid.rs:1085-1091`).

## Tasks
### Task 1: Page header
**Files:** Create `crates/lumina-ui-gpui/src/views/page_header.rs`; Modify `crates/lumina-ui-gpui/src/app/render/toolbar.rs:91-264` (its contents move into `page_header`), `crates/lumina-ui-gpui/src/i18n.rs` / Test `crates/lumina-ui-gpui/src/tests/library_header.rs` (new, register in `tests/mod.rs`). The shell itself (`render/mod.rs:668-744`) is 00 Task 5's change; do not redo it.
- [ ] Write failing test `header_sits_in_the_main_column_with_title_meta_and_actions`: `page-title` left edge ≥ sidebar right (252) + 28; `page-header` height 67.5 (±0.5); `page-meta` reads "{count} items"; `header-import`, `header-filter`, `sort-menu-button` exist inside `page-actions` and sit right of the title; the downloads button keeps its existing selector; `search-input` is not inside `page-header`.
- [ ] Write failing test `import_button_and_sort_button_run_their_actions`: clicking `header-import` calls the import picker (test counter, as in `tests/import.rs`); clicking `sort-menu-button` opens the sort menu; Escape closes it.
- [ ] Implement `page_header(title, meta, actions)` in the new `views/page_header.rs` (selectors `page-header`, `page-title`, `page-meta`, `page-actions`). Pages 04, 05, 08, 11, 13, 15/18 and 17 render their header through it, and it hides while the Search screen (03) shows. Build it inside 00 Task 5's main column: 67.5 h, padding 0 28, bottom border `line()`; title Inter Tight 20/600, −0.3 tracking; meta line mono 10.5 `t3()`; actions gap 10; filter, sort and Import buttons with the values in the table; existing buttons kept per "App has". New strings go in `i18n.rs` (`header.items`, `header.import`).
- [ ] Run `cargo test -p lumina-ui-gpui library_header`; expect pass. Re-run `toolbar` and `windows_and_layout` tests (they measure the old bar).
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): library page header` staging explicit paths.

### Task 2: Header date range
**Files:** Modify `crates/lumina-core/src/lib.rs` (view date span), `crates/lumina-ui-gpui/src/app/active_view.rs:250-260` / Test `crates/lumina-core/tests/core_tests.rs`, `crates/lumina-ui-gpui/src/tests/library_header.rs`
- [ ] Write failing core test `view_date_span_ignores_undated_items`: seeded items in Jun 2018 and Nov 2026 plus one undated item give `(2018-06, 2026-11)`; an all-undated view gives `None`.
- [ ] Write failing UI test `header_meta_shows_count_and_date_range`: `page-meta` text is "{n} items · Jun 2018 — Nov 2026" for the seed. An album with one month shows a single month.
- [ ] Implement: one engine call with the same WHERE as the view count, run on the background executor and cached next to `ActiveView.count`. Use the month buckets for unfiltered All Media.
- [ ] Run `cargo test -p lumina-core view_date_span` and `cargo test -p lumina-ui-gpui header_meta`; expect pass.
- [ ] Gate and commit `feat(design): header date range`.

### Task 3: View mode control (Grid / Map)
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/toolbar.rs`, `crates/lumina-ui-gpui/src/views/sidebar/sections.rs:480-537` / Test `crates/lumina-ui-gpui/src/tests/map.rs` (update `map-toggle` uses at 411-425, 679-688)
- [ ] Write failing test `view_mode_segments_switch_grid_and_map`: `view-mode-grid` has `aria_toggled` true by default. Clicking `view-mode-map` makes `map_shown()` true and selects Places. Clicking `view-mode-grid` returns to the grid. `map-toggle` no longer exists.
- [ ] Implement: segmented control with the table's values, Grid and Map segments only (Mosaic waits on decision 1). Role Button + aria_toggled. Tooltips "Show Grid" and "Show Map" (existing `command.show_grid` / `command.show_map` keys). Remove `map_toggle` from the Places row (`sections.rs:493, 497-537`). This removal is confirmed by the lead; 00 does not keep it.
- [ ] Run `cargo test -p lumina-ui-gpui map`; expect pass.
- [ ] Gate and commit `feat(design): grid/map view mode control`.

### Task 4: Filter chip row and Add filter menu
**Files:** Create `crates/lumina-ui-gpui/src/views/filter_bar.rs`; Modify `crates/lumina-ui-gpui/src/app/search.rs:174-250`, `crates/lumina-ui-gpui/src/app/render/mod.rs:711-719` / Test `crates/lumina-ui-gpui/src/tests/filter_bar.rs` (new)
- [ ] Write failing test `filter_tokens_render_as_chips_with_count_and_clear_all`: query `month:jun year:2025 city:Lisbon camera:"X-T5" rating:>=3` renders `filter-chip-0..3` with icons calendar / map-pin / camera / star. `filter-match-count` reads "{n} matches". Clicking `filter-clear-all` leaves the query empty. A free-text-only query renders no row.
- [ ] Write failing test `chip_remove_and_add_filter_are_keyboard_reachable`: clicking `filter-chip-remove-1` drops only that token. `filter-add` opens `filter-add-menu`. ↓ and ↵ add `is:video`, which appears as a chip. Esc closes the menu without changing the query.
- [ ] Implement: chips from `tokenize_query` + `StructuredQuery::parse` per token (one label function per family: date "Jun 2025", place "Lisbon, Portugal", camera, "Rated 3+", media type, favourite, colour). Use Foundation `theme::chip`. Row 51.5 h, padding 0 28, gap 8, bottom border. Add filter chip and the header filter button open one menu (overlay + occluding card + backdrop). Clear all → `clear_quick_filters`.
- [ ] Run `cargo test -p lumina-ui-gpui filter_bar`; expect pass.
- [ ] Gate and commit `feat(design): filter chip row`.

### Task 5: Tile badges
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/grid.rs:220-240, 505-632`, `crates/lumina-ui-gpui/src/views/marks.rs:150-177` / Test `crates/lumina-ui-gpui/src/tests/thumbnails.rs`, `crates/lumina-ui-gpui/src/tests/multi_select.rs`
- [ ] Write failing test `raw_badge_duration_and_heart_sit_where_the_design_puts_them`: a RAW item has `media-card-raw-{id}` at its well's bottom-right. A video's `media-card-duration-{id}` is at bottom-left (left inset 8). A favourite's `media-card-fav-{id}` is at top-right. A picked favourite's `grid-flag-badge-{id}` is left of the heart with no overlap. A rated video's meta pill sits above the duration badge with no overlap.
- [ ] Write failing test `selection_mode_shows_check_circles_and_dims_unselected`: with two of four items selected, `media-card-check-{id}` exists on every card, and selected cards show the `check` icon. Unselected cards render at opacity 0.62 (assert the value through a test hook on the card style). With no selection there are no check circles and no dim.
- [ ] Add the selectors first: `media-card-raw-{id}` (new), `media-card-duration-{id}` (the duration badge at `grid.rs:583-606` has no selector today) and `media-card-fav-{id}` (new). Also `media-card-check-{id}`.
- [ ] Implement: RAW badge (18 h, padding 0 6, r4, mono 9.5 `accent()` on `media_scrim()`). Duration, missing, flag and edited badges replace their `rgba(0x00000099)` literals with `media_scrim()`. Check circles: 22 px, top-left 8 px inset; selected: `accent()` fill with `check` 13 in accent-fg; unselected: a #00000059 circle (00 keeps it as a one-off opacity) and the card at `.opacity(0.62)`. Shown only while `selected_indices` is non-empty. duration bottom-left with the frame's values; outline heart 14 top-right 8 px inset; meta pill keeps stars and label only; flag badge shifts left when the heart or missing badge shows. Tiles get radius 4.
- [ ] Run `cargo test -p lumina-ui-gpui thumbnails`; expect pass.
- [ ] Gate and commit `feat(design): grid tile badges`.

### Task 6: Grid spacing and the month band restyle
**Files:** Modify `crates/lumina-ui-gpui/src/views/grid.rs:12-116, 449-532`, `crates/lumina-ui-gpui/src/app/render/grid.rs:674-711` / Test `views/grid.rs` unit tests, `crates/lumina-ui-gpui/src/tests/grid_zoom.rs`
- [ ] Update unit tests first: left padding 28, right padding 20, gap 6, card padding 0, rail 83.5. Recompute the expected `photo_side` values in `metrics_*` tests; they fail until the constants change.
- [ ] Implement: split `GRID_ROW_PADDING_X` into left/right (28/20) everywhere it is used, including `column_at_pointer`. `GRID_CARD_GAP` 6, `CARD_PADDING` 0, `YEAR_RAIL_WIDTH` 83.5. Band: Inter Tight 15/600 −0.2 title, count mono 10 `t3()`, 10 px above the first row.
- [ ] Run `cargo test -p lumina-ui-gpui grid`; expect pass (zoom-anchor tests must still keep the item under the pointer).
- [ ] Gate and commit `feat(design): grid spacing and section band`.

### Task 7: Timeline rail
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/rail.rs:8-76`, `crates/lumina-ui-gpui/src/app/library.rs:1057-1072` / Test `crates/lumina-ui-gpui/src/tests/grid_scroll.rs`
- [ ] Write failing test `rail_draws_month_bars_and_scrolls_to_a_year`: rail width 83.5. `timeline-label` exists. `timeline-year-2025` and `timeline-bar-2025-06` exist, and the bar width is proportional to the June count. Clicking `timeline-year-2024` scrolls so the first 2024 item's row is visible, with `active_year` still None. Hovering a bar shows `timeline-bubble` naming the visible month range.
- [ ] Write failing test `rail_hides_outside_the_unfiltered_date_view`: the rail is present on All Media with date sort; absent in a user album, with `city:Lisbon` in the query, with name sort, on Duplicates and on the map.
- [ ] Write failing test `rail_lens_tracks_scroll`: after scrolling the grid, `timeline-lens` covers the months of the visible rows.
- [ ] Implement: values from the table. Month bars from `month_header.counts`, months newest first. Offset per month = sum of newer months' counts. Scroll target = offset / columns × row height, through the grid scroll handle. Rail shown only for date sort + All Media + no query. **This is a behaviour change:** today `rail.rs:9-14` hides the rail only when the buckets are empty, on Duplicates, or on the map, so it shows in albums and searches. Month offsets are library-wide, so outside that view a scrub would land on the wrong rows. Years past the space available collapse into "{year} ↓", which jumps to that year.
- [ ] Run `cargo test -p lumina-ui-gpui grid_scroll`; expect pass.
- [ ] Gate and commit `feat(design): timeline scrubber rail`.

### Task 8: Zoom control
**Files:** Create `crates/lumina-ui-gpui/src/views/zoom_pill.rs`; Modify `crates/lumina-ui-gpui/src/app/render/mod.rs:720-742` / Test `crates/lumina-ui-gpui/src/tests/grid_zoom.rs`
- [ ] Write failing test `zoom_pill_buttons_walk_the_size_ladder`: `grid-zoom-in` changes columns 4 → 3. `grid-zoom-out` changes 3 → 4. At 2 columns `grid-zoom-in` is disabled (aria + no change). The pill is centred in the content column and hidden while the map, duplicates or an empty state shows.
- [ ] Implement: pill with the table's values. Buttons call `zoom_in_grid` / `zoom_out_grid`, with tooltips "Larger thumbnails (⌘+)" and "Smaller thumbnails (⌘-)". No labelled segments until decision 2. Bottom offset 24 above the status bar; it must not cover the selection dock (stack above it when both show).
- [ ] Run `cargo test -p lumina-ui-gpui grid_zoom`; expect pass.
- [ ] Gate (including `make ui-smoke`) and commit `feat(design): grid zoom control`.

### Task 9 (decisions 1–3 decided 2026-11-11: all go to plan 01b): Mosaic, day sections, semantic zoom
Write plan 01b once decided. It covers the variable-height list, justified rows (target row heights 158–224, gap 6, section gap 26), day header rows (date, dominant place, count, "Select >" selecting the day's items, with a palette "Select Day" entry), Years/Months/Days/Moments levels, and keyboard geometry for uneven rows.
