use anyhow::Result;
use chrono::{DateTime, Datelike};
use crossbeam_channel::Sender;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

use crate::geocoder::OfflineGeocoder;
use crate::{EngineEvent, MediaId, MediaItem, MediaType};

const BATCH_SIZE: usize = 256;
/// Files between progress events inside a batch.
const PROGRESS_EVERY: usize = 32;

pub struct Scanner {
    library_root: PathBuf,
    event_tx: Sender<EngineEvent>,
    cancel_flag: Arc<AtomicBool>,
    geocoder: OfflineGeocoder,
    known_signatures: HashMap<PathBuf, (u64, i64)>,
}

impl Scanner {
    pub fn new(library_root: PathBuf, event_tx: Sender<EngineEvent>) -> Self {
        Self::new_with_cancel(library_root, event_tx, Arc::new(AtomicBool::new(false)))
    }

    pub fn new_with_cancel(
        library_root: PathBuf,
        event_tx: Sender<EngineEvent>,
        cancel_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            library_root,
            event_tx,
            cancel_flag,
            geocoder: OfflineGeocoder::new(),
            known_signatures: HashMap::new(),
        }
    }

    pub fn with_known_signatures(mut self, known_signatures: HashMap<PathBuf, (u64, i64)>) -> Self {
        self.known_signatures = known_signatures;
        self
    }

    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        self.cancel_flag.clone()
    }

    pub fn scan(&self) -> Result<usize> {
        self.scan_files()
    }

    /// Every non-hidden entry under the root, as both passes walk it.
    fn walk(&self) -> impl Iterator<Item = walkdir::Result<walkdir::DirEntry>> {
        walk_entries(&self.library_root)
    }

    /// Counts media files by name only, for "X of Y" progress.
    // ponytail: a second directory walk (no file reads); drop it if a huge
    // library shows the pre-count in profiles.
    fn count_media_files(&self) -> usize {
        self.walk()
            .filter_map(|e| e.ok())
            .take_while(|_| !self.cancel_flag.load(Ordering::Relaxed))
            .filter(|e| {
                e.file_type().is_file() && Self::classify_media_extension(e.path()).is_some()
            })
            .count()
    }

    pub fn scan_files(&self) -> Result<usize> {
        let mut total_found: usize = 0;
        let mut skipped: usize = 0;
        let mut batch: Vec<MediaItem> = Vec::with_capacity(BATCH_SIZE);
        // Only a first scan (no catalog yet) shows "X of Y"; rescans and
        // launches skip the extra walk.
        let total = if self.known_signatures.is_empty() {
            self.count_media_files()
        } else {
            0
        };
        let progress = |items_found, skipped, is_complete, current_file| {
            let _ = self.event_tx.send(EngineEvent::ScanProgress {
                items_found,
                is_complete,
                total,
                skipped,
                current_file,
            });
        };
        progress(0, 0, false, None);

        for entry in self.walk() {
            if self.cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let file_type = entry.file_type();
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();
            if Self::classify_media_extension(path).is_none() {
                continue;
            }

            // Fast incremental check: if file size and mtime match catalog signature, skip deep parse
            if !self.known_signatures.is_empty() {
                if let Some(signature) = file_signature(path) {
                    if self.known_signatures.get(path) == Some(&signature) {
                        total_found += 1;
                        continue;
                    }
                }
            }

            if let Some(media_item) = self.inspect_media_file(path) {
                batch.push(media_item);
                total_found += 1;

                if batch.len() >= BATCH_SIZE {
                    let items_to_send =
                        std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                    let _ = self.event_tx.send(EngineEvent::BatchDiscovered {
                        items: items_to_send,
                    });
                    progress(total_found, skipped, false, Some(path.to_path_buf()));
                }
            } else {
                skipped += 1;
            }
            // A heartbeat between batches keeps the current file moving.
            // (An empty batch was just flushed with its own progress event.)
            if (total_found + skipped).is_multiple_of(PROGRESS_EVERY) && !batch.is_empty() {
                progress(total_found, skipped, false, Some(path.to_path_buf()));
            }
        }

        if !batch.is_empty() {
            let last = batch.last().map(|item| item.path.clone());
            let _ = self
                .event_tx
                .send(EngineEvent::BatchDiscovered { items: batch });
            progress(total_found, skipped, false, last);
        }

        progress(total_found, skipped, true, None);
        let _ = self.event_tx.send(EngineEvent::ScanComplete {
            total_items: total_found,
        });

        Ok(total_found)
    }

    pub(crate) fn classify_media_extension(path: &Path) -> Option<MediaType> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "jpg" | "jpeg" | "png" | "webp" | "heic" | "heif" | "tiff" | "tif" => {
                Some(MediaType::Image)
            }
            "arw" | "cr2" | "cr3" | "nef" | "dng" | "raf" | "rw2" | "orf" => Some(MediaType::Raw),
            "mp4" | "mov" | "m4v" | "avi" | "mkv" => Some(MediaType::Video),
            _ => None,
        }
    }

    pub fn inspect_media_file(&self, path: &Path) -> Option<MediaItem> {
        let media_type = Self::classify_media_extension(path)?;

        let metadata = std::fs::metadata(path).ok()?;
        let file_size = metadata.len();
        let mtime_epoch_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let parent_dir = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Extract dimensions quickly from image header if possible
        let header = image::image_dimensions(path).ok();
        let (mut width, mut height) = header.unwrap_or((0, 0));
        let mut exif_size = crate::dims::ExifSize::default();

        // Deep EXIF metadata extraction
        let mut timestamp_epoch_ms = mtime_epoch_ms;
        let mut latitude = None;
        let mut longitude = None;
        let mut altitude = None;
        let mut make = None;
        let mut model = None;
        let mut lens = None;
        let mut focal_length = None;
        let mut f_number = None;
        let mut iso = None;
        let mut exposure_time = None;

        if media_type == MediaType::Image || media_type == MediaType::Raw {
            if let Ok(file) = File::open(path) {
                let mut bufreader = BufReader::new(file);
                let exifreader = exif::Reader::new();
                if let Ok(exif_data) = exifreader.read_from_container(&mut bufreader) {
                    // Timestamp
                    if let Some(field) = exif_data
                        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                        .or_else(|| exif_data.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
                    {
                        let val_str = field.display_value().to_string();
                        if let Some(ts) = chrono_parse_timestamp(&val_str) {
                            timestamp_epoch_ms = ts;
                        }
                    }

                    // Camera & Hardware
                    if let Some(f) = exif_data.get_field(exif::Tag::Make, exif::In::PRIMARY) {
                        make = Some(clean_string(&f.display_value().to_string()));
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::Model, exif::In::PRIMARY) {
                        model = Some(clean_string(&f.display_value().to_string()));
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::LensModel, exif::In::PRIMARY) {
                        lens = Some(clean_string(&f.display_value().to_string()));
                    }

                    // Optics
                    if let Some(f) = exif_data.get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
                    {
                        focal_length = parse_rational(&f.value);
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
                        f_number = parse_rational(&f.value);
                    }
                    if let Some(f) =
                        exif_data.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
                    {
                        iso = parse_integer(&f.value);
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
                    {
                        exposure_time = Some(clean_string(&f.display_value().to_string()));
                    }

                    // Dimensions fallback and orientation from EXIF
                    exif_size = crate::dims::ExifSize::read(&exif_data);

                    // GPS Coordinates
                    let lat_ref = exif_data
                        .get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)
                        .map(|f| f.display_value().to_string());
                    let lat_val = exif_data
                        .get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
                        .and_then(|f| parse_dms(&f.value));
                    let lon_ref = exif_data
                        .get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)
                        .map(|f| f.display_value().to_string());
                    let lon_val = exif_data
                        .get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
                        .and_then(|f| parse_dms(&f.value));
                    let alt_val = exif_data
                        .get_field(exif::Tag::GPSAltitude, exif::In::PRIMARY)
                        .and_then(|f| parse_rational(&f.value));

                    if let Some(l) = lat_val {
                        latitude = Some(if lat_ref.map(|r| r.contains('S')).unwrap_or(false) {
                            -l
                        } else {
                            l
                        });
                    }
                    if let Some(l) = lon_val {
                        longitude = Some(if lon_ref.map(|r| r.contains('W')).unwrap_or(false) {
                            -l
                        } else {
                            l
                        });
                    }
                    altitude = alt_val;
                }
            }
            (width, height) = crate::dims::displayed_size(path, header, exif_size);
        }

        // Deconstruct date components
        let (year, month, day) = epoch_to_date_components(timestamp_epoch_ms);

        // Offline Reverse Geocoding (< 4 microseconds)
        let mut city = None;
        let mut admin1_region = None;
        let mut country_code = None;

        if let (Some(lat), Some(lon)) = (latitude, longitude) {
            if let Some(loc) = self.geocoder.reverse_geocode(lat, lon) {
                city = Some(loc.city);
                admin1_region = Some(loc.admin1_region);
                country_code = Some(loc.country_code);
            }
        }

        let duration_ms = if media_type == MediaType::Video {
            Some(extract_video_duration(path).map(|(ms, _)| ms).unwrap_or(0))
        } else {
            None
        };
        if media_type == MediaType::Video && (width == 0 || height == 0) {
            (width, height) = extract_video_size(path).unwrap_or((0, 0));
        }

        Some(MediaItem {
            id: MediaId(0), // Assigned upon DB insert
            path: path.to_path_buf(),
            parent_dir,
            file_name,
            title: None,
            file_size,
            mtime_epoch_ms,
            media_type,
            width,
            height,
            duration_ms,
            timestamp_epoch_ms,
            year,
            month,
            day,
            latitude,
            longitude,
            altitude,
            country_code,
            admin1_region,
            city,
            make,
            model,
            lens,
            focal_length,
            f_number,
            iso,
            exposure_time,
            is_favorite: false,
            is_hidden: false,
            is_trashed: false,
        })
    }
}

