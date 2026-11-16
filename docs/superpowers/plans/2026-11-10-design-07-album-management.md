# Design 07: Album Management — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Give an open album its own page: a cover header band, an inline selection bar with
album actions, the album grid, and an album settings panel with key photo and Delete Album.
**Design:** `lumina.pen` frame `w4zpqh` "Album Management" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation; Design 05 Albums (overview page for the "Albums"
breadcrumb, `album_cards()` cover fallback); Design 01 Task 5 (grid check circles and the
0.62 dim on unselected tiles in selection mode).
**Icons (fetched in 00 Task 1):** circle-check-big, image-plus, folder-minus, download. Also
used: `hash` (00 fetches it), `folder-plus` (05), `ellipsis` (08); `trash-2`, `play`, `star`,
`chevron-right`, `check` already exist in `resources/icons/`. `user-plus` is not needed (Share dropped).
**App today:** an open album is the browser grid filtered to the album
(`app/render/mod.rs:726-737`), titled by the global toolbar (`app/render/mod.rs:488-490`,
`app/render/toolbar.rs:162-175`). Selection actions are the floating dock
(`app/render/overlays.rs:18-150`: Favorite, Tag, Export…, Share…, Trash, Clear). Album
actions live in `app/albums.rs` (rename 155-213, delete prompt 223-264, pin 266-316) and
`app/album_membership.rs` (picker 34-75, remove from album 151). Delete confirmation:
`views/album_delete.rs:10-30` via `views/smart_album_delete.rs:37` `confirm_dialog`. The
picker: `views/album_picker.rs:14-143`. Palette entries: `app/command_palette.rs:789-833`.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "Lisbon, June", "284 items", "DSCF4612.RAF"):
  render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Header band | none; toolbar title only (`toolbar.rs:162-175`) | 180 h, pad 20 28, cover photo behind a scrim (dark gradient) | new; gradient is 00's `header_fade` |
| Breadcrumb | none | "Albums" #FFFFFF99 Inter 12, `chevron-right` 12, current #FFFFFF 12 | new; "Albums" opens the Design 05 overview |
| Header actions | Slideshow via palette/⌘⌥S (`app/slideshow.rs:71`), no buttons | ghost buttons (transparent, r8, h32, pad 0 11, gap 7, icon 13, Inter 12 t1): "Slideshow" `play`, "Share" `user-plus`, More `ellipsis` 32×32 | new; Share see decisions |
| Title | 15px semibold toolbar title | Inter Tight 30/600, ls -0.6, white | new |
| Meta | none | mono 10.5 #FFFFFFB3 "284 items · 212 photos, 72 videos · 14 — 18 Jun 2025"; avatars 22 (overlap -8); "Shared with 3 people" Inter 11 | new counts line; drop avatars and "Shared with" |
| Selection bar | floating dock at the grid bottom (`overlays.rs:18-150`) | band under header, h55.5, pad 0 28, bg #17120B, border-bottom #3A2E1A | relayout (album view); `accent_surface` fill, `warn_border` border |
| Bar left | "N selected" 600 xs (`overlays.rs:47-53`) | `circle-check-big` 16 accent, "12 selected" 13/500 t1, 1×18 line-2 divider, "Select all 284" accent 11.5, "Clear" t3 11.5 | restyle + new Select all |
| Bar buttons | Favorite / Tag / Export… / Share… / trash icon (`overlays.rs:79-121`) | bg-2 r8 h32 pad 0 11 gap 7, icon 13, Inter 12 t1: "Add to Album" `folder-plus`, "Tag" `hash`, "Set as Cover" `image-plus`, "Download" `download`; 1×20 divider; "Remove" ghost `folder-minus`; "Delete" danger-soft bg, danger text, `trash-2`, 1 px `danger_outline` | relayout + copy |
| Grid | browser grid, 2px amber ring on selected (`app/render/grid.rs:611-632`) | 5 cols, cells 187 h r5 gap 10, pad 22 24 0 28; selected: 22 px accent circle with `check` 13 top-left; unselected (while a selection exists): #00000059 circle and 0.62 dim | owned by Design 01 Task 5 (check circles, 0.62 dim); this plan only lays out the album grid |
| Key photo badge | none | on the cover item: bg #000000B3 r4 h19 pad 0 7, `star` 9 + "KEY PHOTO" mono 8.5 accent ls .4 | new (needs cover) |
| Settings panel | none (inspector column is item metadata, `app/render/mod.rs:766-773`) | 319.5 w, bg-1, border-left line, pad 20 18, gap 20 | new |
| Section labels | `theme::section_label` | Inter 9.5/600 ls 1 t3; action links accent 11 | restyle |
| KEY PHOTO | none | head + "Change"; thumb 76×50 r5; name mono 11 t1; sub "Used on album covers and shares" 10.5 t3 | new (needs cover) |
| SORT ORDER | global sort menu (`toolbar.rs:236-244`) | segmented bg-0 r8 h32 pad 3 gap 2; active bg-3 r6 h26 label 11.5/500 t1; others t3: Custom / Date / Title; hint "Drag items in the grid to reorder." | see decisions |
| SHARING, PEOPLE WITH ACCESS | none | toggles 34×20, link field, Invite, member roles | drop |
| Divider + Delete Album | sidebar hover × (`views/sidebar/albums.rs:198-205`), palette Delete Album | 1 px line; full-width danger-soft r8 h34, 1 px `danger_outline`, `trash-2` 13, "Delete Album" 12/500 danger; hint "The 284 photos stay in your library." 10.5 t3 | new button; opens the existing confirm |

