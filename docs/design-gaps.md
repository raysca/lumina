# Design alignment: deferred gaps

What the `lumina.pen` alignment work (plans in `docs/superpowers/plans/2026-11-10-design-*.md`
and `2026-11-11-design-01b-mosaic.md`) skipped, dropped or left unverified, so we can come back
to it. Each line names where it came from. Remove an entry when it lands; add one whenever a
task defers something.

Last updated: 2026-11-11 (after plans 00–03, 02 and 01b landed).

## Never verified (applies to everything)
- **Visual check against the frames.** No screen has been compared by eye with its `.pen`
  frame. `make ui-smoke` skips its screenshot without Screen Recording permission and only
  shows the Welcome screen.
- **VoiceOver.** Roles and labels were added to new controls, but none were checked with
  VoiceOver (AGENTS §11, §15).
- **Live interaction.** `make ui-e2e` has never run its interaction steps here. Without Screen
  Recording it FAILS at "could not capture the window" (`scripts/live_ui_automation.sh:263`,
  since dff8d3d) before its Accessibility check can report SKIP; the script should map a
  capture failure to SKIP unless `LUMINA_REQUIRE_UI_PERMISSIONS=1`.
- **Release-mode bench.** Most `make bench` runs were debug; only the `is:edited` and FTS fixes
  have release numbers.

## Product decisions: dropped or split out
| Item | Source | Status |
|---|---|---|
| People (faces, avatars, "photos of", Person top match, face boxes, Keywords/People/Places tags) | 00 D4, 02, 03 dec. 2 | Dropped; needs face detection, schema, privacy copy (L) |
| Histogram in the info panel | 02 | Separate plan (M): 64-bin luminance off-thread, cached per item |
| Library size meter in the sidebar | 00 dec. 1 | Separate small plan; also feeds Settings storage card |
| "Photos" (images-only) sidebar row | 00 dec. 2 | Not built |
| DownloadRow pause and format badge | 00 dec. 5 | Waits for design 13 |
| ⌘K hint on the search field | 00 D3, 03 dec. 1 | Dropped: ⌘K is the palette, field shows ⌘F |
| Natural-language query bar (prose → chips) | 03 dec. 3 | Only the display half built |
| "golden hour" / time-of-day search | 03 dec. 4 | Dropped; would need a `time:` core plan |
| "Best match" relevance order | 03 dec. 6 | Dropped; section head shows the real sort |
| Moments grouping level | 01b dec. 2 | Dropped; pill shows Years · Months · Days |
| Per-view Years/Months cards | 01b dec. 3 | Only where the timeline rail shows |
| Street-level map in the location card | 02 | Impossible offline; ask design to redraw without streets |

## Keyboard and accessibility gaps
- **No Tab traversal** for the top bar buttons, filmstrip thumbs, tag chips, search chips,
  Save/Narrow controls or Top-match cards. Every action has a key or palette path instead.
- **Tag ×:** Backspace in the empty tag field removes only the last tag; other tags are
  mouse-only to remove (also in Get Info). (02 Task 4)
- **Top matches:** ↓ from the bar reaches the cards only after Esc closes the suggestions, with
  no on-screen hint. (03 Task 5)
- **"Rate…"** is in the palette but not the menu bar or shortcut sheet. (02 Task 2)
- **Toast copy:** edit/rotate while a popover is open says "Close the open dialog…"; "menu"
  would fit the Sort and Rate popovers better. Slideshow is refused silently. (02 Task 2)
- **Brand row** has no accessibility role; full-screen traffic-light inset and brand-row window
  drag off macOS untested. (00 Task 5)

## Visual departures (no matching token or API)
- Letter-spacing/tracking everywhere (section titles, headers, results title): GPUI has none.
- Photo shadow and filmstrip scrim use `media_scrim` (~65%) instead of 80% / 55%. (02 Tasks 3, 6)
- Filmstrip current thumb: the 2 px accent border sits inside the 74 px box (picture is 70 px).
- Location card land is clipped to a square, not the 6 px rounded corners.
- Long filenames in the info panel wrap instead of truncating (a viewer test asserts wrap).
- Additions not in the frames: the sidebar toggle in the search header when collapsed; the
  "Videos" results title for all-video results.

