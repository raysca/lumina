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

> `benchmark_desktop_pics_cold_scan_and_decodes` benchmarks a real
> `~/Desktop/pics` folder (falling back to synthetic images if it is absent).
> Thumbnail decoding is best-effort because QuickLook/AVFoundation poster
> extraction can be unavailable headlessly; set `LUMINA_STRICT_BENCH=1` to
> require every thumbnail to decode.

## UI testing

Two layers of UI coverage live alongside the unit tests:

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
