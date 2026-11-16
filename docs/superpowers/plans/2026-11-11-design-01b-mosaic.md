# Design 01b: Mosaic, Day Sections and Grouping Levels — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Add the Library frame's Mosaic mode: justified rows grouped by day, each day under a header with its date, dominant place, count and "Select >". Add the zoom pill's grouping levels (Years / Months / Days). Grid stays the default and does not change.
**Design:** `lumina.pen` frame `t0zQL` "Library". The values are quoted in plan 01: target row heights 158–224, tile gap 6, section gap 26, header-to-row 10. Day header: date Inter Tight 15/600 −0.2; `map-pin` 11 + place Inter 12 t2; count mono 10 t3 (gap 9); right "Select" Inter 11.5 t3 + `chevron-right` 12. Pill segments: 30 h, padding 0 14, Inter 11.5 t2; on: accent bg, `#1A1206` 600.
**Depends on:** Design 01 Tasks 1–8 (committed: `2d309eb`..`5fc3392`). Decisions 1–3 of plan 01 moved here on 2026-11-11 (its Task 9).
**Icons:** layout-dashboard (fetched in 00 Task 1), map-pin, chevron-right (exist).
**App today:**
- `crates/lumina-ui-gpui/src/app/render/grid.rs:51-230`: `render_browser_grid`. A `uniform_list` of equal rows (142) with `cols = grid_columns` items each. The processor fetches the visible items through `displayed_rows`/`want_view_rows` (`app/active_view.rs:336-343, 383-386`), never SQL. The card body (badges, ring, check circle) is inline in the processor from about line 206. The sticky month band is at 731-783.
- `crates/lumina-ui-gpui/src/views/grid.rs:12-155`: constants and `grid_metrics`. `GridZoomGeometry`/`GridZoomAnchor` (198-345) assume a fixed row height and column count.
- `crates/lumina-ui-gpui/src/app/keymap/grid.rs`: Shift+↑/↓ (73-80), PageUp/Down (123-133) and ↑/↓ (175-183) all step `±grid_columns`.
- `crates/lumina-ui-gpui/src/app/navigation.rs`: `grid_top_row` (340-349) and `grid_top_item_id` (353-358) divide the scroll offset by the row height. `reveal_grid_selection` (362-370) scrolls to `index / cols`. The marquee's `apply_grid_drag_range` (658-690) selects a row×column rectangle.
- `crates/lumina-ui-gpui/src/app/panels.rs`: `zoom_in_grid`/`zoom_out_grid` (171-186), `step_grid_size` (190), ⌘-wheel (307), pinch (371).
- `crates/lumina-ui-gpui/src/app/render/rail.rs`: month offsets (88-105). `timeline_rail_shown` (201-210) is true only for All Media + date sort + no query. `grid_visible_items` (212-231) and `scroll_grid_to_offset` (247-262) use `offset / cols × row_height`.
- `crates/lumina-ui-gpui/src/views/zoom_pill.rs`: ± buttons only.
- `crates/lumina-ui-gpui/src/app/render/toolbar.rs:332-360`: the `view-mode` segmented control with Grid and Map.
- Core: `MediaItem.width/height/year/month/day/city/country_code` (`crates/lumina-core/src/lib.rs:108-136`). `rotation_quarters` (`catalog/schema.rs:76`). `view_where(scope)` (`catalog/view_scope.rs:34-52`) gives each view's WHERE with the same rows as its count. `Sort::order_by("m.")` (`sort.rs:49`) gives its ORDER BY. Year/month/day `timeline_buckets` with counts and `cover_media_id` (`catalog/schema.rs:666-736`, read at 744). Month counts are cached in `MonthHeader.counts` (`app/state.rs:1048-1052`, filled at `app/library.rs:1075`).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, a keyboard path for every action, a command palette entry for global actions, no UI blocking on IO, tests per change.
- No per-frame SQL. A frame reads only the cached layout and `displayed_rows`.
- Grid mode keeps its code paths. Every Mosaic branch is a `match self.browse_layout`, never a rewrite of the Grid path.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS. Design copy ("Lisbon", "214 items") is sample data.
- Colours and fonts come only through Foundation tokens. Colour is checked by hand against `t0zQL.png` at 1600×1000.
- `app/render/grid.rs` is already 1297 lines (§5.1), so the Mosaic code goes in new files.

