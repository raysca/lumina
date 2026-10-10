# Lumina: Technical Architecture & Specification

Lumina is a pure-Rust, high-performance media manager designed to deliver the fluid 120 FPS timeline experience of Apple Photos alongside the agile folder workflows of Google Picasa.

---

## 1. Executive Summary

Lumina operates under a strict decoupled architecture: a headless, fully portable core engine (`lumina-core`) manages metadata, indexing, caching, and multi-threaded decoding, while swappable frontends (`lumina-ui-*`) handle windowing and GPU composition.

The primary frontend target is **GPUI** (Zed's GPU-accelerated UI framework) on macOS (Metal), designed so that alternative frontends (e.g., Slint, Iced) can be spiked out later with zero changes to the underlying storage or processing engine.

### Key Performance Targets

| Metric | Target | Rationale |
| --- | --- | --- |
| **Frame Budget** | $\le 8.33\text{ ms}$ (120 Hz) | Prevent UI hitching and micro-stutters during trackpad flicking. |
| **Main Thread Latency** | $0\text{ ms}$ I/O blocking | UI thread performs zero disk I/O, EXIF parsing, or decompression. |
| **Cold Scan Throughput** | $\ge 15{,}000\text{ items/sec}$ | Instant catalog population using pre-baked EXIF thumbnails. |
| **Memory Footprint** | $\le 512\text{ MB}$ VRAM / RAM | Hard upper bounds enforced via an in-memory LRU texture pool. |

---

## 2. System Architecture

```text
┌────────────────────────────────────────────────────────┐
│                   PRESENTATION SHELL                   │
│                                                        │
│  [ lumina-ui-gpui ]            (Alt Spike: [ lumina-ui-slint ])
│  • Metal / Vulkan Compositor   • Pure declarative markup
│  • Virtualized uniform_list    • ModelRc data adapter  
│  • Arc<RenderImage> LRU Cache  • Cross-platform shell  
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
│  └───────────────────────┘   └──────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐  │
│  │              Two-Tier Disk Cache                 │  │
│  │  ~/Library/Caches/lumina/ (xxhash3 + WebP blobs) │  │
│  └──────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────┘

```

---

## 3. Repository & Workspace Topology

```text
lumina/
├── Cargo.toml
├── crates/
│   ├── lumina-core/             # Headless engine (Pure Rust, 100% portable)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs           # Public façade (LuminaEngine)
│   │       ├── catalog.rs       # SQLite schema, indexing, search
│   │       ├── scanner.rs       # Multi-threaded filesystem traversal
│   │       ├── watcher.rs       # Debounced directory watcher
│   │       ├── cache.rs         # Two-tier LRU & on-disk cache
│   │       ├── decoder.rs       # SIMD image decoders & downscaling
│   │       └── layout.rs        # Virtual grid coordinate resolution
│   │
│   ├── lumina-ui-gpui/          # Primary macOS / Metal frontend
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs          # App bootstrap & event loop
│   │       ├── bridge.rs        # lumina-core -> RenderImage bridge
│   │       └── views/
│   │           ├── grid.rs      # Virtualized infinite timeline
│   │           └── viewer.rs    # Full-screen single image viewer
│   │
│   └── lumina-ui-slint/         # (Spike) Alternative cross-platform shell
│       ├── Cargo.toml
│       ├── ui/
│       │   └── app.slint
│       └── src/
│           └── main.rs

```

---

## 4. `lumina-core` Specification

### 4.1 Dependency Manifest (`crates/lumina-core/Cargo.toml`)

```toml
[package]
name = "lumina-core"
version = "0.1.0"
edition = "2021"

[dependencies]
# Concurrency & Channels
crossbeam-channel = "0.5"
rayon = "1.10"
parking_lot = "0.12"

# Filesystem & Watching
jwalk = "0.8"
notify = "6.1"
notify-debouncer-mini = "0.4"

# Metadata & Hashing
kamadak-exif = "0.5"
xxhash-rust = { version = "0.8", features = ["xxh3"] }
dirs = "5.0"

# Decoding & Media
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp"] }
zune-jpeg = "0.4"

# Catalog & Storage
rusqlite = { version = "0.31", features = ["bundled"] }

```

### 4.2 Data Types & Engine API (`crates/lumina-core/src/lib.rs`)

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crossbeam_channel::Receiver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
}

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub media_type: MediaType,
    pub width: u32,
    pub height: u32,
    pub timestamp_epoch_ms: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum PixelFormat {
    Rgba8,
    Bgra8,
}

