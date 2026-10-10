# System Design Document: Lumina Media Management Engine v2

**Project Codename**: `lumina`  
**Status**: Architecture Approved  
**Focus Area**: High-Performance Catalog, Semantic Search, Smart Albums, and Scalable Library Management  
**Target Platform**: macOS (Metal / Unified Memory) & Cross-Platform Headless Engine (Pure Rust)  

---

## 1. System Vision & Architecture Principles

Lumina is a zero-latency, local-first photo management engine. It decouples the performance-critical data pipeline (ingest, metadata indexing, vector embedding, and layout calculation) from the presentation shell.

### Core Architectural Tenets
* **Zero UI Thread Starvation**: The UI process/thread never executes synchronous file reads, directory scans, EXIF decoding, SQLite transactions, or vector inferences.
* **Non-Destructive Library Management**: Media files on disk are treated as an immutable source of truth. Moving, tagging, or grouping media is executed through a high-performance relational and vector catalog.
* **Local-First Privacy**: All semantic intelligence (CLIP embeddings, offline reverse geocoding, face clustering) operates 100% on-device without telemetry or cloud dependencies.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        PRESENTATION SHELL                              │
│                                                                        │
│   [ lumina-ui-gpui ]                   (Alt Spike: [ lumina-ui-slint ])│
│   • 120 FPS Virtualized Grid           • Unified Timeline & Folders    │
│   • Search Bar & Smart Album Filters   • Metadata & Location Inspector │
└───────────────────────────────────▲────────────────────────────────────┘
                                    │ IPC / In-Process Channels
                                    │ (Query Results, Viewport Slices)
┌───────────────────────────────────┴────────────────────────────────────┐
│                    lumina-core (Headless Library Engine)               │
│                                                                        │
│   ┌───────────────────────────┐    ┌────────────────────────────────┐  │
│   │     Catalog Database      │    │     Local Vector Engine        │  │
│   │ SQLite WAL + FTS5         │    │ sqlite-vec / HNSW              │  │
│   │ Pre-aggregated Buckets    │    │ Quantized MobileCLIP / ONNX    │  │
│   └─────────────▲─────────────┘    └───────────────▲────────────────┘  │
│                 │                                  │                   │
│   ┌─────────────┴─────────────┐    ┌───────────────┴────────────────┐  │
│   │    Metadata & Geocoding   │    │      Filesystem Ingest         │  │
│   │ nom-exif (Zero-Copy)      │    │ jwalk (Parallel Traversal)     │  │
│   │ Offline KD-Tree (GeoNames)│    │ notify-debouncer-mini (Watcher)│  │
│   └───────────────────────────┘    └────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Storage & Catalog Engine (SQLite WAL)

The catalog uses SQLite with Write-Ahead Logging (WAL) and memory mapping enabled (`PRAGMA mmap_size = 268435456;`). This allows concurrent multi-threaded readers to query the timeline at 120 FPS while background workers continuously ingest and index metadata.

### 2.1 Database Schema (`schema.sql`)

```sql
-- Core media catalog
CREATE TABLE IF NOT EXISTS media_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL UNIQUE,
    parent_dir TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mtime_epoch_ms INTEGER NOT NULL,
    
    -- Dimensions & Media Classification
    media_type TEXT NOT NULL CHECK(media_type IN ('image', 'video', 'raw')),
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    duration_ms INTEGER DEFAULT NULL,
    
    -- EXIF Temporal & Spatial Attributes
    timestamp_epoch_ms INTEGER NOT NULL,
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    day INTEGER NOT NULL,
    latitude REAL DEFAULT NULL,
    longitude REAL DEFAULT NULL,
    altitude REAL DEFAULT NULL,
    
    -- Reverse Geocoded Metadata
    country_code TEXT DEFAULT NULL,
    admin1_region TEXT DEFAULT NULL,
    city TEXT DEFAULT NULL,
    
    -- Camera Hardware Metadata
    make TEXT DEFAULT NULL,
    model TEXT DEFAULT NULL,
    lens TEXT DEFAULT NULL,
    focal_length REAL DEFAULT NULL,
    f_number REAL DEFAULT NULL,
    iso INTEGER DEFAULT NULL,
    exposure_time TEXT DEFAULT NULL,
    
    -- State Flags
    is_favorite INTEGER NOT NULL DEFAULT 0 CHECK(is_favorite IN (0, 1)),
    is_hidden INTEGER NOT NULL DEFAULT 0 CHECK(is_hidden IN (0, 1)),
    is_trashed INTEGER NOT NULL DEFAULT 0 CHECK(is_trashed IN (0, 1))
);

-- Performance Indexes for 120 FPS Scrolling & Hierarchy
CREATE INDEX IF NOT EXISTS idx_timeline_active 
    ON media_items(timestamp_epoch_ms DESC) 
    WHERE is_trashed = 0 AND is_hidden = 0;

CREATE INDEX IF NOT EXISTS idx_folder_listing 
    ON media_items(parent_dir, file_name) 
    WHERE is_trashed = 0;

CREATE INDEX IF NOT EXISTS idx_geo_lookup 
    ON media_items(latitude, longitude) 
    WHERE latitude IS NOT NULL;

-- Temporal Pre-aggregation for Instant Year/Month/Day Jump Lists
CREATE TABLE IF NOT EXISTS timeline_buckets (
    bucket_type TEXT NOT NULL CHECK(bucket_type IN ('year', 'month', 'day')),
    bucket_key TEXT NOT NULL,          -- e.g., '2026', '2026-10', '2026-10-10'
    start_timestamp INTEGER NOT NULL,
    end_timestamp INTEGER NOT NULL,
    item_count INTEGER NOT NULL,
    cover_media_id INTEGER,
    PRIMARY KEY (bucket_type, bucket_key),
    FOREIGN KEY(cover_media_id) REFERENCES media_items(id) ON DELETE SET NULL
);
```