## Current vs design
| Element | App today (file:line) | Design | Change |
|---|---|---|---|
| View mode | Grid, Map (`toolbar.rs:332-360`) | Mosaic, Grid, Map | add `view-mode-mosaic` first |
| Body | `uniform_list`, square wells (`grid.rs:142`) | justified rows 158–224 h, gap 6 | new `list` + `ListState` in Mosaic |
| Sections | one sticky month band (`grid.rs:731-783`) | a header row per day, section gap 26, 10 to the first row | new; the band is hidden in Mosaic |
| Day header | none | date · pin + place · count … Select > | new (core place query) |
| Zoom pill | ± walks 8/6/4/3/2 columns (`zoom_pill.rs`) | Years · Months · Days · Moments segments, ± | segments in Mosaic; ± steps the row height |
| Select a day | none | "Select >" | new + palette "Select Day" |

## Not in the app yet (decided 2026-11-11: all five recommendations accepted)
1. **Mosaic under a non-date sort.** Day sections only make sense in date order. **Recommend:** Mosaic works in every sort (rows come from `Sort::order_by`). Headers and grouping levels appear only under date sort; other sorts get one header-less run.
2. **Moments.** Clusters by time and place inside a day: split where the gap is over 2 h or the city changes, plus a sub-header. Size **M**: the layout query also needs city and timestamp, a cluster pass, and sub-header rows and their tests. Apple dropped Moments from Photos in 2019 in favour of Days. **Recommend: drop the segment for now.** The pill shows Years · Months · Days. Revisit if users ask; the layout pass can carry a cluster key later without a schema change.
3. **Where Years/Months apply.** Their cards come from the library-wide `timeline_buckets`, so they are right only where the rail is (`timeline_rail_shown`). **Recommend:** the segments render only in Mosaic and only there. Elsewhere Mosaic is Days with ± only. Per-view year/month cards would need a grouped query per view; that waits for demand.
4. **Marquee in Mosaic.** Rows are uneven, so a row×column rectangle has no meaning. **Recommend:** a drag selects the reading-order range from the anchor to the target, as Photos does. Grid keeps its rectangle.
5. **Default for new users.** **Recommend: Grid stays the default** (decided in plan 01). The choice persists in config as `browse_layout`.

## Review Focus
1. **100k items:** the layout fetch and justify run off the UI thread. The ListState item count is about rows + days (~25k), never items. Scrolling issues no SQL (check with a `ROWS_READ`-style counter). Resizing the window keeps the old layout until the new one lands, with no blank frame.
2. **Keyboard geometry:** ↑/↓ across a header and into a short last row land on the tile nearest in x. Shift+↑/↓ extends with the same geometry. PageUp/Down move by visible rows. Home/End are unchanged.
3. **The rail and zoom keep their contracts in Mosaic:** a rail year click lands on the first row of that year. ⌘-wheel, pinch and ± keep the item under the pointer (or the selection) in place.
4. **Extreme media:** 0×0 (unprobed video), 10:1 panoramas, rotated items (odd `rotation_quarters` swap the aspect), undated items (year 0 → an "Undated" section with no place), and a day with 3,000 items.
5. **Accessibility and i18n:** headers and Select controls have roles and labels; every new string is an `i18n.rs` key.

