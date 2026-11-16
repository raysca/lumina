# Design alignment: index and build order

Aligns the app to `lumina.pen` (18 screen frames, 3 components, 21 tokens). One plan per
frame, plus Foundation. Each plan lists the current app against the frame, element by element,
with file:line citations, and separates restyling from features the app doesn't have yet.

Status: written by six planning agents, checked by two independent reviewers (every cited
file:line opened, design values checked against the exports), review fixes applied, and
cross-plan ownership settled by the lead. No code has changed.

## Plans and order

| # | Plan | Frame | Tasks | Needs |
|---|------|-------|-------|-------|
| 00 | [Foundation](2026-11-10-design-00-foundation.md) | tokens, NavItem, Chip, DownloadRow, Sidebar | 5 | — |
| 03 | [Search](2026-11-10-design-03-search.md) Task 1 only | `XKkZe` | (1) | 00 |
| 01 | [Library](2026-11-10-design-01-library.md) | `t0zQL` | 9 | 00, 03 T1 |
| 03 | [Search](2026-11-10-design-03-search.md) Tasks 2–6 | `XKkZe` | 5 | 01 T4 |
| 09 | [States — Empty](2026-11-10-design-09-states-empty.md) | `R5RGv` | 6 | 00 |
| 10 | [States — Error](2026-11-10-design-10-states-error.md) | `NWldm` | 6 | 00, 09 |
| 11 | [First Run](2026-11-10-design-11-first-run.md) | `DiFtb` | 4 | 01 T1, 09 T1 |
| 02 | [Detail](2026-11-10-design-02-detail.md) | `K9cBD` | 6 | 00 |
| 06 | [Video Player](2026-11-10-design-06-video-player.md) | `F6sXRs` | 5 | 02 |
| 04 | [Videos + PiP](2026-11-10-design-04-videos-pip.md) | `DoXZy` | 5 | 01 T1 |
| 05 | [Albums](2026-11-10-design-05-albums.md) | `a0sLrG` | 5 | 01 T1, 09 T3 |
| 07 | [Album Management](2026-11-10-design-07-album-management.md) | `w4zpqh` | 5 | 05, 01 T5 |
| 08 | [Tags Management](2026-11-10-design-08-tags-management.md) | `L3F8Cs` | 6 | 01 T1, 05 T2 |
| 16 | [Smart Album — New](2026-11-10-design-16-smart-album-new.md) | `KweSw` | 6 | 00 |
| 17 | [Review Duplicates](2026-11-10-design-17-review-duplicates.md) | `PwE8h` | 5 | 01 T1, 09 |
| 12 | [Import from URL](2026-11-10-design-12-import-from-url.md) | `D119th` | 4 | 00 |
| 13 | [Downloads](2026-11-10-design-13-downloads.md) | `ttuOX` | 6 | 01 T1 |
| 14 | [Downloads — Toolbar Panel](2026-11-10-design-14-downloads-toolbar-panel.md) | `eO3Hw` | 3 | 13 |
| 18 | [Settings — General](2026-11-10-design-18-settings-general.md) | `l85pQN` | 4 | 01 T1 |
| 15 | [Settings — Downloads](2026-11-10-design-15-settings-downloads.md) | `g52QWL` | 4 | 18 T1, 13 |

Groups that can run in parallel after 00 and 01 Task 1, as long as they don't touch the same files:
viewer (02, 06, 04), albums (05, 07, 08, 16), states (09, 10, 11, 17), downloads (12, 13, 14),
settings (18, 15).

## Shared ownership (settled)

- Window shell (full-height 252 px sidebar, main column, traffic-light inset), sidebar
  selectors `sidebar-nav-{key}`, the `new-album` row: 00 Task 5.
- `page_header` (`views/page_header.rs`, `page-header`/`page-title`/`page-meta`/`page-actions`):
  01 Task 1; every main-area page uses it. `page_tabs`: 05 Task 2.
- Empty-state shell with `Hero` size: 09 Task 1. `rule_label`: 09 Task 3.
- Settings shell, category order General, Library, Downloads, Storage, About, and the row
  widgets: 18. Library, Downloads and Storage categories: 15.
- Download popover rows: 14. DownloadRow widget: 00, used by 13's screen. Keyboard order per
  surface (`download_display_order`): 13 Task 5. Pause: 13 Task 1.
- Grid check circles and selection dim: 01 Task 5. Keyword tree removal: 08 Task 1.
- Icons and fonts: a single download step in 00 Task 1, which needs your permission.

## Decisions for you

