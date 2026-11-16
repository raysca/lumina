# Design 02: Detail — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle the photo viewer into the design's two-column layout: a full-height stage
(top bar, photo, side arrows, filmstrip) beside a docked 340 px InfoPanel.
**Design:** `lumina.pen` frame `K9cBD` "Detail" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::chip`).
**Icons (fetched in 00 Task 1):** `wand-sparkles`, `share-2`, `panel-right`, `hash` (tag chip glyph). Already in `resources/icons/`: heart, star, chevron-left, chevron-right, plus, maximize-2/minimize-2.
**App today:**
- Viewer shell: `crates/lumina-ui-gpui/src/views/viewer/mod.rs:30-186`. The info drawer floats
  (`mod.rs:162-184`: absolute, top 60, inset 16, rounded, shadow) and the stage reserves its
  width with padding (`mod.rs:104-106`).
- Top bar: `views/viewer/top_bar.rs:41-230` (52 px, Back "Library", date title, "(n of m)",
  Favourite and Info buttons).
- Prev/next and zoom chrome: `views/viewer/stage_controls.rs:155-197` (nav buttons),
  `:16-153` (Fit button and navigator).
- Photo canvas: `views/viewer/viewport.rs:82` onward.
- Docked panel body: `views/inspector/mod.rs:16-58` → `views/inspector/summary.rs:19-181`
  (title, filename, date, stat strip, Rating & Label, Tags, Location text, footer with Size,
  Dimensions, Get Info…, Reveal).
- `views/floating_inspector.rs` is the separate Get Info window (tabbed `inspector/body.rs`).
  The frame does not show it, so this plan does not change it.
- Edit mode bar: `views/viewer/edit_bar.rs:42` (the frame does not show it, so no change).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data ("DSCF4821.RAF", "23 / 84", "Alfama, Lisboa", "Mara"):
  render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views. The frame's
  stage black `#08080A`, the arrow fill `#16161CE6` and the photo shadow `#000000CC` are not pen
  tokens. Use 00's tokens: `theme::bg_stage()` (stage black), `bg_float()` (arrow fill and
  floating chrome), `media_scrim()` (label pills, dark overlays) and `media_fade()`. Never
  `bg_viewer`/`bg_hud` (00 deletes them).
