# Design 03: Search — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Move search into the sidebar field and give an active search its own header (a token query bar, a result count, Save as Smart Album, "Narrow by" chips, Top matches) above the existing result grid, using only query features the core parser has.
**Design:** `lumina.pen` frame `XKkZe` "Search" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::chip` with `ChipKind::Ghost`, Sidebar with its search slot). Task 1 depends only on 00. Tasks 2–6 also depend on Design 01 Task 4 (filter-token labels, reused here). Design 01 Task 1 lands after this plan's Task 1.
**Icons (fetched in 00 Task 1):** file-image, aperture, video, calendar (calendar is shared with 01). sun is not needed while golden hour is dropped (decision 4). map-pin, sparkles, star, pencil, chevron-right, x and search already exist in `resources/icons/`.
**App today:**
- `crates/lumina-ui-gpui/src/views/search_bar.rs:9-111`: 300×28 field in the toolbar (`app/render/toolbar.rs:208`), placeholder "Search — try camera:sony iso:>3200", inline result count + clear ✕ (66-102). Suggestions dropdown (118-211): recent searches (rotate-ccw icon), tag and operator suggestions, explanation footer (`search-explanation`, role Status).
- `crates/lumina-ui-gpui/src/app/search.rs:1-390`: suggestion list (`SEARCH_FIELD_SUGGESTIONS` 17-46), recent history (145-156, 352-377), `toggle_query_token` / `clear_query_filters` (212-250), `scope_search_to_library` (319-331: a query moves the sidebar to Library), `commit_search` (379-386).
- `crates/lumina-ui-gpui/src/app/keymap/mod.rs:242-247`: ⌘F focuses the field and clears the query. `app/keymap/modals.rs:419-457`: Esc layers (hide suggestions, then blur), ↵ applies the highlight or commits, ↑/↓ move the highlight.
- `crates/lumina-ui-gpui/src/views/command_palette.rs:1-216` + `app/command_palette.rs`: ⌘K palette, 640 wide. It holds Save Search as Smart Album… (`CommandId::SaveSearch`, 860-861, enabled only with a query, 1000; runs `save_current_search_as_album`, `app/album_builder_actions.rs:58-70`), quick filters and Clear Filters (1388-1389), GO TO albums/years.
- `crates/lumina-ui-gpui/src/app/render/grid.rs:786-792, 990-1035`: "No matches for these filters" empty state with removable term chips.
- Core grammar, `crates/lumina-core/src/search/parse.rs:52-126` (documented in `docs/search.md`): free text (FTS5), `tag:` `year:` `month:` `date:` `camera:` `lens:` `city:` `country:` `iso:` `f:` `rating:` `is:favorite|video|photo|raw|trashed|untagged|unrated|unlabeled|pick|reject|unflagged` `has:location|tag|color|rating` `color:` `within:`, and `-` exclusion. Results sort by the chosen sort field (`search/mod.rs:136, 392, 411`); there is no relevance order. There are no people/faces and no time-of-day filter.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- Every token chip is a query token the parser already understands. Nothing on screen may suggest people recognition or semantic search (AGENTS §15).
- Visual properties (colour, radius, type) are checked by hand against `XKkZe.png`. Tests cover selectors, bounds, focus and the query string.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Sidebar search field | not in the sidebar; 300×28 in the toolbar (`toolbar.rs:208`, `search_bar.rs:19-40`) | full sidebar width, 34 h, r8, padding 0 10, gap 8; idle: bg-0, outline line, `search` 14, "Search library" Inter 12.5 t3, shortcut mono 10 t3 right; active: bg-3, outline 1 px accent, query text t1 | relayout + restyle |
| Shortcut hint | none; ⌘F focuses (`keymap/mod.rs:242`) | "⌘K" | copy: show **⌘F** (⌘K is the palette, see decision 1) |
| Sidebar selection while searching | Library row highlighted (`search.rs:319-331`) | no row highlighted | change: no highlight while a query shows; the Back trail logic stays |
| Search header | none; the grid just filters | 131.5 h, padding 0 28, gap 13, bottom border line | new; replaces `page_header` (Design 01), which hides while the Search screen shows |
| Query bar | the field text, mono only inside chips of the empty state | 48 h, bg-1, r11, outline line-2, padding 0 14, gap 9; `search` 16 t3; tokens gap 7 | new |
| Connector words | none | Inter 13.5 t3: "photos of", "in", "at", "during" | new (derived from token family, decision 3) |
| Token chips | removable term chips only in the empty state (`grid.rs:990-1035`) | 26 h, accent-soft, outline 1 px accent, r7, padding 0 9, gap 6; icon 12 accent (map-pin, sun, calendar; avatar 16 for a person); label Inter 12.5/500 accent; `x` 11 | new |
| Caret | text input caret (`app/text_input.rs`) | 1.5×18 accent after the last chip | restyle (typing continues after the chips) |
| Result count | plain number inside the field (`search_bar.rs:73-76`) | right side, "284 results" mono 10.5 t3 | relayout + copy |
| Save as Smart Album | palette only (`command_palette.rs:860`) | 28 h, bg-3, outline line-2, r7, padding 0 10, gap 6, `sparkles` 12, Inter 11.5 t1 | new button, same action |
| Narrow by row | quick filters only in the palette (`command_palette.rs:1388`) | "Narrow by" Inter 11 t3, gap 8; chips 28 h transparent, outline line, r999, padding 0 11, icon 13 + Inter 12 t2: "ƒ < 2.0" (aperture), "videos only" (video), "rated 4+" (star), "edited" (pencil), "RAW" (file-image) | new (`theme::chip` with `ChipKind::Ghost`: transparent, `line` outline, t2 text) |
| TOP MATCHES label | none | Inter 9.5/600, tracking 1, t3, gap 11 to the cards | new |
| Top match cards | none | 4 equal cards gap 10; 64 h, bg-1, outline line, r9, padding 0 12, gap 11; 36 circle (bg-3 with 16 icon, or avatar); title Inter 13/500 t1; sub mono 10 t3 "Place · 1,204 photos"; `chevron-right` 14 | new (Place, Smart Album, Moment kinds; Person dropped) |
| Photos section head | none (the month band shows instead, `grid.rs:671-711`) | "Photos" Inter Tight 15/600 −0.2 + count mono 10 t3, gap 9; right "Best match ▾" Inter 11.5 t2 | new; the sort label shows the real sort |
| Result grid | square wells, 16 padding, 10 gap (`views/grid.rs:12-30`) | 6 squares per row, 208 h, gap 8, r4, padding 22 28 0 28 | restyle via Design 01 Task 6 (one grid; Library's gap 6 used here too, see Review Focus) |
| Face boxes on tiles | none | 52×52 outline 1.5 px accent, r4, at 14,14 | drop (no face detection) |
| Suggestions dropdown | 320 w under the field (`search_bar.rs:135-151`) | not in frame | keep, anchored under the query bar, full bar width |
| Explanation footer | `search-explanation` (`search_bar.rs:197-208`) | not in frame | keep inside the dropdown |
| No-match state | `grid.rs:786-792` | not in frame | keep under the search header; the header still shows "0 results" |

## Not in the app yet (needs a product decision)
1. **⌘K on the search field.** The frame labels the field ⌘K. The app binds ⌘K to the command palette, which AGENTS §11 names as the keyboard path for every global action. **Recommend: drop from design.** The field shows ⌘F. Optional follow-up: palette rows for "Search for '…'" when the typed text matches no command.
2. **People ("photos of [Mara]", Person top match, avatars, face boxes).** Size **L**, core work: face detection, clustering, naming UI, storage and privacy copy. Nothing exists today. **Recommend: drop from design** until a people plan is approved.
3. **Natural-language query bar.** Typing "Mara in Lisbon, golden hour" yields the chips in the frame. Size **L** if it means parsing prose. **Recommend: build the display half only (size S, UI only).** Render each parsed token as a chip, with a connector word from its family: `city:`/`country:` → "in", `date:`/`month:`/`year:` → "during", `camera:`/`lens:` → "with", rating → "rated", media type → none. Free text stays plain text. The stored query string keeps the existing grammar, so saved smart albums, history and tests are unchanged. Typing prose stays free-text FTS.
4. **"golden hour".** No time-of-day or sun-position field exists. **Recommend: drop from design.** If wanted, a separate core plan adds `time:` (local hour range) or derives golden hour from capture time and GPS offline.
5. **"edited" narrow chip.** The app knows which items are edited (`edited_ids`, used by the grid badge) but the parser has no `is:edited`. Size **S**, core work: a parser arm, the SQL clause over the edits table, a `docs/search.md` row and a suggestion entry. **Recommend: build in this plan** (Task 4), or drop the chip if the core change is unwanted.
6. **"Best match" order.** Results have no relevance ranking. FTS5 `bm25()` could provide one for free-text queries only. **Recommend: drop the label.** The section head shows the current sort ("Date taken ▾") and opens the existing sort menu. Relevance ranking would be a separate core plan with a benchmark (AGENTS §10 gate: structured query under 5 ms).
7. **Top matches.** Size **M**, UI plus one cached engine read. Feasible kinds: **Place** (cities matching the text, with counts), **Smart Album** (user smart albums whose title matches, count already cached), **Moment** = month (a month name or year in the query, count from month buckets), and **Tag** (from `library_tag_counts`). **Recommend: build in this plan** with those four kinds and at most 4 cards; hide the row when none match. A click navigates (place → `city:` token, album → select album, month → `month:`+`year:` tokens, tag → tag view).

## App has, design lacks
- **Suggestions dropdown, recent searches, Clear Recent Searches, explanation footer.** Keep. They are the keyboard path into the grammar.
- **Clear search ✕.** Keep, as the last element in the query bar, and also on Esc when the bar is focused with suggestions hidden. Today Esc only blurs, so this is a change: confirm, or keep Esc = blur and rely on ✕ and ⌘⌫.
- **Palette Save Search and quick filters.** Keep. The new button and chips call the same functions.
- **"No matches for these filters" state with removable chips.** Keep below the header.

## Review Focus
1. **Long query (12 tokens, a 60-character place name):** the token row scrolls horizontally inside the bar, keeping the caret visible. Count and Save stay pinned right. Chip labels truncate at 180 px with a tooltip carrying the full value.
2. **Zero results / one result:** "0 results" and "1 result". Save stays enabled (an empty smart album is valid). Top matches still render if any match.
3. **Keyboard:** ⌘F focuses the sidebar field and the bar mirrors it. ← at the caret's start selects the last chip, ⌫ removes it, Tab moves through chips, Save, the Narrow chips and the cards, ↵ activates. Esc layers stay as in `modals.rs:421-449`. Modal key absorption means typing in the bar never deletes media.
4. **Narrow window:** Top match cards drop to 2 per row below ~1100 px content width. The Narrow by row scrolls.
5. **Search while a modal is open, or with the map shown:** a query hides the map (today Places + query goes to Library), and the Save button is disabled while compare or edit mode holds the view.

## Tasks
### Task 1: Search field in the sidebar
**Files:** Modify `crates/lumina-ui-gpui/src/views/search_bar.rs:9-111`, Foundation's sidebar (`crates/lumina-ui-gpui/src/views/sidebar/mod.rs`, search slot), `crates/lumina-ui-gpui/src/app/render/toolbar.rs:208`, `crates/lumina-ui-gpui/src/app/search.rs:319-331` / Test `crates/lumina-ui-gpui/src/tests/search_field.rs` (new)
- [ ] Write failing test `search_field_lives_in_the_sidebar`: `search-input` bounds lie inside the sidebar (x < 252), height 34, width = sidebar width − 24. It is absent from the header. Placeholder reads "Search library". `search-shortcut` reads "⌘F".
- [ ] Write failing test `cmd_f_focuses_the_sidebar_field_and_typing_searches`: ⌘F, type `city:Lisbon`, ↵ gives `search_query == "city:Lisbon"`, records a recent search, and highlights no sidebar row.
- [ ] Implement: the field takes the sidebar slot with the idle/active values in the table. The suggestion dropdown anchors to the query bar when the search header shows, otherwise under the field. No sidebar row is highlighted while `search_query` is non-empty (the selection value can stay `AllLibrary` for navigation).
- [ ] Run `cargo test -p lumina-ui-gpui search`; expect pass (the existing `search_*` suggestion tests too).
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): search field in sidebar`.

