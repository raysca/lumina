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
- **Compare**: `C`, View > Compare or ⌘K Compare opens 2–9 selected items.
  `←`/`→` or `Tab`/`⇧Tab` focus a pane, `P`/`X`/`U` and `0`–`5` mark it, `⌫`
  takes it out (the file stays), `↵` opens it in the viewer (`Esc` returns),
  `+`/`-`/`Z`/`⌘0`–`⌘4` zoom every pane, `⌘Z`/`⇧⌘Z` undo or redo a flag or
  rating, `Esc` closes. Other keys are absorbed. While compare is open,
  in the viewer opened from it too, Back/Forward (`⌘[`/`⌘]`, Go menu),
  Slideshow, Show Map and Delete Rejected… are disabled in the menus, and
  they and ⌘K Show on Map and Show Similar Shots / Exact Copies are left
  out of ⌘K, so the
  view behind compare never changes; expanding PiP closes compare. Compare,
  Slideshow and Show Map do not open from the menu bar while the ⌘K palette
  or a dialog is open. Roles: the view (group,
  "Compare"), its close button (button), each pane (image, labelled with the
  item); not yet checked with VoiceOver.
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
- **Share** (macOS): File > Share… or ⌘K Share… opens the system share
  sheet for the selection (up to 500 items) at the window centre; the
  right-click menu and the batch dock also have Share…. It is not offered while a dialog, edit mode, compare
  or a slideshow is up, nor on Similar shots (no selection). The share
  sheet itself is the system's, with its own keyboard and VoiceOver
  support.
- **Slideshow**: `⌥⌘S`, View > Slideshow or ⌘K Slideshow plays the open
  view full screen from the selection. `Space` pauses and resumes (also on
  a video, which shows its poster and never plays), `←`/`→` step, and one
  `Esc` ends it and puts the selection back. ⌘K Slideshow: Loop and
  Slideshow Interval (3, 5 or 10 seconds) set it up beforehand. While it
  plays every other key and the menu items that open dialogs or act on the
  selection are off; `⌘,` (which pauses it) and `⌘Q` still work. The slide,
  the Paused label and the Back button set no AccessKit role, so VoiceOver
  does not announce them; the keys and the shortcut sheet are the path.
- **Map**: ⌘K Show Map / Show Grid and View > Show Map switch Places
  between the grid and the offline map; the Places row's Map toggle does
  the same by mouse. ⌘K Show on Map (offered when the selected item has a
  location) opens the map centred on that item, the keyboard path for the
  info panel's "Open map" (button, labelled "Open map"). On the map `Tab`, `→` or `↓` focus the next place
  east and `⇧Tab`, `←` or `↑` the previous one west (a focused place near
  the edge is brought to the middle), `Enter` shows its items in the grid
  (Back returns to the map), `⌘+`/`⌘-` zoom with the focused place kept in
  the middle as far as the world's edge allows, and `Esc` returns to the
  grid. Keys typed in the palette or a
  field stay there.
- **Mosaic**: ⌘K Show Mosaic / Show Grid or the toolbar's view-mode
  segments switch the browser between justified rows and the square grid
  (the choice is kept; Grid is the default). Duplicates and a smart album
  that no longer exists always show the grid; the segments are not drawn in
  Duplicates or Recently Deleted, where ⌘K still switches. In Mosaic, `↑`/`↓` move to the tile nearest in x in the row above
  or below, across day headers, and stay put on the first or last row;
  `⇧↑`/`⇧↓` extend the same way; PageUp/PageDown move by the visible rows
  less one; Home/End and `←`/`→` work as in the grid. A drag selects the
  reading-order range between the two tiles. `⌘=`/`⌘-`, ⌘-scroll and pinch
  step the row height and keep the item under the pointer (or the
  selection) in place. Under date sort each day has a header (heading,
  labelled with the date, place and count) and a Select button (button,
  labelled e.g. "Select 3 items from Sat, Jun 14, 2025"; dimmed, described
  "Updating…", while the layout catches up with a scan); ⌘K Select Day
  selects the selected item's day, or the top visible day. Undated items
  form their own "Undated" section, last in newest-first order and first
  in oldest-first. ⌘K Select Day is hidden in the viewer and while the
  layout is updating. In All Media under date sort with no search, the zoom
  pill also offers
  Years · Months · Days (buttons, toggled; ⌘K Show Years / Show Months /
  Show Days): `←`/`→`/`↑`/`↓` move the focus ring over the cards (button,
  labelled e.g. "2025, 1,234 items"), `↵` opens that period in Days with
  its first day on top, `Esc` returns to Days, `⌘=`/`⌘-` still step the
  size, and a timeline-rail click opens Days there. While cards show, the
  grid keys that act on the hidden selection are absorbed; ⌘A, the other
  ⌘ shortcuts (⇧⌘A, ⇧⌘R, ⌥⌘C, ⌘E, ⌘I), ⌘K and menu commands still act
  on it. Undated items have no year or month
  card. None of this is checked with VoiceOver yet.
