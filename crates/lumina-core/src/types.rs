//! The engine's public value types, re-exported from the crate root.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaId(pub u64);

/// A culling flag, stored as `media_items.flag`. Ordered so that the
/// highest wins a merge: pick beats unflagged beats reject. Not `None`,
/// which would shadow `Option::None` in glob imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Flag {
    Reject = -1,
    Unflagged = 0,
    Pick = 1,
}

impl Flag {
    /// The flag for a stored value; anything out of range reads by its sign.
    pub(crate) fn from_db(value: i64) -> Self {
        match value.signum() {
            1 => Flag::Pick,
            -1 => Flag::Reject,
            _ => Flag::Unflagged,
        }
    }
}

/// (latitude, longitude) for a location edit; `None` clears it.
pub type Coordinates = (Option<f64>, Option<f64>);

/// Viewer rotation is stored as clockwise quarter turns in 0..=3.
pub const QUARTER_TURNS: u8 = 4;

/// Normalizes any signed quarter-turn count into the stored 0..=3 range.
///
/// Rotate Left subtracts a turn from 0, which must wrap to 3 instead of
/// underflowing; a full four turns is the identity.
pub fn normalize_quarter_turns(turns: i32) -> u8 {
    turns.rem_euclid(QUARTER_TURNS as i32) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub parent_dir: String,
    pub file_name: String,
    /// User-editable title; falls back to a derived display title when absent.
    pub title: Option<String>,
    pub file_size: u64,
    pub mtime_epoch_ms: i64,

    // Dimensions & Media Classification
    pub media_type: MediaType,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,

    // EXIF Temporal & Spatial Attributes
    pub timestamp_epoch_ms: i64,
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,

    // Reverse Geocoded Metadata
    pub country_code: Option<String>,
    pub admin1_region: Option<String>,
    pub city: Option<String>,

    // Camera Hardware Metadata
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<f64>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub exposure_time: Option<String>,

    // State Flags
    pub is_favorite: bool,
    pub is_hidden: bool,
    pub is_trashed: bool,
}

impl MediaItem {
    /// Returns duration in seconds derived from `duration_ms`.
    pub fn duration_secs(&self) -> f64 {
        self.duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0)
    }

    /// Formats `timestamp_epoch_ms` as a human-readable date/time string.
    pub fn formatted_timestamp(&self) -> String {
        self.format_timestamp("%a %-d %b %Y · %H:%M")
    }

    /// Formats `timestamp_epoch_ms` (UTC) with a chrono `fmt`, falling back
    /// to the catalog's year-month-day when the timestamp is out of range.
    pub fn format_timestamp(&self, fmt: &str) -> String {
        chrono::DateTime::from_timestamp_millis(self.timestamp_epoch_ms)
            .map(|dt| dt.format(fmt).to_string().trim().to_string())
            .unwrap_or_else(|| format!("{:04}-{:02}-{:02}", self.year, self.month, self.day))
    }

    /// Formats `mtime_epoch_ms` as a human-readable date/time string.
    pub fn formatted_mtime(&self) -> String {
        chrono::DateTime::from_timestamp_millis(self.mtime_epoch_ms)
            .map(|dt| dt.format("%e %b %Y, %H:%M").to_string().trim().to_string())
            .unwrap_or_else(|| format!("{:04}-{:02}-{:02}", self.year, self.month, self.day))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineBucket {
    pub bucket_type: String, // 'year', 'month', 'day'
    pub bucket_key: String,  // '2026', '2026-10', '2026-10-10'
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub item_count: usize,
    pub cover_media_id: Option<MediaId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[allow(clippy::large_enum_variant)]
pub enum EngineEvent {
    BatchDiscovered {
        items: Vec<MediaItem>,
    },
    ScanProgress {
        items_found: usize,
        is_complete: bool,
        /// Media files under the root, counted before the walk (0 = unknown).
        total: usize,
        /// Media-named files that could not be read.
        skipped: usize,
        /// The file the scan has most recently reached.
        current_file: Option<PathBuf>,
    },
    ItemRemoved(MediaId),
    /// Files gone from disk. The rows and their metadata are kept, marked
    /// missing; the order of the library is unchanged.
    ItemsMissing(Vec<MediaId>),
    /// Files an import plan has read so far, of `total`; duplicates are
    /// checked once `checked == total`.
    ImportChecking {
        checked: usize,
        total: usize,
    },
    /// Files a running import has copied or failed so far, of `total`.
    ImportProgress {
        done: usize,
        total: usize,
    },
    ScanComplete {
        total_items: usize,
    },
    /// A catalog or scan failure for the error banner, as "Title: detail".
    Error(String),
}

/// What [`LuminaEngine::relink`](crate::LuminaEngine::relink) did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelinkOutcome {
    /// The missing item now points at the chosen file.
    Relinked,
    /// The chosen file was already catalogued as this item (now deleted);
    /// its metadata was merged into the missing item, which took its file.
    Folded(MediaId),
}

/// Result of [`LuminaEngine::write_metadata_sidecars`](crate::LuminaEngine::write_metadata_sidecars).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SidecarWriteReport {
    pub written: usize,
    /// Sidecars another app wrote (or Lumina cannot read), left untouched.
    pub skipped_foreign: Vec<PathBuf>,
}

/// Result of importing XMP sidecars.
#[derive(Debug, Default, Clone)]
pub struct ImportReport {
    pub imported: usize,
    pub updated: usize,
    pub skipped: usize,
    pub failed: Vec<(PathBuf, String)>,
}
