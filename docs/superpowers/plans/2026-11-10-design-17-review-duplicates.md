# Design 17: Review Duplicates — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Replace the scrolling list of duplicate cards with the design's review layout: a group list on the left and a side-by-side comparison on the right, with a chosen keeper per group, Skip, Merge and a keyboard path.
**Design:** `lumina.pen` frame `PwE8h` "Review Duplicates" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (Chip `aisTo` for the filter row, Sidebar, tokens) and Design 09 (the empty-state shell, which this view already uses through `render_empty_state`) and Design 01 Task 1 (`page_header`).
**Icons (fetched in 00 Task 1):** wand-sparkles (Auto-select best), combine (Merge), layers (All chip), equal (Exact match chip), git-compare-arrows (differences bar). Already in `resources/icons`: skip-forward (Skip), images, copy, chevron-left, chevron-right, trash-2.
**App today:**
- `AlbumSelection::Duplicates` renders `views::stacks::render_duplicates` (`src/views/stacks.rs:73-122`, picked at `src/app/render/mod.rs:727-732`).
- That draws an "Exact copies | Similar shots" tab strip (`stacks.rs:91-118`) over one of two bodies:
  - Exact copies: `DuplicatesView` (`src/views/duplicates.rs:14-356`), with a banner and "Merge All Duplicates" (`:56-122`), a card per group with "Exact Match · N copies" and "Merge" (`:127-207`), and 200 px item cards with "Primary"/"Copy" badges (`:210-356`).
  - Similar shots: the stacks review list (`stacks.rs:172-300`).
- The merge actions are at `src/app/library.rs:1128-1225`. The groups come from `Catalog::query_duplicate_groups` (`lumina-core/src/catalog/duplicates.rs:180-268`): exact XXH3 content hash (`lumina-core/src/duplicates.rs:9-25`), sorted by reclaimable bytes descending (`:191`), keeper first by rating, favourite, then earliest (`:234`).
- Tests: `src/tests/duplicates.rs`.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- A merge moves copies to Recently Deleted and stays undoable (the `UndoAction::Trashed` push at `library.rs:1145-1148`). Never hard-delete from this screen.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Page header | Toolbar title "Duplicates" (`render/mod.rs:493`), then the `duplicates-banner` card: "{n} Duplicate Sets ({m} Items)" plus a reclaim sentence (`duplicates.rs:56-100`) | TopBar 67.5 h, px 28. Heading "Review Duplicates" (Inter Tight 20/600, −0.3). Meta mono 10.5 t3 "128 groups · 412 items · 18.4 GB recoverable" | relayout + copy, rendered through `page_header(title, meta, actions)` (01 Task 1; selectors `page-header`, `page-title`, `page-meta`, `page-actions`) |
| Header actions | "Merge All Duplicates" accent pill (`duplicates.rs:101-120`) | "Auto-select best" and "Skip all" (h 32, r 8, px 12, `bg_2`, `line` outline, 12.5/400, icons). "Merge {n} groups" (accent, px 13, 12.5/600 accent-fg). Gap 10 | restyle + 2 new |
| Kind filter | Tab strip of `theme::control` buttons "Exact copies" and "Similar shots" (`stacks.rs:91-118`) | Filter row 51.5 h, px 28. Chips h 28, r full, px 11, gap 8, 12/400: selected `accent_soft` fill, accent outline, t1; others `bg_2`, `line`, t2. "All 128 · Exact match 86 · Similar 42 · Same burst 12 · Resized copies 31" | relayout (kinds are a decision) |
| Sort | fixed order by reclaimable bytes (`catalog/duplicates.rs:191`) | "Sorted by space saved" (mono 10.5 t3) and a 28×28 sort button | new label (button is a decision) |
| Group list | none: every group is drawn in full in one scroll (`duplicates.rs:121-123`) | Left pane 372 wide, `bg_1`, right border `line`. Head 43.5: "GROUPS" (10/600, +0.9 tracking, t3) and "7 of 128" (mono 10.5). Rows 75.5 h with a bottom `line` border. A 2 px marker (accent when selected). Two stacked 40 px thumbnails (r 6, front one has a 2 px `bg_1` outline). Title 12.5/500 (t2, t1 when selected). Meta mono 10.5 t3 "3 copies · 62.1 MB". Kind badge h 20, r 5, mono 10 (Exact: `accent_soft` and accent; others: `bg_3` and t3). Selected row bg `bg_2` | new |
| Compare head | group header "Exact Match · N copies" plus "{size} each · {r} reclaimable" (`duplicates.rs:144-181`) | 55.5 h, px 24. Title Inter Tight 15/600. Kind label mono 10 accent on `accent_soft`. Meta mono 10.5 "3 copies · 62.1 MB · group 7 of 128". Prev/next buttons 30×30, r 8, `bg_2`, `line` | relayout |
| Candidate cards | 200 px cards, 130 px thumbnail, "Primary"/"Copy" badge, file name, size, date, folder (`duplicates.rs:240-354`) | Equal-width cards (flex 1, gap 14), `bg_1`, r 12. The keeper has a 1.5 px accent outline, others 1 px `line`. 400 px image with a top-left tag "ORIGINAL"/"COPY n" (mono 10 on black at 72 %), a "BEST" badge (accent), and the resolution bottom-left | relayout |
| Keeper choice | keeper = first item, fixed (`duplicates.rs:134`); a click only selects (`duplicates.rs:262-272`) | Radio 16 px: selected `accent_soft` fill, accent outline, 7 px dot, "Keeping this copy" in accent 12.5/500; others "Keep this copy" t2 | new |
| Card facts | size, dimensions, date, folder (`duplicates.rs:322-353`) | Key/value rows, gap 8. Key 11.5 t3, value mono 11.5 t1: File size, Format, Edits, Source, Captured (Captured in t3) | relayout |
| Differences | none | Bar `bg_2`, r 10, padding 12×14. "4 differences" 13/500. Chips h 22, r 6, `bg_3`, mono 10.5 t2. Right note mono 11 t3 "Pixel-identical crop · 99.4% visual match" | new |
| Footer | per-group "Merge" button (`duplicates.rs:182-205`) | 63.5 h, `bg_2`, top `line` border, px 24. Summary 13/500 "Keeping 1 of 3 · 2 copies move to Recently Deleted" plus mono 11.5 t3 "· frees 13.9 MB". "Skip group" (h 36, r 9, `bg_3`, `line_2` outline, kbd "S") and "Merge group" (accent, kbd "⏎" on accent-fg at 20 %) | relayout + new Skip |
| Sidebar | `sidebar-nav-Duplicates` today (`views/sidebar/nav.rs:92-94`); key-based `sidebar-nav-duplicates` after 00 Task 5 (update `tests/duplicates.rs:35`, `:60`) | Duplicates row selected, with the count 128 in accent | Foundation Sidebar; check the count colour against the Library frame |
| Inspector | the shared inspector pane can be open next to the view | none shown | keep, hidden by default here (decision) |