---

## 3. Search Engine Architecture

The search engine supports a three-tiered pipeline combining keyword matching, structured query operators, and local semantic embeddings:

```text
User Input: "sunset in Kyoto camera:Sony is:favorite"
                                │
                                ▼
                   [ Query Tokenizer / Parser ]
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
 [ Semantic Vector ]     [ Structured Tokens ]     [ Full-Text Tokens ]
 "sunset"                camera = "Sony"           "Kyoto"
 Vector Similarity       is_favorite = 1           FTS5 Index Match
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                ▼
                 [ Query Plan Synthesizer ]
             Indexed SQLite SELECT with Vector CTE
```

### 3.1 Tier 1: Full-Text Search (SQLite FTS5)
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS media_search_fts USING fts5(
    file_name,
    parent_dir,
    city,
    admin1_region,
    country_code,
    model,
    content='media_items',
    content_rowid='id'
);
```

### 3.2 Tier 2: Structured Query Grammar
* **Temporal**: `year:2026`, `date:2026-09-01..2026-10-10`, `month:october`
* **Optics & Gear**: `camera:"Sony A7IV"`, `lens:35mm`, `iso:>1600`, `f:<2.8`
* **Location**: `city:Kyoto`, `country:JP`
* **Flags**: `is:favorite`, `is:video`, `is:raw`, `has:location`

### 3.3 Tier 3: Local Semantic & Vector Search (Offline AI)
* **Model**: Quantized MobileCLIP / SigLIP running on local GPU / Apple Neural Engine.
* **Storage**: 512-dimensional float embeddings using `sqlite-vec` or embedded HNSW:
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS media_embeddings USING vec0(
    media_id INTEGER PRIMARY KEY,
    embedding float[512]
);
```

---

## 4. Apple-Style Smart Albums Specification

### 4.1 System-Defined Media Types
| Smart Album | Evaluation Predicate Rule |
|---|---|
| **Favorites** | `is_favorite = 1 AND is_trashed = 0` |
| **Videos** | `media_type = 'video' AND is_trashed = 0` |
| **RAW Media** | `media_type = 'raw' OR file_name LIKE '%.arw' OR file_name LIKE '%.cr3'` |
| **Screenshots** | `width / height` matching display aspects OR filename regex `(?i)(screen[_\s]?shot)` |
| **Long Exposures** | `exposure_time >= 1.0` (in seconds) |
| **Panoramas** | `(width / height >= 2.8) OR (height / width >= 2.8)` |
| **Bursts / Sequences** | Grouped items with $\Delta t \le 400\text{ ms}$ from same device model |
| **Recently Deleted** | `is_trashed = 1` (auto-purges after 30 days) |
| **Places / Map View** | Clustered via geospatial bounding box queries on `(latitude, longitude)` |

### 4.2 User-Defined Smart Album Engine
```sql
CREATE TABLE IF NOT EXISTS smart_albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    icon_name TEXT NOT NULL DEFAULT 'folder-heart',
    match_mode TEXT NOT NULL CHECK(match_mode IN ('all', 'any')),
    rule_definition_json TEXT NOT NULL,
    created_at_epoch_ms INTEGER NOT NULL
);
```

---

## 5. Offline Location Engine (Reverse Geocoding)
* **Embedded Dataset**: Bundled binary dataset extracted from GeoNames ($\ge 500$ population, ~180,000 nodes, $\approx 12\text{ MB}$ compressed).
* **Spatial Index**: Loaded at engine startup into a 2D k-d tree in memory.
* **Lookup Resolution**: Resolved in $< 4\,\mu\text{s}$ per image on background ingestion threads.

---

## 6. Performance Gates & Verification Milestones
1. **Gate 1: Catalog Scale**: 250,000 media records, SQLite DB size $< 180\text{ MB}$.
2. **Gate 2: Search Latency**: $< 5\text{ ms}$ structured queries, $< 25\text{ ms}$ semantic queries, $< 1\text{ ms}$ timeline seeks.
3. **Gate 3: UI Fluidity**: Locked ProMotion 120 Hz render ($\le 8.33\text{ ms}$ frame budget).
