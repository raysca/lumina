# Design 08: Tags Management — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Turn the Tag Manager sheet into a Tags page in the main area: page header, tabs,
a keyword table and a tag detail panel with preview, aliases, organise actions and Delete tag.
**Design:** `lumina.pen` frame `L3F8Cs` "Tags Management" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation; Design 01 Library Task 1 (`page_header`); Design 05 Albums Task 2 (`page_tabs`); Design 00 Task 5 (key-based
sidebar selectors, TAGS › Keywords row `sidebar-nav-keywords`).
**Icons (fetched in 00 Task 1):** ellipsis, corner-down-right, merge. Also used: `folder-plus`
(05); `trash-2`, `pencil`, `plus`, `search`, `chevron-right` already exist in `resources/icons/`.
**App today:** a modal sheet hanging from the toolbar, `crates/lumina-ui-gpui/src/views/tag_manager/mod.rs:16-71`
(header 74-119 with "Tag Manager", "N tags", key hints and Done), list column
`views/tag_manager/list.rs:13-121` (440 wide: filter field 123-154, A–Z/Count segmented
47-61, checkbox column header 65-94, 36 px rows with tree indent 174-282, bulk bar 320-390),
detail column `views/tag_manager/detail.rs:14-95` (24 px name + Show in Browser 98-148, NAME
field 161-184, Move to field 187-198, ALIASES chips + field 215-285, TAGGED ITEMS 6-column
grid 288-321, Move to… / Delete Tag / note 54-93). State and actions: `app/tags.rs:329-603`,
`app/keywords.rs`. Opened from the toolbar Tags button (`app/render/toolbar.rs:245-253`), the
palette "Open Tag Manager…" (`app/command_palette.rs:903`), the empty state
(`app/render/grid.rs:1070`). Keys: `app/keymap/modals.rs:319` onward.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "612 keywords", "golden hour", "2,884 items"):
  render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Container | modal sheet, width clamp 640-1000, dim backdrop (`mod.rs:21-58`) | main-area page beside the sidebar; sidebar TAGS › Keywords active | relayout |
| Header | 56 h: tag icon, "Tag Manager", "N tags", key hints, Done (`mod.rs:74-119`) | `page_header`: "Tags" Inter Tight 20/600; meta mono 10.5 t3 "612 keywords · 48 people · 61 places · 94% of library tagged"; icons sliders / arrow-up-narrow-wide 32×32 bg-2 r8; primary "New Tag" `plus` | relayout; meta "N keywords · P% of library tagged"; see decisions for New Tag |
| Tabs | none | `page_tabs`: Keywords 612 (active), People 48, Places 61, Auto-detected 188, Unused 23 | new (Keywords only; see decisions) |
| Filter | `theme::field` flex_1, "Filter tags by name…" (`list.rs:123-154`) | right of tabs: 200×32 bg-1 r8 pad 0 10 gap 8, `search` 13, placeholder "Filter tags" Inter 12 t3 | relayout + copy |
| Sort | segmented A–Z / Count (`list.rs:47-61`) | text menu "Most used ▾" Inter 11.5 t2 | restyle (menu: Most used, A–Z) |
| Table header | 28 h: select-all checkbox, NAME, USAGE (`list.rs:65-94`) | 29.5 h, gap 14, border-bottom line; TAG / USAGE 280 / SOURCE 110 / LAST USED 120 / act 32; Inter 9.5/600 ls .9 t3; table pad 14 24 0 28 | restyle; columns per decisions |
| Row | 36 h, checkbox, indent, name text_sm, usage bar 110×4 + count (`list.rs:200-281`) | 44 h r6 pad 0 8 gap 14; dot 9; name Inter 13 (500 when focused) t1; "+2 aliases" 10.5 t3; usage track 190×6 r-full bg-3, bar #4E4E5C = `chart_grey` (focused: accent), count mono 11 t2; act `ellipsis` 14 in 32×28 r6 | restyle + new alias count, dot, act button |
| Focused row | `theme::row` focused fill (`list.rs:200-203`) | bg-2 + 2 px accent left border | restyle |
| Source column | none | badge h20 r4 pad 0 7: "Auto 0.94" `sparkles` accent on accent-soft / "Manual" `user` t2 on bg-3 | drop (no auto-tagging) |
| Last used | none | mono 10.5 t3 "Today", "2 days ago" | drop or schema (decisions) |
| Detail panel | flex_1, bg_pane, pad 24 gap 20 (`detail.rs:15-24`) | 359.5 w, bg-1, border-left line, pad 20 18, gap 20 | relayout |
| Detail header | 10 px neutral dot, 24 px name, items label, Show in Browser (`detail.rs:98-148`) | dot 11 (tag colour); name Inter Tight 21/600 ls -0.4; `pencil` 14 edit; meta mono 10.5 t3 "2,884 items · created 12 Mar 2023" | restyle; meta "N items" (drop created) |
| NAME field | always-visible field (`detail.rs:161-184`) | none; pencil renames in place | relayout: name becomes the field while editing |
| PREVIEW | TAGGED ITEMS, 6 cols, 64 h (`detail.rs:287-321`, `GRID_COLUMNS` at `detail.rs:12`) | head "PREVIEW" + "View all" accent 11; 2 rows × 4, 68 h, gap 5 | restyle; "View all" = Show in Browser (`detail.rs:143-146`) |
| ALIASES | chips h24 border + ×, field "Add an alias…" (`detail.rs:215-285`) | hint "Items tagged with any of these appear under golden hour." 10.5 t3; chips bg-2 r-full h28 pad 0 11 gap 6, `corner-down-right` 13 + Inter 12 t2; "+ Add" chip transparent t3 | restyle; field opens from "+ Add"; keep × on hover/focus |
| AUTO-APPLY | none | toggle, "1,284 of 2,884 added automatically", confidence slider 0.80 | drop |
| COLOUR | none (neutral dot, "ponytail: … until tags have colours" `detail.rs:106`) | 6 swatches 24 px gap 9 | see decisions |
| ORGANISE | Move to… control (`detail.rs:66-75`), merge only from the bulk bar (`list.rs:345-353`) | rows bg-0 r8 h46 pad 0 11 gap 10, icon 15, label Inter 12 t1, sub 10 t3, `chevron-right` 14: "Merge into another tag…" (`merge`, "Combines items, keeps both names as aliases"), "Save as Smart Album" (`folder-plus`, "Updates itself as new matches arrive") | new rows; keep Move as a third row |
| Delete | control "Delete Tag" + note (`detail.rs:76-92`) | divider; full-width danger-soft r8 h34, `trash-2` 13, "Delete tag" 12/500 danger; hint "Removes it from 2,884 items. Your files are untouched." 10.5 t3 | restyle + copy (keep "and the keywords below it" when it has children) |

