# Accessibility & localization

## Keyboard access

Lumina is designed to be usable without a mouse:

- **Command palette** (`⌘K`) searches and runs every major command: open
  library, rescan, preferences, layouts, sorting, export, XMP import, sidebar
  and inspector toggles, grid zoom, viewer zoom presets (in the photo
  viewer, also on `⌘0`–`⌘4`), quick search filters, Clear Recent Searches,
  Save Search as Smart Album… and more. Its GO TO group reaches every
  sidebar destination (views, years, tags, smart albums). Type to filter (each
  word matches separately), `↑`/`↓` to move, `↵` to run, `Esc` to close.
- **Welcome screen**: `↵` opens an existing library, `⇧⌘I` imports into a
  new library, `⌥↵` explores the sample library and `⇧↵` uses Pictures;
  each control shows its shortcut. With
  recent libraries listed, `↑`/`↓` focus one, `↵` opens it, `⌫` forgets it
  (its folder and catalog stay) and `Esc` clears the focus. ⌘K offers Open
  Recent Library and Clear Recent Libraries once a library is open. The
  recent rows and their × buttons carry a button role and a label, not yet
  checked with VoiceOver.
- **Tags and keywords**: `T` opens the tag field for the selection; `↑`/`↓`
  walk the suggestions and `↵` adds the highlighted one (or the typed text),
  `⌫` in an empty field removes the item's last tag. In the Tag Manager
  (⌘K Open Tag Manager…), `↑`/`↓` move through the keyword tree, `↵`
  renames, `Space` checks, `⌘M` merges and `⌘⌫` deletes the checked rows,
  `⇧⌘A` adds an alias for the focused keyword (`↵` saves, `⌫` in the empty
  field removes its last alias) and `⇧⌘M` moves it under another parent
  (type the parent path, empty for the top level, then `↵`); the shortcut
  sheet lists both. The Tag Manager absorbs `⌘K`, so no command can run
  behind it; with a keyword selected in the sidebar, ⌘K Add Alias for
  Keyword… and Move Keyword… open the Tag Manager on it. The alias
  chips' × buttons and the suggestion chips carry a button role and a label,
  not yet checked with VoiceOver.
- **Errors and Trash**: `Esc` dismisses the error banner (after any toast);
  ⌘K has Retry Failed Action, Empty Trash… and Restore All from Trash.
- **Shortcut sheet** (`⌘/`) lists general, browsing, viewer, video and Tag
  Manager keys;
  `↑`/`↓`, `Page Up`/`Page Down`, `Home`/`End` and `Space` scroll it.
- **Native menu bar** exposes File, View and Help commands; macOS treats these
  as accessible menu items for VoiceOver.
- Arrow/Home/End/Page navigation, `⌘F` search focus, `⌘Z`/`⇧⌘Z` undo/redo,
  tooltips on icon-only controls, and visible focus/selection styling.

## Motion

Menus, dialogs, toasts, new thumbnails and the viewer HUD fade in briefly
(at most 220 ms); closing is instant. With **Reduce Motion** on (System
Settings › Accessibility › Display) the fades stay but the toast no longer
rises and stars and hearts no longer pop. Lumina reads the setting at
launch, so relaunch after changing it.

## Screen readers

GPUI (through GPUI Kit) reports an accessibility tree via AccessKit, but only
for elements that have an `id` and a `.role(...)`, with labels from
`aria_label`. Apart from the one-line text fields (GPUI Kit inputs with the
text-input role and a label, not yet checked with VoiceOver) and the few
controls listed below, Lumina's controls do not set roles yet, so VoiceOver
currently sees little beyond the window frame. Mitigations:

- every operation is reachable from the native menu bar or command palette;
- the shortcut sheet documents all keyboard paths;
- the video scrub preview is pointer-only; `,` `.`, J/K/L and the arrows
  seek and show the frame itself;
- file dialogs, Preferences content and menus are native/accessible.

Controls that do set a role and a label (none checked with VoiceOver yet):

- the welcome screen's open, import and sample buttons, the Pictures link,
  the missing-library notice (alert), and the recent-library rows with their
  × buttons;
- added with the 0.4 precision-browsing work: the viewer's Fit, Fill and
  1:1 preset buttons under the navigator (button, labelled "Fit (⌘0)",
  "Fill (⌘3)", "Actual size (⌘1)");
- also from that work: the search explanation footer (status, labelled with
  the explanation text).

The recent-search history icon and the scrub preview card have no role.

Removing this limitation is now Lumina's own work: give each control a role
and label, then check the result with VoiceOver before claiming support.

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
