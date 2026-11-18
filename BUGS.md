# Known issues

Open bugs, work that still runs on the UI thread or has not been measured,
flaky tests and tooling gaps. Remove an entry when it is fixed; add one when
you find something you are not fixing now. Planned features live in
[ROADMAP.md](ROADMAP.md).

## Bugs

### Organizing

- Setting an item's date to a capture time of 0 through the catalog
  (`Catalog::set_timestamp`) still files it under January 1970; only the
  scanner and the v24 migration store it as undated (year 0).
- An undo that keeps every group merged (each had a copy permanently
  deleted) still says "Undid merge" and leaves an empty redo entry.
- Redo of a duplicate merge trashes a copy even if its file changed on disk
  after the undo, so it may no longer be a byte-identical duplicate.
- While Merge All runs, undo and redo are refused in every view, not only in
  Duplicates (a toast explains why).

### Library

- Opening the same folder twice in quick succession can briefly open two
  engines on one catalog; the later open wins and cancels the earlier scan.
- A Merge All still running when the same library is reopened can commit in
  the old engine with no undo entry (or fail with a busy database, which is
  only logged), and the reopened library's Merge All is not blocked by it.

### Accessibility

- With the viewer open, the year rail, zoom pill and selection dock stay in
  the accessibility tree under it (the toolbar, grid and sidebar are removed).
- Writing `AXSelectedText` (replace the selection) through the macOS
  Accessibility API does not reach Lumina's text fields: accesskit_macos
  0.26.3 implements no `setAccessibilitySelectedText:` (its `node.rs` only has
  `setAccessibilityValue:` -> `SetValue` and `setAccessibilitySelectedTextRange:`
  -> `SetTextSelection`), so `Action::ReplaceSelectedText` never reaches
  gpui-pre's `handle_a11y_action`. Needs that setter upstream in AccessKit.
  Writing `AXValue` (the whole text) does work: GPUI Kit's input handles
  `SetValue` (`gpui-base` `handle_accessibility_set_value`). Not checked with a
  real assistive client.
