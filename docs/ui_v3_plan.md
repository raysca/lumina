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
| 3 | Photo viewer: floating zoom pill, toolbar rating, stage prev/next, no filmstrip | V3Photo | done |
| 4 | Multi-select dock restyle, batch rating and tags up top, partial tags | V3MultiSelect | done |
| 5 | Tag manager as a floating sheet, reusing the album builder overlay | V3TagManager | |
| 6 | Command palette, empty states, undo toast restyle | V3Palette, V3States | |
| 7 | Preferences restyle | V3Prefs | |

## Open items

- Window drag with the transparent titlebar is unverified.
- `make bundle` fails at the icon step unless the generate-icon feature is on.
- Not yet designed: context menu, scan/import progress, shortcut sheet.
- Inspector summary has no histogram (no luminance data in the catalog) and
  no map (no network); location shows as text only.
- Rating has no keyboard shortcut (in the viewer 1/2/4 are zoom presets);
  needs palette commands or a binding.
- Viewer zoom navigator (overview of the visible region) is not built.
- Video zoom has no on-screen control now; Z, scroll and the 0/1/2/4 keys
  still work.
- Multi-select dock has no Rate ▾ menu (rating is in the batch inspector)
  and no Add to Album: there are no manual albums to add to.
- Selection outline sits on the square well, not hugging a contain-fit image
  as in the design; GPUI does not expose the decoded image's fitted rect.
