# Known issues

Open bugs, work that still runs on the UI thread or has not been measured,
flaky tests and tooling gaps. Remove an entry when it is fixed; add one when
you find something you are not fixing now. Planned features live in
[ROADMAP.md](ROADMAP.md).

## Bugs

### Organizing

- While Merge All runs, undo, redo and opening a library are refused in
  every view, not only in Duplicates (a toast explains why).

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
