# Design 16: Smart Album — New — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle the smart album builder into the frame's centred dialog (badge + name
field, Match row, condition card with icon pickers, star rating, options, wide preview strip,
footer band) and close the rule gaps the frame exposes.
**Design:** `lumina.pen` frame `KweSw` "Smart Album — New" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation. The frame's backdrop is the Design 05 Albums page.
**Icons (fetched in 00 Task 1):** chevron-down, calendar. `sparkles`, `x`, `image`, `camera`,
`tag`, `star`, `minus`, `plus`, `check` already exist in `resources/icons/`.
**App today:** `crates/lumina-ui-gpui/src/views/album_builder/mod.rs:19-291` (sheet from the
toolbar, 760 wide, title row 66-93, Match row 94-115, conditions container 117-140, preview
171-214, footer 216-291) and `views/album_builder/rows.rs:32-262` (picker, text field,
popup, square buttons, row). Draft model and labels: `app/smart_draft.rs` (fields 14-29,
value kinds 56-90, period units 93-120, labels 122-146, operators 164-175, value kinds
199-215). Actions: `app/album_builder_actions.rs` (open 19-35, preview 167-185 with
`PREVIEW_LIMIT` 8 at :15, save 398-446). Keys: `app/keymap/modals.rs:549-592`. Delete
confirmation: `views/smart_album_delete.rs:9-30` (unchanged by this frame). Engine:
`crates/lumina-core/src/smart_albums/rules.rs` (fields 16-46, operators 50-63, Tag compile
372-425, Place/Camera contains-only 428-453, durations 687-730).

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "Golden Hour Portraits", "1,248 items match",
  "Fujifilm X-T5"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Backdrop | `theme::modal_dim()` (`mod.rs:33`) | scrim #07070BD9 over the page | restyle: `theme::scrim()` |
| Card | sheet under toolbar: pt 52, w760, max_h 620, bg_dialog, border no top, rounded bottom 10, p_5 (`mod.rs:34-60`) | centred dialog 840 w, bg-1, r16, no padding on the shell (sections pad themselves) | relayout |
| Header | Star 18 accent + borderless 17 px title, flex_1, h34 (`mod.rs:66-93`) | pad 22 26 0 26; badge 36×36 accent-soft r10 with `sparkles` 18 accent; name field 420×42 bg-0 r10 pad 0 13, Inter Tight 17/600 ls -0.3 t1, accent caret 2×20, accent border while focused | restyle |
| Close | none (Cancel / Esc only) | 28×28 bg-2 r8 `x` 14, right of header | new |
| Body | gap_3 (`mod.rs:60`) | pad 18 26 22 26, gap 16 | restyle |
| Match row | "Match" text_sm t_muted, picker w70 with "⌄" glyph (`mod.rs:94-115`, `rows.rs:45`) | "Match" Inter 13.5 t2; picker 92×34 bg-0 r9 pad 0 12, value 13/500 t1, `chevron-down` 13; "of the following conditions:" 13.5 t2 | restyle |
| Conditions container | bordered bg_app, rows divided by border_t, row pad 8/6 (`mod.rs:117-139`, `rows.rs:183-192`) | bg-2 r12 pad 14 gap 10, no dividers; row gap 12 | restyle |
| Field picker | 180 w, label only (`rows.rs:211-220`) | 210×38 bg-0 r9 pad 0 12, icon 14 + Inter 13/500 t1, `chevron-down` 13 | restyle + new icons |
| Operator picker | 150 w (`rows.rs:221-230`) | 140×38, Inter 13/400 t2 | restyle |
| Value: choice | picker flex_1 (`rows.rs:153-168`) | 38 h bg-0 r9; Kind shows `image`/video icon + "Photo" | restyle |
| Value: period | text field + unit picker 90 (`rows.rs:169-243`) | number box 66×38 mono 13 t1 centred + unit picker (flex) "months" | restyle |
| Value: camera | text field, contains/does not contain (`smart_draft.rs:174`) | operator "is", picker "Fujifilm X-T5" | see engine gaps |
| Value: keywords | single-tag picker, operator "is" (`smart_draft.rs:209`) | operator "include any of"; token box bg-0 r9 pad 0 8 gap 6 with chips accent-soft r6 h24 pad 0 7, label 11.5 accent, `x` 10, then "add keyword…" 12 t3 | see engine gaps |
| Value: rating | popup of "★★★★" strings (`smart_draft.rs:37-43`) | 5 `star` icons 15 gap 4 (filled accent up to the value) + "4 stars & up" 12 t2 | new control |
| Row buttons | 28 px square controls, icon 12 (`rows.rs:115-132`) | minus/plus 34×34 bg-1 r8, icon 14 | restyle |
| Field labels | "Date", "Tag", "Stars" (`smart_draft.rs:126-128`) | "Date Captured", "Keywords", "Rating" | copy |
| Invalid row | danger outline on the whole row (`rows.rs:201-210`) | not shown | keep |
| Options | none | row gap 22; checkbox 16 r5 (on: accent + `check` 11; off: bg-0), label 12.5 t1 (off: t2): "Include items from shared albums", "Keep sorted by date captured, newest first", "Pin to sidebar" | new: Pin only (decisions) |
| Live preview head | section label + help xs (`mod.rs:192-210`) | "LIVE PREVIEW" Inter 10/600 ls .9 t3; "The first matches, updated as you edit." 12 t2; gap 10 | restyle |
| Preview tiles | 8 squares 72 px, gap_1p5 (`mod.rs:171-191`, `PREVIEW_TILE` :214) | strip gap 7; tiles 96 h r7, width by aspect (68-92); "More" tile bg-2 r7 h96: "+1,239" mono 14 t1, "more" 10.5 t3 | restyle + new More tile |
| Footer | inline row: status text xs, Cancel + Esc kbd, "OK" (`mod.rs:216-291`) | band bg-2, border-top line, h63.5, pad 0 20; `sparkles` 14 accent + "1,248 items match" 13/500 t1 + "· stays up to date automatically" 12 t3; Cancel bg-3 r9 h36 pad 0 12, 13/500 + kbd bg-0 r5 h20 "Esc" mono 11 t3; "Create Smart Album" accent r9 h36, 13/600 dark + kbd "⏎" on dark 20% | restyle + copy |

