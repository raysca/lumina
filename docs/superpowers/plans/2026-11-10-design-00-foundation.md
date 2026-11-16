# Design 00: Foundation (tokens, fonts, shared widgets, Sidebar) — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Replace the v3 "graphite" palette and IBM Plex with the `lumina.pen` tokens and
fonts, add the three shared widgets (NavItem, Chip, DownloadRow), and rebuild the left
Sidebar to the Library frame, so every screen plan (01–15) can build on them.

**Design:** `lumina.pen` variables (see the token list below); components `S9a9Iv`
"NavItem", `aisTo` "Chip", `agucN` "DownloadRow"; Sidebar = layer `iTB7x` "Sidebar" of
frame `t0zQL` "Library" (export with the pencil MCP to view). Variants were checked in
`a0sLrG`, `g52QWL`, `D119th`, `PwE8h`, `eO3Hw`, `ttuOX`, `DiFtb`, `XKkZe`, `NWldm`.

**Depends on:** nothing. Every other Design plan depends on this one.

**App today:**
- Palette and control helpers: `crates/lumina-ui-gpui/src/theme.rs:29-89` (tokens),
  `:93-145` (controls), `:150-187` (scrim, modal dim, section label, wash, card),
  `:190-191` (`FONT_SANS` / `FONT_MONO` = IBM Plex), `:250-271` (`row`), `:427-511` (tests).
  The file is 511 lines (over the AGENTS.md §5.1 limit) and starts with
  `#![allow(dead_code)]` (`:5`).
- Fonts: `resources/fonts/IBMPlexSans-{Regular,Medium,SemiBold}.otf`,
  `IBMPlexMono-Regular.otf`; embedded with `include_bytes!` in
  `crates/lumina-ui-gpui/src/assets.rs:90-97`, served at `:110-116`, registered in
  `crates/lumina-ui-gpui/src/main.rs:53-61`. Notices: `THIRD_PARTY_NOTICES.md:7-12`,
  `README.md:394`; the notices file is copied into the bundle by
  `scripts/build_release.sh:57` and opened from Preferences › About
  (`views/preferences.rs:362`, test `tests/preferences.rs:140`).
- Icons: Lucide 0.460.0 SVGs vendored in `resources/icons/`, listed in
  `assets.rs:13-59` (`lucide!`), enum in `src/icons.rs:5`.
- Sidebar: `src/views/sidebar/mod.rs:54-87` (container), `sections.rs:12-58` (Library),
  `:61-165` (smart album row), `:168-245` (Smart Albums), `:248-478` (Tags tree),
  `:480-546` (entry + Places map toggle), `:548-621` (`render_nav_item`), `albums.rs:18-71`
  (Albums), `:74-108` (Pinned), `:122-215` (album row), `nav.rs:25-128` (entries),
  `rail.rs:14-97` (collapsed 48 px rail). Default width 220 (`config.rs:461-463`), resizer
  clamp 180–320 (`app/render/mod.rs:649`).
- Root layout: toolbar spans the full window above the sidebar
  (`app/render/mod.rs:668-690`: column → toolbar `:677` → row(sidebar `:687`, resizer
  `:689`, canvas…)). The toolbar reserves an 84 px traffic-light inset
  (`app/render/toolbar.rs:117-122`); traffic lights sit at (18, 19) (`main.rs:74-78`).
- Downloads row: `src/views/downloads_panel.rs:85-218` inside a 320 px popover (`:284-291`). Not edited here: plan 14 owns the popover rows; DownloadRow is a new widget for plan 13's screen.
- Chip-like widgets today: inspector batch tag chips (`views/inspector/batch.rs:335-400`,
  24 h pill) and inspector tag field (`views/inspector/summary.rs:338`). There is no
  filter-chip bar.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- **Nothing is downloaded without the user's explicit permission** (Task 1). Agents must
  stop at Task 1 and ask; they must not fetch font or icon files themselves.
- GPUI 0.3.11 has no letter-spacing API (checked: no `letter_spacing` in `gpui-pre-0.3.11`).
  The design's tracking (section labels +0.9, wordmark −0.3) cannot be reproduced; accept.

## Pen tokens (the target)

| Function | Hex | Function | Hex |
|---|---|---|---|
| `bg_0()` | #0A0A0C | `t1()` | #F2F2F5 |
| `bg_1()` | #121216 | `t2()` | #93939E |
| `bg_2()` | #1A1A20 | `t3()` | #5E5E6A |
| `bg_3()` | #24242C | `accent()` | #E9A23B |
| `line()` | #26262E | `accent_soft()` | #E9A23B26 (rgba) |
| `line_2()` | #35353F | `danger()` | #E8584F |
| `warn()` | #E0A33B | `danger_soft()` | #E8584F1F (rgba) |
| `positive()` | #5BC27A | `positive_soft()` | #5BC27A1F (rgba) |
| `chart_blue()` | #6E9BE8 | `chart_violet()` | #A78BE8 |

### Non-pen colours the frames use (named here so screen plans add no hex literals)

Semantic tokens, one per role; near-identical hexes are folded together. Translucent and
blurred surfaces render opaque (GPUI has no backdrop blur), so they are given as solid.