## Tasks
### Task 1: Extract the card element (no behaviour change)
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/grid.rs:206-` (the inline card body); Create `crates/lumina-ui-gpui/src/app/render/card.rs` / Test: existing `thumbnails`, `multi_select`, `grid_zoom`
- [ ] Move the per-card body into `fn render_media_card(&mut self, item: &MediaItem, index: usize, size: Size<f32>, tex: Option<Arc<RenderImage>>, cx) -> AnyElement`. Width and height are separate so Mosaic can pass non-square tiles. The selectors (`media-card-{id}`, badges, check) stay exactly as they are.
- [ ] Run `cargo test -p lumina-ui-gpui thumbnails multi_select grid_zoom`; expect pass with no test edits.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests) and commit `chore(grid): extract the media card element`.

### Task 2: Core layout and day-place queries
**Files:** Modify `crates/lumina-core/src/catalog/view_scope.rs`, `crates/lumina-core/src/lib.rs` / Test `view_scope.rs` unit tests, `crates/lumina-core/tests/core_tests.rs`, `crates/lumina-core/tests/benchmark_test.rs:202`
- [ ] Write failing core test `view_layout_follows_the_view_order_and_aspect`: seed 5 items with mixed sizes and one item rotated a quarter turn. `view_layout(&ViewScope::Library, Sort::default())` returns `LayoutCell { id, aspect, day }` in the same id order as `get_item_slice(0..5)`. A 4000×3000 item has aspect 1.333. The rotated one has 0.75. A 0×0 item has 1.0. `day` is `yyyymmdd`, or 0 when undated.
- [ ] Write failing core test `day_places_picks_the_most_common_city_per_day`: one day with 3×Lisbon,PT and 1×Porto,PT gives city "Lisbon", country code "PT". A tie goes to the alphabetically first city. A day with no city is absent. Album scope counts only members.
- [ ] Write failing unit test `layout_and_place_queries_use_the_view_indexes`, using the EXPLAIN helper at `view_scope.rs:99-109`. The Library layout plan walks `idx_media_timestamp` with no `TEMP B-TREE` for date sort. The album plan is driven by `album_items`. For `day_places` record the plan in the doc comment. A `USE TEMP B-TREE FOR GROUP BY` is expected and accepted: the query is one background pass per revision. Do **not** add a `(year, month, day, city)` index unless the bench below misses its gate.
- [ ] Implement:
  - `view_layout`: `SELECT m.id, m.width, m.height, m.rotation_quarters, m.year*10000+m.month*100+m.day FROM media_items m WHERE ({view_where}) ORDER BY {sort.order_by("m.")}`. Aspect is clamped to 0.4–3.0 in Rust.
  - `day_places`: `SELECT year*10000+month*100+day, city, country_code, COUNT(*) FROM media_items m WHERE ({view_where}) AND m.city IS NOT NULL GROUP BY 1,2,3`. Fold to the max per day in Rust. Returns `(city, country_code)`; the UI joins them like the inspector summary (`views/inspector/summary.rs:332`), so "Lisbon, PT". No country-name table exists and none is added.
  - Both read through `self.read(...)` (log and default) like `view_date_span`.
- [ ] Bench (§10): in `benchmark_scale_and_gates`, time both queries at the bench scale (100k), before any app use. Record the release numbers in the commit body. Set the gates at about 2× measured: expected near 40 ms for layout and 60 ms for places; these are background budgets, not frame budgets. Run `make bench` alone.
- [ ] Run `cargo test -p lumina-core view_layout day_places layout_and_place`; expect pass.
- [ ] Gate (+ `make bench`) and commit `feat(core): view layout cells and per-day places`.

### Task 3: Justified layout (pure, cached per width)
**Files:** Create `crates/lumina-ui-gpui/src/views/mosaic.rs` (unit tests inline, `#[core::prelude::v1::test]`)
- [ ] Write failing tests:
  - `rows_justify_to_the_width_within_the_height_band`: every full row's tile widths plus gaps of 6 equal the content width (±0.5), and every full row's height is within [158, 224] at the default target 190.
  - `a_section_ends_its_last_row_unstretched`: the last row of a section keeps the target height, left-aligned.
  - `sections_follow_day_runs`: cells for days 20250614×3 then 20250613×2 give two headers, at y 0 and at y = header + rows + 26.
  - `vertical_neighbour_picks_the_nearest_x`: from the right-most tile of a 3-tile row to a 5-tile row below, ↓ picks the tile whose x-centre is closest. ↑ from the first row returns None. Crossing a header works.
  - `row_of_and_y_of_item_round_trip`.
  - `justify_100k_within_budget`: 100k random aspects in under 60 ms (debug). Print the number; the release number goes in the commit body.
- [ ] Implement `MosaicLayout::justify(cells: &[LayoutCell], width: f32, target: f32, sections: bool) -> MosaicLayout`. It is a greedy Flickr-style pass, O(n): add items until the row height at full width drops to ≤ target; close early when the next item would push the height below `0.83 × target`. Output `rows: Vec<MosaicRow { first: usize, len, y, height, kind: Header{day,count} | Tiles }>` plus per-tile `x, w` in a flat `Vec<f32>` (8 bytes × 100k). Helpers: `row_of(index)` (binary search on `first`), `vertical_neighbour(index, ±1)`, `rows_in(y_range)`, `total_height()`. The header row height is 15/600 text + 10. The section gap of 26 is part of the header row's top padding.
- [ ] Size ladder: the 5 `SIZE_COLUMNS` levels map to targets `[110, 150, 190, 250, 320]`, so ±, ⌘=/⌘-, the wheel and pinch keep one ladder and one persisted level.
- [ ] Run `cargo test -p lumina-ui-gpui mosaic::`; expect pass.
- [ ] Gate and commit `feat(mosaic): justified row layout`.

