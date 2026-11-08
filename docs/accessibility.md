# Accessibility & localization

## Keyboard access

Lumina is designed to be usable without a mouse:

- **Command palette** (`⌘K`) searches and runs every major command: open
  library, rescan, preferences, layouts, sorting, export, XMP import, sidebar
  and inspector toggles, grid zoom, viewer zoom presets (in the photo
  viewer, also on `⌘0`–`⌘4`), quick search filters, Clear Recent Searches,
  Save Search as Smart Album… and more. Its GO TO group reaches every
  sidebar destination (every default collection, including Screenshots,
  Places and Missing, plus years, tags, albums and smart albums). Type to filter (each
  word matches separately), `↑`/`↓` to move, `↵` to run, `Esc` to close.
  "missing" alone ranks the Locate Missing File… command first; type "go to
  missing" to open the Missing collection.
- **Sidebar**: `⌃⌘S` hides and shows it, as do the toolbar toggle, View >
  Hide Sidebar / Show Sidebar and ⌘K. The toggle's icon, tooltip, aria label and the
  menu and palette titles say what it will do next. Like the other
  shortcuts, `⌃⌘S` does nothing while a sheet, the palette or a text field
  has the keys. Collapsed, the sidebar is a 48 px rail with one icon button
  per default collection (Untagged, listed under Smart Albums, is not on
  the rail).
- **Albums**: `⌘N` (File > New Album, ⌘K New Album) makes an empty album
  and opens its name field in the sidebar; ⌘K New Album with Selection
  (also in the right-click menu) makes one holding the selection. `↵` saves
  the name, `Esc` keeps the old one, and the field takes every other key.
  With an album open, ⌘K Rename Album… and Delete Album… act on it; the
  delete confirmation takes `↵`/`Esc` and absorbs every other key. `⌘Z`
  undoes a rename or a delete (members come back). `⇧⌘A` (⌘K Add to
  Album…) opens the album picker: type to filter, `↑`/`↓` move, `↵` adds,
  `Esc` closes, and the picker absorbs every other key. `⌫` in an album
  takes the selection out of it (the files stay; `⌘⌫` still opens Delete).
  ⌘K Pin Album / Unpin Album act on the open album or smart album, and ⌘K
  Duplicate Smart Album and Export Smart Album Rules… on smart albums;
  Import Smart Album Rules… is in ⌘K and the File menu. Album rows carry a list item role and a label with the count, not
  yet checked with VoiceOver.
- **Culling**: in the grid and the viewer, `P` picks, `X` rejects, `U`
  removes the flag and `0`–`5` rate the selection (one `⌘Z` undoes the
  batch). In the viewer the key then moves to the next item, so a cull is
  `→`-free: look, press `P` or `X`, look at the next. The keys need no
  modifier (`⌥P` stays Picture-in-Picture) and do nothing while the
  palette, a dialog or a text field has the keys. ⌘K Show Picks / Show
  Rejected filter the view; ⌘K or File > Delete Rejected… opens the Delete
  dialog on the open view's rejects (`↵` Move to Trash, `↑`/`↓` other
  choices, `Esc` cancels), or a toast says there are none. Flag badges are
  drawn, not yet announced to VoiceOver.
- **Similar shots**: ⌘K Show Similar Shots / Show Exact Copies switch the
  Duplicates tabs (from the viewer too, which then closes); a click on a
  tab does the same. On Similar shots `↑`/`↓` move the focused stack (drawn
  with an accent bar and ring, scrolled into view). `↵` runs Keep picks,
  reject the rest; with no pick in the stack a toast asks for one first.
  On the error state `↵` runs Try Again. `⇧↵` runs Reject all but the
  first. Each action is one `⌘Z`, and `Esc` acts as on Exact copies. Every
  other grid key (P/X/U, digits, F, ⌫, ⌘A, ⇧⌘A) is absorbed there and the
  tab holds no selection, so no menu or palette command acts on hidden
  items; `⌥⌘S` plays nothing and says so. Back/Forward onto Duplicates
  opens Exact copies. A stack wider
  than the pane draws the members that fit and a "+k more" count. Roles:
  see the controls list below.