Each plan's "Not in the app yet" section gives sizes and its recommendation. The ones that
change scope:

**Before Foundation starts**
1. One download step (00 Task 1): the fonts Inter 4.1 (Regular, Medium, SemiBold), Inter
   Tight (SemiBold) and JetBrains Mono 2.304 (Regular), all under the OFL (about 30 MB of
   archives, 1.4 MB kept), and 52 Lucide icons from `lucide-static@0.460.0` (ISC, the
   version already in the repo, about 25 KB).
2. Lift t3 to #7C7C88 for legibility (00 D1), keep the neutral focus ring (D2), drop People
   from TAGS (D4), "All Media" and US "Favorites" (D5).

**Big design features (recommended as separate follow-up plans, not part of the restyle)**
3. Mosaic grid grouped by day, Years/Months/Days/Moments zoom, day headers with place
   (01, size L, "01b"). Until then the zoom pill gets only the − and + buttons.
4. Timeline rail that scrolls to a date instead of filtering by year (01, M).
5. Settings as a full-window screen: decided yes (18, M–L). Say so if you'd rather keep the modal.
6. Perceptual "Similar" duplicates (17, L), the histogram (02, M), trim and captions (06, L).

**Recommended drops (conflict with AGENTS.md or have no backing)**
7. People / faces everywhere (02, 03, 05, 08), auto-tagging (08), golden hour (03),
   sharing and shared albums (05, 07, 16), account / diagnostics / crash reports / auto-update
   (18), clipboard watching and Downloads-folder auto-import (15), HEVC conversion and
   subtitles (12, 15), "Download original" and "Create proxy" (10), Connect a camera and
   Apple Photos import (11).
8. Design copy that would overclaim: 30-day trash retention (09), "Faces never leave your
   Mac" (11), iCloud sources (13).

**Small additions the plans build unless you say no**
9. Recently Added row (S), Pause and Pause all downloads (S–M), simultaneous-downloads setting
   (S), `is:edited` and `is:4k` search terms (S), album cover via Set as Cover (needs a
   migration), tag merge keeping aliases, the library-size meter (M).

## Found while planning (fix regardless of the redesign)

- A failed duplicate merge shows a success-style toast with a check mark
  (`app/library.rs:1174`, `:1213`), against AGENTS.md §2. Plan 10 covers it, but it is a
  bug today and worth fixing on its own first.
- The smart album builder's live preview queries the catalog on the UI thread on every edit
  (`album_builder_actions.rs:167-185`). Plan 16's Review Focus says to measure it.
- In `lumina.pen`: the Settings frames repeat "Search settings" in the shortcut slot, the
  Settings PNG shows two selected categories, the Albums frame highlights "Golden Hour" on
  the overview, and Duplicates labels a group "Exact match" whose copies differ in size.

## Testing gaps to revisit (added as plans finish)

Things shipped but not verified. Come back to these before a release; each line says how to close it.

**Plan 09 — States — Empty (fc47326…0280574)**
- [x] By-eye comparison of the four empty states against `R5RGv.png` (done 2026-11-13, live capture). All four match: glyph circle, headline, body, amber primary, mono rule chips with amber icons. Differences, all from plan scope: no drop-target frame around the empty album; search "Drop" chips use an × rather than a per-filter icon (pin, calendar, sun), and the body is "Try a different search." rather than per-term counts; Recently Deleted has no "How deletion works" button.
- [ ] VoiceOver pass on the empty-state buttons and search "Drop" chips (they now set Role::Button and aria labels; never checked with VoiceOver, AGENTS.md §11).
- [ ] Tab-focus test for the empty-state buttons (only click and ⌘K paths are tested).
- [ ] Hero size (`EmptySize::Hero`) has no real caller until plan 11; check the 20 px icon in the 150 px circle looks right then.
- [ ] Smart album chips don't say whether all or any rule must match (design omits it too) — decide, then test.
- [ ] Empty album whose title lookup fails: button shows but ⌘K "Add from Library" doesn't — decide whether to align, then test.
- [ ] Search headline with more than ten terms (digits branch) has no test.
- [ ] Wrap test for many rule chips runs at 900 px without the inspector open; Review Focus asked for sidebar + inspector open.