## Engine support for the frame's rules
| Design rule | Engine today | Gap |
|---|---|---|
| Kind is Photo | `MediaType` Equals "photo" = image + RAW (`rules.rs:457-465`) | none |
| Date Captured is in the last 18 months | `Date` InTheLast "18mo" (`rules.rs:549-553`, `parse_duration_ms` 687) | none; a month is 30 days (ponytail at `rules.rs:709`), so "18 months" is 540 days |
| Camera is Fujifilm X-T5 | `Camera` Contains / NotContains over make+model only (`rules.rs:428-453`); `CameraModel` exists but is not offered (`smart_draft.rs:14-29`) | no "is" for Camera; no list of the library's cameras |
| Keywords include any of A, B | `Tag` Equals / NotEquals one keyword (+ subtree), Contains, IsEmpty, IsNotEmpty (`rules.rs:372-425`) | no multi-value "any of" within an "all" album |
| Rating is at least 4 | `Rating` AtLeast (`rules.rs:50-63`, ops `smart_draft.rs:172`) | none |
| Include items from shared albums | no sharing | not applicable (drop) |
| Keep sorted by date captured, newest first | smart albums store no sort (`SmartAlbum` struct `rules.rs:83-93`; view uses the global sort) | per-album sort |
| Pin to sidebar | `pinned` column + `set_smart_album_pinned` (`lumina-core/src/lib.rs:624`, `catalog/albums.rs:339`; UI caller `app/albums.rs:299`) | none (apply after create) |
| Design 05 chips: time of day, shutter speed, "unedited", "outdoors" | no `RuleField` for time of day, `exposure_time`, edit state or AI labels (`rules.rs:16-46`) | missing fields |

## Not in the app yet (needs a product decision)
- **Keywords "include any of".** Recommend a new `RuleOperator::AnyOf` for `Tag` with
  `RuleValue::List(Vec<String>)` (serde-untagged: a JSON array cannot be mistaken for the
  other variants; old albums still parse), compiled as one `IN (SELECT … WHERE subtree OR …)`
  clause; keyword rename/merge/delete must rewrite list members (`rewrite_tag_rules`,
  defined at `catalog/keyword_ops.rs:143`, called from the rename at :275), and portable export/import must round-trip it. Size M, core
  work. Recommend build in this plan (Task 5); otherwise users need two albums or "any" mode.
- **Camera "is" with a picker.** Recommend adding Equals / NotEquals to `Camera`
  (case-insensitive match on "make model", with the make prefix de-duplicated as the inspector
  shows it) and a distinct-cameras query for the popup. Size S-M, core work. Recommend build
  (Task 6). Keep Contains for saved albums.
