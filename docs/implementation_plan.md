# Lumina: Implementation Plan & Phase Roadmap

This document outlines the phased roadmap and actionable task backlog for implementing Lumina as specified in [`tech.md`](tech.md).

---

## Architecture Overview

```text
┌────────────────────────────────────────────────────────┐
│                   PRESENTATION SHELL                   │
│                                                        │
│  [ lumina-ui-gpui ]                                    │
│  • Onboarding & Native Directory Picker (rfd / GPUI)   │
│  • Persistent library session & switch folder dialog   │
│  • Live streaming scan progress header                 │
│  • Virtualized responsive square grid (uniform_list)   │
│  • Arc<RenderImage> LRU Texture Pool (Zero I/O on UI)  │
└───────────────────────────▲────────────────────────────┘
                            │ Public Boundary API
                            │ (Events, Slices, DecodedFrame)
┌───────────────────────────┴────────────────────────────┐
│                 lumina-core (Headless)                 │
│                                                        │
│  ┌───────────────────────┐   ┌──────────────────────┐  │
│  │   Catalog Database    │   │ Layout Virtualizer   │  │
│  │ SQLite WAL (rusqlite) │   │ Viewport Culling     │  │
│  └───────────────────────┘   └──────────────────────┘  │
│  ┌───────────────────────┐   ┌──────────────────────┐  │
│  │   Ingest & Watcher    │   │ Decoding Pipeline    │  │
│  │ jwalk + notify-mini   │   │ Rayon + zune-jpeg    │  │
│  │                       │   │ + image + macOS AVF  │  │
│  └───────────────────────┘   └──────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │              Two-Tier Disk Cache                 │  │
│  │  ~/Library/Caches/lumina/ (xxhash3 + WebP blobs) │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘
```

---

## Phase 1: Workspace & Headless Core Engine (`lumina-core`)

**Status**: Complete

### 1.1 Workspace Scaffolding
- [x] Create root [`Cargo.toml`](../Cargo.toml) with workspace members `crates/lumina-core` and `crates/lumina-ui-gpui`.
- [x] Scaffold `crates/lumina-core` package with core dependencies:
  - `crossbeam-channel`, `rayon`, `parking_lot`
  - `jwalk`, `notify`, `notify-debouncer-mini`
  - `kamadak-exif`, `xxhash-rust`, `dirs`
  - `image`, `zune-jpeg`
  - `rusqlite` (with `bundled` feature)
- [x] Define core data contracts in `crates/lumina-core/src/lib.rs`:
  - `MediaId`, `MediaType`, `MediaItem`, `PixelFormat`, `DecodedFrame`, `EngineEvent`
  - `LuminaEngine` public API façade (`open`, `total_count`, `get_item_slice`, `request_thumbnail`).

### 1.2 Catalog Database & Persistence (`crates/lumina-core/src/catalog.rs`)
- [x] Implement SQLite schema with WAL mode (`PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`).
- [x] Define tables:
  - `media_items`: `id`, `path`, `file_size`, `mtime_epoch_ms`, `media_type`, `width`, `height`, `duration_secs`, `created_epoch_ms`, `hash`.
  - `libraries`: active and recent library paths.
- [x] Add batched insert/upsert methods for high-throughput cold scans.
- [x] Implement indexed slice retrieval for viewport virtualization: `get_slice(offset, limit)`.

### 1.3 Parallel Scanner & Discovery Pipeline (`crates/lumina-core/src/scanner.rs`)
- [x] Implement multi-threaded filesystem walker using `jwalk::WalkDirGeneric`.
- [x] Filter supported extensions:
  - Images: `.jpg`, `.jpeg`, `.png`, `.webp`, `.heic`
  - Videos: `.mp4`, `.mov`, `.m4v`
- [x] Batch discovered records in blocks of 256 items.
- [x] Stream progress events over crossbeam channels:
  - `ScanProgress { items_found: usize, is_complete: bool }`
  - `BatchDiscovered { items: Vec<MediaItem> }`
  - `ScanComplete { total_items: usize }`

### 1.4 Two-Tier Caching & SIMD Image/Video Decoders (`crates/lumina-core/src/cache.rs` & `decoder.rs`)
- [x] Implement two-tier cache keyed by `xxhash3` of `(path, file_size, mtime)`.
- [x] Manage on-disk thumbnail store at `~/Library/Caches/lumina/thumbnails/`.
- [x] Dedicated Rayon thread pool for background decoding:
  - Fast SIMD JPEG decoding using `zune-jpeg`.
  - PNG/WebP decoding via `image`.
  - Native macOS video poster frame extraction for `.mov` and `.mp4` files via QuickLook / AVFoundation.