## Behaviour limits to revisit
- **Search token bar:** history stores the query reordered (chips first); caret jumps to the
  end when a token closes mid-text; Downloads opened from a search swaps the header; no
  long-query layout test; a mouse-up outside the window can hold the search header until the
  next click. (03 Task 2)
- **Contradictory toggles:** clicking a Narrow chip while its negation (`-is:edited`, `f:<2`
  family) is in the query adds a token that matches nothing. (03 Task 4)
- **Top matches:** places show country codes ("Lisbon, PT"), no country-name table; "sao"
  misses "São Paulo" (LIKE is accent-sensitive); a two-word city leaves its second word;
  albums/tags are not in the cache key. (03 Task 5; same "Lisbon, PT" choice in 01b)
- **Results head:** title reads "Photos" until the first fetch lands. (03 Task 6)
- **Search speed:** a common word plus a rare filter reads every FTS match of the word
  (~2–3 ms count, ~18 ms deep page at 25k, release). No bench row for free-text pages.
- **0,0 GPS:** cameras that write an empty GPS block as 0/0 get a pin in the Gulf of Guinea in
  the location card and on Places. Fix in the scanner.
- **Immersive viewer:** the floating info panel covers the right end of the video transport;
  it appears without fading in with the HUD. (02 Task 1)
- **Viewer top bar:** the 760 px stage width that hides the Back label is a guess; the edit bar
  is still 52 px. (02 Task 2)
- **Info details:** opening Get Info while the docked panel's read is settling waits up to
  150 ms. (02 Task 4)
- **Filmstrip:** shows for a one-item view; video thumbs untested directly; a clicked item that
  vanished in the meantime opens nothing. (02 Task 6)
- **Map:** neither the card nor Places wraps across the date line.

- **Mosaic (01b):**
  - Rows leave the 158–224 band in narrow panes or with very mixed aspects (the tests hold real
    photo shapes in band at 1200 px; uniform 0.4–3.0 aspects leave it often at 800 px). A lone
    portrait in a narrow pane is the one row that stops short of the right edge. (Task 3)
  - Switching view shows an empty list until that view's first layout lands (~30 ms at
    125k, release); outside writes refresh the layout after a 750 ms cool-down, and Select / Select
    Day do nothing until then. (Tasks 4–5)
  - Grid ↔ Mosaic does not carry the scroll position across. ⌘-scroll over the empty area
    below the last row may scroll; ⌘-scroll and pinch do nothing over Years/Months cards.
  - Under Years/Months, ⌘A and ⌘K/menu commands still act on the hidden selection (same as
    the map). Undated items have no year/month card. The rail lens follows the hidden Days
    list. (Task 7)
  - Year/month card sizes (3 × 200 px, 4 × 150 px covers) are not in the frame; unchecked.
  - Select Day, Show Mosaic / Show Grid and Show Years / Months / Days are in ⌘K only: not
    in the menu bar (Show Map is) or the shortcut sheet. Undated sorts first in oldest-first.
  - Bench covers the Library scope only; album/search layout checked by EXPLAIN. No test of
    a 3,000-item day. `day_places` release gate is 110 ms (measured 53); the fix if it
    grows is a `(year, month, day, city)` index. (Task 2)

## Code health
- Files over the ~500-line guideline: `app/navigation.rs` (~1500), `app/text_input.rs`,
  `app/render/grid.rs` (~710, down from 1300), `app/map.rs` (~710), `app/keymap/modals.rs`,
  `views/mosaic.rs` (~600 with tests), `views/inspector/body.rs`, `app/command_palette.rs`
  (~1580), `app/library.rs` (~1425), `config.rs` (~1360, mostly tests), `app/state.rs`
  (~1070), `i18n.rs` (~640).
- There is no CHANGELOG; user-visible changes from these plans are only in accessibility.md
  (Mosaic, Show on Map). README not updated.