### Task 4: Mosaic body (variable-height list)
**Files:** Create `crates/lumina-ui-gpui/src/app/render/mosaic.rs`; Modify `app/render/mod.rs:775` (pick the body), `app/render/toolbar.rs:332-360` (segment), `views/grid.rs:399-470` (`GridState` gets `mosaic: ListState`), `app/active_view.rs` (layout cache), `config.rs` (`browse_layout`, serde default Grid), `i18n.rs` / Test `crates/lumina-ui-gpui/src/tests/mosaic.rs` (new; register in `tests/mod.rs`)
- [ ] Write failing test `mosaic_segment_switches_layout_and_persists_in_config`: `view-mode-grid` is toggled by default. Clicking `view-mode-mosaic` sets `browse_layout == Mosaic`. `mosaic-list` renders, and `grid-month-header` is absent. Clicking Grid restores the `uniform_list` (`media-grid`). Both segments have aria_toggled. Uses `SAVE_FAILS`-free config, no disk write.
- [ ] Write failing test `mosaic_tiles_keep_aspect_and_fill_rows`: seed 6 items via `seed_library` and set width/height with rusqlite (the pattern in `tests/search_header.rs:621-631`) to 3:2, 2:3 and 1:1. The `media-card-{id}` bounds have width/height ratio within 2% of each aspect. Tiles in one row share a top and height in [158, 224]. Row right edges sit at `media-grid-surface` right − 20 (±1).
- [ ] Write failing test `mosaic_scroll_reads_no_sql`: after the layout lands, scroll the list 40 rows through `ListState::scroll_to`. `render_ui` twice. The `ROWS_READ` delta comes only from `want_view_rows` fetches, not from layout, and the window parks (the uniform_list regression at `tests/grid_scroll.rs:7-13` applies here too).
- [ ] Implement:
  - A layout cache keyed `(ViewKey, revision, width as u32, level)`. The cells are fetched with `view_layout` on the background executor next to `Fetched`, and `justify` also runs in the background. When it lands, call `ListState::reset(rows.len())` and restore the top item through `scroll_to(ListOffset { item_ix: row_of(top_item), offset_in_item })`. Until then the old layout stays painted.
  - The list element is `list(state, processor)`. Each row is `.h(px(row.height))`, so GPUI's measure equals the layout. Tiles call `render_media_card` with the row's `x/w`. Visible rows call `want_view_rows(first..last)` exactly as the uniform_list does (`grid.rs:166-172`). Overdraw is one viewport.
  - The width comes from `center_pane_width` (`views/grid.rs:92`) minus 28/20 padding.
  - Wheel, pinch and marquee listeners stay on `media-grid-surface`.
- [ ] Run `cargo test -p lumina-ui-gpui mosaic`; expect pass. Re-run `map`, which also clicks `view-mode-*`.
- [ ] Gate (+ `make ui-smoke`) and commit `feat(design): mosaic layout`.

### Task 5: Day headers, dominant place and Select Day
**Files:** Modify `app/render/mosaic.rs`, `app/active_view.rs` (cache `day_places` with the layout), `app/command_palette.rs` (`CommandId::SelectDay`), `app/navigation.rs` (`select_range`), `i18n.rs` / Test `tests/mosaic.rs`
- [ ] Write failing test `day_header_shows_date_place_and_count`: two days with 3 and 2 items, three of them Lisbon,PT. `mosaic-day-20250614` reads "Sat, Jun 14, 2025". `mosaic-day-place-20250614` reads "Lisbon, PT" and `mosaic-day-count-20250614` reads "3 items". The second header's top is 26 (±1) below the last tile above it. A day with no city has no place element. Undated items get "Undated" last.
- [ ] Write failing test `select_day_selects_exactly_that_days_items`: clicking `mosaic-day-select-20250613` makes `selected_indices` equal to that day's 2 indices, and the batch summary updates. Then ⌘K "Select Day" with one item of the other day selected selects that day's 3 items. With nothing selected, the top visible day is used. Escape clears the selection.
- [ ] Implement:
  - The header row uses the design's values. The date uses `i18n` key `mosaic.day_date` (en-US `%a, %b %-d, %Y`), the count uses `header.items`, and "Undated" uses `mosaic.undated`. The place comes from the cached `day_places`; the cache is refreshed with the layout, never per frame.
  - "Select >" is a button with role Button and aria_label "Select {count} items from {date}" (`mosaic.select_day_label`).
  - The header has role Heading and an aria_label with date, place and count.
  - `select_range(range)` sets `selected_indices`, `selected_index = range.start` and calls `update_batch_summary`.
  - The palette `SelectDay` (key `command.select_day`) shows only in Mosaic under date sort.
