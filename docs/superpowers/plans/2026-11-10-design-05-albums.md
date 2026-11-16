# Design 05: Albums — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Add an Albums overview page to the main area: a page header, view tabs, a row of
smart album cards and a grid of album cover cards, as the frame shows.
**Design:** `lumina.pen` frame `a0sLrG` "Albums" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation; Design 01 Library Task 1 (`page_header` in
`views/page_header.rs`); Design 09 Task 3 (`rule_label`). Design 08 Tags reuses the
`page_tabs` helper this plan adds (Task 2). The sidebar "New Album" row is Design 00 Task 5.
**Icons (fetched in 00 Task 1):** folder-plus, arrow-up-narrow-wide.
**App today:** there is no albums overview. Albums exist only as sidebar rows:
`crates/lumina-ui-gpui/src/views/sidebar/albums.rs:18-71` (ALBUMS section, `+` button at
42-60, rows at 122-214), smart albums at `views/sidebar/sections.rs:168-240`. Choosing a
row filters the one browser grid (`app/render/mod.rs:726-737`) under the global toolbar
(`app/render/toolbar.rs:91-264`), whose title is the album name (`app/render/mod.rs:479-495`).
`AlbumSelection` (`views/sidebar/mod.rs:12-24`) has no overview variant. Album data:
`Album { id, title, created_at_epoch_ms, pinned }` (`crates/lumina-core/src/catalog/curated.rs:17-22`),
counts from `list_albums_with_counts` (`app/albums.rs:83-87`). Albums have no cover and no
updated time (`catalog/schema.rs:115-123`).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "24 albums", "Lisbon, June", "2,884 items"):
  render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Page | none; album choice filters the grid (`app/render/mod.rs:726-737`) | Main column 1348 wide: TopBar, Tabs, Body (pad 22 28 0 28, gap 26) | new |
| TopBar title | global toolbar title, 15px semibold (`toolbar.rs:162-175`) | "Albums", Inter Tight 20/600, ls -0.3, t1; bar 67.5 h, pad 0 28, border-bottom line | new (page header) |
| TopBar meta | none | JetBrains Mono 10.5, t3, gap 3 under title: "24 albums · 12 smart albums · 6 shared" | new; render "N albums · M smart albums" (drop "shared") |
| TopBar actions | toolbar sort / tags / inspector icons (`toolbar.rs:236-262`) | 32×32 bg-2 r8 icons `sliders-horizontal`, `arrow-up-narrow-wide` (15); primary "New Album" bg accent r8 h32 pad 0 13 gap 7, `folder-plus` 14, Inter 12.5/600 dark text | new; New Album calls `new_album` (`app/albums.rs:115`) |
| Tabs | none | bar 51.5 h, pad 0 28, gap 4, border-bottom line; tab h34 r8 pad 0 12 gap 7; active bg-2, label t1 12.5/500, count accent mono 9.5; inactive label t2 400, count t3. Tabs: Albums, Smart Albums, People, Places, Shared | new (Albums, Smart Albums only; see decisions) |
| Smart Albums head | none | "Smart Albums" Inter Tight 15/600 ls -0.2 t1; right "Manage rules" accent Inter 11.5 | new |
| Smart album card | sidebar row only (`sections.rs:61-100`) | bg-1 r10 pad 12 gap 13 h138, row gap 14, 4 per row; mosaic 110 wide, 2×2 tiles 55 h r3 gap 4; `sparkles` 13 accent + title 13/500; "2,884 items · live" mono 10 t3; rule chips bg-0 r5 h21 pad 0 7 gap 5, icon 10 + mono 9.5 t2 | new |
| Albums head | none | "Albums" 15/600; right "Sort: Recently updated ▾" accent 11.5 | new |
| Album card | sidebar row 26 h (`albums.rs:142-213`) | 245 wide, gap 16, row gap 20; Stack 194 h: Back2 217×12 r4, Back1 231×12 r4, Cover 183 h r6; caption gap 4: title Inter 13/500 t1, meta mono 10 t3 "284 items · Jun 2025" | new |
| Locked album | none | `lock` 12 after title, meta "… · Locked" | drop (no such feature) |
| Sidebar ALBUMS add | `+` icon in the section header (`albums.rs:42-60`) | "New Album" row with `plus` icon at the end of the list | owned by Design 00 Task 5 (selector `new-album`); not in this plan |
| Sidebar album icon | `Icon::Image` (`albums.rs:169`) | `folder` | owned by Design 00 Task 5 |
| Sidebar active row | the open album/smart album | frame highlights "Golden Hour" while showing the overview (design inconsistency) | on the overview no row is active |

