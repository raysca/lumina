# Lumina

A native, local-first photo & video manager for macOS, built in Rust with
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (the Zed UI
framework). The interface targets 120 FPS ProMotion rendering while a headless
core handles cataloguing, decoding, and search.

> **Platform status:** macOS 12+ is the only supported platform; no release
> has been tagged yet.
> Linux development is underway, but its build, media and package paths have
> not completed the release acceptance checks. Windows remains planned. See
> [Linux development notes](docs/linux.md) and
> [RELEASING.md](RELEASING.md#windows-planned-linux-in-development). Do not
> treat Linux CI artifacts as supported releases.

## Features

- **SQLite catalog with FTS5** — filename, folder, camera/lens, location, tags,
  and captions are all searchable; timeline buckets (year/month/day) are
  pre-aggregated for instant scrubbing. Keywords can be paths
  (`places/europe/paris`): `tag:places` includes sub-keywords, and so do
  smart-album "tag is" / "is not" rules.
- **Deep metadata** — EXIF camera/exposure/ISO data, GPS, and offline reverse
  geocoding against a built-in table of about 150 major cities (the nearest
  one within about 250 km; `crates/lumina-core/src/geocoder.rs`).
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
- **Edit in…** — set an external editor in Preferences, then `⌘E` (also ⌘K,
  Image > Edit in External Editor and the context menu) opens the selected
  file in it, in place. `⌥⌘E` (Edit a Copy) first copies the file to
  `name (edited).ext` in the same folder (never over an existing file, and
  never under a name a deleted or trashed library item still holds),
  adds the copy to the library with the original's keywords, caption,
  rating, favorite, label, title and rights, selects it and opens the copy;
  the original stays as it was. The editor opens the file without Lumina's
  crop, exposure or rotation, so the copy starts with none of them. With no
  editor set, both open Preferences on the editor row. Neither works on
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
  Preferences ⌘, About, Quit).
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

Linux development builds preview JPEG, PNG, WebP and TIFF through the in-process
decoders or ffmpeg. HEIC/HEIF and RAW previews are unavailable. Wide-gamut ICC
profiles are not color-matched on Linux, so colors may differ from macOS. This
path has not completed the Linux release acceptance checks.

Video files (mp4, mov, m4v, avi, mkv) are catalogued. macOS uses AVFoundation;
Linux development builds use GStreamer for playback and ffmpeg for poster and
scrub previews. Linux behavior is not yet covered by the release acceptance
suite.

Limits:

- Colours outside sRGB are clipped to sRGB, so a wide-colour photo does not use
  the full range of a wide-colour display. The original file is never changed.
- RAW photos use the system's rendering. Lumina is not a RAW developer.
- A HEIC rotated only by its container, with no EXIF orientation, has not been
  tested.
- Colour conversion needs macOS tools. Linux does not currently preview HEIC,
  HEIF or RAW photos; other unsupported formats show a decode error.

## Requirements

- macOS 12 or newer (the supported platform). Linux remains in development;
  its internal amd64 package targets Ubuntu 24.04 (glibc 2.39), and other
  distributions are not yet validated. See `docs/linux.md` and `RELEASING.md`.
- Xcode Command Line Tools (cc, AVFoundation, AppKit) on macOS
- Rust stable, 1.95 or newer — see rust-toolchain.toml

## Quick start

```bash
cargo run -p lumina-ui-gpui        # or: make dev
```

On first run the welcome screen offers **Open Existing Library…** (↵),
**Import into New Library…** (⇧⌘I), **Explore a Sample Library** (⌥↵: 24
generated gradient photos written to
`~/Library/Application Support/lumina/Sample Library/`, then opened like any
other library) and **Use Pictures Folder** (⇧↵). Lumina stores its catalog in
`~/Library/Application Support/lumina/` and thumbnails in
`~/Library/Caches/lumina/thumbnails/`; UI preferences live in
`~/Library/Application Support/lumina/config.json`.

## Commands

| Command | Description |
| --- | --- |
| `make dev` / `make run` | Run the desktop app (debug) |
| `make build` | Build the workspace |
| `make test` | Run all tests |
| `make check` | Fast typecheck |
| `make lint` | Clippy (--all-targets --all-features) |
| `make fmt` / `make fmt-fix` | Check / apply rustfmt |
| `make bench` | Run performance benchmarks |
| `make run-release` | Run the desktop app (optimized release build) |
| `make ui-smoke` | Launch the real app window and verify it appears |
| `make ui-e2e` | OS-level UI automation (needs Accessibility permission; skips without it) |
| `make bundle-deps` | Fetch the pinned yt-dlp and ffmpeg that URL import bundles |
| `make bundle` | Build Lumina.app (universal when both rustup targets are installed) |
| `make bundle-linux` | Build the internal, unsigned Linux `.deb` (development only) |
| `make release` | Build, sign, and package .zip/.dmg with SHA256SUMS |
| `make notarize` | Developer ID notarization + stapling, then package |
| `make icon` | Regenerate resources/Lumina.icns |

> Performance gates live in `crates/lumina-core/tests/benchmark_test.rs` and are
> **opt-in** so the normal test suite is not timing-sensitive: run them with
> `make bench` (which passes `--include-ignored`). The media benchmark uses
> synthetic images by default; set `LUMINA_BENCH_MEDIA_DIR` to a folder of your
> own photos to run it on those. Thumbnail decoding is best-effort headlessly, and `LUMINA_STRICT_BENCH=1` requires every
> thumbnail to decode.

## UI testing