- New user-facing strings go through `src/i18n.rs`.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Layout | Stage is full width. Drawer floats over it at top 60/right 16/bottom 16, 340 wide, rounded, `shadow_2xl` (`viewer/mod.rs:162-184`) | Row: Stage `flex:1` + InfoPanel 340 wide (339.5), full height 1000, bg `bg-1`, border-left 1 `line`, padding 20, gap 22 | relayout |
| Stage background | `bg_viewer()` / `bg_media()` when immersive (`viewer/mod.rs:54-58`); both names go away in 00 | `#08080A` → `theme::bg_stage()` (immersive keeps `bg_media()`, the media black 00 keeps) | restyle (token) |
| Top bar | 52 h, px 16, `bg_app`, border-bottom (`top_bar.rs:103-108`) | 60 h, padding 0 20, no fill, no border, space-between | restyle |
| Traffic-light spacer | 64 px spacer (`top_bar.rs:124`) | none | keep while windowed (macOS lights sit there). Review Focus |
| Back button | Chevron 13 + "Library", `bg_control`, border, radius MD (`top_bar.rs:126-162`) | h 30, padding 0 10 0 6, gap 8, bg `bg-2`, radius 8, no border; chevron-left 15 `t2`; label Inter 12/400 `t2` | restyle + copy (label: see decisions) |
| Title / position | Centre: bold date `viewer-filename` + "(23 of 84)" `viewer-position` (`top_bar.rs:65-97`) | Centre: "23 / 84" only, JetBrains Mono 11 `t3` | relayout + copy; the date moves to the panel's When line |
| Favourite | 28 px button, heart 16 (`top_bar.rs:178-209`) | 30×30, radius 8; active: bg `bg-2` + amber heart | restyle |
| Star | none | 30×30 star button | new (S; see decisions) |
| Edit (wand) | Only the E key, palette "Edit Photo" (`command_palette.rs:1407`) and the context menu | 30×30 `wand-sparkles` button | new button on the existing `enter_edit_mode` (`app/quick_edit.rs:89`) |
| Share | Only the palette `CommandId::Share` (`command_palette.rs:1367`) and the batch dock (`app/render/overlays.rs:228`) | 30×30 `share-2` button | new button on the existing `share_selection` (macOS only, like the dock) |
| Info toggle | 28 px `Icon::Info`, selected bg (`top_bar.rs:210-227`) | 30×30 `panel-right` icon, bg `bg-2` while open | restyle (icon swap) |
| Action gap | 4 (`top_bar.rs:174`) | 6 | restyle |
| Photo | Contain-fit in the stage (`viewport.rs`) | 492×736 in the sample: contain-fit, radius 3, shadow 0 16 48 `#000000CC` | restyle |
| Prev / Next | 40 round, `bg_app`, border `border()`, 40 from the edges (`stage_controls.rs:157-197`) | 40 round, fill `#16161CE6`, 1 px outline `line-2`, 26 from the stage edges, centred on the viewer | restyle |
| Zoom Fit and navigator | Shown only while zoomed (`stage_controls.rs:16-153`) | not shown (frame is at Fit) | keep. Navigator offset: drop the drawer maths (`stage_controls.rs:77-81`), the panel is docked now |
| Filmstrip | none | Bottom band 95.5 h, border-top 1 `line`, padding 0 20, gap 6, centred. Thumbs 58×58 radius 3 with a 55% dark overlay. Current thumb 74×74 with a 2 px `accent` outline, no overlay | new (M; see decisions) |
| Panel: file header | Title 16/600 + mono filename + date (`summary.rs:51-80`) | Row: filename JetBrains Mono 13 `t1` + format badge (h 18, padding 0 6, radius 4, `accent-soft` bg, mono 9.5 `accent`, e.g. "RAW"). Below, gap 5: When line mono 10.5 `t3` "Sat 14 Jun 2025 · 18:42:09 · 48.2 MB" | relayout + copy (date gains seconds; size moves here from the footer) |
| Panel: section titles | `theme::section_label` (`summary.rs:183-190`) | Inter 9.5/600, letter-spacing 1, `t3`, uppercase. Optional action on the right: Inter 11 `accent` | restyle |
| Panel: Histogram | none | Box h 84, bg `bg-0`, radius 5, 1 px outline `line`, padding 8 10. 64 bars 3 wide with gap 1 and radius 1, `#E8E8EFB3`. Clipped ends (first and last 2 bins) in `accent` | new (M; see decisions) |
| Panel: Capture | Stat strip with ƒ, shutter, ISO, mm cells (`summary.rs:195-242`) | Rows 19 h: key Inter 11.5 `t3`, value JetBrains Mono 11 `t1`, right-aligned. Camera (make + model), Lens, Exposure "ƒ/2.0 · 1/640 · ISO 400", Focal length "23 mm · 35 eq." | relayout (strip → rows). Focal 35 mm is `MediaDetails.focal_length_35mm` (`lumina-core/src/scanner.rs:383`), reached via `InfoDetails.file` (`app/info_details.rs:20`, loaded off-thread) |
| Panel: Location | "LOCATION" + place text (`summary.rs:113-118`) | Title + "Open map" action. Map card h 132, radius 6, outline `line`, pin 12 `accent` with halo 34 `accent-soft` + 1 px accent outline, label pill bottom-left (bg `#000000B3`, radius 4, h 20, mono 9.5 "Alfama, Lisboa · 38.712, −9.133") | restyle + new mini map (S–M; see decisions) |
| Panel: People | none | Avatars 40 round with 10.5 `t2` names, "Add" circle with 1 px `line-2` outline | not in app (see decisions) |
| Panel: Tags | Plain chips with ×, inline "add…" (`summary.rs:338-455`) | `Chip` component: h 28, radius 999, padding 0 11, bg `bg-2`, outline `line`, amber "#" glyph, Inter 12 `t1`. "Add" chip is transparent with a `line-2` outline and `t3` text | restyle to `theme::chip`. Keep the remove × on hover/focus |
| Panel: Rating & Label | Stars + colour dots (`summary.rs:82-91`) | absent | keep (see "App has, design lacks") |
| Panel: footer | Size, Dimensions, Get Info…, Reveal (`summary.rs:131-179`) | absent | keep a slim footer (see below) |

