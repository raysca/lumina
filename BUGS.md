# Known issues

Open bugs, work that still runs on the UI thread or has not been measured,
flaky tests and tooling gaps. Remove an entry when it is fixed; add one when
you find something you are not fixing now. Planned features live in
[ROADMAP.md](ROADMAP.md).

## Bugs

### Organizing

- Undoing a duplicate merge puts the trashed copies back, but not the metadata
  that was merged onto the keeper. Core now returns each keeper's pre-merge
  metadata (`Catalog::merge_duplicate_group` / `merge_all_duplicates`, via
  `engine.catalog()`) and restores it with `Catalog::restore_merged_keepers`;
  the undo record (`UndoAction::Trashed` in
  `crates/lumina-ui-gpui/src/app/duplicates_review.rs`) still has to keep
  the snapshots and restore them on undo.
- In Duplicates, the ORIGINAL label and the group title follow the automatic
  pick, not the copy chosen as keeper; "ORIGINAL" is not always the earliest
  copy.

### Places, dates and search

- Neither the location card nor the Places map wraps across the date line.
- Undated items (capture time 0) are now stored as year 0, month 0, so the
  month band says "Undated", but the timeline rail and the year buckets label
  their `0000-00` bucket as year "0"
  (`crates/lumina-ui-gpui/src/app/render/rail.rs`); they should say "Undated".
- The Videos header understates the total running time: clips whose duration
  has not been read add their size but not their length, with no hint.

### Accessibility

- Writing `AXSelectedText` (replace the selection) through the macOS
  Accessibility API does not reach Lumina's text fields: accesskit_macos
  0.26.3 implements no `setAccessibilitySelectedText:` (its `node.rs` only has
  `setAccessibilityValue:` -> `SetValue` and `setAccessibilitySelectedTextRange:`
  -> `SetTextSelection`), so `Action::ReplaceSelectedText` never reaches
  gpui-pre's `handle_a11y_action`. Needs that setter upstream in AccessKit.
  Writing `AXValue` (the whole text) does work: GPUI Kit's input handles
  `SetValue` (`gpui-base` `handle_accessibility_set_value`). Not checked with a
  real assistive client.

## Performance (work on the UI thread, or not measured)

- Merge All in Duplicates runs its transaction on the UI thread.
- Duplicates filters every group several times per frame (about 50k hash probes
  at 10k groups). Not measured.