## Not in the app yet (needs a product decision)
- **Key photo / Set as Cover.** Needs `albums.cover_media_id INTEGER NULL REFERENCES
  media_items(id) ON DELETE SET NULL`, a migration and `user_version` bump, the column in
  `ALBUM_COLUMNS`/`map_album` (`catalog/curated.rs:30-37`), undo, and the Design 05
  `album_cards()` preferring it. Size M, core work. Recommend: build in this plan (Tasks 4-5).
- **Custom sort order with drag to reorder.** Members follow the view sort; no position
  column (`catalog/schema.rs:115-117`). Size L (position column, drag in the virtual grid,
  keyboard reorder). Recommend separate plan; until then drop the SORT ORDER section (the
  global sort menu covers Date/Title) rather than ship a control that does nothing.
- **Sharing (Share button, Shared album, link, uploads, comments, people with access,
  avatars).** Needs a server; conflicts with AGENTS §1 and §5.4. Recommend drop from design.
  The header "Share" may instead open the macOS share sheet for the selection
  (`share_selection`, used by the dock at `overlays.rs:228-243`); recommend drop the header
  button and keep Share… in the More menu (macOS only).
- **"Download".** Files are local; the existing action is Export… (`export_pick_folder`,
  `overlays.rs:101-106`). Recommend copy "Export…", not "Download".
- **Selection bar for every view or only albums.** The frame shows an inline bar; other
  frames may keep the dock. Recommend: album view only in this plan; the Library plan
  decides whether it becomes global. Favorite leaves the bar but keeps its key (F) and
  palette entry.
- **Selection bar colours** #17120B / #3A2E1A: Foundation tokens `accent_surface` (fill)
  and `warn_border` (border).
- **Header scrim / white text** (#FFFFFF, #FFFFFF99, #FFFFFFB3): the gradient is 00's
  `header_fade`; text uses Foundation's media-overlay colours (the viewer already draws
  text on photos).

## App has, design lacks
- Floating dock actions Favorite and Share… (`overlays.rs:79-110`): keep as keys/palette
  and in the More menu.
- Pin / Unpin album (`app/albums.rs:288`), Rename (`albums.rs:155`), New Album with
  Selection: put in the More (…) menu; keep palette entries.
- Item inspector (I): keep; while Info is on it takes the right column, the album panel
  returns when Info is off.
- Year rail and status bar in the album grid: keep (Library plan owns them).

## Review Focus
- Empty album: header shows "0 items", no date range; grid shows the existing album empty
  state (`app/render/grid.rs:845-849`); panel's KEY PHOTO says "No key photo" and Change is
  disabled.
- Long album title: 30 px title truncates in one line; breadcrumb current truncates first.
- Key photo item trashed or purged: cover falls back to the newest member (ON DELETE SET
  NULL for purge; trashed covers are skipped by the query).
- "Select all 284" with a filtered/searched album selects what the grid shows, and the
  label uses that count; Clear and Esc both clear.
- Narrow window (<1100 px): the settings panel collapses (toggle in More / palette) so the
  grid keeps at least 3 columns; the bar's buttons collapse to icons with tooltips.

## Tasks
### Task 1: Album header band
**Files:** Create `views/album_page/header.rs` (new module), modify `app/render/mod.rs:726-737`;
core `crates/lumina-core/src/catalog/curated.rs` (`album_stats(id)`: photos, videos, min/max
capture time in one query) / Test `crates/lumina-ui-gpui/src/tests/album_page.rs` (new),
`crates/lumina-core/tests/core_tests.rs`
- [ ] Write failing tests `album_stats_counts_kinds_and_range` (core) and
      `album_header_shows_title_counts_and_actions`: `album-header` height 180,
      `album-header-title` = album title, `album-header-meta` "N items · P photos, V videos ·
      d — d Mon YYYY", `album-crumb-root` opens the overview, `album-slideshow-btn` starts
      the slideshow.
- [ ] Implement: `album_stats` runs on the background executor
      (`cx.background_executor().spawn`, as `app/smart_album_share.rs:168` does) when the
      album opens or its revision changes, never inside the synchronous
      `refresh_album_counts` (`app/library.rs:1027`) and never per frame; the meta line
      shows the count alone until stats land. Cover image from the texture cache under
      `header_fade`; ghost buttons; More menu (Rename, Pin/Unpin,
      Share… on macOS, Delete Album…) with keyboard navigation and Esc.
