# Lumina

A native, local-first photo & video manager for macOS, built in Rust with
[GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (the Zed UI
framework). The interface targets 120 FPS ProMotion rendering while a headless
core handles cataloguing, decoding, and search.

## Features

- **SQLite catalog with FTS5** — filename, folder, camera/lens, location, tags,
  and captions are all searchable; timeline buckets (year/month/day) are
  pre-aggregated for instant scrubbing.
- **Deep metadata** — EXIF camera/exposure/ISO data, GPS, and offline reverse
  geocoding against a bundled GeoNames dataset.
- **Smart albums** — 9 system albums plus a composable JSON rule builder.
- **Fast grid & viewer** — virtualized grid, 2–8 column density, zoom/pan and
  1:1 pixel inspection, filmstrip navigation, keyboard-first shortcuts.
- **Video playback** — native AVFoundation bridge with scrubbing, frame
  stepping, speed, mute, looping, and a floating Picture-in-Picture window.
- **Organization** — tags, ratings, color labels, captions, favorites, albums,
  multi-select batch actions, and a full Trash lifecycle.
- **Metadata portability** — exporting a selection copies the originals and
  writes Adobe-compatible XMP sidecars (`xmp:Rating`, `dc:subject`,
  `dc:description`) so ratings, tags, and captions survive outside the catalog;
  the same sidecars can be generated in place via `write_metadata_sidecars`.
- **Undo / redo** — `⌘Z` / `⇧⌘Z` for ratings, color labels, favorites, captions,
  titles, tag add/remove, and trash/restore. Metadata edits also refresh the
  item's XMP sidecar automatically; permanent deletion is deliberately not
  undoable.
- **Editable metadata & XMP import** — click the title, date-taken or coordinate
  row in the inspector to edit it (SQLite, undoable; titles also go to
  `dc:title`; date edits rebuild timeline buckets). "Import XMP Metadata" in the
  card context menu applies sidecar titles, ratings, captions and tags.
- **Floating panels** — detachable inspector and PiP windows.
- **Keyboard-first** — arrows/Home/End/Page navigation with scroll-into-view, and
  a built-in shortcut cheat sheet (⌘/) plus manual rescan (⌘R).

> **Status note:** local semantic/AI search ("Phase 10" in the roadmap) is a
> planned feature and is **not implemented** in this tree.

## Requirements

- macOS 12 or newer (the video bridge and packaging targets are macOS-only)
- Xcode Command Line Tools (cc, AVFoundation, AppKit)
- Rust stable — see rust-toolchain.toml

## Quick start

```bash
cargo run -p lumina-ui-gpui        # or: make dev
```

Use **Choose Folder** to open a photo library. Lumina stores its catalog in
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
| `make bundle` / `make dmg` | Build Lumina.app / installer DMG |
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
  are killed if they hang, so a bad file cannot pin a decode worker forever.

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

- [Product & engineering roadmap](docs/roadmap_v2.md)
- [System design](docs/system_design_v2.md)
- [Technical notes](docs/tech.md)
- [Implementation plan](docs/implementation_plan.md)

## Third-party assets

IBM Plex Sans/Mono fonts (SIL OFL 1.1) and Lucide icons (ISC) are bundled; see
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Rust crate licenses are
recorded in Cargo.lock.

## License

No license has been selected for this repository yet. Add a LICENSE file
before distributing binaries or accepting outside contributions.