## Not in the app yet (needs a product decision)
- **Filmstrip** (M, UI only). The neighbours come from `displayed_rows` and the textures from the
  grid `texture_cache`, the same sources the grid uses (`app/render/grid.rs:155-200`). Render
  only a window around `selected_index`, because a view can hold 100k items. A click calls
  `navigate_selection`. No core work. **Recommend: build in this plan (Task 6).**
- **Histogram** (M). It needs a 64-bin luminance count of the decoded viewer image, computed on
  the background executor and cached per item. No catalog change. The decode already happens
  for display, so this only re-reads its bytes. **Recommend: separate plan.** Until then, leave
  the section out (no placeholder chart).
- **Location mini map** (S–M). Lumina makes no network calls (`tabs/location.rs:10`), so the
  street blocks in the frame are impossible. The offline basemap and projection in
  `app/map.rs` (`BASEMAP`, `Projection`) can draw a land-outline crop with the pin. "Open map"
  switches to Places with the map centred on the item. **Recommend: build in this plan
  (Task 5) with the land outline. Ask design to redraw the card without streets.**
- **People** (L, core). The catalog has no people or face model (no match in
  `lumina-core/src` outside downloads and sample data). It needs a schema, a migration, a UI to
  assign names, and possibly face detection. **Recommend: drop from this design and record it
  as a roadmap item.**
- **Star button** (S). There is a 0–5 rating (`app/culling.rs:95`) but no single star toggle.
  Options: open a 1–5 rating popover, or toggle 5★/0. **Recommend: a popover anchored to the
  button, with 1–5 keys working inside it.**
- **Back label**. The frame says "Saturday, 14 June" here and "Videos" in frame 06.
  **Recommend: the view the viewer returns to (sidebar selection name: "All Media", album
  name, "Videos").** The app has no day sections to return to.

## App has, design lacks
- Rating stars and colour label (`summary.rs:82-91`). Keep as a compact "RATING" section after
  Capture. Culling relies on it and it is mouse-reachable only here.
- Display title (`summary.rs:56-61`). Remove from the panel header; the frame leads with the
  filename. The title stays in Get Info.
- Footer with Get Info… and Reveal (`summary.rs:131-179`). Keep both buttons in a slim footer:
  Get Info is the only mouse path to editing caption, date and location. Remove the Size and
  Dimensions rows (size moves to the When line, dimensions to the badge tooltip).
- Zoom navigator and Fit button while zoomed (`stage_controls.rs`). Keep.
- PiP hand-off pill (`stage_controls.rs:202-260`). Keep (video only).
- Compact window hides the Back label (`top_bar.rs:48-51,145-152`). Keep.
- Trash lifecycle block (`summary.rs:119`). Keep.

## Review Focus
- **Narrow window (< 1000 px):** stage minus 340 leaves little room. The photo keeps
  contain-fit, arrows stay 26 from the edges, the filmstrip shows fewer thumbs, and the top bar
  never overlaps (position text truncates first).
- **Huge view (100k items) / first or last item:** filmstrip renders a bounded window. At the
  ends the current thumb sits off-centre, with no empty slots and no wrap-around.
- **Missing metadata:** no camera, lens, exposure or GPS. Capture rows are omitted one by one,
  and the section disappears when empty. Location hides without coordinates. No "—" rows.
- **Long filename / long tag:** filename truncates with the badge kept visible. Chips wrap to
  new rows and the panel scrolls (`inspector-summary-scroll`).
- **Panel closed (I):** the stage takes the full width. The date then shows nowhere, which is
  acceptable because the Back label names the view.