/// Get Info facts not kept in the catalog: read from one file on demand, so
/// the scan and every catalog query stay as lean as before.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaDetails {
    /// File-system creation (birth) time, where the platform records it.
    pub created_epoch_ms: Option<i64>,
    pub color_space: Option<String>,
    pub exposure_bias_ev: Option<f64>,
    pub exposure_program: Option<String>,
    pub metering_mode: Option<String>,
    pub flash: Option<String>,
    pub white_balance: Option<String>,
    pub software: Option<String>,
    pub focal_length_35mm: Option<u32>,
}

/// Every non-hidden entry under `dir`: hidden files and folders (and all
/// inside them) are skipped, and symlinks are not followed.
fn walk_entries(dir: &Path) -> impl Iterator<Item = walkdir::Result<walkdir::DirEntry>> {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
}

/// Files under `dir`, skipping hidden names and not following symlinks.
pub fn walk_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    walk_entries(dir)
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
}

/// Media files under `dir`, by the scanner's rules.
pub fn walk_media(dir: &Path) -> impl Iterator<Item = PathBuf> {
    walk_files(dir).filter(|path| is_supported_media(path))
}

/// `(file_size, mtime_epoch_ms)` as the catalog stores them.
pub(crate) fn file_signature(path: &Path) -> Option<(u64, i64)> {
    let metadata = std::fs::metadata(path).ok()?;
    let mtime_epoch_ms = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Some((metadata.len(), mtime_epoch_ms))
}