- **Slideshow**: `⌥⌘S`, View > Slideshow or ⌘K Slideshow plays the open
  view full screen from the selection. `Space` pauses and resumes (also on
  a video, which shows its poster and never plays), `←`/`→` step, and one
  `Esc` ends it and puts the selection back. ⌘K Slideshow: Loop and
  Slideshow Interval (3, 5 or 10 seconds) set it up beforehand. While it
  plays every other key and the menu items that open dialogs or act on the
  selection are off; `⌘,` (which pauses it) and `⌘Q` still work. The slide,
  the Paused label and the Back button set no AccessKit role, so VoiceOver
  does not announce them; the keys and the shortcut sheet are the path.
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
- **Metadata templates**: ⌘K Metadata Templates… (or File > Metadata
  Templates…) opens the editor; `Tab`/`⇧Tab` move between its fields,
  `↑`/`↓` show the previous or next template (past the last is a new one;
  with unsaved typing they stay in the field, and a list row or a click
  outside the sheet asks to save or discard instead), `↵` saves, `⇧⌘⌫`
  deletes the shown template and closes the editor (the toast's Undo or
  `⌘Z` brings it back) and `Esc` closes without saving. The editor absorbs
  every other key, so nothing reaches the library behind it, and it does
  not open over another sheet or dialog. ⌘K Apply Metadata Template… lists
  the template names; `↑`/`↓` and `↵` apply one to the selection. The
  editor's fields, list rows and buttons, and the batch inspector's template
  picker and Apply button, carry roles and labels, not yet checked with
  VoiceOver.
- **Rotation**: `R` / `⇧R` rotate the viewer photo or the grid selection
  right / left, also ⌘K Rotate Right / Rotate Left and Image > Rotate
  Right / Rotate Left; the keys type into a focused text field instead.
- **Quick edits**: `E` in the viewer, ⌘K Edit Photo or Image > Edit Photo
  opens edit mode; `R` / `⇧R` rotate, `-` / `=` change exposure, `A`
  toggles auto enhance, `C` enters crop mode and then cycles the aspect,
  `⇧C` turns crop mode on or off (off keeps the crop and edit mode),
  `Enter` is Done and `Esc` Cancel. While cropping, arrows move the crop
  rectangle (1 %, `⇧` 10 %), `⌥` + arrows resize it, and `Esc` leaves
  crop mode only, without the new crop. Every other key is absorbed while
  editing (`⌘⌫` cannot trash the photo, arrows never step to another
  photo). Revert to Original is in ⌘K and the Image menu. The edit
  bar's buttons and the grid's "Edited" badge carry roles, labels and
  tooltips, not yet checked with VoiceOver.
- **External editor**: `⌘E` opens the selected item in the editor set in
  Preferences and `⌥⌘E` opens a copy of it (Edit a Copy), also ⌘K, the
  Image menu and the context menu. With no editor set they open
  Preferences on its General tab, where the editor row is. A focused text
  field and edit mode absorb both keys.
- **Errors and Trash**: `Esc` dismisses the error banner (after any toast);
  ⌘K has Retry Failed Action, Empty Trash… and Restore All from Trash.
- **Shortcut sheet** (`⌘/`) lists general, browsing, viewer, video, Tag
  Manager and metadata template keys;
  `↑`/`↓`, `Page Up`/`Page Down`, `Home`/`End` and `Space` scroll it.
- **Native menu bar** exposes File, View, Image, Go and Help commands; macOS treats these
  as accessible menu items for VoiceOver.
- Arrow/Home/End/Page navigation, `⌘F` search focus, `⌘Z`/`⇧⌘Z` undo/redo,
  tooltips on icon-only controls, and visible focus/selection styling.

## Motion

Menus, dialogs, toasts, new thumbnails and the viewer HUD fade in briefly
(at most 220 ms); closing is instant. With **Reduce Motion** on (System
Settings › Accessibility › Display) the fades stay but the toast no longer
rises, stars and hearts no longer pop, and slideshow slides change without
a fade. Lumina reads the setting at
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
  the explanation text);
- the viewer's edit bar (toolbar, labelled "Edit photo"): Cancel, Rotate
  Left/Right, Darker, Brighter, Auto Enhance (labelled on/off), Revert to
  Original, Crop (labelled on/off) and Done (buttons), and the exposure
  value (status); while cropping, a second row holds the aspect presets
  (buttons, each with its own label and tooltip);
- the crop overlay (group, labelled "Crop area") and its four corner
  handles (buttons, labelled by corner, with a tooltip);
- the grid's "Edited" badge on edited or rotated photos (image, labelled
  "Edited");
- added with the sidebar revamp: the sidebar's default-collection rows
  (button, labelled with the name and count, e.g. "Favorites, 22 items"),
  the collapsed rail's icon buttons (button, same label, tooltip
  "Favorites · 22") and the toolbar's sidebar toggle (button, labelled
  "Hide Sidebar (⌃⌘S)" or "Show Sidebar (⌃⌘S)"). User smart-album and tag
  rows have no role yet; Untagged (under Smart Albums) does;
- the Duplicates tabs (tab list; each tab a tab, labelled "Exact copies" or
  "Similar shots", selected when shown, with a tooltip), the Similar shots
  list (list, labelled "Similar shots"), each stack (list item, labelled
  e.g. "Burst · 3 items", selected when focused), its thumbnails (image,
  labelled with the file name), its two actions and Try Again (buttons,
  with a tooltip and key shortcuts `Enter` / `Shift+Enter`), and the
  loading, empty and error notices (status).

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
