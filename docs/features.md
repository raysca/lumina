# Lumina features

The complete feature reference. For the overview, see the [README](../README.md).

## Features

- **SQLite catalog with FTS5** — filename, folder, camera/lens, location, tags,
  and captions are all searchable; timeline buckets (year/month/day) are
  pre-aggregated for instant scrubbing. Keywords can be paths
  (`places/europe/paris`): `tag:places` includes sub-keywords, and so do
  smart-album "tag is" / "is not" rules.
- **Deep metadata** — EXIF camera/exposure/ISO data, GPS, and offline reverse
  geocoding against a built-in table of about 150 major cities (the nearest
  one within about 250 km; `crates/lumina-core/src/geocoder.rs`).
  The date taken is EXIF DateTimeOriginal, else DateTimeDigitized, else
  DateTime; a video's is its QuickTime creation time. With none, it is the
  file's creation time (its modification time when that is older), never the
  day it was added. Libraries catalogued before this are re-dated once, after
  a scan: only items whose date is the day they were added.
- **Smart albums** — 12 system albums plus a Photos-style builder (⌥⌘N or ⌘K):
  match all/any of conditions on kind, favorite, date (including "in the
  last N days/weeks/months"), tag, stars, pick/reject flag, color label,
  camera, lens, aperture, ISO, focal length, filename and place, with a live
  count and preview. Albums can be re-edited (⌘K "Edit Smart Album…"), deleted from
  the sidebar row (⌘Z restores), duplicated (⌘K "Duplicate Smart Album"), and
  their rules exported to or imported from a JSON file (⌘K "Export / Import
  Smart Album Rules…").
- **Albums** — hand-picked albums beside the smart ones: New Album (⌘N),
  Add to Album (⇧⌘A or the context menu), ⌫ in an album to take items out
  (the files stay), rename and delete with undo. ⌘K "Pin Album" keeps an
  album or smart album in a Pinned group at the top of the sidebar.
- **Fast grid & viewer** — virtualized grid with five densities (8, 6, 4, 3
  or 2 columns; ⌘= / ⌘-, ⌘-scroll or pinch, anchored at the pointer or the
  selection), sort by date,
  name, size or rating (ascending/descending, persisted), zoom (pinch or
  ⌘-scroll at the pointer; ⌘0 Fit, ⌘1 100%, ⌘2 200%, ⌘3 Fill, ⌘4 400%),
  pan (scroll or drag) and 1:1 pixel inspection,
  previous/next navigation, keyboard-first shortcuts.
- **Video playback** — native AVFoundation bridge with scrubbing (hovering
  or dragging the scrubber previews real frames; a playing video pauses while
  you drag), frame stepping, speed, mute (both remembered between launches), looping and a loop range for the open
  video (⌥I start, ⌥O end, ⌥X clear), chapter marks on the scrubber with
  ⌥← / ⌥→ for the previous / next chapter, and an Audio menu (also ⌘K
  "Audio Track: …") when a file offers more than one alternative audio
  track; the choice lasts until the video closes. Subtitles are not shown
  yet. Picture-in-Picture (⌥P) is a detached
  player: it keeps playing while you browse and view photos, and opening
  another video plays it there.
- **Organization** — tags, ratings, color labels, captions, favorites, albums,
  multi-select batch actions, and a full Trash lifecycle.
- **Culling** — keyboard-first, in the grid and the viewer: `P` pick, `X`
  reject, `U` remove the flag, `0`–`5` rate, each on the whole selection with
  one undo; in the viewer a key moves on to the next item. `is:pick`,
  `is:reject` and `is:unflagged` filter a search, ⌘K "Show Picks" / "Show
  Rejected" add them, and a smart album can match on the flag. File >
  Delete Rejected… (also ⌘K) sends the open view's rejects to the Delete
  dialog (Move to Trash, undoable, by default). `C` (View > Compare, ⌘K)
  compares 2–9 selected items side by side, 2×2 or 3×3 with one shared
  zoom: ←/→ or Tab focus a pane, `P`/`X`/`U` and `0`–`5` mark it, `⌫`
  takes it out of the comparison (the file stays), `Enter` opens it in the
  viewer and `Esc` comes back. `⌥⌘S` (View > Slideshow, ⌘K) plays the
  view full screen from the selection, 3, 5 or 10 seconds a slide (⌘K
  Slideshow Interval): `Space` pauses, `←`/`→` step, `Esc` ends it. Videos
  show their poster and do not play; it stops at the end unless ⌘K
  "Slideshow: Loop" is on.
- **Map** — Places has a Map toggle in the sidebar (also ⌘K "Show
  Map" / "Show Grid" and View > Show Map). ⌘K "Show on Map", or "Open
  map" on the info panel's location card, opens it centred on the selected
  item. The map is drawn offline from a
  bundled world outline and never loads tiles or uses the network. Items
  with a location gather in numbered circles; `Tab` or the arrows focus
  them west to east, `Enter` (or a click) shows that place's items in the
  grid, `⌘+`/`⌘-` zoom (six steps, keeping the focused place in the
  middle as far as the world's edge allows), dragging pans, and `Esc`
  returns to the grid. If the locations cannot be read, the banner's
  Retry or showing the map again reads them again.
- **Similar shots** — Duplicates has a second tab (⌘K "Show Similar Shots")
  listing RAW + JPEG pairs and bursts from one camera. Per stack, "Keep
  picks, reject the rest" (`Enter`) or "Reject all but the first" (`⇧Enter`,
  keeps the image of a pair, the earliest shot of a burst); `↑`/`↓` move
  between stacks and each action is one `⌘Z`. Nothing is deleted there:
  rejects wait for Delete Rejected….
- **Keywords** — a keyword can be a path (`places/usa/new york`). The Tags
  page (sidebar TAGS › Keywords) shows the tree, each level indented under its
  parent (a parent no item carries directly still gets a row), and filtering
  on a keyword (`tag:places`, View all in the Tags page detail panel, a smart-album "tag is" rule)
  includes everything below it. The tag field suggests keywords whose whole
  path or any segment starts with what you type, shown as
  `places › usa › new york`. Aliases (Tags page detail panel, ⇧⌘A there,
  or ⌘K "Add Alias for Keyword…" on a keyword's view) let `nyc` add `places/usa/new york`; the alias is
  never stored on a photo, and an alias cannot be a keyword already in use.
  "Move to…" (⇧⌘M on the Tags page, or ⌘K "Move Keyword…") moves a keyword and its children
  under another parent, and the rename and move prompts say how many items
  change. "Merge into…" (detail panel, a row's … menu, or ⌘K "Merge Tag
  Into…") moves a keyword's items onto another and keeps the old name as an
  alias of it; "Save as Smart Album" (same places, ⌘K "Save Tag as Smart
  Album") opens the smart-album builder with that keyword's rule. Importing XMP metadata treats a sidecar keyword the same way: one
  that matches an alias is stored as the keyword the alias points to.
- **Metadata portability** — exporting a selection copies the originals and
  writes Adobe-compatible XMP sidecars (`xmp:Rating`, `xmp:Label`, `dc:subject`,
  `lr:hierarchicalSubject`, `dc:description`) so ratings, color labels, tags, and
  captions survive outside the catalog (Lightroom has no Orange label and shows
  it as a custom one). Keyword paths go to `lr:hierarchicalSubject` as
  `places|europe|paris`, and `dc:subject` lists each name once (`places`,
  `europe`, `paris`), as Lightroom and Bridge expect; importing a sidecar with a
  hierarchy (Lightroom's included) brings back the paths, not the loose names.
  A keyword containing `|` stays flat in `dc:subject`;
  the same sidecars can be generated in place via `write_metadata_sidecars`.
  Sidecars inside the library are hidden in Finder (⇧⌘. shows them) but stay
  next to their media, where other apps look for them. Moving a whole folder
  in Finder takes them along; moving a single file leaves its sidecar behind
  until Lumina next rewrites it. Exported sidecars stay visible. Lumina never
  rewrites or deletes a sidecar another app wrote (darktable and digiKam use
  the same `photo.jpg.xmp` name): it still reads it on import, keeps your edit
  in the catalog, and the status bar says how many sidecars were left unchanged.
  Rights travel too: creator (`dc:creator`), copyright (`dc:rights`), usage
  terms (`xmpRights:UsageTerms`) and contact email and URL
  (`Iptc4xmpCore:CreatorContactInfo` `CiEmailWork` / `CiUrlWork`, also read in
  Lightroom's attribute form). Get Info shows the ones an item has, read-only;
  they are not searchable. Importing a sidecar sets the rights fields it names
  and never clears the others, and merging duplicates keeps them.
- **Export as JPEG** — File > Export as JPEG… (or `⌘K`) writes each selected
  photo as a JPEG at most 2048 px on the long side, with Lumina's rotation,
  crop and tone edits applied and no metadata at all (no EXIF, so no GPS
  position, and no sidecar). Photos are never scaled up. Videos are copied
  as originals; the report counts them and says when a video's rotation is
  left behind. Existing files are never overwritten. File > Export
  Originals… (also the context menu and batch dock Export…) copies the
  originals with XMP sidecars.
- **Share** — File > Share…, ⌘K Share…, Share… in the right-click menu or
  the batch dock's Share… button opens the macOS share sheet for the
  selected originals (macOS only, up to 500 items at a time). Missing files
  are left out; if every selected file is missing a toast says so. Lumina sends nothing itself: the sharing
  services are the system's.
- **Metadata templates** — save a creator, copyright, usage terms, contact
  email and website plus keywords as a named template (File > Metadata
  Templates…, or ⌘K Metadata Templates…). Apply one to the selection with ⌘K
  Apply Metadata Template… (then pick the name) or the batch inspector's
  TEMPLATE section. Applying overwrites only the fields the template sets,
  adds its keywords (aliases resolve as when typing), rewrites the XMP
  sidecars, and is one `⌘Z`. Templates live in your settings, not the
  library, so every library sees them.
  A date taken or position you edited (or imported from a sidecar) is written
  as `exif:DateTimeOriginal` and `photoshop:DateCreated` (wall time, no time
  zone) and `exif:GPSLatitude` / `exif:GPSLongitude`; one read from the file
  itself is not, so the file stays the source. Importing a sidecar date drops
  its time zone and keeps the wall time (a date with no time changes only the
  day), and an imported date or place survives
  a rescan like an edit.
- **Undo / redo** — `⌘Z` / `⇧⌘Z` for ratings, color labels, favorites, captions,
  titles, tag add/remove, global tag rename/merge/delete (one step even for a
  bulk delete), metadata templates, rotation, and trash/restore. Metadata
  edits also refresh the item's XMP sidecar automatically; permanent deletion
  is deliberately not undoable.
- **Editable metadata & XMP import** — click the title, date-taken or coordinate
  row in the inspector to edit it (SQLite, undoable; titles also go to
  `dc:title`; date edits rebuild timeline buckets). The date field takes a
  date (`2024-05-01`) or a signed shift (`+2h`, `-1d`, `+1d 3h 15m`, `-0:30`);
  with several items selected, the batch inspector's DATE row (or ⌘K Adjust
  Date Taken…) moves each item by the shift, keeping their spacing, and one
  ⌘Z restores them all. "Import XMP Metadata" in the
  card context menu applies sidecar titles, ratings, color labels, captions and
  tags (a sidecar without a label leaves the catalog's label alone).
- **Import** — drop files or folders on the window, or File ▸ Import… (⇧⌘I),
  to copy them into `Imports/YYYY/YYYY-MM-DD/` in the library by date taken.
  A dialog shows what will be copied and what skipped (already in the
  library, unsupported, unreadable) first; originals are never changed.
  Opening a folder still indexes it in place.
- **Missing files** — a file deleted or moved outside Lumina keeps its catalog
  row, metadata and thumbnail, with a badge and a Missing view. Files moved or
  renamed inside the library reconnect by inode; "Locate…" (context menu or
  ⌘K) points an item at its file by hand.
- **Rotate** — `R` turns the photo open in the viewer, or every photo
  selected in the grid, a quarter turn clockwise and `⇧R` counter-clockwise
  (also ⌘K, the Image menu and the context menu). Videos in the selection
  are skipped. The turn is stored in the library, never written to the
  file, and one `⌘Z` puts each photo back at its own previous angle.
- **Quick edits** — `E` in the viewer (also ⌘K and Image > Edit Photo)
  opens an edit bar: rotate (`R` / `⇧R`), exposure in ⅓-stop steps up to
  ±2 (`-` / `=`), auto enhance (`A`), and Revert to Original. `Enter` saves
  (one `⌘Z` undoes it) and `Esc` cancels; closing the viewer, showing
  another photo or opening another library discards unsaved adjustments.
  While editing, menu items that would open a sheet or act on other photos
  are disabled. Edits are stored in the library
  and applied when photos are shown, so the original file and the
  thumbnail cache are never changed; a small badge marks edited photos in
  the grid. Image > Revert to Original clears the edits and rotation of the
  whole selection in one undo step. Videos are not edited. Edits are not
  written to XMP sidecars, and Export Originals copies the originals without
  them (the export report says so); Export as JPEG applies them.
- **Crop** — in edit mode, `C` (or the Crop button) shows the whole
  photo, turned as it is shown, with the crop rectangle over it; `C` again
  cycles the aspect: Original, Square, 4:3, 3:2, 16:9 and Free (also the
  buttons under the edit bar). Each preset centres the largest rectangle
  of that aspect, measured on screen, so Square stays square on a turned
  photo. Drag a corner handle, or use the arrows to move the rectangle by
  1 % (`⇧` 10 %) and `⌥` + arrows to resize it; it stays inside the
  photo. `⇧C` (or the Crop button again) applies the crop and keeps
  editing, `Enter` saves it with the other edits (one `⌘Z`), and `Esc`
  leaves crop mode without the new crop and keeps edit mode open. The crop is stored
  independently of rotation, so turning a cropped photo keeps the same
  region; Revert to Original removes it.
- **Edit in…** — set an external editor in Settings, then `⌘E` (also ⌘K,
  Image > Edit in External Editor and the context menu) opens the selected
  file in it, in place. `⌥⌘E` (Edit a Copy) first copies the file to
  `name (edited).ext` in the same folder (never over an existing file, and
  never under a name a deleted or trashed library item still holds),
  adds the copy to the library with the original's keywords, caption,
  rating, favorite, label, title and rights, selects it and opens the copy;
  the original stays as it was. The editor opens the file without Lumina's
  crop, exposure or rotation, so the copy starts with none of them. With no
  editor set, both open Settings on the editor row. Neither works on
  several items at once or while editing a photo in Lumina.
  When the editor saves in place, the library picks the change up on its
  own: the item keeps its id, keywords and other metadata, and its
  thumbnail (and the viewer, if it is open) shows the saved file. A "Save
  As" file saved into the library folder appears as a new item. Lumina's
  crop, exposure and rotation stay applied on top of the saved file, so if
  the editor already cropped or turned it, use Image > Revert to Original.
- **Floating panels** — detachable inspector and PiP windows.
- **Keyboard-first** — a searchable command palette (⌘K) for every major action,
  arrows/Home/End/Page navigation with scroll-into-view, a built-in shortcut
  cheat sheet (⌘/), manual rescan (⌘R), Hide/Show Sidebar (⌃⌘S), and a native
  macOS menu bar (Open Library, Rescan, Hide/Show Sidebar, Inspector,
  Settings ⌘, About, Quit).
- **Settings** — a full-window page (⌘,) with five categories: General,
  Library, Downloads, Storage and About. It covers the launch view and the
  external editor; the library's location, size, rescan and a storage
  breakdown; the downloads folder and how many downloads run at once; the
  thumbnail cache (size, clear, reveal) and the settings and log files; and
  the version, licence and acknowledgements.

> **Status note:** local semantic/AI search (milestone 1.2 in the roadmap) is a
> planned feature and is **not implemented** in this tree.

## Supported formats

Photos are decoded upright and converted to sRGB, with the exceptions in the
table below. The catalog stores each photo's
displayed size, so a rotated portrait counts as a portrait. This table says
what the format tests (`crates/lumina-core/tests/format_matrix.rs`) actually
check on macOS, using files generated at test time.

| Format | Extensions | Upright | Wide colour to sRGB | Displayed size |
| --- | --- | --- | --- | --- |
| JPEG | jpg, jpeg | Tested, with EXIF rotation | Tested (Display P3) | Tested, with EXIF rotation |
| PNG, WebP | png, webp | Tested, unrotated only | Tested (Display P3) | Tested |
| HEIC | heic, heif | Tested, with EXIF rotation | Tested (Display P3) | Tested, with EXIF rotation |
| TIFF | tiff, tif | Tested at 8 MB or less, with EXIF rotation | Tested at 8 MB or less (Display P3); larger files take the same conversion, not tested | Tested, with EXIF rotation |
| RAW | arw, cr2, cr3, nef, dng, raf, rw2, orf | Not yet checked on real files | Not yet checked on real files | Not yet checked on real files |

Linux builds preview JPEG, PNG, WebP and TIFF through the in-process
decoders or ffmpeg. HEIC/HEIF and RAW previews are unavailable. Wide-gamut ICC
profiles are not color-matched on Linux, so colors may differ from macOS.

Video files (mp4, mov, m4v, avi, mkv) are catalogued. macOS uses AVFoundation;
Linux builds use GStreamer for playback and ffmpeg for poster and
scrub previews.

Limits:

- Colours outside sRGB are clipped to sRGB, so a wide-colour photo does not use
  the full range of a wide-colour display. The original file is never changed.
- RAW photos use the system's rendering. Lumina is not a RAW developer.
- A HEIC rotated only by its container, with no EXIF orientation, has not been
  tested.
- Colour conversion needs macOS tools. Linux does not currently preview HEIC,
  HEIF or RAW photos; other unsupported formats show a decode error.


## Libraries

On first run the welcome screen offers **Open Existing Library…** (↵),
**Import into New Library…** (⇧⌘I), **Explore a Sample Library** (⌥↵: 24
generated gradient photos written to
`~/Library/Application Support/lumina/Sample Library/`, then opened like any
other library) and **Use Pictures Folder** (⇧↵). Lumina stores its catalog in
`~/Library/Application Support/lumina/` and one thumbnail folder per library in
`~/Library/Caches/lumina/thumbnails/`; UI preferences live in
`~/Library/Application Support/lumina/config.json`. Linux uses the XDG data,
cache and config directories instead (see [linux.md](linux.md)).

**Removing a library** (⌘K › Delete Library…, the trash action on a welcome
recent, or Settings › Library › Known Libraries) deletes that library's catalog
and cached thumbnails and returns to the welcome screen. The library folder and
every photo and video stay where they are. Media paths are stored relative to
the library root as well as absolutely, so a library whose volume comes back at
a different mount point (an external drive, or a macOS volume mounted as
`Photos 1`) is found again from the volume recorded in its catalog and opened
without rescanning.

URL import (File > Import Video from URL…, ⌥⌘U) needs the pinned `yt-dlp` and
`ffmpeg`. The packaged app ships them. A build run outside a `.app`
(`make dev`, `cargo run`) looks beside its binary, then in Homebrew and
`PATH`, so run `make bundle-deps` once to add the pinned copies; without one,
paste-a-link import stops with "A part of Lumina needed for downloads is
missing from this build." Direct links to a video file (`…/clip.mp4`) never
need them. See [network-import.md](network-import.md).