## Not in the app yet (needs a product decision)
- **Page instead of sheet.** Moves the Tag Manager into the main area as a destination
  (sidebar Keywords, toolbar Tags button, palette, empty state all open it). Keys stop being
  modal: they apply while the page has focus, and grid shortcuts must not fire on it. Size M,
  UI only. Recommend: build in this plan.
- **People, Places, Auto-detected tabs; SOURCE column; AUTO-APPLY.** No faces, no automatic
  tagging or confidence in the engine (AGENTS §15: AI not implemented). Size L each.
  Recommend drop from design. Places as a tab is Design 05's Places decision.
- **Unused tab.** A keyword exists only while an item carries it (`mod.rs:94-95`), so the
  count is always 0. Recommend drop.
- **New Tag.** Same reason; there is nothing to create without items. Recommend drop, or make
  it start tagging the grid selection (`begin_tagging_selection`, `app/tags.rs:206`) only
  when a selection exists. Recommend drop.
- **LAST USED and "created 12 Mar 2023".** `media_tags` has no timestamp
  (`catalog/schema.rs:132-137`). Size M (schema + FTS/undo paths). Recommend drop.
- **Tag colour (row dot, header dot, COLOUR swatches).** Not stored; the sidebar hashes a
  colour from the name (`views/sidebar/mod.rs:26-49`). Persisting needs a `tag_colors`
  table carried through rename, merge, move and delete. Size M, core work. Recommend: dots
  use `tag_color()` now (no schema); the picker is a separate plan. The design swatches
  (#7AA2E8 #8BD4A8 #C98BD4 #E88B8B #A8A8B4) are not pen tokens; map to the app's label
  colours if it is built.
- **Merge "keeps both names as aliases".** `merge_tags` is a rename (`crates/lumina-core/src/catalog/keyword_ops.rs:290-292`)
  and adds no alias. Recommend: add the source as an alias of the target in the same
  transaction and the same `TagChange` (undo removes it). Size S, core work (Task 5).
- **Usage bar fill #4E4E5C**: Foundation token `chart_grey`.

## App has, design lacks
- Row checkboxes, select all, bulk bar with Merge / Delete (`list.rs:80-91,320-390`): keep;
  they are the multi-tag keyboard path (Space, ⌘A, ⌘M, ⌘⌫).
- Keyword hierarchy indent (`list.rs:171,226-231`) and ancestor rows without counts: keep.
- Move to… (⇧⌘M, `detail.rs:66-75`): keep as an ORGANISE row.
- Header key hints and Done (`mod.rs:104-117`): remove Done (a page has no Done); move the
  hints into the shortcut sheet.
- Alias × remove buttons and "⌫ in an empty field removes the last": keep.

## Review Focus
- No tags: table shows "No tags yet. Add tags from the inspector."; detail shows its empty
  copy; meta reads "0 keywords · 0% of library tagged".
- Long or deep keyword paths: name truncates; the indent stops growing past a cap so the
  usage column keeps 280 px; tooltip has the full path.
- Thousands of keywords: rows are virtualised (uniform 44 px list), filter typing stays
  responsive; preview thumbnails come from the texture cache.
- Keyboard: ↑↓ move focus, ↵ renames, Space checks, Esc clears checks then leaves the
  filter; the row `…` menu opens with the context-menu key or ⇧F10 and every item has a
  palette equivalent.
- Narrow window: the detail panel keeps 359.5 and the table drops LAST USED first, then the
  usage track, so names never collapse below ~160 px.

## Tasks
### Task 1: Tags page destination
**Files:** `views/sidebar/mod.rs:12-24` (new page variant), `views/sidebar/sections.rs:248-` (remove
the sidebar keyword tree, `keyword_tree` rows and the inline `sidebar-tag-rename` at
`sections.rs:280-300` onward; the Keywords row itself comes from Design 00 Task 5), `app/render/mod.rs:726-737,852-853`, `app/tags.rs:329-350`,
`app/keymap/modals.rs:319` (move to a page key handler), `views/tag_manager/mod.rs` /
Test `crates/lumina-ui-gpui/src/tests/tag_manager.rs`
- [ ] Write failing test `tags_page_opens_in_main_area`: palette "Open Tags" shows
      `tags-page` with `page-title` "Tags"; `tag-manager-backdrop` does not exist; sidebar
      `sidebar-nav-keywords` is active; no per-keyword rows remain in the sidebar
      (`sidebar-tag-rename` and the tree rows are gone; rename, filter-by-tag and delete stay
      reachable from the page and palette); ⌘[ returns to the previous view; ↑ moves tag focus.
- [ ] Implement: render the page in the centre column; remove the sidebar keyword tree
      (owned here) and move or update the tests that use sidebar tag selectors (`sidebar-tag*` in
      `tests/sidebar.rs`, `tags_and_albums.rs`, `keywords.rs`, `browser.rs`,
      `ui_automation.rs`) to the page; keep open/close functions as
      navigate-to/away; update existing tests' `open()` helper (`tests/tag_manager.rs:29`).
- [ ] Run `cargo test -p lumina-ui-gpui tag_manager`; expect pass (existing tests updated,
      not deleted).
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): tags page` staging explicit paths.

### Task 2: Header, tab, filter and sort
**Files:** `views/tag_manager/mod.rs:74-119`, `views/tag_manager/list.rs:38-63,123-169` /
Test `tests/tag_manager.rs`
- [ ] Write failing test `tags_header_filter_and_sort`: `page-meta` reads "2 keywords · P% of
      library tagged" (P from `query_untagged_count`), `page-tab-keywords` count 2,
      `tag-manager-search` width 200 and placeholder "Filter tags", `tag-sort-menu` offers
      Most used / A–Z and sets `TagSortOrder`.
- [ ] Implement with `page_header` / `page_tabs`; percentage computed with the counts refresh,
      not per frame.
- [ ] Run `cargo test -p lumina-ui-gpui tags_header`; expect pass.
- [ ] Gate and commit `feat(design): tags header and filter`.

### Task 3: Keyword table rows
**Files:** `views/tag_manager/list.rs:65-115,174-286` / Test `tests/tag_manager.rs`
- [ ] Write failing test `tag_rows_match_design`: row `tag-manager-row-<tag>` height 44,
      `tag-manager-usage-<tag>` width 190, fill width scales to the most used tag,
      `tag-row-aliases-<tag>` "+2 aliases" when 2 aliases exist, `tag-row-menu-<tag>` opens a
      menu with Rename, Merge into…, Save as Smart Album, Delete; checkbox still toggles.
- [ ] Implement columns TAG / USAGE 280 / act 32 (no SOURCE, no LAST USED), dot via
      `tag_color()`, focused row bg-2 + accent left border; add `Icon` variants for
      ellipsis, corner-down-right, merge (SVGs from 00 Task 1) and the existing `trash-2.svg`
      to `Icon::ALL`; fetch nothing here. Usage bar fill `chart_grey`.
- [ ] Run `cargo test -p lumina-ui-gpui tag_rows`; expect pass.
- [ ] Gate and commit `feat(design): keyword table`.

### Task 4: Detail panel layout
**Files:** `views/tag_manager/detail.rs:14-321`, `app/tags.rs:551-566` (load 8 items) /
Test `tests/tag_manager.rs`
- [ ] Write failing test `tag_detail_panel_sections`: `tag-manager-detail` width 359.5;
      `tag-detail-edit` (pencil) turns `tag-manager-detail-name` into the rename field and ↵
      renames; `tag-detail-view-all` sets the search to `tag:<path>`; `tag-manager-items`
      has 8 tiles of height 68; `tag-detail-alias-add` opens `tag-detail-alias-input`;
      `tag-manager-delete-<tag>` deletes with undo and the hint shows the real count.
- [ ] Implement exact values from the table; keep `tag-detail-move` as an ORGANISE row.
- [ ] Run `cargo test -p lumina-ui-gpui tag_detail`; expect pass.
- [ ] Gate and commit `feat(design): tag detail panel`.

### Task 5: Merge into another tag (single tag) and alias on merge
**Files:** core `crates/lumina-core/src/catalog/keyword_ops.rs:288-292`; UI `app/tags.rs:499-549`,
`views/tag_manager/detail.rs` / Test `crates/lumina-core/tests/core_tests.rs`, `tests/tag_manager.rs`
- [ ] Write failing tests `merge_keeps_source_as_alias` (core: after merge, typing the old
      name resolves to the target; undo removes the alias) and
      `merge_into_row_picks_target_by_keyboard` (UI: `tag-detail-merge` opens a target
      picker filtered by typing, ↵ merges, items move, sidecars rewritten).
- [ ] Implement: `merge_tags` sets the alias inside the rename transaction (skip when the
      source is a parent with children of its own); single-tag picker reuses the move field
      pattern; palette "Merge Tag Into…".
- [ ] Run `cargo test -p lumina-core merge_keeps` and `cargo test -p lumina-ui-gpui merge_into`.
- [ ] Gate and commit `feat(design): merge tag into`.

### Task 6: Save as Smart Album
**Files:** `app/album_builder_actions.rs:58-99` (reuse the prefilled builder),
`views/tag_manager/detail.rs` / Test `tests/smart_album_builder.rs`
- [ ] Write failing test `tag_save_as_smart_album_prefills_tag_rule`: `tag-detail-save-smart`
      opens `album-builder-card` with one row Tag is `<path>` and title `<leaf>`; ↵ creates it.
- [ ] Implement; palette "Save Tag as Smart Album".
- [ ] Run `cargo test -p lumina-ui-gpui tag_save_as_smart`; expect pass.
- [ ] Gate and commit `feat(design): save tag as smart album`.

Manual checks: dots, badge-free rows, focused row border, panel colours against
`L3F8Cs.png` with `make ui-smoke`.

## Decisions (2026-11-12: recommendations accepted)
- Build the Tags page in the main area instead of the sheet (Task 1). The sidebar's TAGS
  section gets a "Keywords" row (`sidebar-nav-keywords`) if Design 00 Task 5 hasn't added
  it; the per-keyword tree leaves the sidebar.
- Dropped: People, Places, Auto-detected and Unused tabs; SOURCE column; AUTO-APPLY;
  New Tag; LAST USED; "created …" meta. The tabs row shows Keywords only.
- Tag colour: dots use the existing hashed `tag_color()`; no COLOUR picker (separate plan).
- Merge keeps the source name as an alias of the target, in the same transaction and undo
  (Task 5).
- Usage bar fill: add Foundation token `chart_grey` (#4E4E5C) if missing.
- Keep: row checkboxes, select all, bulk bar (Merge / Delete), hierarchy indent, Move to…
  as an ORGANISE row, alias × and ⌫-removes-last. Remove Done; key hints move to the
  shortcut sheet.
- Order: Tasks 1 to 6 one at a time (they share views/tag_manager/*).