- **Keyboard:** Tab reaches the top-bar buttons in order. I, F, E, [ and ] keep working.
  Escape closes the star popover before the viewer.

## Tasks

### Task 1: Docked InfoPanel layout
**Files:** Modify `crates/lumina-ui-gpui/src/views/viewer/mod.rs:23-25,94-106,161-184`,
`views/viewer/stage_controls.rs:76-81`, `views/viewer/viewport.rs:40-79` (drawer maths) /
Test `crates/lumina-ui-gpui/src/tests/viewer_stage.rs`
- [ ] Write failing test `viewer_info_panel_is_docked_full_height`. With a 1600×1000 window and
      the panel open, `viewer-info-drawer` bounds are right 1600, width 340, top 0, height 1000,
      and `viewer-top-bar` right edge = 1260.
- [ ] Implement: change the backdrop to a row. Stage column (`flex_1`) holds top bar, viewport
      and filmstrip. Panel is a sibling: 340 wide, bg `bg_1`, border-left `line`, no radius, no
      shadow. Remove `DRAWER_INSET` padding hacks. Keep the selector `viewer-info-drawer`
      (used at `tests/viewer_and_video.rs:1962,2043,2415`, `tests/overlay_isolation.rs:171` and `tests/video_player.rs:303`; update their bounds expectations to the docked geometry). Immersive mode still hides the panel with the
      HUD.