**Plan 08 — Tags (f5c1977…a92e0ee)**
- [x] By-eye check against `L3F8Cs.png` (done 2026-11-13): table, usage bars, aliases count, detail panel (name and pencil, PREVIEW 2×4, ALIASES chips with + Add, ORGANISE rows, Delete tag with hint) match. Not shown, by plan decision: People, Places, Auto-detected and Unused tabs; Source and Last used columns; Auto-apply; colour picker; New Tag button. The 960 px deep-indent item below was not checked.
- [ ] VoiceOver pass on the keyword table, detail panel and merge picker.
- [ ] Save Tag as Smart Album with tag names containing spaces or quotes has no test.
- [ ] Deeply indented names near the 960 px minimum width can drop below 160 px; check visually.

**Plan 10 — States — Error (in progress)**
- [x] Task 1/2: by-eye check of the error banner and toasts against `NWldm.png` (done 2026-11-13). Layout, colours and shadow match. Two findings below.
- [x] Fixed in cb3534d (`{err:#}` everywhere; Locate banner reworded; downloads keep the outer message on purpose). Remaining polish: the detail can repeat the title in core wording ("creating an album: database is locked"). Was: banner detail shows the outer context, not the cause: a forced album failure read "Could not create album" / "creating an album" (two lines) where the cause was "database is locked". `set_error(format!("…: {err}"))` drops the anyhow chain; 24 sites in `app/` use `{err}` against 37 using `{err:#}`. Switch to `{err:#}` (or show only the root cause) and test one.
- [ ] Toast accent bar does not follow the card's rounded corners: a 3 px div clamps its 9 px radius to 1.5 px, and GPUI's `overflow_hidden` clips to a rectangle, so the bar runs straight past the corner curve (`views/toast.rs:291-301`). Same pattern in `views/stacks.rs:340`. Cosmetic.
- [ ] Task 2: more refusals still show the green success check — culling.rs:237, undo.rs:480, import.rs:305, smart_album_share.rs:42/67, album_builder_actions.rs:42/51, albums.rs:133, album_membership.rs:42/135, stacks.rs:233, slideshow.rs:84, loop_range.rs:93. Convert to `show_warning_toast` and test one.
- [ ] Task 2: no test that a long toast title/subtitle truncates before the action button.
- [ ] Task 3: the stale Retry's `cancel_scan()` is not asserted (LuminaEngine exposes no scan state); a core unit test that `cancel_scan` sets the flag would close it.
- [x] Task 3: by-eye check of the unreachable-library state (done 2026-11-13): matches; with the folder put back, Retry opened the library live.
- [ ] Task 3: VoiceOver check of its Role::Alert.
- [ ] Task 3: clicking a recent row still checks the folder and opens on the UI thread (only Retry moved off it).
- [ ] Task 4: merge-all failure path has no test (same RAISE(ABORT) trigger would cover it).
- [ ] Task 4: a full disk stops the copy; files after it are neither copied nor counted as failed, so "Imported X of Y. N files failed." doesn't add up — consider "N files not copied".
- [ ] Task 4: several failures at once — a newer `set_error` replaces the import detail (pre-existing; Review Focus).
- [x] Task 5: by-eye check of the warn bar and offline card (done 2026-11-13): matches. Known gaps: the design's in-card 220×150 preview with "PREVIEW ONLY" badge was not built, so the second button is Dismiss rather than Keep preview.
- [ ] Task 5: slideshow hiding the card has no test; a missing video (smaller stage box) is untested.
- [ ] Task 5: mounting the drive while the item is on screen doesn't refresh the drive name until the selection changes.
- [ ] Task 5: Locate from the viewer refuses when the viewer was opened from a multi-selection ("Select one missing item").
- [x] Task 6: by-eye check of the playback-failed card and the PiP compact row (done 2026-11-13): both match.
- [ ] Task 6: under the failed card the transport stays live (amber Play button, scrubber, speed); consider dimming or disabling it while `load_error` is set.
- [ ] Accessibility (seen during the pass): with the viewer open, the AX tree still lists the hidden sidebar buttons; text set through AX (`AXSelectedText`) does not reach GPUI fields, only real key events do.
- [ ] Task 6: playback card has no edit-mode/slideshow guard (offline card does); a vanished file not yet marked missing shows "may be damaged" copy instead of "can't be found".
- [ ] Plan 10 housekeeping: views/floating_pip.rs (≈550) and views/viewer/viewport.rs (≈530) are past the ~500-line split guide.

