# Design alignment: deferred gaps

What the `lumina.pen` alignment work (plans in `docs/superpowers/plans/2026-11-10-design-*.md`
and `2026-11-11-design-01b-mosaic.md`) skipped, dropped or left unverified, so we can come back
to it. Each line names where it came from. Remove an entry when it lands; add one whenever a
task defers something.

Last updated: 2026-11-11 (after plans 00–03, 02 and 01b landed).

## Never verified (applies to everything)
- **Visual check against the frames.** Library (Mosaic), Detail, Search, Videos and the PiP
  window were captured at 1600×1000 on generated photos and compared with their frames
  (2026-11-11). Four problems found were fixed: the PiP window buttons, the info panel's
  right gap, the filmstrip dimming and ragged Mosaic last rows. No capture script is checked
  in; frames from plan 05 on are not compared. Remaining small differences are listed under
  "Visual departures".
- **PiP window (04 Task 5).** Seen once on a real screen (window buttons now hidden, badge at
  12 px). Not checked closely: whether radius 10 and the `line_2` outline sit cleanly on the
  OS window's corners, the fade directions, the OS shadow standing in for the design's, and placement 56 px
  in from the main window (only the math is tested; no test uses a display whose origin is not
  zero). `make ui-smoke` does not open PiP. Its keys work only after the window is clicked.
- **VoiceOver.** Roles and labels were added to new controls, but none were checked with
  VoiceOver (AGENTS §11, §15).
