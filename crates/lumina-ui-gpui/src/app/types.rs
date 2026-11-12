use lumina_core::{DecodedFrame, DeletionTarget, EngineEvent, MediaId};
#[cfg(test)]
use lumina_core::{MediaItem, MediaType};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagSortOrder {
    Alphabetical,
    CountDesc,
}

impl TagSortOrder {
    /// The Tags page sort menu's rows, in order.
    pub const ALL: [TagSortOrder; 2] = [TagSortOrder::CountDesc, TagSortOrder::Alphabetical];

    /// The menu row and trigger label: "Most used", "A–Z".
    pub fn label(self) -> String {
        crate::i18n::t(match self {
            TagSortOrder::Alphabetical => "tags.sort_az",
            TagSortOrder::CountDesc => "tags.sort_most_used",
        })
    }
}

/// The Tags page row `…` menu's items, in order. Merge into… (Design 08
/// Task 5) and Save as Smart Album (Task 6) join when they exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagRowAction {
    Rename,
    Move,
    Delete,
}

impl TagRowAction {
    pub const ALL: [TagRowAction; 3] = [
        TagRowAction::Rename,
        TagRowAction::Move,
        TagRowAction::Delete,
    ];

    /// The menu row's selector suffix: `tag-row-menu-<id>`.
    pub fn id(self) -> &'static str {
        match self {
            TagRowAction::Rename => "rename",
            TagRowAction::Move => "move",
            TagRowAction::Delete => "delete",
        }
    }

    pub fn label(self) -> String {
        crate::i18n::t(match self {
            TagRowAction::Rename => "tags.row_rename",
            TagRowAction::Move => "tags.row_move",
            TagRowAction::Delete => "tags.row_delete",
        })
    }

    /// The same action's key on the page, shown at the row's right.
    pub fn shortcut(self) -> &'static str {
        match self {
            TagRowAction::Rename => "↵",
            TagRowAction::Move => "⇧⌘M",
            TagRowAction::Delete => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteIntent {
    EmptyTrash,
    /// Delete from the Trash view: "Move to Trash" means the macOS Trash.
    DeleteSelected,
    /// Delete from the library (v3): "Move to Trash" means Lumina's
    /// Recently Deleted, which is undoable.
    LibrarySelection,
    /// Delete Rejected… (v5 B3): as `LibrarySelection`, but it acts on
    /// `item_ids`, never on grid positions, since the rejects need not be
    /// selected or even loaded.
    Rejected,
}

#[derive(Debug, Clone)]
pub struct DeleteConfirmState {
    pub intent: DeleteIntent,
    pub target: DeletionTarget,
    pub item_ids: Vec<MediaId>,
    pub item_count: usize,
    pub permanent_acknowledged: bool,
    /// Header summary: "2 photos, 1 video · 41.2 MB" and up to three thumbs.
    pub photos: usize,
    pub videos: usize,
    pub total_bytes: u64,
    pub preview_ids: Vec<MediaId>,
}

#[allow(clippy::large_enum_variant)]
pub enum UiMessage {
    Engine(EngineEvent),
    /// An engine event tagged with the library that sent it; events from a
    /// library that has since been replaced are dropped.
    LibraryEvent {
        generation: u64,
        event: EngineEvent,
    },
    ThumbnailLoaded {
        id: MediaId,
        frame: DecodedFrame,
    },
    /// A thumbnail decode failed for good (already recorded in the grid's
    /// failure set); the handler only repaints.
    ThumbnailFailed,
    ViewerLoaded {
        id: MediaId,
        frame: DecodedFrame,
    },
    /// The viewer's full-size decode of `id` failed.
    ViewerFailed {
        id: MediaId,
    },
    // Only the real folder dialog sends this; tests count the request instead.
    #[cfg_attr(test, allow(dead_code))]
    FolderPicked(PathBuf),
    // Only the real folder dialog sends this outside tests.
    #[cfg_attr(test, allow(dead_code))]
    ExportFolderPicked(PathBuf, lumina_core::ExportPreset),
    /// The folder chosen for "Import into New Library…": open it, then ask
    /// for the photos to copy in once its first scan is done.
    NewLibraryPicked(PathBuf),
    /// The file chosen to reconnect a missing item. (Tests skip the picker.)
    #[cfg_attr(test, allow(dead_code))]
    LocatePicked(lumina_core::MediaId, PathBuf),
    /// Folders chosen in the Import… picker. (Tests skip the picker.)
    #[cfg_attr(test, allow(dead_code))]
    ImportPicked(Vec<PathBuf>),
    VideoFrameTick,
    /// A large selection's summary, read off the UI thread.
    BatchSummaryLoaded(Box<crate::app::batch_summary::LoadedSummary>),
    #[cfg_attr(test, allow(dead_code))]
    SelectedTagsLoaded {
        request: u64,
        single_tags: Vec<String>,
        counts: Vec<(String, usize)>,
    },
    #[cfg_attr(test, allow(dead_code))]
    DuplicatesLoaded {
        generation: u64,
        groups: Vec<lumina_core::DuplicateGroup>,
        items_count: usize,
    },
    OpenFloatingInspector,
    OpenFloatingPip,
    FloatingInspectorClosed,
    /// A PiP window went away; carries its id so a late message from an old
    /// window cannot end a newer session.
    FloatingPipClosed(gpui::WindowId),
}

#[derive(Debug, Clone, Default)]
pub struct BatchSelectionSummary {
    pub count: usize,
    pub total_bytes: u64,
    pub photos_count: usize,
    pub videos_count: usize,
    pub raw_count: usize,
    /// Selection ids in grid order; the batch inspector reads ratings and
    /// the thumbnail stack from these instead of re-querying each frame.
    pub ids: Vec<MediaId>,
    /// Capture-time range of the selection.
    pub oldest_ms: i64,
    pub newest_ms: i64,
    /// A large selection's summary is loading off the UI thread: only
    /// `count` is known yet.
    pub pending: bool,
}

/// Test helper: a solid vertical gradient image.
#[cfg(test)]
pub fn generate_gradient_image(
    top: (u8, u8, u8),
    bottom: (u8, u8, u8),
    width: u32,
    height: u32,
) -> std::sync::Arc<gpui::RenderImage> {
    // Compute in usize so large (but still valid) dimensions cannot overflow.
    let expected_len = (width as usize) * (height as usize) * 4;
    let mut data = Vec::with_capacity(expected_len);
    for y in 0..height {
        let t = if height > 1 {
            y as f32 / (height - 1) as f32
        } else {
            0.0
        };
        let r = (top.0 as f32 * (1.0 - t) + bottom.0 as f32 * t).round() as u8;
        let g = (top.1 as f32 * (1.0 - t) + bottom.1 as f32 * t).round() as u8;
        let b = (top.2 as f32 * (1.0 - t) + bottom.2 as f32 * t).round() as u8;
        for _ in 0..width {
            data.push(b);
            data.push(g);
            data.push(r);
            data.push(255);
        }
    }
    // Never panic in the render path: a malformed buffer falls back to a blank
    // image instead of unwinding through GPUI.
    let buffer = image::RgbaImage::from_raw(width, height, data)
        .unwrap_or_else(|| image::RgbaImage::new(width.max(1), height.max(1)));
    std::sync::Arc::new(gpui::RenderImage::new(smallvec::smallvec![
        image::Frame::new(buffer)
    ]))
}

/// Duration text for the video badge; the play icon is rendered separately.
/// Returns an empty string when the real duration is unknown rather than
/// fabricating a value from the file name.
pub fn format_duration_badge(secs: f64) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return String::new();
    }
    let m = (secs as u64) / 60;
    let s = (secs as u64) % 60;
    format!("{}:{:02}", m, s)
}

