# Design 09: States — Empty — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle the grid empty states to the design's shared layout and copy for four cases: empty album, empty smart album, no search results and Recently Deleted.
**Design:** `lumina.pen` frame `R5RGv` "States — Empty" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation. The toolbar title and count row is owned by the Library screen plan (see the ContextBar row below).
**Icons (fetched in 00 Task 1):** image-plus (album glyph), library (Add from Library), search-x (no-results glyph), sliders-horizontal (Edit rules). Already in `resources/icons`: sparkles, trash-2, x, info, search, folder, sliders-vertical.
**Owned here for other plans:** the empty-state shell with its `Hero` size variant (Task 1, used by 11 Task 4) and `rule_label` (Task 3, reused by 05).
**App today:** every empty view goes through one function, `LuminaApp::render_empty_state` in
`crates/lumina-ui-gpui/src/app/render/grid.rs:763-1062`. It is called from `render_browser_grid`
(`grid.rs:48-55`) and from Duplicates (`views/duplicates.rs:20-22`). The copy is inline in `grid.rs:792-916`,
except for the album strings in `src/i18n.rs:344-346`. The tests are `src/tests/states.rs:130-262` and
`src/tests/palette_and_empty.rs:71-195`.
(The brief named `views/viewer_state.rs`. That file holds the viewer zoom and pan state (`ViewerState`), not empty states.)