## Not in the app yet (needs a product decision)
- **Similar and Resized copies (perceptual matching).** Only byte-identical groups exist. "Similar" and "Resized copies" need a perceptual hash: a new table, scanner work and a migration. That is L and core. Recommendation: separate plan. In this plan the chip row shows only the kinds the app can fill: "All", "Exact match", plus "Same burst" and "RAW + JPEG" counts that link to the existing Similar shots review (`stacks.rs:172`). The stacks view culls with flags, it does not merge, so it keeps its own body.
- **The design contradicts itself:** an "Exact match" group whose copies differ in resolution, size and format. Exact groups are byte-identical (same hash), so their differences can only be folder, captured date, title and edits. Ask the designer to relabel that sample group "Resized".
- **Per-group keeper choice and Auto-select best.** Session state `HashMap<hash, MediaId>` that defaults to today's heuristic (rating, favourite, earliest), and "BEST" marks the heuristic's pick. Size S, UI only. `merge_duplicate_group(keep, trash)` already takes any keeper. Recommendation: build it here.
- **Skip group / Skip all.** For the session only (hide until the next refresh) it is S. Persisted "not duplicates" needs a core table and a migration (M). Recommendation: session-only here, and persistence in a separate plan if users ask.
- **Sort button.** Only one sort exists, and it matches "space saved". Recommendation: show the label only and drop the button until there is a second sort.
- **"Source" (AirDrop, WhatsApp).** The app does not know where a file came from. Recommendation: show the camera (`make`/`model`, `lumina-core/src/lib.rs:139-140`) or else the folder (`parent_dir`), labelled "Source".
- **"Edits: 3 adjustments".** The app knows whether an item has edits (the cached `edited_ids`). It does not keep an adjustment count without reading each edits row. Recommendation: show "Edited" or "None".
- **"Pixel-identical crop · 99.4% visual match".** Needs perceptual comparison. Recommendation: drop it with the Similar plan.
- **Group title ("Forest Path, Sintra").** Use the keeper's `title`, otherwise its file stem.