pub struct DecodedFrame {
    pub id: MediaId,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    BatchDiscovered { count: usize },
    ItemAdded(MediaItem),
    ItemRemoved(MediaId),
    ScanComplete { total_items: usize },
}

pub struct LuminaEngine {
    // Internal state: SQLite connection pool, watcher, Rayon runtime
}

impl LuminaEngine {
    /// Bootstraps catalog, starts disk watcher, and begins parallel scan
    pub fn open(library_root: PathBuf) -> (Arc<Self>, Receiver<EngineEvent>) {
        todo!("Initialize database, disk cache, and spawn ingest channels")
    }

    /// Total count of items in the current active filter
    pub fn total_count(&self) -> usize {
        todo!("Return indexed count")
    }

    /// Read-only slice access for virtualized layout models
    pub fn get_item_slice(&self, range: std::ops::Range<usize>) -> Vec<MediaItem> {
        todo!("Slice items from in-memory index or SQLite index cache")
    }

    /// Asynchronous thumbnail request dispatched to Rayon thread pool
    pub fn request_thumbnail<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        todo!("Check disk cache -> Decode via Rayon -> Invoke callback")
    }
}

```

### 4.3 Ingestion & File Discovery (`crates/lumina-core/src/scanner.rs`)

* **Parallel Traversal:** `jwalk::WalkDirGeneric` skips hidden folders and classifies extensions across available CPU cores.
* **Batching:** Discovered records are dispatched in blocks of 256 items over crossbeam channels to avoid UI wake-up thrashing.
* **Hot Reloading:** `notify-debouncer-mini` applies a 500 ms sliding debounce window across directory mutations.

---

## 5. Primary Frontend: `lumina-ui-gpui`

### 5.1 Dependency Manifest (`crates/lumina-ui-gpui/Cargo.toml`)

```toml
[package]
name = "lumina-ui-gpui"
version = "0.1.0"
edition = "2021"

[dependencies]
lumina-core = { path = "../lumina-core" }
gpui = { git = "https://github.com/zed-industries/zed" }
parking_lot = "0.12"
lru = "0.12"
anyhow = "1.0"

```

### 5.2 GPU Frame Bridge (`crates/lumina-ui-gpui/src/bridge.rs`)

```rust
use gpui::{AppContext, RenderImage};
use lumina_core::DecodedFrame;
use std::sync::Arc;

pub fn frame_to_gpui_image(
    frame: DecodedFrame,
    cx: &mut AppContext,
) -> anyhow::Result<Arc<RenderImage>> {
    // Zero-copy transfer of raw pixel bytes to GPUI Metal texture handle
    let image = cx.create_image(
        frame.width,
        frame.height,
        &frame.data,
    )?;
    Ok(image)
}

```

### 5.3 Viewport Virtualization (`crates/lumina-ui-gpui/src/views/grid.rs`)

To guarantee 120 FPS on Apple ProMotion displays:

* **Mathematical Culling:** Uses GPUI's `uniform_list`, resolving visible row indices via integer division in $O(1)$ time:

$$\text{Row}_{\text{start}} = \lfloor \text{ScrollOffset} / \text{RowHeight} \rfloor$$


* **In-Memory LRU Texture Cache:** Caches up to 800 active `Arc<RenderImage>` handles.
* **Pre-fetching:** Issues decode requests for two rows ahead and behind the active viewport window.

---

## 6. Implementation Roadmap

### Phase 1: Engine Foundation (`lumina-core`)

* [ ] Scaffold Cargo workspace and directory layout.
* [ ] Implement `jwalk` scanner and `notify-debouncer-mini` pipeline.
* [ ] Implement two-tier cache with `xxhash3` directory hashing and WebP serialization.
* [ ] Integrate `zune-jpeg` and `image` SIMD decoders in a dedicated Rayon pool.

### Phase 2: GPUI Shell Implementation (`lumina-ui-gpui`)

* [ ] Build GPUI application shell and window bootstrapping.
* [ ] Wire `crossbeam-channel` messages into GPUI's asynchronous runtime (`cx.spawn()`).
* [ ] Implement `uniform_list` 6-column photo grid with neutral gray placeholders.
* [ ] Integrate background texture loading and LRU eviction.

### Phase 3: Polish & Evaluation

* [ ] Add full-screen photo inspection overlay toggled on double-click.
* [ ] Benchmark frame times on macOS using Metal System Trace in Instruments.
* [ ] Profile cold-boot catalog scanning against a 50,000-photo synthetic test folder.