- **Keep sorted … newest first.** Per-smart-album sort needs a column + migration and the view
  to honour it. Size M. Recommend drop from design (the global sort applies).
- **Include items from shared albums.** No sharing (AGENTS §5.4). Recommend drop.
- **Missing fields shown in Design 05** (time of day, shutter speed, edited/unedited): each
  is S-M engine work (columns exist for `exposure_time` and `media_edits`; time of day needs
  local-time extraction). Recommend a separate "more rule fields" plan.
- **Centred dialog vs sheet.** The app's builder and tag manager share the "sheet from the
  toolbar" shell (`mod.rs:34-38`, `views/tag_manager/mod.rs:30-34`). Recommend follow the
  frame: centred dialog for the builder only.

## App has, design lacks
- Danger outline and footer error text for invalid rows (`rows.rs:201-210`, `mod.rs:219-230`):
  keep; the footer shows the error instead of the count while invalid.
- "No conditions — every item matches." empty row (`mod.rs:149-169`): keep.
- Edit mode (`open_album_builder_for_edit`, `album_builder_actions.rs:24`): same dialog, button
  reads "Save Smart Album" and the badge stays.
- Full keyboard model (Tab focus order, popups, ⌘↵, Esc; `keymap/modals.rs:549-592`): keep;
  add ←/→ for the star control and Backspace on an empty keyword input to drop the last chip.

## Review Focus
- Zero matches: footer "No items match", strip shows one empty-state line, More tile hidden,
  Create still allowed (albums fill later).
- Huge counts: "+N more" uses grouped digits (1,239) and never goes negative when matches ≤ 8.
- Many conditions: the condition card scrolls inside the dialog; header, options and footer
  stay visible on a 900 px tall window (dialog max height = window − 80).
- Long names/values: name field truncates visually but keeps typing; keyword chips wrap to a
  second line instead of overflowing the row.
- Preview performance: preview and count still run on every edit on the UI thread
  (`album_builder_actions.rs:167-185`); move the fetch off the UI thread (the
  `app/active_view.rs` pattern) if a large library makes typing stutter; measure before/after.

## Tasks
### Task 1: Dialog shell, header and close
**Files:** `views/album_builder/mod.rs:19-115` / Test `crates/lumina-ui-gpui/src/tests/smart_album_builder.rs`
- [ ] Update `v3_sheet_layout_matches_the_design` (`tests/smart_album_builder.rs:353`) to
      fail first: `album-builder-card` width 840 and vertically centred in the window;
      `album-builder-badge` 36×36; `album-builder-title` width 420, height 42;
      `album-builder-close` 28×28 closes without saving; `album-builder-match` width 92.
- [ ] Implement exact values from the table; add `Icon::ChevronDown` to `Icon::ALL` (SVG from
      00 Task 1; fetch nothing here), replace the "⌄" glyph in `rows::picker`; backdrop
      `theme::scrim()`.
