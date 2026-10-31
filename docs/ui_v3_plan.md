# UI v3 plan

Moves the GPUI app to the v3 design ("Aperture rebuilt for GPUI"). Each artboard
on the design canvas is one slice. Each slice ships as its own commit with
`debug_selector`s on new controls, labels through `i18n.rs`, a ⌘K palette
entry for new actions, and a UI automation test (AGENTS.md §6–7, §11).

Tokens and control helpers already live in `crates/lumina-ui-gpui/src/theme.rs`.

## Decisions from design review

- No Browser / Split / Viewer layout toggle. Navigation is the sidebar, double
  click to open, and a "‹ Library" back link in the viewer.
- Thumbnails have no card surface and no filename. Favorite, rating and colour
  label sit in a small pill on the photo; selection is the accent outline.
- The tag manager is a floating sheet, like the smart album builder, not a
  docked pane.

## Slices

| # | Slice | Artboard | Status |
|---|-------|----------|--------|
| 1 | Grid: meta pill on the photo, no info band | V3Grid | done |
| 2 | Docked inspector: one-column summary; Get Info… opens the tabbed window | V3Inspector | done |
| 3 | Photo viewer: Fit button + navigator while zoomed, "N of M", stage prev/next, no filmstrip | V3Photo | done |
| 4 | Multi-select dock restyle, batch rating and tags up top, partial tags | V3MultiSelect | done |
| 5 | Tag manager: list + detail panel in the existing floating sheet; ↑↓/↵ keys | V3TagManager | done |
| 6 | Command palette, empty states, undo toast restyle | V3Palette, V3States | done |
| 7 | Preferences restyle | V3Prefs | done |

## Open items

- Window drag with the transparent titlebar is unverified.
- `make bundle` fails at the icon step unless the generate-icon feature is on.
- Not yet designed: context menu, scan/import progress, shortcut sheet.
- Inspector summary has no histogram (no luminance data in the catalog) and
  no map (no network); location shows as text only.
- Rating has no direct key binding (in the viewer 1/2/4 are zoom presets);
  the ⌘K palette has Rate 1–5 Stars and Clear Rating.
- Viewer pinch-to-zoom: GPUI 0.2.2 delivers no magnify event; the wheel,
  +/−, 0/1/2/4 and Z zoom.
- Inspector: Modified/Created/Added rows keep the short date format; only
  the capture time reads "Mon 13 Jun 2022 · 10:03". Overflow scroll and the
  video strip have no dedicated UI test yet.
- Batch inspector keeps an EDIT ALL group (title, caption, date, location)
  because it is the only batch path for those fields; colour labels for a
  selection live in the context menu, Favorite/Delete in the dock.
- Preferences: the editor note names the real menu item, "Edit in External
  Editor", not the mock's "Open With"; "last scan" is per session (not
  persisted across launches).
- Error banner: Retry is wired for Clear Cache and Restore All (both
  idempotent); other errors
  show the banner without Retry (`set_error_with_retry` opts in).
- Get Info: "Show nearby photos" searches the same city (no geo radius
  index); "Created" is the file-system birth time and the EXIF extras are
  read from the file when the window shows it; items catalogued before
  schema v9 show "—" for "Added to Lumina".
- Viewer and Get Info "Open With ▾" app list is not built (needs a native
  LSCopyApplicationURLsForURL bridge); both offer Open in Default App.
- Video scrubber (agreed deviation): no frame strip and a time-only hover
  preview; the AVFoundation bridge cannot decode arbitrary frames on demand.
- Video zoom has no on-screen control now; Z, scroll and the 0/1/2/4 keys
  still work.
- Multi-select dock has no Rate ▾ menu (rating is in the batch inspector)
  and no Add to Album: there are no manual albums to add to.
- Tag group, colour and keyboard shortcut from the design are not built:
  tags are plain names in the catalog (needs a schema change and a spec).
- Docked inspector: the video strip has duration and resolution but no fps
  (not in the catalog), and the place reads "City, Region, CC": the offline
  geocoder has no neighbourhood and no country names.
- Tag Manager has no "+ New Tag", "· M unused" count or "unused" row marker:
  a tag exists only while an item carries it, so an empty tag cannot occur
  until tags get their own table. TAGGED ITEMS shows the newest 24 and only
  thumbnails the grid has already decoded; the rest are placeholders.
- Selection outline sits on the square well, not hugging a contain-fit image
  as in the design; GPUI does not expose the decoded image's fitted rect.

## Picture-in-Picture (agreed design)

PiP is the detached video player. One player session exists, so at most one
video plays; it is presented either in the viewer or in PiP.

- Open with the viewer's PiP button, ⌥P (viewer or library, selected video),
  the context menu (videos only, acts on the clicked item) or ⌘K. The viewer
  closes and the video continues in PiP.
- While PiP is open, browsing, viewing photos, switching albums and searching
  leave it playing; PiP has its own frame slot.
- Opening another video (double-click, Enter, Space, Play Video) loads it into
  PiP; the viewer does not open. A video reached inside the viewer shows its
  poster with a "Play in Picture-in-Picture" hand-off (Space).
- Expand returns the video to the viewer at the same position. Closing PiP
  stops playback and saves the resume point.
- Moving the PiP video to Trash or deleting it closes PiP and says so in the
  toast. Opening another library closes PiP.
- Space in the grid is unchanged. ⌘K has Toggle Picture-in-Picture and
  Play/Pause Picture-in-Picture.

Not handled: a PiP video removed from disk outside Lumina keeps playing until
the watcher's next rescan drops it from the library.

## Smart album builder (agreed design)

Modelled on Photos' New Smart Album sheet.

- Open with ⌥⌘N, File → New Smart Album…, ⌘K or the sidebar +. Edit with
  ⌘K "Edit Smart Album…" (the album shown in the grid) or the sidebar row.
- Sheet: name field (empty means "Untitled Smart Album"), "Match all/any of
  the following conditions", one `[field] [operator] [value] [−] [+]` row per
  condition, a strip of the first 8 matching thumbnails and "N items match"
  beside Cancel / OK. Count and preview re-query when a rule changes, never
  on render.
- Conditions: Kind (photo includes RAW / video), Favorite, Date (is, before,
  after, in the range, in the last N days/weeks/months), Tag (picked from the
  library's tags, or contains / empty), Stars (is, at least, at most),
  Color label, Camera (make or model), Lens, Aperture, ISO, Focal length,
  Filename, Place
  (city, region or country code). Fields and values saved by older builds
  (Year, City, Kind RAW / non-RAW photo…) still show, edit and save.
- Typed dates use `YYYY-MM-DD` and `YYYY-MM-DD to YYYY-MM-DD`, compared
  against the naive capture time, like the timeline. A month is 30 days.
- OK is disabled and the row is outlined while any value is empty or invalid.
  A saved rule the engine rejects shows the reason in the footer instead of
  "No items match", and OK stays disabled.
- Keyboard: Tab / Shift-Tab move focus; Enter, Space or Down open a popup;
  arrows and Enter pick; Esc closes a popup, then the sheet; Enter in a text
  field or ⌘Enter is OK.
- Removed: presets, placeholder preview tiles and the "Show in sidebar"
  toggle that never saved.