- **Videos**: the Videos view shows 16:9 tiles with a title and a date ·
  resolution · duration line (Grid only; Mosaic keeps true shapes). Its
  chip row has All clips and 4K (buttons, toggled), a clip count (status)
  and, while 4K is on, Clear all. ⌘K Filter: 4K Videos (offered only in
  Videos) turns 4K on or off, and Back returns to plain Videos. The duration
  badge (image, labelled "Video, 1:18", or "In Picture in Picture, 1:18" for
  the clip in PiP) marks that clip with a sound icon.
- **Picture in Picture**: `⌥P` (or ⌘K Toggle Picture-in-Picture) opens the
  selected video in a floating window and closes it again. Once that window has been clicked it takes `Space`/`K`
  (play/pause), `J`/`←` and `→` (5 s back or forward), `⌥←`/`⌥→`
  (back to the chapter start, or on to the next chapter; 5 s without
  chapters; the chapter name shows in the main window), `M` (mute) and `L`
  (loop). Its Expand, Close, skip, play/pause and mute controls are
  buttons with labels and tooltips; the timeline is a slider, with no value
  reported yet. Expand hands the
  video back to the viewer at the same position; closing the window stops
  it. Not checked with
  VoiceOver.
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
- **Loop range**: with a video open in the viewer, `⌥I` sets the loop start,
  `⌥O` the loop end and `⌥X` clears them (also ⌘K Set Loop Start, Set Loop
  End, Clear Loop Range). A dialog absorbs the keys, the palette takes them as typed text, and in a
  text field they type the ⌥ character instead. Setting a point shows its time
  in a toast.
- **Chapters and audio tracks**: with a video open in the viewer, `⌥←` /
  `⌥→` go to the previous / next chapter and a toast names it (`⌥←` restarts
  the current chapter when more than 3 s into it, otherwise goes to the one
  before). Without chapters they seek 5 s back / forward like `←` / `→`; a
  dialog, the palette or a text field takes them first. The transport's
  Audio menu (shown only when the file offers more than one alternative
  audio track) has the keyboard path ⌘K "Audio Track:
  {name}", where the playing track has a check. The button (with its
  expanded state), the menu and its rows (radio items, the playing one
  toggled on) carry roles and labels, not yet checked with VoiceOver. The choice lasts until the video closes.
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
- the map (group, labelled "Map of photo locations"; the land drawing is
  decorative), each place (button, labelled e.g. "3 items near 35.0,
  135.8"; focus is drawn as a ring), the zoom buttons (button, labelled "Zoom
  In (⌘=)" / "Zoom Out (⌘-)"), the empty-state message and the read-error message (both status) and the
  Places row's Map toggle (button, labelled "Map", toggled on while the
  map shows). The places are laid out inside the
  map's canvas, so whether VoiceOver reaches them is not known;
- the Duplicates tabs (tab list; each tab a tab, labelled "Exact copies" or
  "Similar shots", selected when shown, with a tooltip), the Similar shots
  list (list, labelled "Similar shots"), each stack (list item, labelled
  e.g. "Burst · 3 items", selected when focused), its thumbnails (image,
  labelled with the file name), its two actions and Try Again (buttons,
  with a tooltip and key shortcuts `Enter` / `Shift+Enter`), and the
  loading, empty and error notices (status).
- the batch dock's Share… button (button, labelled "Share the selected
  originals", with the same tooltip).
- Mosaic: the view-mode segments (button, toggled), each day header
  (heading, labelled with date, place and count) and its Select button
  (button, inside the heading), the zoom pill's Years / Months / Days
  segments (button, toggled, with a tooltip) and the year and month cards
  (button, labelled e.g. "2025, 1,234 items").

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