/// True for a file name Lumina catalogues (by extension, any case).
pub fn is_supported_media(path: &Path) -> bool {
    Scanner::classify_media_extension(path).is_some()
}

/// Reads [`MediaDetails`] for `path`. Missing or unreadable fields are `None`;
/// this never fails, since Get Info just shows fewer rows. Does file IO: call
/// it off the UI thread.
pub fn read_media_details(path: &Path) -> MediaDetails {
    let mut details = MediaDetails {
        created_epoch_ms: std::fs::metadata(path)
            .and_then(|m| m.created())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64),
        ..Default::default()
    };
    let Ok(file) = File::open(path) else {
        return details;
    };
    let Ok(exif_data) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) else {
        return details;
    };
    let text = |tag: exif::Tag| {
        exif_data
            .get_field(tag, exif::In::PRIMARY)
            .map(|f| sentence_case(&clean_string(&f.display_value().to_string())))
            .filter(|s| !s.is_empty())
    };
    details.color_space = text(exif::Tag::ColorSpace);
    details.exposure_program = text(exif::Tag::ExposureProgram);
    details.metering_mode = text(exif::Tag::MeteringMode);
    details.flash = text(exif::Tag::Flash);
    details.white_balance = text(exif::Tag::WhiteBalance);
    details.software = exif_data
        .get_field(exif::Tag::Software, exif::In::PRIMARY)
        .map(|f| clean_string(&f.display_value().to_string()))
        .filter(|s| !s.is_empty());
    details.exposure_bias_ev = exif_data
        .get_field(exif::Tag::ExposureBiasValue, exif::In::PRIMARY)
        .and_then(|f| match f.value {
            exif::Value::SRational(ref v) => v.first().map(|r| r.to_f64()),
            _ => parse_rational(&f.value),
        });
    details.focal_length_35mm = exif_data
        .get_field(exif::Tag::FocalLengthIn35mmFilm, exif::In::PRIMARY)
        .and_then(|f| parse_integer(&f.value))
        .filter(|&mm| mm > 0);
    details
}