/// "1:18", or "2:02:02" from an hour up: the transport's clock readout.
pub fn format_clock(secs: f64) -> String {
    let total = if secs.is_finite() && secs > 0.0 {
        secs as u64
    } else {
        0
    };
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn format_time_fractional(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00.00".to_string();
    }
    let hundredths_total = (seconds * 100.0).round() as u64;
    let total_secs = hundredths_total / 100;
    let hundredths = hundredths_total % 100;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}.{:02}", hours, mins, secs, hundredths)
    } else {
        format!("{}:{:02}.{:02}", mins, secs, hundredths)
    }
}

pub fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Month",
    }
}

/// Deterministic demo items for unit tests only; never part of the production
/// render path.
#[cfg(test)]
pub fn get_sample_items() -> Vec<MediaItem> {
    let names = [
        ("IMG_2036.HEIC", 2022, 1, 15, false, false, None),
        ("IMG_2037.MOV", 2022, 1, 15, true, true, Some(15000)),
        ("IMG_2038.HEIC", 2022, 1, 15, false, false, None),
        ("IMG_2039.MOV", 2022, 1, 15, false, true, Some(31000)),
        ("IMG_2040.HEIC", 2022, 1, 15, false, false, None),
        ("IMG_2041.HEIC", 2022, 1, 15, false, false, None),
        ("IMG_2042.HEIC", 2022, 6, 11, false, false, None),
        ("IMG_2043.HEIC", 2022, 6, 11, false, false, None),
        ("IMG_2044.MOV", 2022, 6, 12, true, true, None),
        ("IMG_2045.HEIC", 2022, 6, 13, false, false, None),
        ("IMG_2046.HEIC", 2022, 6, 14, false, false, None),
        ("IMG_2047.MOV", 2022, 6, 14, false, true, Some(68000)),
    ];

    names
        .into_iter()
        .enumerate()
        .map(|(idx, (name, y, m, d, fav, vid, dur))| {
            let is_2044 = name == "IMG_2044.MOV";
            MediaItem {
                id: MediaId(idx as u64 + 1),
                path: PathBuf::from(format!("/sample/{}", name)),
                parent_dir: "/sample".to_string(),
                file_name: name.to_string(),
                title: None,
                file_size: if is_2044 { 18_400_000 } else { 4_500_000 },
                mtime_epoch_ms: 1642200000000,
                media_type: if vid {
                    MediaType::Video
                } else {
                    MediaType::Image
                },
                width: if is_2044 { 1920 } else { 4032 },
                height: if is_2044 { 1080 } else { 3024 },
                duration_ms: if is_2044 { Some(42000) } else { dur },
                timestamp_epoch_ms: 1642200000000,
                year: y,
                month: m,
                day: d,
                latitude: Some(38.7223),
                longitude: Some(-9.1393),
                altitude: None,
                country_code: Some("PRT".to_string()),
                admin1_region: Some("Lisbon".to_string()),
                city: Some("Lisbon".to_string()),
                make: Some("Apple".to_string()),
                model: Some("iPhone 13 Pro".to_string()),
                lens: None,
                focal_length: Some(26.0),
                f_number: Some(1.5),
                iso: Some(64),
                exposure_time: Some("1/120".to_string()),
                is_favorite: fav,
                is_hidden: false,
                is_trashed: false,
            }
        })
        .collect()
}