- **Live interaction.** With Screen Recording and Accessibility granted (2026-11-11),
  `make ui-e2e` passes all steps (7/7 runs after 3f4a5ed, which keeps the PiP panel visible
  while Lumina is inactive; before it, the PiP hid on app switch and the e2e passed about 1 in
  3). The new "PiP stayed on screen with another app active" step does not assert that Finder
  really became frontmost, and uses a fixed 1 s sleep. It covers the inspector, shortcut
  sheet, viewer, PiP open/keep/close, a favourite and a smart album; it does not open Mosaic,
  Videos tiles, the 4K chip or the PiP card's controls. Without Screen Recording it still
  FAILS at "could not capture the window" (`scripts/live_ui_automation.sh:263`) before its
  Accessibility check can report SKIP; the script should map a capture failure to SKIP unless
  `LUMINA_REQUIRE_UI_PERMISSIONS=1`.
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
| Video chips HDR, Slow motion, Live Photos | 04 dec. 1 | Hidden; need scanner fields, migration and query (separate core plan, L). Only "All clips" and "4K" ship |
| "Play all" on the Videos section | 04 dec. 3 | Omitted; needs a video queue (separate plan, M) |
| "Recently captured" per-section headers in Videos | 04 dec. 4 | Month band with "clips" count instead |
| PiP as a card inside the main window | 04 dec. 5 | Kept as a floating OS window, restyled to the card |
| PiP buffered-range bar | 04 | Dropped: local files have no buffering |
| Albums overview tabs People, Places, Shared and the meta's "· N shared" | 05 dec. | Only Albums and Smart Albums tabs; meta reads "N albums · M smart albums". Places tab is a separate plan |
| Albums overview header sliders and sort icons | 05 Task 2 | Dropped: nothing to filter on the page; sort is the "Sort ▾" control (05 Task 4). The header keeps the sidebar toggle, Downloads (while it has rows) and New Album; the inspector is hidden on the page |
| Album card min width 200 in the column rule | 05 Task 4 | 230: the frame's 5 cards of 245 at a 1600 window do not fit a 200 minimum (that gives 6 of 201) |
| Album card cover resolution | 05 Task 4 | Covers use the grid's 320 px thumbnail; a 245 card on a 2× display may look soft |
| Smart album rule chips | 05 Task 5 | One chip per real rule (the frame's time-of-day, shutter, "unedited" and "outdoors" chips have no engine rule), at most two and "+N". Labels reuse the builder's English field and value names (`smart_draft`), not i18n keys. The mosaic is the album's first 4 matches in its default order, from the 320 px thumbnail |
| Smart Albums tab grid | 05 Task 5 | Not virtualised (the Albums tab's list is); 4 cards per row, 2 below a 900 wide main column |
| "Sort: Recently updated" | 05 dec. | Name (default), Date created, Newest photo, Item count; no updated time is stored |
| Video trim (scissors), CC/captions and caption overlay, audio waveform | 06 dec. 1 | Omitted; separate plans (export path, legible-media API, off-thread audio decode). No dead controls |
| HDR/fps badge and Playback rows Frame rate, Codec, Bitrate, Audio | 06 dec. 1 | Omitted; needs a `VideoInfo` read from the loaded player (separate plan). Playback shows Duration and Resolution only |
| "Scenes" with Re-detect | 06 dec. 2 | Titled CHAPTERS, shown only when the file has chapters, no Re-detect (no scene detection) |
| Frame-step buttons in the transport | 06 dec. 3 | Removed; `,` / `.` and ⌘K Previous / Next Frame remain |

## Keyboard and accessibility gaps
- **No Tab traversal** for the top bar buttons, filmstrip thumbs, tag chips, search chips,
  Save/Narrow controls or Top-match cards. Every action has a key or palette path instead.
- **Tag ×:** Backspace in the empty tag field removes only the last tag; other tags are
  mouse-only to remove (also in Get Info). (02 Task 4)
- **Top matches:** ↓ from the bar reaches the cards only after Esc closes the suggestions, with
  no on-screen hint. (03 Task 5)
- **"Rate…"** is in the palette but not the menu bar or shortcut sheet. (02 Task 2)
- **Albums overview keys** (Tab / ⇧Tab between the tab bar, New Album, the smart cards, Sort
  and the album cards,
  ←/→ switch tabs, arrows move between cards, ↵ runs or opens the focused control, ⌘⌫ asks
  to delete the focused album, Esc clears the focus) are not in the shortcut sheet; ⌘K Show
  Albums / Show Albums Tab / Show Smart Albums Tab and Go > Albums are the listed paths. A
  focused card is renamed or deleted with ⌘K Rename Album… / Delete Album…, a focused smart
  card edited or deleted with ⌘K Edit / Delete Smart Album…; "Manage rules" has no Tab stop
  (⌘K Show Smart Albums Tab). (05 Tasks 2, 4, 5)
- **Toast copy:** edit/rotate while a popover is open says "Close the open dialog…"; "menu"
  would fit the Sort and Rate popovers better. Slideshow is refused silently. (02 Task 2)
- **Brand row** has no accessibility role; full-screen traffic-light inset and brand-row window
  drag off macOS untested. (00 Task 5)

## Visual departures (no matching token or API)
- Letter-spacing/tracking everywhere (section titles, headers, results title): GPUI has none.
- Photo shadow uses `media_scrim` (~65%) instead of 80%. (02 Task 3) Other filmstrip thumbs
  now draw at the frame's 55% opacity.
- Filmstrip current thumb: the 2 px accent border sits inside the 74 px box (picture is 70 px).
- Location card land is clipped to a square, not the 6 px rounded corners.
- Long filenames in the info panel wrap instead of truncating (a viewer test asserts wrap).
- Additions not in the frames: the sidebar toggle in the search header when collapsed; the
  "Videos" results title for all-video results; the header's Tag and Info buttons; the
  "N of M selected" status bar; the selection action bar with one item selected; the info
  panel's Rating row and Get Info / Reveal buttons; Screenshots, Duplicates and Recently
  Deleted sidebar rows.
- Small copy/format differences seen in the 2026-11-11 capture: day headers read
  "Wed, Nov 11, 2026" (frame "Saturday, 14 June"); the zoom pill puts − and + together on the
  right; "Save Search as Smart Album" (frame "Save as Smart Album"); capitalised Narrow chips;
  no accent border on the sidebar search field while searching; Videos header rounds 12 s of
  clips up to "1 m"; the "All clips" chip has no icon; Videos/Search grids keep a 20 px right
  gutter (frame 28).

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
- **PiP and Spaces:** the PiP stays in its own Space; it does not follow the user across
  Spaces or over full-screen apps like the system PiP (would need
  `CanJoinAllSpaces | FullScreenAuxiliary`). The floating Inspector window still hides while
  Lumina is inactive (same NSPanel default), which seems right for an inspector.
- **Videos tiles and filters (04 Tasks 2–4):** caption line heights (16/13 px) are estimates;
  at 124–135 px tile width a rated clip's meta pill can touch the top badge row. The column
  count still comes only from zoom (2–8), so at 8 columns in a narrow window thumbs are tiny.
  Grid scroll offset is kept in pixels across a view switch, now that Videos rows are taller
  than square rows. The now-playing badge marks a clip in PiP whether playing or paused. Typing
  exactly `is:video is:4k` from any view shows the Videos page. An empty Videos view reads
  "0 clips · 0 B" and still shows its chip row. The "No 4K clips" empty-state copy hard-codes 3840.
- **Video player (06):**
  - ↑/↓ change the volume anywhere in the video viewer, not only on the slider (no control
    other than text fields takes focus); ⌥/⇧ with them do nothing. (Task 4)
  - Chapter labels over the strip are laid out from an estimated mono glyph width, so CJK or
    very long names can overlap or be dropped early. (Task 3)
  - After a failed save the volume keeps its new level (the banner says so), like speed and
    mute. (Task 4)
  - No Tab order through the transport (play, ±5 s, speed, mute, volume, loop, full screen);
    every control has a key or palette path. (Task 2)
  - The transport readout is m:ss, so a clip under 2 s shows whole seconds ("0:01 / 0:01")
    where the old card showed hundredths. (Task 2)
- **Videos header (04 Task 1):** unprobed clips add their size but not their duration, so the
  total running time is understated with no hint. `format_bytes` divides by 1024 but prints
  "MB"/"GB" and always one decimal ("211.0 GB" vs the design's "211 GB"). Undated items
  (capture time 0, stored as 1970-01-01) are titled "January 1970" in the month band in every
  view, not "Undated".

- **Mosaic (01b):**
  - Rows leave the 158–224 band in narrow panes or with very mixed aspects (the tests hold real
    photo shapes in band at 1200 px; uniform 0.4–3.0 aspects leave it often at 800 px). A lone
    portrait in a narrow pane stops short of the right edge. A day's short last row is re-flowed
    with up to three rows above it so all fill the width in band; a day too small for that
    (a few photos, or shapes no split can fit) still ends in a ragged row. (Task 3)
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