## App has, design lacks
- The "Similar shots" review: RAW+JPEG pairs and bursts, with flag actions and keyboard hints (`stacks.rs:172-300`). Keep it, reached from the chip row. It is not merged into this screen.
- Double-click a card to open the viewer (`duplicates.rs:263-265`). Keep it, with ⌘↓ as its keyboard path (↵ is Merge in this view, Task 5).
- The folder path on every card (`duplicates.rs:345-352`). Keep it as a "Folder" fact row: two copies often differ only by folder.
- The ⌘K commands `merge_all_duplicates` (Merge All Duplicates), `show_exact_copies` (Show Exact Copies) and `show_similar_shots` (Show Similar Shots) (`app/command_palette.rs:327-345`). Keep them all.

## Review Focus
- Zero groups after the last merge: the Design 09 empty state ("No Duplicates Found", `render/grid.rs:806-810`). The selected group index is reset, never out of bounds.
- Thousands of groups: the group list is a `uniform_list` (virtualised, as the stacks list is). Thumbnails come from the cache only and never decode on the UI thread. "7 of 128" stays exact.
- A group of 6+ copies: the cards keep a 180 px minimum width and scroll horizontally. The keeper card is never pushed off without a scroll affordance.
- Keyboard: ↑/↓ move between groups, ←/→ or 1–9 choose the keeper (never a rating), S skips, ↵ merges, and ⌘Z undoes the merge (Task 5). While a modal is open these keys are absorbed (AGENTS.md §6.3).
- Long titles or folder names: truncate with ellipsis in the list rows and the compare head, and show the full text in a tooltip.