## Not in the app yet (decided 2026-11-11: recommendations accepted)
Build the overview page; covers and "Mon YYYY" from the newest member via `album_cards()`;
drop "Updated … ago" and "Recently updated" (sort by Name, Date created, Newest photo, Item
count); drop People, Shared and Locked; Places tab is a separate plan; one chip per real rule.
Plates: add `theme::plate()` / `theme::plate_2()` if Foundation does not have them. Task 5
needs a small `rule_label` (Design 09 Task 3 is not built): add it here, one chip per rule.
- **Albums overview page itself.** A new destination (`AlbumSelection::Albums` or a page
  enum), reached from a click on the sidebar ALBUMS label, a palette command "Show Albums",
  and the Go menu. Size M, UI only plus one core query (covers). Recommend: build in this plan.
- **Album covers and date line.** No cover column. Recommend the newest member by capture
  date as the cover and "Mon YYYY" of the newest member as the meta, both from one new
  grouped engine query `album_cards()` (album id, cover media id, newest timestamp). Size S,
  core work (add to `catalog/curated.rs`, test in `crates/lumina-core/tests/core_tests.rs`).
  A chosen key photo is Design 07's schema change; when it lands, it overrides this.
- **"Updated 2d ago" and "Sort: Recently updated".** Needs an `updated_at` column bumped on
  every membership change. Recommend drop; offer Name, Date created, Newest photo, Item count.
- **People tab.** No faces or people in the catalog (schema has no such table,
  `catalog/schema.rs:36-240`). Size L, ML work; AGENTS §15. Recommend drop from design.
- **Places tab.** The Places system album and map exist (`smart_albums/system.rs`,
  `views/map.rs`); a tab of place cards needs grouping by city. Size M. Recommend separate plan.
- **Shared tab and "6 shared".** Conflicts with AGENTS §5.4 (no remote libraries or sharing
  accounts). Recommend drop from design.
- **Smart album rule chips as drawn.** Chips summarise rules. The sample chips use rules the
  engine does not have: time of day ("17:00 — 19:30", "after 21:00"), shutter speed
  ("1/500 s +"), edited state ("unedited"), "outdoors" (an AI label). `ƒ < 2.0`, `ISO > 1600`,
  RAW are supported (`smart_albums/rules.rs:16-46`). Render one chip per real rule with
  Design 09 Task 3's `rule_label` (`rule_summary`, `app/smart_draft.rs:410`, returns one
  sentence for the whole album, so it is not used for chips); engine fields are Design 16's
  decision.
- **Stack back plates** use #2A2A33 / #3A3A46; Foundation provides them as `theme::plate()`
  (Back2) and `theme::plate_2()` (Back1).

## App has, design lacks
- Global toolbar with sidebar toggle, search field, downloads, sort, tags and inspector
  (`toolbar.rs:113-264`): keep sidebar toggle and search (owned by the Library/Foundation
  plans); on this page sort lives in the "Sort ▾" control.
- Sidebar row hover actions rename / delete (`albums.rs:190-205`) and double-click rename:
  keep. Add the same to a card context menu so the overview is not a dead end.
- Status bar and selection dock: hide on the overview (no media selection here).

## Review Focus
- Zero albums and zero smart albums: each section shows a one-line empty state with its
  create action; the page never renders an empty grid with a header only.
- Long album names: title truncates with "…" in the 245 card; tooltip shows the full name.
- Narrow window: card columns = floor((width + 16) / (200 + 16)), min 1; smart cards wrap
  to 2 per row below ~900 px; no horizontal scroll.
- Keyboard: Tab reaches tabs, New Album and cards; arrows move card focus, Enter opens the
  album, ⌘⌫ asks to delete the focused album; the focus ring stays visible and scrolls in.
- Hundreds of albums: the album grid is virtualised (reuse the grid's row virtualisation
  pattern) and covers decode through the texture cache, not per frame SQL.

