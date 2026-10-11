use anyhow::Result;
use crossbeam_channel::Sender;
use jwalk::WalkDirGeneric;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::geocoder::OfflineGeocoder;
use crate::{EngineEvent, MediaId, MediaItem, MediaType};

const BATCH_SIZE: usize = 256;

pub struct Scanner {
    library_root: PathBuf,
    event_tx: Sender<EngineEvent>,
    cancel_flag: Arc<AtomicBool>,
    geocoder: OfflineGeocoder,
}

impl Scanner {
    pub fn new(library_root: PathBuf, event_tx: Sender<EngineEvent>) -> Self {
        Self {
            library_root,
            event_tx,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            geocoder: OfflineGeocoder::new(),
        }
    }

    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        self.cancel_flag.clone()
    }

    pub fn scan(&self) -> Result<usize> {
        let total_found = Arc::new(AtomicUsize::new(0));
        let mut batch: Vec<MediaItem> = Vec::with_capacity(BATCH_SIZE);

        let walker = WalkDirGeneric::<((), ())>::new(&self.library_root)
            .skip_hidden(true)
            .process_read_dir(|_depth, _path, _read_dir_state, children| {
                // Filter out hidden folders/files
                children.retain(|dir_entry_result| {
                    dir_entry_result
                        .as_ref()
                        .map(|entry| {
                            let name = entry.file_name.to_string_lossy();
                            !name.starts_with('.')
                        })
                        .unwrap_or(false)
                });
            });

        for entry in walker {
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
            if let Some(media_item) = self.inspect_media_file(&path) {
                batch.push(media_item);
                let current = total_found.fetch_add(1, Ordering::Relaxed) + 1;

                if batch.len() >= BATCH_SIZE {
                    let items_to_send = std::mem::replace(&mut batch, Vec::with_capacity(BATCH_SIZE));
                    let _ = self.event_tx.send(EngineEvent::BatchDiscovered {
                        items: items_to_send,
                    });
                    let _ = self.event_tx.send(EngineEvent::ScanProgress {
                        items_found: current,
                        is_complete: false,
                    });
                }
            }
        }

        if !batch.is_empty() {
            let current = total_found.load(Ordering::Relaxed);
            let _ = self.event_tx.send(EngineEvent::BatchDiscovered { items: batch });
            let _ = self.event_tx.send(EngineEvent::ScanProgress {
                items_found: current,
                is_complete: false,
            });
        }

        let total = total_found.load(Ordering::Relaxed);
        let _ = self.event_tx.send(EngineEvent::ScanProgress {
            items_found: total,
            is_complete: true,
        });
        let _ = self.event_tx.send(EngineEvent::ScanComplete { total_items: total });

        Ok(total)
    }

    pub fn inspect_media_file(&self, path: &Path) -> Option<MediaItem> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (media_type, is_media) = match ext.as_str() {
            "jpg" | "jpeg" | "png" | "webp" | "heic" | "heif" | "tiff" | "tif" => {
                (MediaType::Image, true)
            }
            "arw" | "cr2" | "cr3" | "nef" | "dng" | "raf" | "rw2" | "orf" => {
                (MediaType::Raw, true)
            }
            "mp4" | "mov" | "m4v" | "avi" | "mkv" => (MediaType::Video, true),
            _ => (MediaType::Image, false),
        };

        if !is_media {
            return None;
        }

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
        let (mut width, mut height) = image::image_dimensions(path).unwrap_or((0, 0));

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
                    if let Some(f) = exif_data.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
                        focal_length = parse_rational(&f.value);
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
                        f_number = parse_rational(&f.value);
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
                        iso = parse_integer(&f.value);
                    }
                    if let Some(f) = exif_data.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
                        exposure_time = Some(clean_string(&f.display_value().to_string()));
                    }

                    // Dimensions fallback from EXIF
                    if width == 0 || height == 0 {
                        if let Some(f) = exif_data.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY) {
                            width = parse_integer(&f.value).unwrap_or(0);
                        }
                        if let Some(f) = exif_data.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY) {
                            height = parse_integer(&f.value).unwrap_or(0);
                        }
                    }

                    // GPS Coordinates
                    let lat_ref = exif_data.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY).map(|f| f.display_value().to_string());
                    let lat_val = exif_data.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY).and_then(|f| parse_dms(&f.value));
                    let lon_ref = exif_data.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY).map(|f| f.display_value().to_string());
                    let lon_val = exif_data.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY).and_then(|f| parse_dms(&f.value));
                    let alt_val = exif_data.get_field(exif::Tag::GPSAltitude, exif::In::PRIMARY).and_then(|f| parse_rational(&f.value));

                    if let Some(l) = lat_val {
                        latitude = Some(if lat_ref.map(|r| r.contains('S')).unwrap_or(false) { -l } else { l });
                    }
                    if let Some(l) = lon_val {
                        longitude = Some(if lon_ref.map(|r| r.contains('W')).unwrap_or(false) { -l } else { l });
                    }
                    altitude = alt_val;
                }
            }
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

        let duration_ms = if media_type == MediaType::Video { Some(0) } else { None };

        Some(MediaItem {
            id: MediaId(0), // Assigned upon DB insert
            path: path.to_path_buf(),
            parent_dir,
            file_name,
            file_size,
            mtime_epoch_ms,
            media_type,
            width,
            height,
            duration_ms,
            duration_secs: 0.0,
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
    let secs = epoch_ms / 1000;
    let days = secs / 86400;

    // Approximate Gregorian calendar calculation
    let year = 1970 + (days / 365) as i32;
    let mut day_of_year = (days % 365) as u32;

    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    for &m_days in &month_days {
        if day_of_year < m_days {
            break;
        }
        day_of_year -= m_days;
        month += 1;
    }
    let day = day_of_year + 1;
    if month > 12 {
        month = 12;
    }

    (year, month, day)
}

fn chrono_parse_timestamp(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let date_parts: Vec<i64> = parts[0]
        .split(['-', ':'])
        .filter_map(|p| p.parse().ok())
        .collect();
    let time_parts: Vec<i64> = parts[1]
        .split(':')
        .filter_map(|p| p.parse().ok())
        .collect();

    if date_parts.len() == 3 && time_parts.len() >= 2 {
        let year = date_parts[0];
        let month = date_parts[1];
        let day = date_parts[2];
        let hour = time_parts[0];
        let min = time_parts[1];
        let sec = if time_parts.len() > 2 { time_parts[2] } else { 0 };

        let days = (year - 1970) * 365 + (month - 1) * 30 + day;
        let secs = days * 86400 + hour * 3600 + min * 60 + sec;
        return Some(secs * 1000);
    }
    None
}