### Task 2: Search header with the token query bar
**Files:** Create `crates/lumina-ui-gpui/src/views/search_header.rs`; Modify `crates/lumina-ui-gpui/src/app/render/mod.rs:711-744`, `crates/lumina-ui-gpui/src/app/search.rs` (token → chip model, reusing Design 01's filter labels) / Test `crates/lumina-ui-gpui/src/tests/search_header.rs` (new)
- [ ] Write failing test `query_tokens_render_as_chips_with_connectors`: query `city:Lisbon month:jun year:2025 tram` renders `search-token-0` "Lisbon" (connector "in") and `search-token-1` "Jun 2025" (connector "during"; label from Design 01's filter-label function), with "tram" as plain text. `search-result-count` reads "{n} results". The header height is 131.5 (±0.5).
- [ ] Write failing test `removing_a_token_chip_edits_the_query`: clicking `search-token-remove-0` leaves `month:jun year:2025 tram`. With the bar focused and the caret at the start of the typed text, ⌫ removes the last chip.
- [ ] Write failing test `search_screen_rule_free_text_or_focus`: `tram` shows `search-header` and no `page-header`. `city:Lisbon` alone shows `page-header` and the Library chip row, with no `search-header`. Focusing the empty field with ⌘F shows `search-header`. Blurring with no query restores `page-header`.
- [ ] Implement the decided rule: free text in the query, or a focused search field, shows the Search screen (this header, with `page_header` hidden); filter tokens alone show the Library with its chip row (Design 01 Task 4). Values from the table. Month+year tokens merge into one chip. The existing text input renders after the chips.
- [ ] Run `cargo test -p lumina-ui-gpui search_header`; expect pass.
- [ ] Gate and commit `feat(design): search header and token bar`.

### Task 3: Save as Smart Album button and Narrow by chips
**Files:** Modify `crates/lumina-ui-gpui/src/views/search_header.rs`, `crates/lumina-ui-gpui/src/i18n.rs` / Test `crates/lumina-ui-gpui/src/tests/search_header.rs`, `crates/lumina-ui-gpui/src/tests/smart_album_builder.rs:437`
- [ ] Write failing test `save_button_opens_the_prefilled_builder`: clicking `search-save-smart-album` opens the album builder with the query rule (same assertions as `save_search_opens_prefilled_builder_and_creates_album`).
- [ ] Write failing test `narrow_chips_toggle_query_tokens`: clicking `search-narrow-video` appends `is:video` and the chip shows on. Clicking it again removes the token. `search-narrow-fnumber` toggles `f:<2`. `search-narrow-rating` toggles `rating:>=4`. `search-narrow-raw` toggles `is:raw`.
- [ ] Implement: the button calls `save_current_search_as_album`, labelled from `command.save_search` without the ellipsis. Narrow chips call `toggle_quick_filter`. Chips are `ChipKind::Ghost`. On-state uses accent-soft + accent outline (same as token chips). Labels: "ƒ < 2.0", "Videos only", "Rated 4+", "RAW" (and "Edited" after Task 4).
- [ ] Run `cargo test -p lumina-ui-gpui save_search narrow_chips`; expect pass.
- [ ] Gate and commit `feat(design): save and narrow controls in search header`.

### Task 4 (decision 5): `is:edited` filter
**Files:** Modify `crates/lumina-core/src/search/parse.rs:82-98`, `crates/lumina-core/src/search/mod.rs`, `crates/lumina-core/src/search/explain.rs`, `docs/search.md:31-37`, `crates/lumina-ui-gpui/src/app/search.rs:17-46` / Test `crates/lumina-core/tests/core_tests.rs`, `crates/lumina-ui-gpui/src/tests/search_header.rs`
- [ ] Write failing core test `is_edited_matches_items_with_edits_or_rotation`: three seeded items, one with a `media_edits` row and one with `rotation_quarters` = 1, give two results. `-is:edited` returns the third. `explain()` says "Edited".
- [ ] Implement: a parser arm, the SQL clause `(m.rotation_quarters != 0 OR EXISTS (SELECT 1 FROM media_edits e WHERE e.media_id = m.id))` (`catalog/schema.rs:76, 167-175`; `media_id` is the primary key, so check `EXPLAIN QUERY PLAN` shows a PK lookup), the explanation phrase, a docs row, a suggestion entry, and the Narrow chip.
- [ ] Run `cargo test -p lumina-core is_edited` and `make bench` (structured query gate); expect pass.
- [ ] Gate and commit `feat(search): is:edited filter`.

### Task 5: Top matches
**Files:** Create `crates/lumina-ui-gpui/src/views/top_matches.rs`; Modify `crates/lumina-core/src/lib.rs` (cities matching a prefix, with counts), `crates/lumina-ui-gpui/src/app/search.rs` / Test `crates/lumina-core/tests/core_tests.rs`, `crates/lumina-ui-gpui/src/tests/search_header.rs`
- [ ] Write failing core test `matching_places_returns_city_country_and_count`: the seed with Lisbon×3 and Porto×1, text "lis", gives `[("Lisbon","PT",3)]`.
- [ ] Write failing UI test `top_matches_show_place_album_month_and_navigate`: query "lisbon june" shows `top-match-place-0` and `top-match-month-0`. Clicking the place card replaces "lisbon" with `city:Lisbon`. Clicking a smart-album card selects it. With no match, `top-matches` is absent.
- [ ] Implement: matches computed off the UI thread when the query changes (debounced with the existing view refresh) and cached by query. At most 4 cards, in order Place, Smart Album, Moment, Tag. Card values from the table. Cards are keyboard-focusable with role Button and a label such as "Place, Lisbon, Portugal, 1,204 photos".
- [ ] Run `cargo test -p lumina-core matching_places` and `cargo test -p lumina-ui-gpui top_matches`; expect pass.
- [ ] Gate and commit `feat(design): search top matches`.

### Task 6: Photos section head
**Files:** Modify `crates/lumina-ui-gpui/src/views/search_header.rs` (or the grid section band, `app/render/grid.rs:671-711`) / Test `crates/lumina-ui-gpui/src/tests/search_header.rs`
- [ ] Write failing test `results_head_shows_count_and_opens_sort`: `search-results-title` "Photos" (or "Photos & Videos" when the results mix both), `search-results-count` equals `displayed_count()`, and clicking `search-results-sort` opens `sort-field-date`.
- [ ] Implement: the head replaces the month band while the search header shows. The sort label is the current sort field + ▾ (decision 6).
- [ ] Run `cargo test -p lumina-ui-gpui results_head`; expect pass.
- [ ] Gate (including `make ui-smoke`) and commit `feat(design): search results head`.