Three layers of UI coverage live alongside the unit tests:

1. **In-process UI automation** (`cargo test -p lumina-ui-gpui ui_automation`):
   drives the real GPUI view tree with synthetic platform input — mouse clicks
   at rendered element bounds, key events, double/right clicks and window
   resizes — then asserts application state. Covers toolbar/sidebar controls,
   search click+typing+clear, grid selection, double-click viewer, context menu,
   Home/End/Page navigation, modal backdrop dismissal, the shortcut sheet, and
   window-bounds observation. Runs headlessly and in CI.
2. **Live window smoke test** (`make ui-smoke`): launches the release/debug
   binary with an isolated `HOME`, waits for a native window via CoreGraphics,
   saves a screenshot to `target/ui-smoke/window.png` when Screen Recording is
   permitted, and shuts the app down. It skips cleanly on headless CI and needs
   no Accessibility permissions.
3. **OS-level UI automation** (`make ui-e2e`): launches the real binary with an
   isolated `HOME` and a seeded library, waits for the native window, then posts
   real macOS keyboard events (CGEvent) and verifies outcomes through screenshots
   (pixel diffs), `config.json`, and the SQLite catalog:
   - `i` toggles the inspector and persists `inspector_visible`
   - `⌘/` opens the shortcut sheet; `Esc` closes it (window screenshot diff)
   - `Space` opens the viewer; `Esc` returns (window screenshot diff)
   - `Home` + `F` favorites the first item (verified in `catalog.db`)

   Launch/window/screenshot/config/catalog checks run without extra permissions.
   The synthetic-input tests require **Accessibility** ("PostEvent") permission for
   the app running the harness, for example your terminal (System Settings →
   Privacy & Security → Accessibility). Without it the script reports SKIP and exits 0; set
   `LUMINA_REQUIRE_UI_PERMISSIONS=1` to make a missing grant fatal.

## Diagnostics & safety

- **Logs**: structured log file at `<data dir>/lumina/logs/lumina.log` (rotated at
  5 MB) plus stderr; set `LUMINA_LOG=debug` (or another log level) for more detail.
  A panic hook records crashes to the same file.
- **Single instance**: only one Lumina process may run at a time, enforced with an
  OS file lock; a second launch exits with an explanatory error.
- **Catalog protection**: schema version is stamped in `PRAGMA user_version`,
  `busy_timeout` is enabled, a quick integrity check runs on open, and a
  single-generation `.db.bak` snapshot is written with `VACUUM INTO` when the
  catalog changed (at most once per app launch).
- **Thumbnail cache budget**: startup cleanup removes entries older than 30 days
  and evicts oldest files down to a 1.5 GB budget (cache keys are format-versioned).
- **Decode safety**: native `sips`/QuickLook helpers run with a 10 s timeout and
  are killed if they hang (an unreadable photo fails at once; only RAW files
  fall back to QuickLook), and the in-process AVFoundation video frame grab is
  cancelled after 10 s, so a bad file cannot pin a decode worker forever.

## Continuous integration

- **test**: rustfmt, clippy `-D warnings`, workspace tests, debug build, live
  UI smoke and the OS-level UI harness on macOS (both skip cleanly on headless
  runners).
- **linux**: native Ubuntu 24.04 build with rustfmt, clippy, workspace tests,
  and X11 and Wayland live smokes under Xvfb (Linux is in development, not
  supported).
- **package-linux**: builds the unsigned amd64 `.deb`, installs, launches and
  removes it, and keeps it as a 14-day internal workflow artifact.
- **bundle-macos**: builds an ad-hoc signed arm64 `Lumina.app` zip as a 14-day
  internal workflow artifact (not Developer ID signed or notarized).
- **perf**: runs the opt-in benchmark suite (`make bench`, serialized for stable
  measurements); debug-mode budgets are intentionally looser than the strict
  release gates (seek <1 ms, structured query <5 ms, smart album <8 ms).
- **quality**: `cargo llvm-cov` summary + LCOV artifact, and a non-blocking
  `cargo audit` advisory report.
- **release** (`.github/workflows/release.yml`): tag-driven universal macOS
  build, optional Developer ID signing/notarization, zip/DMG/checksums and a
  draft GitHub release. Its `linux` job builds and checks the unsigned `.deb`
  and keeps it as a 30-day workflow artifact; it is not attached to the
  release.

## Layout

```
crates/
  lumina-core/       Headless engine: catalog, scanner, decoder, search,
                     smart albums, geocoder, caching
  lumina-ui-gpui/    GPUI app: app/ state & actions, views/, player/, icons,
                     theme, assets
resources/           Bundled fonts, app icon, Info.plist
docs/                Linux, URL import, search and accessibility notes
```

## Documentation

- [Product roadmap](ROADMAP.md)
- [Known issues](BUGS.md)
- [Search syntax](docs/search.md)
- [Import Video from URL](docs/network-import.md)
- [Linux development notes](docs/linux.md)
- [Releasing & notarization](RELEASING.md)
- [Accessibility & localization](docs/accessibility.md)
- [Engineering & review guide](AGENTS.md)

## Third-party assets

Inter, Inter Tight and JetBrains Mono fonts (SIL OFL 1.1), Lucide icons (ISC) and the Natural
Earth 1:110m land outline (public domain, for the offline map) are bundled; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Dependency versions are pinned
in Cargo.lock; each crate keeps its own license.

## License

Lumina is released under the [MIT License](LICENSE).

Copyright © 2026 Lumina contributors. Bundled third-party assets remain under
their own licenses (see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