- [x] Callback-driven asynchronous thumbnail dispatch: `request_thumbnail(...)`.

### 1.5 Real-Time Directory Watcher (`crates/lumina-core/src/watcher.rs`)
- [x] Integrate `notify-debouncer-mini` with a 500 ms debounce window.
- [x] Detect runtime file additions, modifications, and deletions in the active library directory and update catalog state.

---

## Phase 2: GPUI Frontend Shell (`lumina-ui-gpui`)

**Status**: Complete

### 2.1 GPUI Application Bootstrap & Configuration
- [x] Scaffold `crates/lumina-ui-gpui` with `gpui`, `lumina-core`, `rfd`, `lru`, and `parking_lot`.
- [x] Setup app entrypoint and window lifecycle in `crates/lumina-ui-gpui/src/main.rs`.
- [x] Implement persistent configuration store (`~/Library/Application Support/lumina/config.json`) to persist library root across app launches.

### 2.2 Onboarding & Folder Selection View
- [x] Implement empty/onboarding state when no library is selected:
  - Clean hero graphic / typography.
  - "Select Photos Folder" primary action button.
- [x] Connect native macOS folder picker (`rfd::FileDialog` / GPUI native dialog).
- [x] Add top bar navigation with current folder display and "Change Folder" / "Rescan" actions.

### 2.3 Streaming Scan Progress Header
- [x] Wire `crossbeam-channel` engine events into GPUI asynchronous event loop (`cx.spawn()`).
- [x] Render live scan status indicator in the top header:
  - Dynamic counter: "Scanning... 1,420 items found"
  - Animated progress indicator during active indexing.
  - Smooth transition to total item count upon `ScanComplete`.
- [x] Allow instant navigation and scrolling in the grid while items continue to stream in.

### 2.4 120 FPS Virtualized Grid View (`crates/lumina-ui-gpui/src/views/grid.rs`)
- [x] Implement grid virtualization using GPUI's `uniform_list`.
- [x] Mathematical row culling in $O(1)$ time:
  $$\text{Row}_{\text{start}} = \lfloor \text{ScrollOffset} / \text{RowHeight} \rfloor$$
- [x] Responsive column layout to maintain square cards.
- [x] Render neutral gray placeholders while textures are loading to eliminate UI stuttering.
- [x] Render video card badges (translucent play icon & duration tag).

### 2.5 GPU Frame Bridge & LRU Texture Management (`crates/lumina-ui-gpui/src/bridge.rs`)
- [x] In-memory LRU texture pool caching up to 800 active `Arc<RenderImage>` handles.
- [x] Asynchronous thumbnail dispatch:
  - Viewport visible items query LRU cache.
  - Cache misses trigger background decode requests via `lumina-core`.
  - Prefetch 2 rows above and below the visible viewport.
- [x] Transfer raw pixel bytes to GPUI Metal texture handles via `cx.create_image(...)`.
- [x] Zero I/O or decoding work on the UI main thread to preserve the $\le 8.33\text{ ms}$ (120 Hz) frame budget.

---

## Phase 3: Verification, Polish & Performance Benchmarking

**Status**: Complete

### 3.1 Detail View & Interactions
- [x] Implement single-item inspection view on click.
- [x] Reveal in Finder integration and metadata inspection bar.

### 3.2 Automated Testing
- [x] Unit tests for `lumina-core`:
  - `scanner`: directory traversal, symlink handling, supported file extensions.
  - `catalog`: SQLite migrations, batch inserts, sliced queries.
  - `cache`: `xxhash3` cache keys, disk hit/miss validation.
- [x] Concurrency tests: verify multiple thumbnail decode requests under heavy load without deadlock.
- [x] Desktop pics real-world integration test (`Desktop/pics`).

### 3.3 Performance & 120 FPS Audit
- [x] Profile cold scan throughput against test media folders: achieved **49,883 items/sec** (exceeding the $\ge 15{,}000\text{ items/sec}$ target).
- [x] Verify zero main thread blocking ($0\text{ ms}$ disk I/O on UI thread).
- [x] Confirm Metal GPU pipeline builds, links, and runs cleanly on macOS.