| Function | Value | Used by (frame: layer) | Replaces |
|---|---|---|---|
| `on_accent()` | #1A1206 | text/icons on an accent fill ("Import", "Days"; 83 uses) | rename of `accent_fg` (8.56:1) |
| `scrim()` | rgba #07070BD9 | modal scrim behind dialogs (`D119th`, `KweSw`: Scrim) | current `scrim()` #000000CC |
| `bg_float()` | #16161C | popovers and HUDs: Downloads popover (`eO3Hw`, #14141AF5 drawn opaque), ZoomControl (#16161CF2 in `t0zQL`, `D119th`, `eO3Hw`), other #16161C surfaces | `bg_hud` |
| `bg_sunken()` | #0C0C10 | recessed surfaces; States frames' root (#0E0E12 in `R5RGv`, `NWldm`, `l85pQN`) folds into it | — |
| `bg_stage()` | #08080A | viewer stage: Detail (`K9cBD`) #08080A, Video Player (`F6sXRs`) #060608 folds into it | `bg_viewer` |
| `bg_media()` | #000000 | letterbox behind media (kept) | — |
| `media_scrim()` | rgba #000000A6 | badges/buttons over media: Duration, Badge, PiP `x`/`maximize-2` (`t0zQL`, `DoXZy`, `K9cBD`, `F6sXRs`, `PwE8h`; #000000B3/#000000B8 fold into it); `w4zpqh` Check #00000059 stays a one-off `.opacity` | grid/marks `rgba(0x00000099)` literals |
| `media_fade()` | rgba #000000CC → transparent | edge gradients over the stage (`K9cBD`/`F6sXRs`: 90° #000000CC, −90° #000000E6) via `linear_gradient` | — |
| `on_media()` | #FFFFFF | text/icons over media and scrims | grid `rgb(0xffffff)` literals |
| `danger_surface()` | #1E100F | error panels (States — Error) | `danger_bg` where it is a panel |
| `danger_border()` | #45211E | border of a danger panel | old `danger_border` (same name, new value) |
| `on_danger()` | #F0C4C0 | text on `danger_surface` (11.76:1) | — |
| `warn_surface()` | #1C1708 | warning panels (States — Error) | — |
| `warn_border()` | #3A2E1A | border of a warn panel | — |
| `on_warn()` | #E8D7B0 | text on `warn_surface` (12.57:1) | — |
| `accent_surface()` | #17120B | selection bar (accent-tinted bar behind a multi-selection) | — |
| `plate()` / `plate_2()` | #2A2A33 / #3A3A46 | album stack back plates (`a0sLrG`, `w4zpqh`: Back2/Back1) | — |
| `accent_outline()` | rgba #E9A23B59 | accent-tinted 1 px outlines (selected cards, accent-marked rows) | — |
| `danger_outline()` | rgba #E8584F4D | 1 px outline of a failed card / danger row | — |
| `header_fade()` | gradient #07070AF7 → #07070A8C | page header over scrolling content (`linear_gradient`, top to bottom) | — |
| `chart_grey()` | #4E4E5C | neutral segment of usage bars (Settings "Cache", `g52QWL`) | — |

`danger` and `warn` on their surfaces stay readable (5.23:1 and 8.06:1). Also seen but left to
screen plans because the role is unclear: #32323A (59 small layers in `t0zQL`, `D119th`,
`eO3Hw`, likely timeline-rail ticks: plan 01 names it or uses `line_2`), #E8E8EFB3
(`K9cBD` b2…b7, likely histogram bars: plan 02), #FFFFFF2E (`F6sXRs` w53…, waveform: plan 04).

How the frames use them: bg-0 main canvas and inset fields (sidebar search), bg-1 sidebar /
cards / panels, bg-2 chips, buttons, badges, bg-3 selected row / selected segment / tracks,
line 1 px outlines, line-2 stronger outlines ("Add filter" chip), t1 primary text,
t2 idle row labels and icons, t3 counts, placeholders, section labels, meta lines.

## Palette mapping (every existing token)

Call-site counts are `grep -rE '(^|[^a-z_.])<token>\(\)'` over `crates/lumina-ui-gpui/src`
excluding `theme.rs` (sites / files); treat them as approximate. About 1,300 `theme::`
references in 60 files in total.