## Tasks
### Task 1: Master–detail layout and selection state
**Files:** Modify `src/views/duplicates.rs` (rewrite; split it into `views/duplicates/{mod,list,compare}.rs` to stay under about 500 lines), `src/app/stacks.rs:23-95` (selected group index, keeper map, skipped set) / Test `src/tests/duplicates.rs`
- [ ] Write a failing test `duplicates_review_lists_groups_and_compares_selected`. With two groups, `duplicates-group-list` is 372 ±1 px wide, there are `duplicates-group-row-0..1`, and `duplicates-compare` shows `duplicate-candidate-{id}` for each item of group 0. Clicking row 1 switches the candidates.
- [ ] Implement: the header through `page_header` (heading, meta, actions), the list pane and the compare head with the values from the table. Keep the existing selectors `duplicates-view` and `duplicate-card-{id}` (rename the latter to `duplicate-candidate-{id}` only with a test update noted in the commit).
- [ ] Run `cargo test -p lumina-ui-gpui duplicates`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): duplicates review layout`, staging explicit paths.

### Task 2: Keeper choice, Auto-select best, merge footer
**Files:** Modify `src/views/duplicates/compare.rs`, `src/app/library.rs:1128-1225` / Test `src/tests/duplicates.rs:51-98`
- [ ] Write a failing test `choosing_a_keeper_merges_the_others`. Click `duplicate-keep-{second_id}`, then `duplicates-merge-group`: the second item stays active, the first is trashed, and ⌘Z restores it. `duplicates-auto-best` resets the keeper to the heuristic pick. The footer reads "Keeping 1 of 2 · 1 copy moves to Recently Deleted".
- [ ] Implement: the radios, the BEST badge, the footer summary with real freed bytes (the sum of non-keeper `file_size`), "Merge group", and "Merge {n} groups". Merge all honours chosen keepers (core `merge_all_duplicates` needs a keeper map argument, S).
- [ ] Run `cargo test -p lumina-ui-gpui duplicates`; expect pass.
- [ ] Gate and commit `feat(design): pick the copy to keep`.

### Task 3: Skip group / Skip all (session)
**Files:** Modify `src/app/stacks.rs`, `src/views/duplicates/*` / Test `src/tests/duplicates.rs`
- [ ] Write a failing test `skip_group_moves_on_and_skip_all_empties_the_review`. `duplicates-skip-group` advances the selection, and the count "1 of 1" updates. `duplicates-skip-all` shows the empty state. Nothing changes in the catalog (the item count is unchanged).
- [ ] Implement session-only skipping, cleared by `refresh_duplicates`.
- [ ] Run `cargo test -p lumina-ui-gpui skip_group`; expect pass.
- [ ] Gate and commit `feat(design): skip duplicate groups`.

### Task 4: Differences bar and kind chips
**Files:** Modify `src/views/duplicates/compare.rs`, `src/views/stacks.rs:73-122` / Test `src/tests/duplicates.rs`
- [ ] Write a failing test `differences_list_only_fields_that_differ`. Two exact copies in different folders give one chip, "Folder", and the headline "1 difference". The chip `duplicates-kind-similar` opens the Similar shots body (`stacks.tab == Similar`).
- [ ] Implement: compare dimensions, size, format (extension), edited, source and folder. Replace the tab strip with Foundation `chip(...)` items: "All", "Exact match" and the Similar shots link with its count. Keep `Role::TabList` and `Role::Tab` semantics on the chips.
- [ ] Run `cargo test -p lumina-ui-gpui differences`; expect pass.
- [ ] Gate and commit `feat(design): duplicate differences and kind chips`.

### Task 5: Keyboard path
**Files:** Modify `src/app/keymap/mod.rs` (a `handle_duplicates_review_key` called before the global Enter handling at `:303-315` and the culling keys at `:320`; skipped when a text field or modal has focus), `src/app/command_palette.rs:327-345` (add Skip Group, Merge Group, Auto-select Best) / Test `src/tests/duplicates.rs`

Key contract (lead decision): while `selection == Duplicates` on the Exact view, the review handler runs first and consumes its keys. ↑/↓ move between groups; ←/→ and digits 1–9 pick the keeper and do **not** set a rating (the culling handler would otherwise rate the selection, `app/culling.rs:276-283`); S skips; ↵ merges the shown group (instead of opening the viewer, `keymap/mod.rs:303-315`); ⌘↓ opens the focused candidate in the viewer; ⌘Z undoes through the existing global path.
- [ ] Write a failing test `duplicates_keyboard_review`. After focusing the app, `down` selects group 1, `2` keeps the second copy, `s` skips, `enter` merges (and `is_fullscreen_viewer` stays false), and `secondary-z` undoes. With the delete modal open, `enter` does not merge.
- [ ] Write a failing test `digit_picks_keeper_without_rating`: with a candidate selected, `2` makes the second copy the keeper and every item's `rating` in the catalog is unchanged.
- [ ] Implement the handler and add the three palette commands (gated on `selection == Duplicates`). On the Similar shots tab the existing stacks keys apply unchanged.
- [ ] Run `cargo test -p lumina-ui-gpui duplicates_keyboard`; expect pass.
- [ ] Gate and commit `feat(design): keyboard review for duplicates`.

Manual visual checks: the keeper outline, the badge colours, the stacked list thumbnails and the kbd chips. Compare them against `PwE8h.png`. Check the 400 px preview sharpness: if the thumbnail cache size looks soft at 400 px, measure before raising the cache resolution (AGENTS.md §10).

## Decisions (2026-11-13, "go with your recommendations")
- Perceptual matching ("Similar", "Resized copies", visual-match note): separate plan. The chip row shows "All", "Exact match" and a link to the existing Similar shots review with its count.
- The design's "Exact match" group with differing resolution/size/format is sample data; exact groups differ only by folder, date, title and edits. Designer to relabel it "Resized".
- Per-group keeper choice and Auto-select best: built here, session state `HashMap<hash, MediaId>` defaulting to the existing heuristic; "BEST" marks the heuristic pick.
- Skip group / Skip all: session only, cleared by `refresh_duplicates`. Persisted "not duplicates" is a separate plan if asked.
- Sort: label "Sorted by space saved" only, no button until a second sort exists.
- "Source": the camera (make/model), else the folder.
- "Edits": "Edited" or "None".
- Group title: keeper's title, else its file stem.
- Inspector: hidden by default on this view.
- Keep the Similar shots review, double-click/⌘↓ to open the viewer, the Folder fact row, and the three existing ⌘K commands.