**Plan 11 — First Run (5da2b7d, d1ca889, c457e72, 3c971ca)**
- [ ] By-eye comparison against `DiFtb.png`: not exported (the pencil app had another document open). Glow (box shadow, not a radial gradient), ring, type and spacing checked only against the plan's table and the smoke screenshot.
- [ ] VoiceOver: dormant sidebar rows are Role::Button with an "Unavailable until a library is open" description (GPUI 0.3.11 has no aria_disabled); the busy sample link has a description too. Neither checked with VoiceOver. Swap to aria_disabled when GPUI has it.
- [ ] Middle truncation of recent paths is GPUI's `text_ellipsis_middle`; tests only prove one line inside the row and the full path in the tooltip/marker.
- [ ] No test with 10 recents; the 900×600 test scrolls to the 6th.
- [ ] Focus ring on recents and buttons is a border colour the harness can't observe.
- [ ] Hero size is tested only for "No photos or videos found" in the real window; "Scanning…" and "Your library is empty" (in-library) have no Hero assertion. No smoke screenshot of an empty library (the smoke script always starts with no library).
- [ ] Housekeeping: `tests/palette_and_empty.rs` ≈535 lines; `nav_row` takes 7 positional args (switch to an options struct if one more is added); live sidebar still hardcodes "SMART ALBUMS"/"TAGS" (`views/sidebar/sections.rs:163,246`) while the dormant one uses i18n keys; `nav.rs:28-35` sample counts for no-library are likely dead now that the live sidebar never draws without a library.

**Plan 16 — New Smart Album (886f5a1, b00c3ed, c099262, 57fc017, dde8461, 738f96c)**
- [ ] By-eye comparison against the KweSw frame not done; sizes checked against the plan's table and tests only. The Task 5 smoke screenshot was skipped (no Screen Recording that run).
- [ ] VoiceOver: stars, chips' ×, Pin checkbox have roles/labels but were not checked with VoiceOver.
- [ ] Chip labels ("Date"/"Tag"/"Stars") and builder labels ("Date Captured"/"Keywords"/"Rating") differ on purpose; confirm with design.
- [ ] Design deviations kept: accent caret, no letter-spacing (GPUI 0.3.11), More tile width 68 (not in design), empty-strip copy is ours.
- [ ] Chip × is not a Tab stop; keyboard removal is ←/→ highlight + ⌫.
- [ ] Camera "is" is case-sensitive: "Canon" and "canon" list as two same-named options; NULL vs '' make can also duplicate. Group by display name if it shows up in real libraries.
- [ ] Pin failure shows the "Created" toast next to the error banner.
- [ ] Footer status lines ("These conditions can't run…", "Fix the highlighted conditions"), "Esc"/"⏎", "add keyword…", "Remove <tag>" are not in i18n.
- [ ] Preview cost (≤5.6 ms/edit at 25k, release) is the author's one-off measurement, not a bench gate.
- [ ] Deleting a keyword leaves it in any-of lists (matches the single-keyword rule); a deleted member matches nothing.
- [ ] Housekeeping: rules.rs ≈833, keyword_ops.rs ≈531, keymap/modals.rs ≈573, smart_albums/mod.rs ≈571 lines; `camera_name` lives in app::smart_draft but the inspector imports it; `album_draft.focus` is still pub (focus_on is the intended path).

**Plan 17 — Review Duplicates (346a247, 2c38ea9, ce7cff2, e35adc3, 5347f75)**
- [ ] By-eye comparison against `PwE8h` not done (pencil had another document open); every smoke screenshot was skipped (no Screen Recording). Keeper outline, badge colours, stacked thumbnails, kbd chips and 400 px preview sharpness unchecked.
- [ ] VoiceOver: rows, radios, chips, prev/next and Merge have roles/labels but were not checked with VoiceOver. Prev/next at the ends show dimmed with no disabled state (GPUI 0.3.11); the selected card is visual only (no aria_selected).
- [ ] Design deviations kept: no "+0.9" tracking on GROUPS, ORIGINAL/COPY tag on media_scrim (65 %, not 72 %), list row padding 18 px and thumbnail offset 8 px are ours, group meta size is the total of all copies.
- [ ] Similar/Resized/visual-match need perceptual hashing (separate plan); "All" and "Exact match" show the same exact groups; the Similar chip has no count until that review has loaded once.
- [ ] Skips are session-only and clear on re-entering Duplicates (not on refresh, which runs after every merge). Persisted "not duplicates" is a separate plan if asked.
- [ ] Undo of a merge restores the trashed copies but not the metadata merged onto the keeper (existing behaviour, now applies to the chosen keeper).
- [ ] ORIGINAL/BEST and the group title follow the heuristic pick, not the chosen keeper; "ORIGINAL" may not be the earliest copy.
- [ ] `visible()` filters all groups several times per frame (≈50k hash probes at 10k groups); not measured.
- [ ] Footer "Merge group" ⏎ chip and the Skip "S" hint aren't in a bench or screenshot; no test with 6+ copies for the horizontal scroll.
- [ ] Housekeeping: command_palette.rs ≈1,830 lines needs a split; tests/duplicates_review.rs ≈461 lines.