| Old token (hex) | Sites | New | Notes |
|---|---|---|---|
| `bg_media` #000000 | 10/7 | **keep** | Media letterbox black; pen has none. |
| `bg_viewer` #141517 | 11/8 | `bg_stage` / `bg_0` | Viewer stage → `bg_stage`; root background (`render/mod.rs:670`) → `bg_0`. |
| `bg_app` #1C1D20 | 31/23 | `bg_0` | **Exception:** idle sidebar row fill (`sections.rs:87,393,562`, `albums.rs:160`) becomes transparent, else rows paint bg-0 stripes on the bg-1 sidebar. Rail (`rail.rs:21`) → `bg_1`. |
| `bg_pane` #202125 | 25/21 | `bg_1` | Sidebar, inspector, panels. |
| `bg_dialog` #232428 | 11/8 | `bg_1` | Dialog cards (DownloadRow card is bg-1 too). |
| `bg_raised` #26272B | 30/19 | `bg_2` | Raised surfaces and the row hover (5 `s.bg(` sites). |
| `bg_hud` #2A2C30 | 18/11 | `bg_float` | Menus, popovers, tooltip, HUD. Floating shadow stays; border `line`. |
| `bg_control` #2C2E32 | 41/25 | `bg_2` | Buttons, fields, chips. |
| `bg_control_hover` #34363B | 52/33 | `bg_3` | |
| `border_subtle` #2E3034 | 48/26 | `line` | |
| `border` #36383D | 39/27 | `line` | Pen buttons, chips, fields all outline with `line`. |
| `border_strong` #44464C | 37/28 | `line_2` | |
| `focus_ring` #AAAAA8 | 25/18 | **keep** (decision D2) | Pen has no focus colour; `XKkZe` draws a focused search with an accent outline. |
| `text` #E7E6E3 | 155/56 | `t1` | |
| `text_muted` #A0A2A7 | 95/39 | `t2` | |
| `text_subtle` #909298 | 93/37 | `t2` | Not `t3`: these are hints, counts and secondary values that must stay readable (D1). Screen plans opt into `t3` per element where the frame uses it and the text is non-essential. |
| `text_disabled` #5A5D63 | 7/4 | `t3` | Disabled text is exempt from contrast rules. |
| `text_row` #C9CACD | 6/4 | `t2` | Idle row label (design). |
| `text_label` #8E9096 | 2/2 | `t3` | Section labels (design); see D1. |
| `accent` #F0B54B | 64/30 | `accent` | New value #E9A23B. |
| `accent_hover` #F4C46A | 6/6 | **keep, retune** to #ECB058 (accent + 15 % white) | Pen has no hover. |
| `accent_fg` #1A1206 | 33/19 | `on_accent` (rename) | The frames use exactly #1A1206 on accent ("Import", "Days"). 8.56:1. |
| `selection_bg` #3A3426 | 26/16 | `bg_3` | Design selection is neutral: bg-3 + t1 label + accent icon. |
| `selection_bg_hover` #453D2B | 11/7 | `bg_3` | A selected row does not change on hover; delete the token. |
| `selection_border` #6B5A33 | 6/5 | none / `line_2` | Selected segments have no border in the frames; drop the border where it only marked selection (`control_on`, `segmented_item`), use `line_2` where a border is structural. |
| `selection_fg` #F5DDA8 | 18/11 | `t1` | |
| `selection_count` #C9AE76 | 4/2 | `t3` | Count in a selected row stays t3 in the frames (2.41:1 on bg-3, D1). |
| `accent_hint` #5A4214 | 1/1 | **keep** | Shortcut hint on an accent menu row (`context_menu.rs:474`). |
| `rating` #F0B54B | 3/3 | **keep, retune** to #E9A23B | It was the accent value; keep them equal. |
| `rating_unset` #77797F | 3/3 | **keep** | 3.54:1 on bg-3, passes 3:1. |
| `flag` #F28C38 | 0 | **delete** | Unused. |
| `label_red` … `label_purple` | 4–6 each | **keep** | Colour labels; pen has none. Note `label_purple` is 2.97:1 on bg-3 (dot only, fine). |
| `danger` #C4343A | 6/3 | `danger` | New value #E8584F. **Text on a danger fill** (`smart_album_delete.rs:131`, `delete_modal.rs:148`): t1 on #E8584F is 3.16:1 — use `bg_0` (5.6:1) unless the dialog plan's frame says otherwise. |
| `danger_text` #F08A7A | 20/17 | `danger` | 5.29:1 on bg-1, 4.36:1 on bg-3 (below 4.5 on bg-3 only). |
| `danger_bg` #3A2224 | 8/7 | `danger_soft` | |
| `danger_bg_hover` #4A2A2C | 2/2 | **keep, retune** to rgba #E8584F33 | |
| `danger_border` #7A3A3C | 7/6 | `danger_border` (#45211E) | Same name, error-frame value. Destructive-button outlines that must read as red use `danger`. |
| `success` / `success_bg` | 0 | **delete** | Unused. |
| `success_text` #7FCDA8 | 3/2 | `positive` | |
| `scrim()` / `modal_dim()` | 7/5, 14/13 | `scrim` new value #07070BD9 / keep `modal_dim` | Overlay scrims over media use `media_scrim`. |
| `selection_wash()` #F0B54B1F | 2/2 | `accent_soft` | 15 % instead of 12 %. |
| `card_surface()` | 4/2 | **keep** | Transparent. |
| — | — | `warn`, `positive_soft`, `chart_blue`, `chart_violet` | New, no caller in Foundation (Settings storage bar and states plans use them). |

Hex literals outside `theme.rs` (21, not tokens): `app/render/rail.rs:23` (#222326 →
`line`), white-on-scrim badge text in `app/render/grid.rs:522-603,1106-1111` and
`views/marks.rs:170` (→ `media_scrim` / `on_media`), `views/viewer/transport.rs:312` (#F0B54B38 →
`accent_soft`), transparent borders (`toolbar.rs:13`, `inspector/body.rs:521`,
`album_builder/rows.rs:209`, `rail.rs:44`; keep). The coloured and media ones change here; transparent borders stay.

### Contrast (WCAG ratios computed for the pen values)

| Text | bg-0 | bg-1 | bg-2 | bg-3 |
|---|---|---|---|---|
| t1 | 17.70 | 16.72 | 15.50 | 13.78 |
| t2 | 6.51 | 6.15 | 5.70 | 5.07 |
| **t3** | **3.09** | **2.92** | **2.71** | **2.41** |
| accent | 9.13 | 8.63 | 8.00 | 7.11 |
| danger | 5.60 | 5.29 | 4.90 | 4.36 |
| positive | 8.89 | 8.40 | 7.79 | 6.92 |

t3 fails 4.5:1 (and even 3:1 on bg-1..bg-3). The frames use t3 for essential text in these
places: sidebar section labels (LIBRARY, ALBUMS…), sidebar counts, search placeholder and
the ⌘K hint, "Library size" value, DownloadRow meta line (sizes and speed), "1,284 matches",
grid segment labels ("Grid", "Map"), "Select". The current test `theme.rs:487-510` requires
≥4.0 for `text_subtle`; that is why `text_subtle` maps to t2, not t3.

## Current vs design

| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Palette | graphite tokens, `theme.rs:29-89` | pen tokens above | restyle (mapping table) |
| Sans font | IBM Plex Sans 400/500/600 (`theme.rs:190`, `assets.rs:90-95`) | Inter 400 (604 uses), 500 (188), 600 (129) | restyle |
| Display font | none | Inter Tight 600 (70 uses: wordmark, page titles) | new `FONT_DISPLAY` |
| Mono font | IBM Plex Mono 400 (`theme.rs:191`) | JetBrains Mono 400 (614 uses) | restyle |
| Bold | `FontWeight::BOLD` at 31 sites, no Bold face bundled (synthesised) | no 700 anywhere | BOLD → SEMIBOLD |
| Shell | full-width 52 px toolbar above sidebar and canvas (`render/mod.rs:668-690`, `toolbar.rs:113-122`) | sidebar runs the full window height; the main column beside it has its own 67.5 px page header | relayout (Foundation owns the shell; plan 01 owns the header contents, `page_header`, reused by plans 03, 04, 11) |
| Sidebar container | `bg_pane`, border `border_subtle`, `py 4 px 8`, width 220 default, under the toolbar (`sidebar/mod.rs:60-72`, `render/mod.rs:677-687`) | full window height, bg-1, right border 1 line, padding 16/12, groups gap 18, width 252 (content 228) | relayout |
| Traffic lights | in the toolbar's 84 px inset (`toolbar.rs:117-122`) | not drawn; brand row at top 16 | relayout: the brand row reserves the lights (84 px left inset on macOS, brand text starts after it, row is a window drag area) |
| Brand row | none | `aperture` 19 accent + "Lumina" Inter Tight 18/600 t1, gap 9, padding 0 2 | new |
| Search field | toolbar, 300×28 (`views/search_bar.rs:19-40`) | sidebar, 228×34, bg-0, outline line, r8, padding 0 10; `search` 14 t3, gap 8, placeholder Inter 12.5 t3 "Search library", hint mono 10 t3 — **"⌘F"**, not the frame's "⌘K" (D3); focused (`XKkZe`): bg-3, accent outline, text t1 | new slot: Foundation renders an empty 34 px slot between brand and sections (`sidebar-search-slot`); **plan 03 Task 1 fills it** with the field |
| Section label | 11/600 `text_label`, pt 16 pb 4, px 8 (`theme.rs:165-171`, `sections.rs:20-27`) | Inter 10/600 t3, uppercase, then 5 px spacer; rows gap 3 | restyle |
| Section header buttons | folder "switch-library" (`sections.rs:28-50`), `+` new album (`albums.rs:42-60`), `+` new smart album (`sections.rs:190-208`) | none in headers | keep folder and smart `+` (App has, design lacks); Albums `+` → "New Album" row |
| Nav row (NavItem) | 26 h, px 8, gap 8, r6, idle fill `bg_app`, label 13 `text_row`, active `selection_bg` + `selection_fg` + 500, count `text_xs` mono `text_subtle` (`sections.rs:548-621`) | 32 h, padding 0 10, r8, transparent idle; icon 16 t2, gap 10; label Inter 13/400 t2; count mono 10 t3, right; active bg-3, label t1/500, icon accent; "attention" count accent (`D119th` Downloads "3", `PwE8h` Duplicates "128") | restyle → `theme::nav_item` |
| Count format | `count.to_string()` (`sections.rs:616`; also `:119` smart rows) | grouped "48,291" | new `i18n::count` |
| Library rows | All Photos & Videos, Favorites, Videos, Downloaded*, Screenshots, Places, Duplicates, Missing*, Recently Deleted (`nav.rs:44-120`) | All Media `images`, Photos `image`, Videos `video`, Favourites `heart`, Recently Added `clock` (+ Downloads / Duplicates in some frames) | copy + see "Not in the app yet" |
| Album rows | `Icon::Image` 16, 26 h, hover rename/delete (`albums.rs:142-213`) | `folder` 16, NavItem | restyle |
| New Album | header `+` (`albums.rs:42-60`) | last row: `plus` 16 t2 + "New Album" t2, no count | relayout (keep selector `new-album`) |
| Smart album rows | sparkles 16, 26 h, hover edit/delete (`sections.rs:61-165`) | `sparkles` 16, NavItem | restyle |
| Untagged | built-in row in Smart Albums (entry `nav.rs:117-123`, rendered `sections.rs:238-244`) | absent | keep |
| Tags section | keyword tree with colour dots, rename on double-click (`sections.rs:248-478`) | three rows: Keywords `hash`, People `users`, Places `map-pin` with counts | see "Not in the app yet" (D4) |
| Places row | Library section with Map toggle (`sections.rs:480-546`) | under TAGS, no toggle | relayout; the map toggle is removed by plan 01 Task 3 (header Map segment), not kept here |
| Library size meter | none | bottom (push spacer): "Library size" Inter 11 t2, value mono 10 t3 "412 GB / 1 TB", track 4 h bg-3 r999, fill accent; gap 7, padding 0 2 | see "Not in the app yet" |
| Pinned section | `albums.rs:74-108` | absent | keep |
| Collapsed rail | 48 px (`rail.rs`) | absent | keep, restyle tokens only |
| Chip | none shared; batch chip 24 h pill (`batch.rs:343-361`) | 28 h, padding 0 11, gap 6, r999, bg-2, outline line; icon 13 t2; label Inter 12/400 t1. Add variant: transparent, outline line-2, `plus`, label t3. Selected variant (Design 13): accent-soft fill, accent outline, t1. Ghost: transparent, outline line, t2 | new `theme::chip` |
| DownloadRow (new widget; panel rows unchanged here) | 1-line title `text_xs`, status line, 3 px bar, text buttons "Retry", "Resume", "✕" (`downloads_panel.rs:85-218`) | card bg-1 r12 outline line, padding 14/16, gap 14; thumb 72×46 r7; mid column gap 9: title Inter 13.5/500 t1 + status mono 11 accent right ("39% · 19s left"), track 4 h bg-3 r999 fill accent, meta mono 11 t3 ("host · 148.2 MB of 384.0 MB · 12.4 MB/s"); right gap 8: format badge (h22 r6 bg-2 outline line, mono 10.5 t2 "2160p MP4"), 30×30 r8 bg-2 outline line icon buttons (`pause` t1, `x` t2) | new widget |
| Radii | 4 / 6 / 8 (`theme.rs:16-21`) | rows and buttons 8, segments 6, cards 12, chips and tracks 999 | NavItem and buttons use `RADIUS_LG`; add `RADIUS_XL = 12` for cards |

### Sidebar variants across frames
- **Identical** (brand, search, sections, meter): `t0zQL`, `a0sLrG`, `L3F8Cs`, `KweSw`,
  `w4zpqh`, `DoXZy`. Only the active row differs (Golden Hour, Keywords, Lisbon June,
  Videos…). `a0sLrG` and `t0zQL` differ in nothing else.
