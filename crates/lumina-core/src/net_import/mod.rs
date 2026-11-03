//! Network video import: downloads media from Direct URLs, HLS (.m3u8), and YouTube.
//!
//! Media is staged in `<library>/Imports/YYYY/YYYY-MM-DD/` with `.lumina-import`
//! temp suffixes, verified upon complete download, catalogued, and attributed with
//! source URL and uploader in SQLite and companion `.xmp` sidecars.

pub mod direct;
pub mod helper;
pub mod hls;

use anyhow::{anyhow, Result};
use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::import::IMPORTS_DIR;
use crate::{LuminaEngine, MediaItem};

/// The classification of an incoming network media source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkSourceKind {
    /// Direct video file download over HTTP/HTTPS (.mp4, .mov, .webm, etc.).
    DirectUrl,
    /// HTTP Live Streaming playlist (.m3u8).
    HlsStream,
    /// YouTube video link (watch, shorts, youtu.be).
    YouTube,
    /// Generic web video platform supported by yt-dlp helper (e.g. Vimeo).
    GenericPlatform,
}

impl NetworkSourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DirectUrl => "Direct Video",
            Self::HlsStream => "HLS Stream",
            Self::YouTube => "YouTube",
            Self::GenericPlatform => "Web Video",
        }
    }
}

/// Classifies a raw URL string into a [`NetworkSourceKind`].
pub fn classify_url(raw_url: &str) -> Result<NetworkSourceKind> {
    let trimmed = raw_url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(anyhow!("Invalid URL: must begin with http:// or https://"));
    }

    let lower = trimmed.to_lowercase();

    // YouTube detection
    if lower.contains("youtube.com/watch")
        || lower.contains("youtu.be/")
        || lower.contains("youtube.com/shorts/")
        || lower.contains("youtube.com/live/")
    {
        return Ok(NetworkSourceKind::YouTube);
    }

    // HLS detection
    if lower.contains(".m3u8") {
        return Ok(NetworkSourceKind::HlsStream);
    }

    // Direct video extensions
    if let Some(path_part) = lower.split('?').next() {
        if path_part.ends_with(".mp4")
            || path_part.ends_with(".mov")
            || path_part.ends_with(".webm")
            || path_part.ends_with(".m4v")
            || path_part.ends_with(".mkv")
            || path_part.ends_with(".avi")
        {
            return Ok(NetworkSourceKind::DirectUrl);
        }
    }

    // Default to generic platform (which can be resolved via yt-dlp or curl)
    Ok(NetworkSourceKind::GenericPlatform)
}

/// Sanitizes a string to make it safe for use as a file name.
pub fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => {
                out.push('_');
            }
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let trimmed = out.trim().trim_matches('.');
    if trimmed.is_empty() {
        "video".to_string()
    } else if trimmed.len() > 120 {
        trimmed[..120].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

/// Information inspected about a remote media source prior to download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMediaInfo {
    pub url: String,
    pub kind: NetworkSourceKind,
    pub title: String,
    pub duration_secs: Option<f64>,
    pub uploader: Option<String>,
    pub thumbnail_url: Option<String>,
    pub expected_bytes: Option<u64>,
}

/// Current phase of a download task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadPhase {
    Connecting,
    Downloading,
    Finalizing,
    Complete,
    Failed,
    Cancelled,
}

/// Real-time progress report emitted during download execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkDownloadProgress {
    pub phase: DownloadPhase,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: u64,
    pub eta_secs: Option<u64>,
    pub message: String,
}

impl NetworkDownloadProgress {
    pub fn percent(&self) -> Option<f32> {
        let total = self.total_bytes?;
        if total == 0 {
            None
        } else {
            Some(((self.bytes_downloaded as f64 / total as f64) * 100.0) as f32)
        }
    }
}

/// Callback type for receiving progress updates.
pub type ProgressCallback = Box<dyn Fn(NetworkDownloadProgress) + Send + Sync>;

