use gpui::RenderImage;
use lumina_core::{DecodedFrame, DeletionTarget, EngineEvent, MediaId, MediaItem, MediaType};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagSortOrder {
    Alphabetical,
    CountDesc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteIntent {
    EmptyTrash,
    DeleteSelected,
}

#[derive(Debug, Clone)]
pub struct DeleteConfirmState {
    pub intent: DeleteIntent,
    pub target: DeletionTarget,
    pub item_ids: Vec<MediaId>,
    pub item_count: usize,
    pub permanent_acknowledged: bool,
}

#[allow(clippy::large_enum_variant)]
pub enum UiMessage {
    Engine(EngineEvent),
    ThumbnailLoaded { id: MediaId, frame: DecodedFrame },
    ViewerLoaded { id: MediaId, frame: DecodedFrame },
    FolderPicked(PathBuf),
    VideoFrameTick,
    OpenFloatingInspector,
    OpenFloatingPip,
    FloatingInspectorClosed,
    FloatingPipClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    #[default]
    Browser,
    Split,
    Viewer,
}

#[derive(Debug, Clone, Default)]
pub struct BatchSelectionSummary {
    pub count: usize,
    pub total_bytes: u64,
    pub photos_count: usize,
    pub videos_count: usize,
    pub raw_count: usize,
    pub fav_count: usize,
}

pub type RgbColor = (u8, u8, u8);
pub type GradientSpec = (RgbColor, RgbColor, u32, u32);

pub fn get_sample_gradient(name: &str) -> GradientSpec {
    if name.contains("2036") {
        ((48, 22, 50), (175, 48, 102), 120, 80)
    } else if name.contains("2037") {
        ((145, 128, 80), (75, 62, 38), 65, 110)
    } else if name.contains("2038") {
        ((155, 148, 136), (72, 68, 60), 65, 110)
    } else if name.contains("2039") {
        ((55, 32, 46), (198, 105, 112), 120, 75)
    } else if name.contains("2040") {
        ((35, 45, 52), (88, 112, 125), 110, 88)
    } else if name.contains("2041") {
        ((165, 122, 65), (105, 70, 30), 95, 110)
    } else if name.contains("2042") {
        ((138, 162, 180), (192, 180, 158), 120, 80)
    } else if name.contains("2043") {
        ((132, 158, 175), (188, 178, 160), 65, 110)
    } else if name.contains("2044") {
        ((82, 148, 185), (175, 205, 220), 88, 105)
    } else if name.contains("2046") {
        ((165, 115, 82), (85, 45, 28), 65, 110)
    } else if name.contains("2047") {
        ((58, 92, 42), (112, 138, 68), 120, 85)
    } else {
        ((60, 60, 65), (35, 35, 40), 100, 100)
    }
}

pub fn generate_gradient_image(
    top: (u8, u8, u8),
    bottom: (u8, u8, u8),
    width: u32,
    height: u32,
) -> Arc<RenderImage> {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
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
    let buffer = image::RgbaImage::from_raw(width, height, data).unwrap();
    Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
        buffer
    )]))
}

pub fn get_sample_bg_color(name: &str) -> u32 {
    if name.contains("2036") {
        0x3b1c31
    } else if name.contains("2037") {
        0x5c4d32
    } else if name.contains("2038") {
        0x555047
    } else if name.contains("2039") {
        0x5a3948
    } else if name.contains("2040") {
        0x2e3840
    } else if name.contains("2041") {
        0x6a4e28
    } else if name.contains("2042") {
        0x5c6d78
    } else if name.contains("2043") {
        0x6b7479
    } else if name.contains("2044") {
        0x456f87
    } else if name.contains("2045") {
        0x2b3846
    } else if name.contains("2046") {
        0x63422e
    } else if name.contains("2047") {
        0x384c2a
    } else {
        0x2c2e32
    }
}

/// Duration text for the video badge; the play icon is rendered separately.
pub fn format_duration_badge(secs: f64, name: &str) -> String {
    if secs > 0.0 {
        let m = (secs as u64) / 60;
        let s = (secs as u64) % 60;
        format!("{}:{:02}", m, s)
    } else if name.contains("2037") {
        "0:15".to_string()
    } else if name.contains("2039") {
        "0:31".to_string()
    } else if name.contains("2047") {
        "1:08".to_string()
    } else {
        "0:30".to_string()
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