- **Extra Library row**: `D119th`, `eO3Hw`, `ttuOX` add "Downloads" after Recently Added with
  an accent count (unfinished downloads); `PwE8h` adds "Duplicates" with an accent count.
  NavItem's attention count covers both. **Downloads row:** Foundation adds a `downloads`
  nav entry after the Library rows (icon `arrow-down-to-line`, attention count = unfinished
  downloads), shown while the download queue has rows; until plan 13 adds
  `AlbumSelection::Downloads` and the screen, clicking it opens the existing downloads panel
  (`downloads-toolbar-btn` action). Plan 13 rewires it to the screen and marks it active.
  The conditional "Downloaded" album stays (it is the finished items). Duplicates already
  exists as a row; its count becomes the attention style.
- **Search states**: `XKkZe` focused with a query (bg-3, accent outline, t1 text);
  `DiFtb` (empty library) search at 50 % opacity with "Nothing to search yet", rows at 45 %
  opacity, no counts, sections without items. Owned by Design 03 and Design 09.
- **Settings sidebar** (`g52QWL`, `NWldm`): same shell (brand, search "Search settings",
  meter) with one SETTINGS section: General, Library, Import, Downloads, Sync & Backup,
  Appearance, Shortcuts, Privacy, Advanced. Owned by the Settings plan; it reuses
  `theme::nav_item` and the sidebar shell. Note `g52QWL` has a copy bug: the shortcut slot
  repeats "Search settings".

## Not in the app yet (needs a product decision)

1. **Library size meter** (bottom of every sidebar). Needs the catalog's total file size
   (a `SUM(file_size)` over active media, S, core) and the volume's capacity (statfs on the
   library root, S–M, core; no `libc` dependency today). Must refresh off the UI thread after
   scans and deletes. Also unclear: is "1 TB" the disk size or a user quota? **Recommend:
   separate small plan** (it also feeds the Settings "Library storage" card in `g52QWL`).
   Foundation leaves the push spacer and no meter.
2. **"Photos" row** (images only). No `SystemAlbum::Photos` (`smart_albums/system.rs:5-19`);
   `media_type` exists, so it is an S core change (new system album + count). **Recommend:
   build in this plan's Task 5** only if approved; otherwise omit the row.
3. **"Recently Added" row**. `media_items.added_epoch_ms` already exists
   (`lumina-core/src/catalog/schema.rs:77`, migration `:355`; read in `catalog/files.rs:95`,
   `media.rs:86`, `InfoDetails.added_epoch_ms`). A new system album with an
   `added_epoch_ms >= now − 30 days` rule plus its count is size **S, no migration**.
   **Recommend: build in Task 5** (needs the `clock` icon).
4. **TAGS as Keywords / People / Places** (D4). Keywords opens the Keywords screen
   (`L3F8Cs`, its own plan); People needs face recognition, which does not exist (AGENTS.md
   §15: no AI features), L. **Recommend:** drop People from the design; keep today's keyword
   tree under TAGS until the Keywords screen lands, then replace it with the Keywords row;
   move Places under TAGS now.
5. **DownloadRow pause button and format badge.** A running download cannot be paused
   (`Paused` means "interrupted by quitting", `catalog/downloads.rs:15-16`), and no format
   string is stored. **Recommend:** Foundation's widget accepts optional badge and any action
   buttons; Design 13 decides Pause (it already plans it). Thumbnails: use the media
   thumbnail once `media_id` is set, a bg-2 placeholder with `film` before; do not fetch a
   remote thumbnail.

## App has, design lacks
- **Open Library folder button** in the LIBRARY header (⌘O): keep.
- **New Smart Album `+`** in the SMART ALBUMS header (⌥⌘N): keep (only create affordance).
- **Screenshots, Recently Deleted (Trash), Missing, Untagged, Duplicates** rows: keep all.
  Trash is the only path to restore; Missing is the fix-up path.
- **Pinned** section and **collapsed rail** (⌃⌘S): keep; rail only gets new tokens.
- **Row hover actions** (rename / delete / edit rules) and the tag rename field: keep, as
  trailing children of `nav_item`, still hover-revealed.
- **Sidebar toggle button** in the header: keep (Design 01 places it).
- **Neutral focus ring**: keep (D2).

## Decisions flagged (for the lead / user)
- **D1 t3 contrast.** Follow the frames (t3 #5E5E6A on essential text, 2.4–3.1:1) or lift
  t3 to #7C7C88 (4.53:1 on bg-1, 4.80 on bg-0, still 3.74 on bg-3). **Recommend lifting t3
  to #7C7C88** and keeping `text_subtle` → t2; otherwise section labels and counts fall below
  WCAG AA.
- **D2 focus.** `XKkZe` shows focus as an accent outline; the app's rule (`theme.rs:1-4`,
  test `:470-485`) is "amber means selection, focus is neutral". **Recommend keeping
  `focus_ring`** (8.0:1 on bg-1).
- **D3 ⌘K hint on search.** Settled with plan 03: ⌘K is the command palette
  (`app/keymap/mod.rs:161`) and ⌘F focuses search (`:241-246`), so the sidebar field shows "⌘F".
- **D4 TAGS section**, see "Not in the app yet" 4.
- **D5 copy.** "All Media" (design) vs "All Photos & Videos"; "Favourites" vs "Favorites".
  en-US is the only locale (AGENTS.md §11): **recommend "All Media" and "Favorites"**.
- **D6 existing users' width.** Default becomes 252; a saved `sidebar_width` (e.g. 220) is
  kept. Recommend not migrating saved widths.

## Review Focus
1. **Traffic lights over the brand row** (macOS): with the sidebar full height the lights at
   (18, 19) sit in the brand row. Expected: the brand row keeps an 84 px left inset on macOS
   (12 px elsewhere), the mark and wordmark start after the lights, and the row drags the
   window. With the sidebar collapsed or hidden, the main header takes the 84 px inset instead.
   Verify by `make ui-smoke` screenshot.
2. **Long album / smart album names** at 252 px: label truncates with an ellipsis, the count
   and hover actions never wrap or overflow.