- [ ] Run `cargo test -p lumina-ui-gpui smart_album_builder`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): smart album dialog shell` staging explicit paths.

### Task 2: Condition card and row controls
**Files:** `views/album_builder/rows.rs:32-262`, `app/smart_draft.rs:122-146` (labels, new
`field_icon`) / Test `tests/smart_album_builder.rs`
- [ ] Write failing test `condition_rows_match_design`: `album-rule-field-0` 210×38,
      `album-rule-op-0` width 140, `album-rule-remove-0` 34×34, conditions container has no
      row dividers (rows spaced 10), Date row shows `album-rule-value-<i>` width 66 and a unit
      picker; field popup lists "Date Captured", "Keywords", "Rating".
- [ ] Implement: field icons (image, calendar, camera, tag, star, …; `Icon::Calendar`, SVG from 00 Task 1),
      labels, operator text t2, value variants per the table.
- [ ] Run `cargo test -p lumina-ui-gpui condition_rows`; expect pass.
- [ ] Gate and commit `feat(design): smart album condition rows`.

### Task 3: Star rating control
**Files:** `views/album_builder/rows.rs` (Rating value), `app/album_builder_actions.rs`,
`app/keymap/modals.rs:549-592` / Test `tests/smart_album_builder.rs`
- [ ] Write failing test `rating_stars_click_and_arrow_keys`: Rating row shows
      `album-rule-star-<i>-1..5`; clicking star 4 sets the value "4" and label "4 stars & up";
      with the value focused, ← makes it 3; saved rule is `Rating AtLeast 3`; each star has
      role Button and aria label "N stars".
- [ ] Implement; label per operator: "N stars & up" / "N stars or fewer" / "N stars".
- [ ] Run `cargo test -p lumina-ui-gpui rating_stars`; expect pass.
- [ ] Gate and commit `feat(design): rating stars in smart albums`.

### Task 4: Options, preview strip and footer
**Files:** `views/album_builder/mod.rs:171-291`, `app/album_builder_actions.rs:398-446`
(pin after create) / Test `tests/smart_album_builder.rs`
- [ ] Write failing test `footer_preview_and_pin_option`: tiles height 96; with 10 matches
      `album-builder-more` reads "+2" / "more"; with 3 matches it is absent; footer
      `album-builder-count` "N items match" and `album-builder-save` text "Create Smart
      Album" ("Save Smart Album" when editing); checking `album-builder-pin` (Space when
      focused) creates the album pinned (catalog `pinned` = 1). Pin failure shows the error
      banner and the album still exists.
- [ ] Implement exact values; tile width from the item's aspect ratio clamped 68-92.
- [ ] Run `cargo test -p lumina-ui-gpui footer_preview`; expect pass.
- [ ] Gate and commit `feat(design): smart album preview and footer`.

### Task 5: Keywords "include any of" (engine + UI)
**Files:** core `crates/lumina-core/src/smart_albums/rules.rs:50-75,372-425`,
`catalog/keyword_ops.rs:143` (`rewrite_tag_rules`), portable format `rules.rs:218-328`; UI
`app/smart_draft.rs:164-215,218-352`, `views/album_builder/rows.rs` (token box) /
Test `crates/lumina-core/src/smart_albums/mod.rs` tests, `tests/smart_album_builder.rs`
- [ ] Write failing tests `tag_any_of_matches_either_keyword_and_subtrees`,
      `old_rules_json_still_parses`, `rename_rewrites_any_of_members` (core) and
      `keywords_tokens_add_and_remove` (UI: pick two tags, chips `album-rule-token-<i>-<tag>`,
      × removes, ⌫ in the empty input removes the last, saved rule is AnyOf).
- [ ] Implement `RuleOperator::AnyOf` + `RuleValue::List`; operator label "include any of";
      empty list is invalid (row outlined).
- [ ] Run `cargo test -p lumina-core smart_albums` and `cargo test -p lumina-ui-gpui keywords_tokens`.
- [ ] Gate (include `make bench`: smart album gate < 8 ms release) and commit
      `feat(core): tag any-of smart album rule`.

### Task 6: Camera "is" with the library's cameras
**Files:** core `rules.rs:428-453`, new distinct-cameras query in the catalog; UI
`app/smart_draft.rs:164-215`, `app/album_builder_actions.rs:194-250` (popup options) /
Test core tests, `tests/smart_album_builder.rs`
- [ ] Write failing tests `camera_equals_matches_make_model_case_insensitively` (core) and
      `camera_is_offers_library_cameras` (UI: popup lists the fixture's two cameras).
- [ ] Implement Equals / NotEquals for `Camera`; keep Contains / NotContains.
- [ ] Run `cargo test -p lumina-core camera_equals` and `cargo test -p lumina-ui-gpui camera_is`.
- [ ] Gate (include `make bench`) and commit `feat(core): camera is rule`.

Manual checks: centred dialog, accent focus border on the name field, chip and star colours,
footer band against `KweSw.png` with `make ui-smoke`.

## Decisions (2026-11-13, "go with your recommendations")
1. Keywords "include any of": build (Task 5): `RuleOperator::AnyOf` + `RuleValue::List`, rename/merge/delete rewrite members, portable format round-trips.
2. Camera "is" with the library's cameras: build (Task 6); Contains / NotContains stay for saved albums.
3. "Keep sorted by date captured, newest first": dropped (the global sort applies).
4. "Include items from shared albums": dropped (no sharing, AGENTS §5.4).
5. Time of day, shutter speed, edited/unedited fields: a separate "more rule fields" plan, not this one.
6. Centred dialog for the smart album builder only; the tag manager keeps its shell.
7. Options row shows "Pin to sidebar" only.
8. Preview performance: measure the UI-thread preview on a large library in Task 4; move it off the UI thread only if typing stutters.
