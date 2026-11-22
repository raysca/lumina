# Developing Lumina

Building, testing and packaging Lumina from source. For what the app does, see
the [README](../README.md) and the [feature reference](features.md).

## Requirements

- **macOS:** macOS 12 or newer and the Xcode Command Line Tools (cc,
  AVFoundation, AppKit).
- **Linux:** Ubuntu 24.04 (glibc 2.39) or newer is the tested environment.
  Install the system development packages listed in
  [`.github/workflows/ci.yml`](../.github/workflows/ci.yml); see
  [linux.md](linux.md) for details.
- Rust stable, 1.95 or newer (see `rust-toolchain.toml`).

## Run from source

```bash
cargo run -p lumina-ui-gpui        # or: make dev
```

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
- **Thumbnail cache budget**: entries are stored compressed (JPEG, or PNG when
  transparent); startup cleanup removes entries unused for 30 days and evicts the
  least recently used down to a 1.5 GB budget (cache keys are format-versioned).
- **Decode safety**: photos decode in process through ImageIO (an unreadable
  photo fails at once); the QuickLook helper, used only when ImageIO cannot
  render a RAW file, runs with a 10 s timeout and is killed if it hangs, and
  the in-process AVFoundation video frame grab is
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