**Plan 12 — Import from URL (7a690a3, 6e8fbf5, 91d2109, b182a07)**
- [ ] By-eye comparison against `D119th` not done (pencil had another document open): scrim, card shadow, accent/danger field outline, badge and kbd colours, footer bg-2 clipped to the 16 px corners.
- [ ] `positive_soft` alpha (0x26) is estimated, and the token isn't in `pen_tokens_have_design_values`; read `$positive-soft` from lumina.pen and add it.
- [ ] VoiceOver: close, clear, footer buttons have roles/labels; not checked with VoiceOver.
- [ ] Design deviations kept: no title tracking, close icon 15 px, body padding estimated, meta line one mono string (design has uploader in Inter 11.5), duration chip on 70 % black (design 72 %), static checking line (no looping animation helper).
- [ ] Format badge shows the container from `info.extension`, which is hard-coded "mp4" for page links and HLS; resolution/codec badges wait for the quality plan.
- [ ] A check in flight isn't cancelled or closed on a library switch (pre-existing for the prior-download lookup); a late Ready could queue into the new library.
- [ ] No test proves the real job skips inspection when given the info (running it would download); two-line branch read by eye. Title 2-line clamp and long-URL clipping checked by reading only.
- [ ] Inspection can block a background thread up to INSPECT_TIMEOUT (60 s).
- [ ] Housekeeping: app/downloads/mod.rs ≈631 and tests/downloads.rs ≈1,089 lines (already over before); the secondary button and clear button hover borrow `theme::plate()`.
- [ ] Separate plans from the decisions: remote thumbnail, QUALITY select (with Design 15), SAVE TO album.

**Plan 13 — Downloads (086b4b3, 66eba89, b59dd3d, 07f33ea, 6bf9dd5, cd075e2)**
- [ ] By-eye comparison against `ttuOX` not done: section headers, card and compact row spacing, thumbnail placeholder, status bar, filter chip colours, confirm prompt.
- [ ] VoiceOver: list/listitem roles, chips, row buttons (labels name the row) and the row menu have roles; not checked with VoiceOver. Section headers have no heading role.
- [ ] A paused row whose job is still finishing isn't counted toward MAX_RUNNING or the quit prompt; a requeued row keeps the old cancel Arc while waiting; a stale NotCatalogued result loses its message.
- [ ] Throughput is sampled only on job events (no timer), so a stalled job keeps its last speed until the next event.
- [ ] Settings button on the filter bar opens Preferences > General until Design 15; ⌥⌘↑ is documented in network-import.md but not on the shortcut sheet.
- [ ] Clear queue removes every waiting link with no confirmation or undo (explicit action, kept).
- [ ] Untested: restore while the queue is still loading; Clear completed failure revert; keyboard movement with a filter chip on; aria labels (not assertable).
- [ ] Toolbar dropdown keeps five sections until Design 14; completed rows show no file size (not known); no format badge on active rows.
- [ ] Housekeeping: app/downloads/mod.rs ≈624, views/downloads_rows.rs ≈500, tests/downloads.rs ≈1,120 lines.

**Plan 14 — Downloads toolbar popover (b7c03d7, e10efcd, 7e98161)**
- [ ] By-eye comparison against `eO3Hw` not done: popover rendered opaque (no backdrop blur in GPUI), shadow_2xl instead of 0 18 48, track colours, focus bg-2, failure dot hanging 2 px past the 32 px button's corner.
- [ ] VoiceOver: dialog card, List/ListItem rows, labelled buttons; not checked with VoiceOver. `.aria_label` is set twice on row buttons (second should win).
- [ ] Design deviations kept: button 32 px (Design 01) not 34; badge doesn't count failures (the dot does); no title letter-spacing; one body padding/gap; Earlier rows keep ✕ at 25 px; no time left on running rows; no size on done rows.
- [ ] Header settings button hidden until Design 15.
- [ ] A re-pasted link's highlight sets neither focus nor scroll; with many active rows it can sit below the fold (pre-existing).
- [ ] Pause/resume/retry selectors are shared with the Downloads screen; ambiguous only if the popover is opened over the screen with ⌥⌘L.
- [ ] Thumbnails for catalogued active rows are untested (tests hit the placeholder); dates are en-US only (chrono %b).