/// "normal program" -> "Normal program" for display.
fn sentence_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn clean_string(s: &str) -> String {
    s.trim().trim_matches('"').trim().to_string()
}

fn parse_rational(value: &exif::Value) -> Option<f64> {
    if let exif::Value::Rational(ref rationals) = value {
        if let Some(r) = rationals.first() {
            return Some(r.to_f64());
        }
    }
    None
}

fn parse_integer(value: &exif::Value) -> Option<u32> {
    match value {
        exif::Value::Short(ref v) => v.first().map(|&x| x as u32),
        exif::Value::Long(ref v) => v.first().copied(),
        _ => None,
    }
}

fn parse_dms(value: &exif::Value) -> Option<f64> {
    if let exif::Value::Rational(ref rationals) = value {
        if rationals.len() >= 3 {
            let deg = rationals[0].to_f64();
            let min = rationals[1].to_f64();
            let sec = rationals[2].to_f64();
            return Some(deg + min / 60.0 + sec / 3600.0);
        }
    }
    None
}

fn epoch_to_date_components(epoch_ms: i64) -> (i32, u32, u32) {
    if let Some(dt) = DateTime::from_timestamp_millis(epoch_ms) {
        (dt.year(), dt.month(), dt.day())
    } else {
        (1970, 1, 1)
    }
}

fn chrono_parse_timestamp(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let date_parts: Vec<i32> = parts[0]
        .split(['-', ':'])
        .filter_map(|p| p.parse().ok())
        .collect();
    let time_parts: Vec<u32> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();

    if date_parts.len() == 3 && time_parts.len() >= 2 {
        let year = date_parts[0];
        let month = u32::try_from(date_parts[1]).ok()?;
        let day = u32::try_from(date_parts[2]).ok()?;
        let hour = time_parts[0];
        let min = time_parts[1];
        let sec = if time_parts.len() > 2 {
            time_parts[2]
        } else {
            0
        };

        let date = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
        let time = chrono::NaiveTime::from_hms_opt(hour, min, sec)?;
        let dt = chrono::NaiveDateTime::new(date, time);
        return Some(dt.and_utc().timestamp_millis());
    }
    None
}

fn parse_mvhd_duration(data: &[u8]) -> Option<(u64, f64)> {
    let pos = data.windows(4).position(|w| w == b"mvhd")?;
    let v_pos = pos + 4;
    if v_pos >= data.len() {
        return None;
    }
    let version = data[v_pos];
    if version == 0 && pos + 24 <= data.len() {
        let ts = u32::from_be_bytes([
            data[pos + 16],
            data[pos + 17],
            data[pos + 18],
            data[pos + 19],
        ]) as u64;
        let dur = u32::from_be_bytes([
            data[pos + 20],
            data[pos + 21],
            data[pos + 22],
            data[pos + 23],
        ]) as u64;
        if ts > 0 {
            let secs = (dur as f64) / (ts as f64);
            let ms = ((dur as u128 * 1000) / ts as u128).min(u64::MAX as u128) as u64;
            return Some((ms, secs));
        }
    } else if version == 1 && pos + 36 <= data.len() {
        let ts = u32::from_be_bytes([
            data[pos + 24],
            data[pos + 25],
            data[pos + 26],
            data[pos + 27],
        ]) as u64;
        let dur = u64::from_be_bytes([
            data[pos + 28],
            data[pos + 29],
            data[pos + 30],
            data[pos + 31],
            data[pos + 32],
            data[pos + 33],
            data[pos + 34],
            data[pos + 35],
        ]);
        if ts > 0 {
            let secs = (dur as f64) / (ts as f64);
            let ms = ((dur as u128 * 1000) / ts as u128).min(u64::MAX as u128) as u64;
            return Some((ms, secs));
        }
    }
    None
}