3. **Huge counts** ("1,234,567"): mono 10 count stays right-aligned, label truncates first.
4. **Narrow window / sidebar resized to 180**: rows, search slot and brand stay inside;
   nothing overlaps the right border.
5. **Keyboard focus**: sidebar rows keep their `Role::Button` + aria labels; focus ring
   visible on the new bg-1; ⌃⌘S, ⌘O, ⌥⌘N and palette entries unchanged.

## Tasks

### Task 1: Ask permission, then fetch fonts and icons (user step)
**Files:** `resources/fonts/`, `resources/icons/`
- [ ] **Stop and ask the user** for explicit permission to download (state names, sources,
      sizes; do not proceed without a clear yes):
      - Inter 4.1 static OTF: `Inter-Regular`, `Inter-Medium`, `Inter-SemiBold` (+ `LICENSE.txt`)
        from the `rsms/inter` GitHub release `v4.1` (zip ≈ 25 MB; the three files ≈ 300 KB each).
      - Inter Tight SemiBold static TTF (+ `OFL.txt`) from `google/fonts` `ofl/intertight`
        (≈ 300 KB).
      - JetBrains Mono 2.304 `JetBrainsMono-Regular` TTF (+ `OFL.txt`) from the
        `JetBrains/JetBrainsMono` GitHub release `v2.304` (zip ≈ 5 MB; file ≈ 270 KB).
      - **52 Lucide icons** (ISC licence), pinned to `lucide-static@0.460.0`, the version
        already vendored. Fetch one file per icon, `icons/<name>.svg`, from the npm package
        (e.g. `https://cdn.jsdelivr.net/npm/lucide-static@0.460.0/icons/<name>.svg`), not the
        whole package; ≈ 0.45 KB each, ≈ 25 KB total. This is the deduplicated union of every
        plan's "Icons (fetched in 00 Task 1)" line plus 00's own, minus what is already in
        `resources/icons/`:
        aperture, arrow-down, arrow-right, arrow-up, arrow-up-narrow-wide, audio-lines,
        calendar, check-check, chevron-down, circle-alert, circle-check, circle-check-big,
        clock, combine, corner-down-right, download, ellipsis, equal, eraser, file-image,
        folder-down, folder-minus, folder-open, folder-plus, folder-sync, git-compare-arrows,
        grid-3x3, hard-drive, hash, image-plus, import, layers, layout-dashboard, library,
        link, link-2, list-ordered, map, merge, panel-right, refresh-cw, search-x, settings-2,
        share-2, shield-check, sliders-horizontal, unplug, video, wand-sparkles, wrench,
        zoom-in, zoom-out.
        Who needs them: 00 (aperture, video, hash, clock), 01, 02, 03, 04, 05, 06, 07, 08, 09,
        10, 11, 12, 13, 14, 15, 16, 17, 18 (see each plan's Icons line). `users` is not
        fetched (People dropped, decision 4). If a name is missing from 0.460.0, stop and
        report it rather than taking a newer version.
      - Total download, one approval: three font sources above (≈ 30 MB of archives, ≈ 1.4 MB
        of files kept) plus ≈ 25 KB of icons.
      - Notices: the icons are covered by the existing Lucide ISC entry in
        `THIRD_PARTY_NOTICES.md` (it already names `lucide-static` 0.460.0) and
        `resources/icons/LICENSE`; no new entry, but keep the version unchanged. The fonts get
        new OFL entries in Task 2.
- [ ] After approval: put each archive in its own empty directory, copy only the listed
      files into `resources/fonts/` and `resources/icons/`, record version and SHA-256 of each
      font file for the notices. Icons stay unregistered until a plan adds them to `lucide!`
      (`assets.rs:13-59`) and `Icon::ALL` with its first caller (`assets.rs:5-7` embeds only icons
      `Icon` uses; `icons.rs:170` checks each `Icon::ALL` entry resolves), so each plan
      registers its own icons with their first caller. Delete `IBMPlex*.otf` in Task 2, not here.

**Done 2026-11-11 (user approved).** 52 icons from `lucide-static@0.460.0` (jsdelivr, one file
each, all present) are in `resources/icons/`, not registered yet. The fonts in `resources/fonts/`, about 2.5 MB kept
(the Inter OTFs are about 620 KB each, not 300 KB):

| File | Source | SHA-256 |
|---|---|---|
| Inter-Regular.otf | rsms/inter v4.1 `Inter-4.1.zip` `extras/otf/` | d4f2b9e148059a15f014cb0f0b8fea8cd11bfa447dd483bedf1b0adc0e2ba799 |
| Inter-Medium.otf | same | 6c222ed2c3660524dd0c14fb745e00fee65c5361ab28f60fba6730bf4e648763 |
| Inter-SemiBold.otf | same | 0a4d30778fa2dc239368d90fd854b88b27e7ede737450da72686a90eca66f02c |
| InterTight-SemiBold.ttf | googlefonts/inter-gf-tight @c194f94c (the commit google/fonts pins; google/fonts carries only the variable file) `fonts/ttf/` | 975a7f22d6e9a45a7989e6d3cbec6ff5b3089cc2272099ac95638b36d4fc948a |
| JetBrainsMono-Regular.ttf | JetBrains/JetBrainsMono v2.304 zip `fonts/ttf/` | a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f |

Licences: `Inter-LICENSE.txt`, `InterTight-OFL.txt`, `JetBrainsMono-OFL.txt` (OFL 1.1) next to them.

### Task 2: Fonts
**Files:** Modify `crates/lumina-ui-gpui/src/theme.rs:190-191`, `src/assets.rs:90-116`,
`src/main.rs:53-61`, `THIRD_PARTY_NOTICES.md:7-12`, `README.md:394`; delete
`resources/fonts/IBMPlex*.otf`; Test `src/assets.rs` (unit test module)
- [ ] Write failing test `bundled_fonts_carry_their_family_names`: for each embedded font,
      bytes start with `OTTO` or `00 01 00 00`, and the name table contains the family
      ("Inter", "Inter Tight", "JetBrains Mono") as UTF-16BE; `FONTS.len() == 5`.
- [ ] Implement: `FONT_SANS = "Inter"`, new `FONT_DISPLAY = "Inter Tight"`,
      `FONT_MONO = "JetBrains Mono"`. One `pub static FONTS: [&[u8]; 5]` in `assets.rs`
      replacing the four `PLEX_*` statics and their `load` arms; `main.rs` registers `FONTS`
      and the log says "bundled fonts". Replace the 31 `FontWeight::BOLD` sites with
      `SEMIBOLD` (no 700 in the design or the bundle). Notices: one section per family with
      copyright line, version, OFL 1.1, and the OFL text; README line 394.
- [ ] Run `cargo test -p lumina-ui-gpui assets`; expect pass. Manual: `make ui-smoke`
      screenshot shows Inter / JetBrains Mono (compare glyphs with `t0zQL.png`).
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): bundle Inter, Inter Tight and JetBrains Mono` staging explicit paths.

### Task 3: Palette swap
**Files:** Modify `crates/lumina-ui-gpui/src/theme.rs` (split into `theme/mod.rs`
tokens + constants, `theme/controls.rs` control/field/menu/row/segmented/badge/kbd/tooltip,
`theme/widgets.rs` for Task 4; `pub use` keeps every `theme::x` path), and the ~60 files with
call sites; Test `theme/mod.rs` tests
- [ ] Write failing tests (replace `focus_ring_is_neutral_and_visible` and
      `text_pairs_meet_aa`): `pen_tokens_have_design_values` (each pen and non-pen function equals its hex);
      `pen_text_pairs_meet_aa`: t1, t2, accent ≥ 4.5 on bg_0..bg_3; danger ≥ 4.5 on bg_0..bg_2;
      `on_accent` on accent ≥ 4.5; `on_danger` / `on_warn` on their surfaces ≥ 4.5; `rating_unset` and `focus_ring` ≥ 3 on bg_0..bg_3;
      t3 is documented as non-essential only (no AA assertion unless D1 lifts it, then ≥ 4.5
      on bg_0 and bg_1).
- [ ] Implement: pen functions in the `tokens!` macro (rgba for the `_soft` ones), kept
      colours listed under a "not in pen" heading, then migrate call sites row by row from the
      mapping table (mechanical rename per row, then the listed exceptions by hand), delete
      the old names; add the non-pen tokens and rename `accent_fg` → `on_accent`. Controls: `control_on` / `segmented_item` active = bg_3 + t1, no amber
      border. `section_label` = 10 px semibold t3. Remove `#![allow(dead_code)]` and fix what
      it hid (AGENTS.md §5.1). Fix the two coloured hex literals.
