# Accessibility & localization

## Keyboard access

Lumina is designed to be usable without a mouse:

- **Command palette** (`⌘K`) searches and runs every major command: open
  library, rescan, preferences, layouts, sorting, export, XMP import, sidebar
  and inspector toggles, grid zoom and more. Type to filter, `↑`/`↓` to move,
  `↵` to run, `Esc` to close.
- **Shortcut sheet** (`⌘/`) lists navigation, editing, viewer and video keys.
- **Native menu bar** exposes File, View and Help commands; macOS treats these
  as accessible menu items for VoiceOver.
- Arrow/Home/End/Page navigation, `⌘F` search focus, `⌘Z`/`⇧⌘Z` undo/redo,
  tooltips on icon-only controls, and visible focus/selection styling.

## Screen readers

GPUI 0.2.2 does not expose an NSAccessibility element tree, so VoiceOver
currently sees only the window frame, not individual controls. Mitigations:

- every operation is reachable from the native menu bar or command palette;
- the shortcut sheet documents all keyboard paths;
- file dialogs, Preferences content and menus are native/accessible.

Removing this limitation requires accessibility support in GPUI (role/label
metadata and focus ordering). That is tracked as an upstream dependency rather
than worked around with fake controls.

## Automated checks

- In-process UI automation drives the real view tree with synthetic clicks,
  keystrokes and resizes (`cargo test -p lumina-ui-gpui ui_automation`).
- The OS-level harness (`make ui-e2e`) drives the live window when
  Accessibility/PostEvent permission is available.
- Live smoke (`make ui-smoke`) verifies the real window renders.

## Localization

User-facing command labels go through `src/i18n.rs`; the catalog is keyed and
currently ships **en-US** only. `LUMINA_LANG` is read and unknown locales fall
back to English. Adding a locale means adding a catalog arm and a `Locale`
variant; remaining hard-coded strings can be migrated incrementally without
changing UI structure.