/// Displayed size of the first visual track, from its `tkhd` box. Audio
/// tracks have a zero size and are skipped. A 90°/270° matrix (portrait phone
/// video stored landscape) swaps width and height.
fn parse_tkhd_size(data: &[u8]) -> Option<(u32, u32)> {
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };
    let mut from = 0;
    while let Some(found) = data[from..].windows(4).position(|w| w == b"tkhd") {
        let pos = from + found;
        from = pos + 4;
        // A real box header carries the fixed tkhd size; random video bytes
        // that happen to spell "tkhd" do not.
        if !matches!(pos.checked_sub(4).and_then(be32), Some(92 | 104)) {
            continue;
        }
        // version + flags, then times/ids (20 bytes in v0, 32 in v1), then
        // 16 reserved/layer/volume bytes and the 36-byte matrix.
        let matrix = pos
            + 4
            + 4
            + if data.get(pos + 4) == Some(&1) {
                32
            } else {
                20
            }
            + 16;
        let (a, b) = (be32(matrix)? as i32, be32(matrix + 4)? as i32);
        let (w, h) = (be32(matrix + 36)? >> 16, be32(matrix + 40)? >> 16);
        if w > 0 && h > 0 {
            return Some(if a == 0 && b != 0 { (h, w) } else { (w, h) });
        }
    }
    None
}

/// Runs `parse` over the head of a movie file, then over its tail, where
/// non-faststart MP4/MOV files keep the `moov` atom.
fn scan_movie_header<T>(path: &Path, parse: impl Fn(&[u8]) -> Option<T>) -> Option<T> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; 128 * 1024]; // 128 KiB head check
    if let Ok(n) = file.read(&mut head) {
        if let Some(res) = parse(&head[..n]) {
            return Some(res);
        }
    }
    let file_len = file.metadata().ok()?.len();
    if file_len > 128 * 1024 {
        file.seek(SeekFrom::Start(file_len.saturating_sub(256 * 1024)))
            .ok()?;
        let mut tail = Vec::with_capacity(256 * 1024);
        file.read_to_end(&mut tail).ok()?;
        return parse(&tail);
    }
    None
}

/// Displayed `(width, height)` of an MP4/MOV video, read from its headers.
pub fn extract_video_size(path: &Path) -> Option<(u32, u32)> {
    scan_movie_header(path, parse_tkhd_size)
}

pub fn extract_video_duration(path: &Path) -> Option<(u64, f64)> {
    if let Some(res) = scan_movie_header(path, parse_mvhd_duration) {
        return Some(res);
    }

    if let Ok(out) = std::process::Command::new("mdls")
        .arg("-name")
        .arg("kMDItemDurationSeconds")
        .arg(path)
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(val_str) = s.split('=').nth(1) {
                if let Ok(secs) = val_str.trim().parse::<f64>() {
                    if secs > 0.0 {
                        let ms = (secs * 1000.0) as u64;
                        return Some((ms, secs));
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mvhd_duration_v1_large_duration_does_not_overflow() {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"mvhd");
        data[4] = 1; // version 1
        let ts: u32 = 1000;
        data[24..28].copy_from_slice(&ts.to_be_bytes());
        let dur: u64 = u64::MAX / 500;
        data[28..36].copy_from_slice(&dur.to_be_bytes());

        let res = parse_mvhd_duration(&data);
        assert!(res.is_some());
        let (ms, _secs) = res.unwrap();
        assert_eq!(ms, dur);
    }
}