## Tasks
### Task 1: Overview destination and navigation
**Files:** Modify `views/sidebar/mod.rs:12-24`, `app/library.rs:757-780`, `app/render/mod.rs:479-495,726-737`,
`app/command_palette.rs` (new `ShowAlbums`), `src/i18n.rs` / Test `crates/lumina-ui-gpui/src/tests/albums_overview.rs` (new)
- [ ] Write failing test `albums_overview_opens_from_sidebar_and_palette`: click
      `sidebar-albums-header`, assert `albums-overview` renders and `media-grid-surface` does
      not; same via palette "Show Albums"; Back (⌘[) returns to the previous view.
- [ ] Implement: add the selection variant, route the centre column to
      `views/albums_overview/mod.rs` (new module, keep under ~500 lines), make the ALBUMS
      label a button (role Button, aria label), add the palette and Go menu entries. No
      sidebar row is active while the overview shows (assert no `sidebar-nav-*` active).
- [ ] Run `cargo test -p lumina-ui-gpui albums_overview`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): albums overview destination` staging explicit paths.

### Task 2: Page header and tabs helpers
**Files:** Modify `views/page_header.rs` (created by Design 01 Task 1; add `page_tabs`) / Test `tests/albums_overview.rs`
- [ ] Write failing test `albums_header_and_tabs_match_design`: `page-header` height 67.5,
      `page-title` text "Albums", `page-meta` "N albums · M smart albums" from catalog
      counts, `page-tab-albums` and `page-tab-smart-albums` height 34, `new-album-page-btn`
      exists; clicking it creates an album in rename mode (as `new_album`).
- [ ] Implement `page_tabs(&[(id, label, count, active)])` next to Design 01's `page_header`, and render the Albums header with `page_header`,
      with the exact values in the table (tokens: t1, t3, bg-2, accent, line; fonts
      FONT_DISPLAY title, FONT_MONO meta/counts). Add `Icon::FolderPlus` and
      `Icon::ArrowUpNarrowWide` to `icons.rs` (`Icon::ALL`); the SVGs come from 00 Task 1.
      No ⌘1/⌘2: tabs are reached by click, Tab focus then ←/→, and palette entries
      ("Show Albums Tab", "Show Smart Albums Tab"); test the ←/→ path.
- [ ] Run `cargo test -p lumina-ui-gpui albums_header`; expect pass.
- [ ] Gate and commit `feat(design): page header and tabs`.

### Task 3: Album cover query (core)
**Files:** Modify `crates/lumina-core/src/catalog/curated.rs`, `crates/lumina-core/src/lib.rs` /
Test `crates/lumina-core/tests/core_tests.rs`
- [ ] Write failing test `album_cards_pick_newest_active_member`: three albums (one empty, one
      with a trashed newest item); assert cover ids and newest timestamps, empty album gives
      `None`, trashed items are skipped.
- [ ] Implement one grouped query over `album_items` joined to active `media_items`; check
      `EXPLAIN QUERY PLAN` uses the album_items primary key; doc comment says why. The UI
      calls it on the background executor (`cx.background_executor().spawn`, as `app/smart_album_share.rs:168` does), not inside the synchronous
      `refresh_album_counts` (`app/library.rs:1027`); results land in a cached map and notify.
- [ ] Run `cargo test -p lumina-core album_cards`; expect pass.
- [ ] Gate and commit `feat(core): album card covers`.

### Task 4: Album card grid
**Files:** `views/albums_overview/albums.rs` (new), `app/albums.rs:83-87` (start the
background `album_cards` fetch after the counts refresh) / Test `tests/albums_overview.rs`
- [ ] Write failing test `album_cards_render_and_open`: two albums; `album-card-<id>` width
      245 at a 1600 window, `album-card-cover-<id>` height 183, caption shows title and
      "N items · Mon YYYY"; click opens `AlbumSelection::Album(id)`; Enter on the focused
      card does the same; right-click offers Rename and Delete.
- [ ] Implement the Stack (back plates `plate()` / `plate_2()`, r4; cover r6), caption, hover, focus
      ring, empty album placeholder (bg-2 with `folder` icon), sort menu (Name, Date
      created, Newest photo, Item count; persisted like the grid sort).
- [ ] Run `cargo test -p lumina-ui-gpui album_cards`; expect pass.
- [ ] Gate and commit `feat(design): album cards`.

### Task 5: Smart album cards
**Files:** `views/albums_overview/smart.rs` (new); chips use `rule_label` from Design 09
Task 3 (depends on it) / Test `tests/albums_overview.rs`
- [ ] Write failing test `smart_album_cards_show_mosaic_count_and_chips`: one smart album
      `ISO > 1600`; `smart-card-<id>` height 138, 4 mosaic tiles, count "N items · live",
      chip text "ISO > 1600"; "Manage rules" switches to the Smart Albums tab; clicking a
      card opens it; the palette's Edit Smart Album (`edit_selected_smart_album`,
      `app/album_builder_actions.rs:35`) acts on the focused card.
- [ ] Implement: up to 4 cards on the Albums tab, all on the Smart Albums tab; mosaic from
      the first 4 matches, fetched once per album revision (cached map, not per frame).
- [ ] Run `cargo test -p lumina-ui-gpui smart_album_cards`; expect pass.
- [ ] Gate and commit `feat(design): smart album cards`.

Manual checks (headless tests cannot see them): compare colours, radii, fonts and the stack
plates against `a0sLrG.png` with `make ui-smoke`.