- [ ] Run `cargo test -p lumina-ui-gpui viewer_stage`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): dock the viewer info panel` staging explicit paths.

### Task 2: Top bar
**Files:** Modify `views/viewer/top_bar.rs:14-230`, `src/i18n.rs` / Test
`tests/viewer_stage.rs`, `tests/viewer_and_video.rs`
- [ ] Write failing test `viewer_top_bar_matches_detail_design`: bar height 60. Selectors
      `viewer-back`, `viewer-position` (text "23 / 84" form), `viewer-fav-btn`,
      `viewer-rate-btn`, `viewer-edit-btn`, `viewer-share-btn` (macOS), `viewer-info-btn`
      present in that order. Clicking `viewer-edit-btn` sets `edits.session`. Update the
      existing assertions on `viewer-filename` (`tests/viewer_and_video.rs:729-776`) to the new
      contract and say so in the commit body.
- [ ] Implement: 60 h, padding 0 20, transparent, no border. Back is `bg_2`, radius 8, h 30,
      chevron 15 `t2`, label 12 `t2`, with the label set to the view name. Position is mono 11
      `t3`. Buttons are 30×30, radius 8, gap 6. Active state is bg `bg_2` (fav: amber heart;
      info: `panel-right` icon). Each button gets a tooltip with its key and `.role(Button)` +
      `aria_label`. Hide Edit for videos, during a slideshow and in edit mode.
      Star popover: `viewer-rate-btn` opens `viewer-rate-popover` (occluding card + dismiss
      backdrop, anchored under the button) with five stars `viewer-rate-star-1..5`. Keys 1–5
      set the rating via `set_rating_selected` (`app/culling.rs:95`), 0 clears it, Esc closes the
      popover (before the viewer). Add palette command "Rate…" that opens it. Test: open the
      popover, press 3 → rating 3 and popover closed; reopen, press 0 → rating 0; Esc closes
      without closing the viewer.
- [ ] Run `cargo test -p lumina-ui-gpui viewer`; expect pass.
- [ ] Gate and commit `feat(design): restyle the viewer top bar`.

### Task 3: Stage chrome (photo frame, arrows)
**Files:** Modify `views/viewer/stage_controls.rs:155-197`, `views/viewer/viewport.rs` /
Test `tests/viewer_stage.rs`
- [ ] Write failing test `viewer_nav_buttons_sit_26px_from_stage_edges`: `viewer-prev` left =
      stage left + 26, `viewer-next` right = stage right − 26, both 40×40 and vertically centred
      on the viewport.
- [ ] Implement: arrow fill `theme::bg_float()`, 1 px `line_2` outline, chevron `t1`. Photo
      radius 3 + shadow 0 16 48 (manual check against `K9cBD.png`).
- [ ] Run `cargo test -p lumina-ui-gpui viewer_stage`; expect pass.
- [ ] Gate and commit `feat(design): restyle viewer stage chrome`.

### Task 4: Panel header, Capture and Tags sections
**Files:** Modify `views/inspector/summary.rs:19-242,338-460`, `views/inspector/body.rs:47`
(`render_meta_row` styling) / Test `tests/inspector.rs`
- [ ] Write failing test `info_panel_sections_follow_detail_design`. For a fixture photo with
      EXIF: selectors `inspector-filename`, `inspector-format-badge` ("RAW" for `MediaType::Raw`,
      else the uppercased extension), `inspector-when` contains `HH:MM:SS` and the size,
      `inspector-capture-camera/lens/exposure/focal` present, tag chips `tag-chip-<tag>` (existing names: `tag-chip-*`, `tag-filter-*`, `tag-remove-*`) and
      `tag-add` present. For a photo with no EXIF: no `inspector-capture` section.
- [ ] Implement: header row + badge; When line (`%a %-d %b %Y · %H:%M:%S · size`); section title
      style 9.5/600 tracking 1 `t3`; Capture key/value rows (19 h, 11.5 Inter `t3` / 11 mono
      `t1`); tags through `theme::chip` with the "#" glyph and an "Add" outline chip, keeping
      Enter/Esc typing and suggestions. Move Rating & Label below Capture. Slim footer keeps
      Get Info… and Reveal.
- [ ] Run `cargo test -p lumina-ui-gpui inspector`; expect pass.
- [ ] Gate and commit `feat(design): restyle the info panel sections`.

### Task 5: Location card (offline mini map)
**Files:** Modify `views/inspector/summary.rs:113-118`, reuse `app/map.rs` (`BASEMAP`,
`Projection`) / Test `tests/map.rs`
- [ ] Write failing test `location_card_open_map_focuses_places`: with a geotagged item,
      `inspector-location-map` and `inspector-open-map` exist. Clicking Open map sets the
      selection to Places with the map centred on the item's coordinates. With no GPS, neither
      exists.
- [ ] Implement: card h 132, radius 6, outline `line`, land-outline crop, pin 12 `accent` +
      halo 34 `accent_soft`, label pill (place · `format_coordinates`, `tabs/location.rs:137`).
      Add a palette command "Show on Map" for the keyboard path.
- [ ] Run `cargo test -p lumina-ui-gpui map`; expect pass.
- [ ] Gate and commit `feat(design): offline location card in the info panel`.

### Task 6: Filmstrip
**Files:** Create `views/viewer/filmstrip.rs` (keep < 500 lines), modify
`views/viewer/mod.rs` / Test `tests/viewer_stage.rs`
- [ ] Write failing test `viewer_filmstrip_centres_current_and_navigates`. With 30 items and #10
      open (`selected_index` = 9, zero-based): `viewer-filmstrip` height 95.5±1.
      `viewer-filmstrip-current` is 74×74. At most 15 thumbs are rendered (7 either side of
      the current one, as in the frame: `FILMSTRIP_MAX = 15`); with 200 items the count is still
      15. Clicking the thumb right of the current one opens item #11: `selected_index` = 10 and
      `viewer-filmstrip-current` now belongs to that item's id.
- [ ] Implement: border-top `line`, padding 0 20, gap 6, thumbs 58 radius 3 with a 55% scrim,
      and the current thumb 74 with a 2 px `accent` outline. The thumbs come from the grid
      texture cache, with missing ones queued through `queue_thumbnail_request_item`. No SQL per
      frame: reuse the `displayed_rows` window. Hide the strip in immersive mode, slideshow and
      edit mode.
- [ ] Run `cargo test -p lumina-ui-gpui viewer_stage`; expect pass.
- [ ] Gate (include `make ui-smoke`) and commit `feat(design): viewer filmstrip`.

Manual visual checks (compare with `K9cBD.png`): colours, radii, photo shadow, arrow fill,
chip "#" glyph colour, badge tint.
