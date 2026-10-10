use anyhow::Result;
use crossbeam_channel::Sender;
use jwalk::WalkDirGeneric;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::{EngineEvent, MediaId, MediaItem, MediaType};

const BATCH_SIZE: usize = 256;

pub struct Scanner {
    library_root: PathBuf,
    event_tx: Sender<EngineEvent>,
    cancel_flag: Arc<AtomicBool>,
}

impl Scanner {
    pub fn new(library_root: PathBuf, event_tx: Sender<EngineEvent>) -> Self {
        Self {
            library_root,
            event_tx,
            cancel_flag: Arc::new(AtomicBool::new(false)),
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
            if let Some(media_item) = Self::inspect_media_file(&path) {
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

    pub fn inspect_media_file(path: &Path) -> Option<MediaItem> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let (media_type, is_media) = match ext.as_str() {
            "jpg" | "jpeg" | "png" | "webp" | "heic" => (MediaType::Image, true),
            "mp4" | "mov" | "m4v" => (MediaType::Video, true),
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

        // Try extracting EXIF capture date if image
        let timestamp_epoch_ms = if media_type == MediaType::Image {
            Self::read_exif_timestamp(path).unwrap_or(mtime_epoch_ms)
        } else {
            mtime_epoch_ms
        };

        Some(MediaItem {
            id: MediaId(0), // Assigned upon DB insert
            path: path.to_path_buf(),
            media_type,
            width: 0,
            height: 0,
            duration_secs: 0.0,
            file_size,
            mtime_epoch_ms,
            timestamp_epoch_ms,
        })
    }

    fn read_exif_timestamp(path: &Path) -> Option<i64> {
        let file = File::open(path).ok()?;
        let mut bufreader = BufReader::new(file);
        let exifreader = exif::Reader::new();
        let exif_data = exifreader.read_from_container(&mut bufreader).ok()?;

        let field = exif_data
            .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
            .or_else(|| {
                exif_data.get_field(exif::Tag::DateTime, exif::In::PRIMARY)
            })?;

        let val_str = field.display_value().to_string();
        // Format: "YYYY-MM-DD HH:MM:SS"
        chrono_parse_timestamp(&val_str)
    }
}

fn chrono_parse_timestamp(s: &str) -> Option<i64> {
    // Basic parser for "YYYY-MM-DD HH:MM:SS" or "YYYY:MM:DD HH:MM:SS"
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let date_parts: Vec<i64> = parts[0]
        .split(|c| c == '-' || c == ':')
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

        // Approximate days calculation for sorting
        let days = (year - 1970) * 365 + (month - 1) * 30 + day;
        let secs = days * 86400 + hour * 3600 + min * 60 + sec;
        return Some(secs * 1000);
    }
    None
}