- [ ] Run `cargo test -p lumina-ui-gpui mosaic`; expect pass.
- [ ] Gate and commit `feat(design): mosaic day headers and Select Day`.

### Task 6: Keyboard, marquee, rail and zoom in Mosaic
**Files:** Modify `app/keymap/grid.rs:73-80, 123-133, 175-183`, `app/navigation.rs:340-370, 658-690`, `app/render/rail.rs:212-262`, `app/panels.rs:190, 307, 371` / Test `tests/mosaic.rs`
- [ ] Write failing test `arrow_keys_use_nearest_x_in_uneven_rows`: build a layout with rows of 3 and then 5 tiles. Select the third tile and press ↓: the selected index is `vertical_neighbour` (assert the concrete index), and the selected card's bounds are inside `media-grid-surface`. ↑ returns. Shift+↓ extends to the same index. PageDown moves at least 3 rows. Focus the app first (§6.3).
- [ ] Write failing test `marquee_selects_the_reading_order_range`: drag from tile 2 to tile 7 with synthetic mouse events at rendered bounds. `selected_indices == {2..=7}`.
- [ ] Write failing test `rail_year_click_and_zoom_keep_position_in_mosaic`: with 200 items over 3 years, clicking `timeline-year-2024` puts the first 2024 tile's row at the top (±1 row), and `timeline-lens` covers the visible months. Then `grid-zoom-in` with tile N selected keeps `media-card-N` visible, and the target height steps one level.
- [ ] Implement: each site branches on `browse_layout`.
  - Keys: ↑/↓ → `layout.vertical_neighbour`; Page → move by `rows_in(viewport)`.
  - `grid_top_row` and `grid_top_item_id` read `ListState::logical_scroll_top()`. `reveal_grid_selection` → `scroll_to_reveal_item(row_of(i))`.
  - Drag → contiguous range.
  - Rail: `scroll_grid_to_offset` → `scroll_to(row_of(offset))`. `grid_visible_items` → `rows_in(scroll_top..+viewport)`.
  - Wheel and pinch: find the item under the pointer by hit-testing the layout, step the level, and after the new layout lands, keep that item's row at the same pointer y.
- [ ] Run `cargo test -p lumina-ui-gpui mosaic grid_zoom grid_scroll multi_select`; expect pass (Grid tests unchanged).
- [ ] Gate (+ `make ui-smoke`, `make ui-e2e` with the skip reason reported) and commit `feat(mosaic): keyboard, marquee, rail and zoom geometry`.

### Task 7: Years / Months / Days levels on the zoom pill
**Files:** Modify `views/zoom_pill.rs`; Create `app/render/bucket_cards.rs`; Modify `config.rs` (`group_level`, default Days), `i18n.rs`, `app/command_palette.rs` / Test `tests/mosaic.rs`
- [ ] Write failing test `zoom_pill_levels_show_year_and_month_cards`: in Mosaic on All Media with date sort, `zoom-level-days` is on. Clicking `zoom-level-years` renders `year-card-2025` with "{n} items" from the year bucket, and its cover requests the bucket's `cover_media_id` thumbnail. ↵ on a focused month card (arrows move focus) switches to Days scrolled to that month. In an album the segments are absent and ± still works. In Grid the segments are absent.
- [ ] Implement:
  - The segments use the design's values, with role Button + aria_toggled and tooltips.
  - Cards come from `get_timeline_buckets("year"|"month")`, read with the month counts at `app/library.rs:1075` (no new query). They render in a `uniform_list` of fixed card rows (3 per row for years, 4 for months) using `render_media_card`-style covers.
  - Card click and ↵ → `group_level = Days` + rail offset scroll.
  - Palette: "Show Years", "Show Months", "Show Days" (keys `command.show_years|months|days`), shown only where the segments render.
  - Update the zoom_pill module doc, which still says the segments wait on a decision.
- [ ] Run `cargo test -p lumina-ui-gpui mosaic zoom`; expect pass.
- [ ] Gate (+ `make ui-smoke`) and commit `feat(design): years, months and days levels`.

### Task 8: Docs
- [ ] `docs/accessibility.md`: the Mosaic keyboard model, and the header and Select roles (not checked with VoiceOver yet; do not claim more). Add a CHANGELOG entry. Mention Select Day in the shortcut sheet if one is added. Commit `docs: mosaic and grouping levels`.