- [ ] Run `cargo test -p lumina-ui-gpui theme` and `cargo test -p lumina-ui-gpui design_system`;
      expect pass. Manual: compare `make ui-smoke` screenshot with `t0zQL.png` for surfaces.
- [ ] Gate and commit `feat(design): swap the palette to the lumina.pen tokens`.

**Done 2026-11-11, with these departures (Tasks 2 and 3):** `FONT_DISPLAY` and the tokens
with no caller yet (`warn`, `positive_soft`, `chart_*`, `bg_sunken`, `danger_surface`,
`on_danger`, `warn_*`, `accent_surface`, `plate*`, `accent_outline`, `danger_outline`,
`header_fade`, `media_fade`, `chart_grey`) are not added: each arrives with its first caller,
because clippy -D warnings rejects unused items now that the theme's `allow(dead_code)` is gone.
`theme/widgets.rs` waits for Task 4. The unused `badge` helper was deleted. D1 (t3 = #7C7C88)
and D2 (neutral focus) were applied as recommended.

### Task 4: Shared widgets NavItem, Chip, DownloadRow, count format
**Files:** Create `crates/lumina-ui-gpui/src/theme/widgets.rs`; Modify
`src/i18n.rs` (`count`), `src/views/sidebar/sections.rs:540-621` (move `render_nav_item`
out), `src/views/inspector/batch.rs:335-361`; the DownloadRow widget goes in
`theme/widgets.rs` (no edits to `views/downloads_panel.rs`: plan 14 owns the popover rows,
plan 13's screen is DownloadRow's first caller); Test `src/tests/design_system.rs`,
`theme/widgets.rs` unit tests, `src/i18n.rs`
- [ ] Write failing tests: `count_groups_thousands` (0 → "0", 48291 → "48,291",
      1234567 → "1,234,567"); `nav_item_is_32px_with_right_count` (row height 32, count
      bounds right of label, on a seeded library); `chip_is_28px_pill` (batch tag chip height
      28 via `batch-tag-*`); `download_row_lays_out` (row rendered in a test window at
      1292 px: thumb 72×46, track spans the mid column, buttons 30×30 inside the card; a long
      title truncates).
- [ ] Implement `theme::nav_item(id, icon, label, count: NavCount, active)` with
      `NavCount::{None, Plain(usize), Attention(usize)}`: values from the table (32 h,
      padding 0 10, r8, gap 10, icon 16 t2/accent, label 13 t2 → t1/500 active, count mono 10
      t3 / accent, hover bg_2, idle transparent); keeps `Role::Button` + `nav::aria_label`;
      callers append trailing children (hover actions).
- [ ] Implement `theme::chip(id, icon: Option<Icon>, label, ChipKind::{Normal, Selected, Add, Ghost})`
      (28 h, padding 0 11, gap 6, r999, icon 13; Normal bg_2 + line + label t1, Selected
      accent_soft + accent outline + t1, Add transparent + line_2 + `plus` + t3, Ghost transparent + `line` outline + t2). First caller:
      inspector batch chips (partial chips add `.border_dashed()`); keep `batch-*` selectors.
- [ ] Implement `theme::download_row(id, DownloadRowContent { thumb, title, status, tone,
      fraction, meta, badge })` plus trailing action children, with the card values from the
      table; mid column `flex_1 min_w_0`, title truncates; thumb and badge optional; tone sets
      the status/fill colour (running accent, finishing positive, paused t2, failed danger with
      a `danger_outline` card outline). Plan 13 supplies content, actions and selectors (host
      from `canonical_url`, bytes and speed from `NetworkDownloadProgress`).
- [ ] Run `cargo test -p lumina-ui-gpui design_system downloads i18n`; expect pass.
      Manual: compare with `S9a9Iv.png`, `aisTo.png`, `agucN.png`.
- [ ] Gate and commit `feat(design): NavItem, Chip and DownloadRow widgets`.

### Task 5: Shell and Sidebar to the Library frame

**Selector contract (stable after this task; other plans rely on it):**
- Sidebar: `sidebar`, `sidebar-brand`, `sidebar-search-slot` (plan 03 fills it),
  `new-album` (the "New Album" row, replacing the header `+`), and key-based rows only:
  `sidebar-nav-{key}` from `NavEntry.key` (`sidebar-nav-all`, `sidebar-nav-favorites`,
  `sidebar-nav-videos`, `sidebar-nav-recently-added`, `sidebar-nav-downloads`,
  `sidebar-nav-duplicates`, `sidebar-nav-places`, `sidebar-nav-keywords`,
  `sidebar-nav-untagged`, …), icons `sidebar-nav-{key}-icon`. No label-based
  `sidebar-nav-<Label>` selectors remain.
- Header: this task owns the shell (full-height sidebar, main column, traffic-light inset) and
  leaves the main column's header slot to plan 01 Task 1, which builds only the contents,
  `page_header` (`views/page_header.rs`) with `page-header`, `page-title`, `page-meta`,
  `page-actions`, `header-filter`, `header-import`, `sort-menu-button`; the downloads button
  keeps its selector. Until then the slot renders today's toolbar content.

**Files:** Modify `crates/lumina-ui-gpui/src/app/render/mod.rs:668-700` (root split),
`src/app/render/toolbar.rs:113-122` (inset), `src/config.rs:461-463` (default 252),
`src/views/sidebar/{mod.rs:54-87, sections.rs, albums.rs:18-71,122-215, nav.rs:44-120, rail.rs}`,
`src/assets.rs` + `src/icons.rs` (new icons); Test `src/tests/sidebar.rs`,
`src/tests/design_system.rs:66-77`, `src/tests/windows_and_layout.rs`
- [ ] Write failing tests: `sidebar_is_full_height_252` (`sidebar` bounds top 0, height =
      window height, width 252 on a fresh config); `sidebar_brand_row_present`
      (`sidebar-brand` exists above `sidebar-nav-all`); `recently_added_counts_last_30_days`
      (core test: items with `added_epoch_ms` inside / outside the window); `new_album_is_last_album_row`
      (`new-album` below the last `album-row-*`; click creates an album as today);
      `places_row_is_under_tags` (`sidebar-nav-places` below `sidebar-tags-header`);
      `downloads_row_follows_the_queue` (`sidebar-nav-downloads` absent with an empty queue,
      present with an attention count after a queued download).
      Update `sidebar_rows_are_26px` → 32 and rename it.
- [ ] Implement the shell: row(sidebar, resizer, column(error banner, header, canvas…)).
      The column's top bar stays today's toolbar content until plan 01 replaces it with
      `page_header` (67.5 h); keep a single `HEADER_HEIGHT` constant so
      `design_system.rs:8` (sheets hang under the header) follows. The header drops the 84 px
      inset while the sidebar is expanded and keeps it when collapsed or hidden.
- [ ] Implement the sidebar shell: bg_1, right border line, padding 16/12, group gap 18,
      brand row reserving the traffic lights (84 px left inset on macOS, window drag area;
      `aperture` 19 accent + "Lumina" `FONT_DISPLAY` 18/600 t1, gap 9, selector
      `sidebar-brand`), the empty 34 px `sidebar-search-slot` that plan 03 fills, scrolling sections, a flex push spacer at the bottom (meter only if
      approved).
- [ ] Implement sections with `theme::nav_item`: label 10/600 t3 + 5 px spacer + rows gap 3.
      Library rows per D5 copy, `video` icon for Videos, Recently Added (`clock`, new system
      album over `added_epoch_ms`, last 30 days; Photos only if decision 2 approves); Places moves under TAGS; album rows
      use `folder`; "New Album" row replaces the Albums header `+`; Downloads row per the
      variants note (shown while the queue has rows, opens the panel until plan 13); selectors become
      key-based (`sidebar-nav-{key}`, as the rail's `sidebar-rail-{key}` already are) — this
      is a test-contract change: update `tests/sidebar.rs`, `design_system.rs` and any other
      `sidebar-nav-<Label>` users in the same commit. Counts through `i18n::count`.
- [ ] Run `cargo test -p lumina-ui-gpui sidebar design_system windows_and_layout`; expect pass.
- [ ] Manual: `make ui-smoke` screenshot vs `t0zQL.png` sidebar (spacing, colours, fonts,
      traffic lights not over the brand). Visual properties are not asserted headlessly.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`, `make ui-e2e`
      reporting SKIP reasons honestly) and commit `feat(design): sidebar to the Library frame`.

**Done 2026-11-11 (Tasks 4 and 5).** Task 4: `5d4b08b` (`nav_item`, `chip`, `i18n::count`).
Task 5 landed as two commits: `8d47010` (rows: All Media, Recently Added system album and
keyboard path, Downloads row, Places under Tags, New Album row) and `467b85d` (full-height
252px sidebar, brand row beside the traffic lights, search slot for plan 03, shared
`HEADER_HEIGHT`, and a `clamp_center` sub-pixel fix found by the width change). Both reviewed
independently with no blockers; nits fixed and re-checked. ui-e2e interaction was skipped
(no Accessibility permission). Deferred: header-inset test, brand-row window drag off macOS,
full-screen traffic-light inset, brand row accessibility role.
