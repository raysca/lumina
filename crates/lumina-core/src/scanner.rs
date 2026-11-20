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

mod details;
mod mp4;
use details::{clean_string, parse_dms, parse_integer, parse_rational};
pub use details::{read_media_details, MediaDetails};
pub use mp4::{extract_video_duration, extract_video_size};

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
            geocoder: OfflineGeocoder::shared(),
            known_signatures: HashMap::new(),
        }
    }

    pub fn with_known_signatures(mut self, known_signatures: HashMap<PathBuf, (u64, i64)>) -> Self {
        self.known_signatures = known_signatures;
        self
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
                    // Cameras with no fix write an empty GPS block as 0,0:
                    // a pin in the Gulf of Guinea, not a place.
                    if latitude == Some(0.0) && longitude == Some(0.0) {
                        (latitude, longitude, altitude) = (None, None, None);
                    }
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
/// `(year, month, day)` of a capture time, or `(0, 0, 0)` for an undated
/// one (capture time 0, as in `view_scope`), so the month band and the
/// timeline buckets file it under "Undated", not January 1970.
pub(crate) fn epoch_to_date_components(epoch_ms: i64) -> (i32, u32, u32) {
    match DateTime::from_timestamp_millis(epoch_ms) {
        Some(dt) if epoch_ms != 0 => (dt.year(), dt.month(), dt.day()),
        _ => (0, 0, 0),
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
