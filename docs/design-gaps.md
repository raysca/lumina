# Design alignment: deferred gaps

What the `lumina.pen` alignment work (plans in `docs/superpowers/plans/2026-11-10-design-*.md`
and `2026-11-11-design-01b-mosaic.md`) skipped, dropped or left unverified, so we can come back
to it. Each line names where it came from. Remove an entry when it lands; add one whenever a
task defers something.

Last updated: 2026-11-11 (after plans 00–03 and 02 landed; 01b accepted).

## Never verified (applies to everything)
- **Visual check against the frames.** No screen has been compared by eye with its `.pen`
  frame. `make ui-smoke` skips its screenshot without Screen Recording permission and only
  shows the Welcome screen.
- **VoiceOver.** Roles and labels were added to new controls, but none were checked with
  VoiceOver (AGENTS §11, §15).
- **Live interaction.** `make ui-e2e` skips without Accessibility permission.
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

## Code health
- Files over the ~500-line guideline: `app/render/grid.rs` (~1300), `app/text_input.rs`,
  `app/keymap/modals.rs`, `views/inspector/body.rs`, `app/map.rs` (~680). 01b Task 1 starts on
  `grid.rs`.
- Docs not yet updated for user-visible changes from these plans (README / accessibility.md /
  CHANGELOG), except Show on Map.
