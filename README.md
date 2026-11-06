# Lumina

A native, local-first photo & video manager for macOS, built in Rust with
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (the Zed UI
framework). The interface targets 120 FPS ProMotion rendering while a headless
core handles cataloguing, decoding, and search.

> **Platform status:** macOS 12+ is the only supported and released platform
> today. Windows and Linux are planned future targets; the current UI, player,
> decoder fallbacks and release scripts still contain macOS-specific paths. See
> [RELEASING.md](RELEASING.md#windows-and-linux-planned). Do not treat current
> packages or scripts as Windows/Linux support.

## Features

- **SQLite catalog with FTS5** — filename, folder, camera/lens, location, tags,
  and captions are all searchable; timeline buckets (year/month/day) are
  pre-aggregated for instant scrubbing. Keywords can be paths
  (`places/europe/paris`): `tag:places` includes sub-keywords, and so do
  smart-album "tag is" / "is not" rules.
- **Deep metadata** — EXIF camera/exposure/ISO data, GPS, and offline reverse
  geocoding against a bundled GeoNames dataset.
- **Smart albums** — 10 system albums plus a Photos-style builder (⌥⌘N or ⌘K):
  match all/any of conditions on kind, favorite, date (including "in the
  last N days/weeks/months"), tag, stars, color label, camera, lens,
  aperture, ISO, focal length, filename and place, with a live count and
  preview. Albums can be re-edited (⌘K "Edit Smart Album…") and deleted from
  the sidebar row.
- **Fast grid & viewer** — virtualized grid with five densities (8, 6, 4, 3
  or 2 columns; ⌘= / ⌘-, ⌘-scroll or pinch, anchored at the pointer or the
  selection), sort by date,
  name, size or rating (ascending/descending, persisted), zoom (pinch or
  ⌘-scroll at the pointer; ⌘0 Fit, ⌘1 100%, ⌘2 200%, ⌘3 Fill, ⌘4 400%),
  pan (scroll or drag) and 1:1 pixel inspection,
  previous/next navigation, keyboard-first shortcuts.
- **Video playback** — native AVFoundation bridge with scrubbing (hovering
  or dragging the scrubber previews real frames; a playing video pauses while
  you drag), frame stepping, speed, mute (both remembered between launches) and looping. Picture-in-Picture (⌥P) is a detached
  player: it keeps playing while you browse and view photos, and opening
  another video plays it there.
- **Organization** — tags, ratings, color labels, captions, favorites, albums,
  multi-select batch actions, and a full Trash lifecycle.
- **Metadata portability** — exporting a selection copies the originals and
  writes Adobe-compatible XMP sidecars (`xmp:Rating`, `xmp:Label`, `dc:subject`,
  `dc:description`) so ratings, color labels, tags, and captions survive outside
  the catalog (Lightroom has no Orange label and shows it as a custom one);
  the same sidecars can be generated in place via `write_metadata_sidecars`.
  Sidecars inside the library are hidden in Finder (⇧⌘. shows them) but stay
  next to their media, where other apps look for them. Moving a whole folder
  in Finder takes them along; moving a single file leaves its sidecar behind
  until Lumina next rewrites it. Exported sidecars stay visible.
- **Undo / redo** — `⌘Z` / `⇧⌘Z` for ratings, color labels, favorites, captions,
  titles, tag add/remove, and trash/restore. Metadata edits also refresh the
  item's XMP sidecar automatically; permanent deletion is deliberately not
  undoable.
- **Editable metadata & XMP import** — click the title, date-taken or coordinate
  row in the inspector to edit it (SQLite, undoable; titles also go to
  `dc:title`; date edits rebuild timeline buckets). "Import XMP Metadata" in the
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
- **Floating panels** — detachable inspector and PiP windows.
- **Keyboard-first** — a searchable command palette (⌘K) for every major action,
  arrows/Home/End/Page navigation with scroll-into-view, a built-in shortcut
  cheat sheet (⌘/), manual rescan (⌘R), and a native macOS menu bar (Open
  Library, Rescan, Sidebar/Inspector, Preferences ⌘, About, Quit).
- **Preferences** — in-app General/Storage/About panel: library path and item
  count, rescan, log/database locations, thumbnail-cache size with clear/reveal,
  and version/settings-file details.

> **Status note:** local semantic/AI search ("Phase 10" in the roadmap) is a
> planned feature and is **not implemented** in this tree.

## Requirements

- macOS 12 or newer for the current release (Windows and Linux are planned but
  not yet supported; their video/player/packaging backends are not implemented)
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
| `make ui-smoke` | Launch the real app window and verify it appears |
| `make bundle` | Build Lumina.app (universal when both rustup targets are installed) |
| `make release` | Build, sign, and package .zip/.dmg with SHA256SUMS |
| `make notarize` | Developer ID notarization + stapling, then package |
| `make icon` | Regenerate resources/Lumina.icns |

> Performance gates live in `crates/lumina-core/tests/benchmark_test.rs` and are
> **opt-in** so the normal test suite is not timing-sensitive: run them with
> `make bench` (which passes `--include-ignored`). The desktop-pics benchmark
> falls back to synthetic images when `~/Desktop/pics` is absent; thumbnail
> decoding is best-effort headlessly, and `LUMINA_STRICT_BENCH=1` requires every
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
   the harness app (System Settings → Privacy & Security → Accessibility → DeepSeek
   Harness / your terminal). Without it the script reports SKIP and exits 0; set
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
  are killed if they hang, and the in-process AVFoundation video frame grab is
  cancelled after 10 s, so a bad file cannot pin a decode worker forever.

## Continuous integration

- **test**: rustfmt, clippy `-D warnings`, workspace tests, debug build, live
  UI smoke and the OS-level UI harness (both skip cleanly on headless runners).
- **perf**: runs the opt-in benchmark suite (`make bench`, serialized for stable
  measurements); debug-mode budgets are intentionally looser than the strict
  release gates (seek <1 ms, structured query <5 ms, smart album <8 ms).
- **quality**: `cargo llvm-cov` summary + LCOV artifact, and a non-blocking
  `cargo audit` advisory report.
- **release** (`.github/workflows/release.yml`): tag-driven universal build,
  optional Developer ID signing/notarization, zip/DMG/checksums and a draft
  GitHub release.

## Layout

```
crates/
  lumina-core/       Headless engine: catalog, scanner, decoder, search,
                     smart albums, geocoder, caching
  lumina-ui-gpui/    GPUI app: app/ state & actions, views/, player/, icons,
                     theme, assets
resources/           Bundled fonts, app icon, Info.plist
docs/                Roadmap, system design, technical notes
```

## Documentation

- [Product roadmap](ROADMAP.md)
- [Everyday interaction UX audit](docs/ux_audit.md)
- [v0.2 implementation plan (historical)](docs/roadmap_v2.md)
- [System design](docs/system_design_v2.md)
- [Technical notes](docs/tech.md)
- [Search syntax](docs/search.md)
- [Implementation plan](docs/implementation_plan.md)
- [Releasing & notarization](RELEASING.md)
- [Accessibility & localization](docs/accessibility.md)
- [Engineering & review guide](AGENTS.md)

## Third-party assets

IBM Plex Sans/Mono fonts (SIL OFL 1.1) and Lucide icons (ISC) are bundled; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Dependency versions are pinned
in Cargo.lock; each crate keeps its own license.

## License

Lumina is released under the [MIT License](LICENSE).

Copyright © 2026 Lumina contributors. Bundled third-party assets remain under
their own licenses (see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