The frame is a catalogue of states, not a screen. Each case is a 760×400 panel with a 45.5 px
"ContextBar" (title on the left, mono count on the right) over the empty body. In the app that bar is the
toolbar title row (`app/render/toolbar.rs:150-189`). Only the body is in scope here.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- Copy must stay true to the app. The design's "rest here for 30 days" is false today (see the Trash row).

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Shell | Column with `items_center`, `max_w 440`, `gap_2p5`, `p_8` (`grid.rs:927-946`) | Content `padding 20px 28px`, centred column. Spacers: 14 px glyph→headline, 8 px headline→body, 16–18 px body→actions | relayout |
| Glyph | 44 px rounded square, `RADIUS_LG`, `bg_pane`, `border_subtle`, 20 px icon in `text_subtle` (`grid.rs:947-958`) | 54 px circle, `bg_2`, 1 px `line` outline, icon about 20 px in t2 | restyle |
| Headline | `text_base` semibold, `text()` (`grid.rs:959-966`) | Inter Tight 18/600, letter-spacing −0.3, t1, centred | restyle |
| Body | `text_xs`, `text_muted`, `max_w 360` (`grid.rs:967-974`) | Inter 12.5/400, line-height 19, t2, centred, width 360–430 (use `max_w 430`) | restyle |
| Primary button | `theme::control_primary` + `px_3` (`grid.rs:1037-1041`) | h 34, r 8, px 14, gap 7, accent fill, label 12.5/600 in accent-fg (#1A1206), leading icon | restyle (icon slot) |
| Secondary button | `theme::control` (`grid.rs:1039`) | h 34, r 8, px 14, `bg_2`, 1 px `line_2` outline, label 12.5/400 t1, leading icon | restyle |
| Actions row | `mt_1p5`, `gap_2` (`grid.rs:1023-1030`) | gap 9, centred | restyle |
| Empty album: copy | "“{title}” is empty" / "Photos and videos you put in this album show here." (`i18n.rs:344-346`) | "Drop photos here" / "Items stay in your library — an album is just a view of them." | copy (see decisions) |
| Empty album: frame | none | DropTarget 460×190, r 12, 1.5 px `line_2` outline, bg `bg_1` at 60 % | new (decision) |
| Empty album: action | "Show all photos" → `EmptyAction::ShowAll` (`grid.rs:838-852`) | primary "Add from Library" with a library icon | copy + behaviour |
| Smart album: body | `rule_summary(...)` as one sentence (`grid.rs:819-828`, `app/smart_draft.rs:410`) | Body "This album fills itself. As soon as a photo matches, it appears here."; then a row of rule chips: h 24, r 6, px 9, gap 6, `bg_1`, `line` outline, JetBrains Mono 10 in t2, field icon; chip gap 7 | relayout + new chips |
| Smart album: headline | "Nothing matches “{title}” yet" (`grid.rs:826`) | "Nothing matches these rules yet" | copy |
| Smart album: actions | primary "Edit Rules…" only (`grid.rs:820-823`); the test asserts one button (`states.rs:229-232`) | primary "Edit rules" (sliders icon), secondary "Delete album" (trash icon) | new secondary |
| Search: headline | "No matches for these filters" (`grid.rs:793-797`) | "Nothing matches all {n} filters" (n = term count, in words up to ten) | copy |
| Search: body | "Remove a filter to widen the search:" (`grid.rs:796`) | per-term counts: "Mara has 418 photos and Reykjavík has 0. Try loosening one of them." | copy (counts are a decision) |
| Search: chips | Removable term chips: h 24, px 10, rounded, `selection_*` colours, mono, trailing ×, `empty-search-term-{i}` (`grid.rs:982-1021`) | "Drop “term”" chips: h 28, r full, px 11, gap 6, `accent_soft` fill, 1 px `accent_outline` (00 token, #E9A23B59), accent text Inter 12/400, leading icon by term kind | restyle + copy |
| Search: date widening | none | "Widen to 2024 — 2025" chip | not in app (decision) |
| Search: clear all | none (`library.rs:852` `clear_search` exists) | secondary "Clear all filters" with × icon | new |
| Trash: copy | "Recently Deleted is empty" / "Items you delete stay here until you empty the trash or restore them." (`grid.rs:801-805`) | "Nothing waiting to be removed" / "Deleted items rest here for 30 days. Lumina never touches the originals on disk unless you ask it to." | copy, without "30 days": nothing purges automatically |
| Trash: action | none (the test asserts no `empty-primary`, `states.rs:137-145`) | secondary "How deletion works" with an info icon | new (decision) |
| ContextBar | Toolbar title 15 px semibold; the count shows only for Trash when > 0 (`toolbar.rs:150-189`) | 45.5 px bar, `bg_1`; title Inter 12.5/500; right side mono 10 t3 ("0 items", "Live · 0 items", "0 of 48,291") | Library plan; not in scope |

## Not in the app yet (needs a product decision)
- **Drop onto an album (Drop photos here).** Dropping files from Finder on an empty album would import them and add them to the album. Today only the import dialog accepts `ExternalPaths` (`views/import_dialog.rs:215-226`). Size M: it needs an import that targets an album, but no core schema change. Recommendation: separate plan. Until that ships, use honest copy: headline "“{title}” is empty", the design's body line, and the primary "Add from Library". Do not draw a drop target that does nothing.
- **Add from Library.** No picker flow exists. Smallest version: go to All Media and show a toast "Select items, then use Add to Album (⌘K)". That reuses `open_album_picker` (`app/album_membership.rs:34`). Size S. Recommendation: build it in this plan, with a ⌘K entry "Add from Library" (enabled only while an empty album is shown).
- **Per-term counts in the search body ("Mara has 418 photos…").** This needs one count query per term. Run them on the background executor and keep the structured-query gate (< 5 ms each in release). Size S–M, engine API only. Recommendation: build it behind a fallback body ("Try loosening one of them.") that shows until the counts arrive.
- **Term-kind icons (place, date, tag) on the chips.** These need term classification from `lumina_core::search`. Recommendation: use a generic icon now, and kind icons only if the parser already exposes kinds.
- **"Widen to 2024 — 2025".** This needs date-range awareness in the search suggestions. Size M, core. Recommendation: drop it from this plan.
- **"How deletion works".** There is no help system. Recommendation: open a short bundled doc through `open_bundled_doc` (`app/preferences.rs:205`) if the docs owner agrees, otherwise drop the button. Size S. It is Task 6, which runs only if approved.
- **30-day retention.** This would need an automatic purge. AGENTS.md says no automatic deletion; that rule is stated for backup, but the spirit is the same here. Recommendation: drop it from the design copy.

## App has, design lacks
- Empty states for Favorites, Tag, Duplicates, smart album not found, empty library, scanning and generic views (`grid.rs:806-915`). Keep them all and give them the new shell.
- The empty library shows its path on its own mono line, `empty-library-path` (`grid.rs:975-982`). Keep it; the test depends on it (`states.rs:248-260`).
- Tooltips on the term chips ("Remove {term}"). Keep them, reworded "Drop {term}".

## Review Focus
- Long album or smart album names (60+ characters) in the album headline: wrap at `max_w 430` and never push the buttons out of view.
- A smart album with eight or more rules: the chips wrap (`flex_wrap`, centred). No overflow at a 900 px window with the sidebar and inspector open.
- A search with one term: the headline reads "Nothing matches this filter", not "all one filters".
- Keyboard: Tab or ⌘K reaches every new button. "Clear all filters" has a new ⌘K path, Clear Search (Task 4; today there is only Clear Filters, `app/command_palette.rs:58`, `:752`), and Esc in the search field still clears it.
- A narrow window (720 px center pane): the glyph, headline and actions stay centred and the body wraps.

## Tasks
### Task 1: Shared empty-state shell
**Files:** Modify `crates/lumina-ui-gpui/src/app/render/grid.rs:927-1062` / Test `crates/lumina-ui-gpui/src/tests/states.rs`
- [ ] Write failing test `empty_state_shell_matches_design`: on an empty album, `empty-state-glyph` is 54×54 and `empty-state-headline` and `empty-state-body` exist. The gap from the glyph's bottom to the headline's top is 14 ±1 px. `empty-primary` is 34 px tall. With the `Hero` size, `empty-state-glyph` is 150×150 and `empty-primary` is 40 px tall.
- [ ] Implement: the values from the table (glyph circle, Inter Tight 18/600 headline, 12.5/19 body at `max_w 430`, 16 px before the actions, gap 9). Add an optional leading icon to `empty_action_button`. Add the `empty-state-glyph`, `empty-state-headline` and `empty-state-body` selectors. Add an `EmptySize { Standard, Hero }` variant: `Hero` has a 150 px glyph and a 40 px primary button, for Design 11 (First Run, Task 4); `Standard` is the default for every view here.
- [ ] Run `cargo test -p lumina-ui-gpui empty_`; expect pass. Compare against `R5RGv.png` by hand (colours, radius).
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): empty-state shell` staging explicit paths.

### Task 2: Empty album
**Files:** Modify `grid.rs:838-852`, `src/i18n.rs:344-346`, `src/app/command_palette.rs` / Test `src/tests/states.rs`
- [ ] Write failing test `empty_album_offers_add_from_library`: `empty-state-body` reads exactly "Items stay in your library — an album is just a view of them." (the literal, not the key: `album.empty_body` already exists at `i18n.rs:346`, so a key check could not fail). `empty-primary` reads "Add from Library"; clicking it sets the selection to `AllLibrary` and shows a toast that mentions Add to Album. ⌘K "Add from Library" does the same.
- [ ] Implement: new `album.empty_body` copy, keeping the "“{title}” is empty" headline (see decisions). New `EmptyAction::AddFromLibrary` with the primary label "Add from Library", and a `CommandId::AddFromLibrary` palette entry (label through `i18n.rs`) that runs it.
- [ ] Run `cargo test -p lumina-ui-gpui empty_album`; expect pass.
- [ ] Gate and commit `feat(design): empty album copy and Add from Library`.

### Task 3: Empty smart album
**Files:** Modify `grid.rs:818-837`, `app/smart_draft.rs:410-440` (split out a public per-rule `rule_label(rule) -> String`; this task owns it and Design 05 reuses it, so `rule_summary` becomes the join of `rule_label`s) / Test `src/tests/states.rs:205-233`
- [ ] Update the test `empty_smart_album_offers_only_edit_rules_and_library_path_is_mono`, renamed `empty_smart_album_shows_rule_chips_edit_and_delete`. Assert one `empty-rule-chip-{i}` per rule, whose text equals `rule_label(&rules[i])`. `empty-primary` opens the builder. `empty-secondary` sets the smart album delete prompt (`prompt_delete_smart_album`, `app/album_builder_actions.rs:109`). Changing this test is a test-contract change, so say so in the commit body.
- [ ] Implement: headline "Nothing matches these rules yet", the new body, the chips (h 24, r 6, px 9, mono 10, t2 on `bg_1` with a `line` outline), and the secondary "Delete album". Keep the ⌘K Delete Smart Album entry (`app/command_palette.rs:873`).
- [ ] Run `cargo test -p lumina-ui-gpui empty_smart_album`; expect pass.
- [ ] Gate and commit `feat(design): smart album empty state with rule chips`.

### Task 4: No search results
**Files:** Modify `grid.rs:792-798`, `grid.rs:982-1021`, `src/app/command_palette.rs` (new `CommandId::ClearSearch`) / Test `src/tests/palette_and_empty.rs:71-195`
- [ ] Write failing test `empty_search_drop_chips_and_clear_all`: with the query `a b`, the headline reads "Nothing matches both filters". `empty-search-term-0` contains "Drop". Clicking `empty-secondary` ("Clear all filters") empties `search_query`. ⌘K "Clear Search" also empties it and is hidden when the query is empty. The existing chip tests stay green, quoted terms included.
- [ ] Implement: the chip restyle (h 28, r full, px 11, `accent_soft` fill, `accent_outline` border, accent text 12/400), the label "Drop “{term}”", the headline by term count, the secondary button calling `clear_search` (`app/library.rs:852`), and the `ClearSearch` palette command (this plan owns it; Clear Filters at `command_palette.rs:752` stays). Leave the per-term counts to their decision row.
- [ ] Run `cargo test -p lumina-ui-gpui empty_search`; expect pass.
- [ ] Gate and commit `feat(design): no-results empty state`.

### Task 5: Recently Deleted
**Files:** Modify `grid.rs:801-805` / Test `src/tests/states.rs:130-183`
- [ ] Extend `recently_deleted_counts_notes_and_hides_buttons_when_empty`: `empty-state-headline` reads "Nothing waiting to be removed" and the body has no "30 days". `empty-primary` and `empty-secondary` stay absent (Task 6 changes the second assertion if it runs).
- [ ] Implement: the copy without "30 days", for example "Deleted items stay here until you empty Recently Deleted. Lumina never touches the originals on disk unless you ask it to." Check the last sentence against `views/delete_modal.rs` before shipping it.
- [ ] Run `cargo test -p lumina-ui-gpui recently_deleted`; expect pass.
- [ ] Gate and commit `feat(design): Recently Deleted empty copy`.

### Task 6: "How deletion works" (only if approved)
Skip this task entirely unless the product owner approves the button and a bundled doc.
**Files:** Modify `grid.rs:801-805`, add the doc under `resources/` with the bundling rule used by `open_bundled_doc` (`app/preferences.rs:70`, `:205`) / Test `src/tests/states.rs`
- [ ] Write failing test `empty_trash_explains_deletion`: `empty-secondary` reads "How deletion works" and clicking it asks `open_bundled_doc` for the new doc (test-thread counter). Update Task 5's absence assertion for `empty-secondary`.
- [ ] Implement the secondary button with the info icon.
- [ ] Run `cargo test -p lumina-ui-gpui empty_trash`; expect pass.
- [ ] Gate and commit `feat(design): explain deletion from the empty trash`.

Manual visual checks (not testable headlessly): the glyph fill and outline colours, the chip alpha values, and the button radius. Compare them against `R5RGv.png`.

## Decisions (2026-11-13, "go with your recommendations")
- Empty album: no drop target; keep the "“{title}” is empty" headline, the design's body line, primary "Add from Library" (go to All Media + toast pointing at Add to Album ⌘K), with a ⌘K entry. Drop-onto-album import is a separate plan.
- Search: per-term counts deferred; body uses the fallback "Try loosening one of them." Generic chip icon unless the parser already exposes term kinds. "Widen to 2024 — 2025" dropped.
- Trash: no "30 days" (nothing purges automatically). Task 6 "How deletion works" is not approved: skipped.
- ContextBar stays with the Library plan.