- [ ] Run `cargo test -p lumina-ui-gpui album_header` and `cargo test -p lumina-core album_stats`.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): album header band` staging explicit paths.

### Task 2: Inline selection bar in album view
**Files:** `views/album_page/selection_bar.rs` (new), `app/render/overlays.rs:18-25` (no dock
in album view) / Test `tests/album_page.rs`
- [ ] Write failing test `album_selection_bar_actions`: select 2 of 3 members;
      `album-selection-bar` renders and `multi-select-action-dock` does not;
      `album-select-all` selects 3; `album-bar-remove` removes the selection from the album
      (catalog rows) with undo; `album-bar-delete` opens the trash confirmation;
      `album-bar-add` opens `album-picker`; `album-bar-tag` starts tagging.
- [ ] Implement with the exact values in the table; "Export…" calls the existing export;
      add `Icon` variants for circle-check-big, hash, image-plus, folder-minus, download and
      the existing `trash-2.svg` to `Icon::ALL` (SVGs from 00 Task 1; fetch nothing here).
      Fill `accent_surface`, border `warn_border`; Delete has a 1 px `danger_outline`.
- [ ] Run `cargo test -p lumina-ui-gpui album_selection_bar`; expect pass.
- [ ] Gate and commit `feat(design): album selection bar`.

### Task 3: Album settings panel with Delete Album
**Files:** `views/album_page/panel.rs` (new), `app/render/mod.rs:747-773` / Test `tests/album_page.rs`
- [ ] Write failing test `album_panel_delete_opens_confirm`: `album-panel` width 319.5 with
      Info off, gone with Info on; `album-panel-delete` opens `album-delete-dialog`; the hint
      reads "The N items stay in your library. (singular "The 1 item stays…"; "items" because albums can hold videos)" with the real count; confirm deletes and
      returns to the overview.
- [ ] Implement panel shell, section labels, divider, danger button (danger-soft fill,
      danger text, 1 px `danger_outline`),
      hint copy via `i18n.rs`; palette "Show Album Settings" toggles it.
- [ ] Run `cargo test -p lumina-ui-gpui album_panel`; expect pass.
- [ ] Gate and commit `feat(design): album settings panel`.

### Task 4: Key photo schema (core)
**Files:** `crates/lumina-core/src/catalog/schema.rs`, `catalog/curated.rs:17-37`, `lib.rs` /
Test `crates/lumina-core/tests/core_tests.rs`
- [ ] Write failing test `album_cover_set_cleared_on_purge_and_migrated`: migrate an old DB
      (column added, NULL), set cover, purge the media item, cover reads NULL.
- [ ] Implement idempotent migration, `user_version` bump, `set_album_cover(id, Option<MediaId>)`;
      update every album SELECT/mapper (grep `ALBUM_COLUMNS`, `map_album`).
- [ ] Run `cargo test -p lumina-core album_cover`; expect pass.
- [ ] Gate and commit `feat(core): album key photo`.

### Task 5: Set as Cover and KEY PHOTO section
**Files:** `app/albums.rs` (new `set_album_cover_from_selection`, undo action in `app/undo.rs`),
`views/album_page/panel.rs`, card badge in the album grid / Test `tests/album_page.rs`
- [ ] Write failing test `set_as_cover_updates_panel_badge_and_undo`: select one member,
      `album-bar-set-cover` stores it (catalog), `album-panel-key-photo-name` shows its file
      name, `key-photo-badge-<id>` renders on that card, ⌘Z restores the previous cover;
      with 2 selected the button is disabled with a tooltip.
- [ ] Implement; "Change" in the panel enters a pick mode (next clicked or Enter-ed item
      becomes the cover, Esc cancels); palette "Set as Album Cover".
- [ ] Run `cargo test -p lumina-ui-gpui set_as_cover`; expect pass.
- [ ] Gate and commit `feat(design): album key photo`.

Manual checks: `header_fade` legibility over light covers, colours and
radii against `w4zpqh.png` with `make ui-smoke`.

## Decisions (2026-11-12: recommendations accepted)
- Key photo / Set as Cover: build it (Tasks 4-5).
- SORT ORDER section: dropped; custom order with drag is a separate plan.
- Sharing (header Share, Shared album, link, people, avatars): dropped. Share… (macOS
  share sheet for the selection) lives in the More menu.
- "Download" reads "Export…" and calls the existing export.
- The inline selection bar is for the album view only; other views keep the dock.
  Favorite leaves the bar but keeps F and its palette entry.
- Tokens: add `accent_surface` (#17120B) and `warn_border` (#3A2E1A); the design's
  `danger_outline` is the existing `danger_border`; the header scrim is a new
  `header_fade` gradient token if none exists.
- Order: Tasks 1, 2, 3, 4, 5 one at a time (Tasks 1 and 4 both touch `curated.rs`).