impl LuminaEngine {
    /// Computes the target import destination path for a network import item.
    /// `<library_root>/Imports/YYYY/YYYY-MM-DD/<sanitized_title>.mp4`
    pub fn destination_for_network_import(&self, title: &str, ext: &str) -> PathBuf {
        let now = Local::now();
        let year_dir = format!("{:04}", now.year());
        let date_dir = format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day());

        let target_dir = self
            .library_root
            .join(IMPORTS_DIR)
            .join(year_dir)
            .join(date_dir);

        let clean_title = sanitize_filename(title);
        let base_ext = ext.trim_start_matches('.');
        let filename = format!("{clean_title}.{base_ext}");
        let candidate = target_dir.join(&filename);

        if !candidate.exists() {
            return candidate;
        }

        // Add numerical suffix if filename exists
        for i in 1..10000 {
            let disambiguated = format!("{clean_title} - {i}.{base_ext}");
            let path = target_dir.join(disambiguated);
            if !path.exists() {
                return path;
            }
        }

        candidate
    }

    /// Ingests a downloaded file into the catalog, records source attribution,
    /// and writes a companion `.xmp` sidecar.
    pub fn ingest_network_imported_file(
        &self,
        final_path: &Path,
        source_url: &str,
        uploader: Option<&str>,
        title: Option<&str>,
        tags: &[String],
        is_favorite: bool,
    ) -> Result<MediaItem> {
        let scanner =
            crate::scanner::Scanner::new(self.library_root.clone(), self.event_tx.clone());
        let mut item = scanner.inspect_media_file(final_path).ok_or_else(|| {
            anyhow!(
                "Downloaded file at {:?} is not a supported media format",
                final_path
            )
        })?;

        if let Some(t) = title {
            if !t.trim().is_empty() {
                item.title = Some(t.trim().to_string());
            }
        }
        item.is_favorite = is_favorite;

        // Ingest into catalog
        let mut inserted_list = self.catalog.insert_or_update_batch(&[item])?;
        let inserted = inserted_list
            .pop()
            .ok_or_else(|| anyhow!("Failed to insert media item into catalog"))?;
        let media_id = inserted.id;

        // Record source attribution in SQLite
        self.catalog
            .record_media_source(media_id, source_url, uploader)?;

        // Attach tags if requested
        if !tags.is_empty() {
            self.catalog.add_tags_batch(&[media_id], tags)?;
        }

        // Keep timeline buckets fresh for zoom levels (AGENTS.md 5.3)
        self.catalog.compute_timeline_buckets()?;

        // Write companion .xmp sidecar
        let sidecar = crate::xmp::SidecarMetadata {
            title: inserted.title.clone(),
            caption: None,
            rating: 0,
            color_label: 0,
            tags: tags.to_vec(),
            source_url: Some(source_url.to_string()),
            credit: uploader.map(|s| s.to_string()),
        };
        if let Err(e) = crate::xmp::write_sidecar(final_path, &sidecar) {
            log::warn!(
                "Failed to write companion XMP sidecar for {:?}: {e:#}",
                final_path
            );
        }

        Ok(inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_url() {
        assert_eq!(
            classify_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap(),
            NetworkSourceKind::YouTube
        );
        assert_eq!(
            classify_url("https://youtu.be/dQw4w9WgXcQ").unwrap(),
            NetworkSourceKind::YouTube
        );
        assert_eq!(
            classify_url("https://youtube.com/shorts/abcdef123").unwrap(),
            NetworkSourceKind::YouTube
        );
        assert_eq!(
            classify_url("https://example.com/live/playlist.m3u8?token=xyz").unwrap(),
            NetworkSourceKind::HlsStream
        );
        assert_eq!(
            classify_url("https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4").unwrap(),
            NetworkSourceKind::DirectUrl
        );
        assert_eq!(
            classify_url("https://vimeo.com/12345678").unwrap(),
            NetworkSourceKind::GenericPlatform
        );
        assert!(classify_url("not-a-url").is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("My Great Video: Episode 1 / Part 2?"),
            "My Great Video_ Episode 1 _ Part 2_"
        );
        assert_eq!(sanitize_filename("   ...clean name...   "), "clean name");
        assert_eq!(sanitize_filename(""), "video");
    }
}
