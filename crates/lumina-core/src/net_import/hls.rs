//! HLS (.m3u8) HTTP Live Streaming parser and downloader helper.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use super::helper::{download_yt_dlp, find_yt_dlp};
use super::{sanitize_filename, NetworkMediaInfo, NetworkSourceKind, ProgressCallback};

/// Inspects an HLS stream playlist (.m3u8).
pub fn inspect_hls_url(url: &str) -> Result<NetworkMediaInfo> {
    let output = Command::new("curl")
        .arg("-s")
        .arg("-L")
        .arg("--max-time")
        .arg("10")
        .arg(url)
        .output()
        .context("Failed to fetch m3u8 playlist via curl")?;

    if !output.status.success() {
        return Err(anyhow!("Failed to connect to HLS stream"));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    if !body.contains("#EXTM3U") {
        return Err(anyhow!(
            "URL does not return a valid HLS (#EXTM3U) playlist"
        ));
    }

    let is_live = !body.contains("#EXT-X-ENDLIST") && !body.contains("#EXT-X-STREAM-INF");
    if is_live {
        return Err(anyhow!(
            "URL is a continuous live stream with no defined end. Only VOD (recorded) streams can be imported."
        ));
    }

    let path_part = url.split('?').next().unwrap_or(url);
    let slug = path_part.rsplit('/').next().unwrap_or("stream");
    let without_ext = slug.rsplit_once('.').map(|(base, _)| base).unwrap_or(slug);
    let title = sanitize_filename(without_ext);

    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind: NetworkSourceKind::HlsStream,
        title,
        duration_secs: None,
        uploader: None,
        thumbnail_url: None,
        expected_bytes: None,
    })
}

/// Downloads an HLS stream using yt-dlp/ffmpeg helper, or errors if unavailable.
pub fn download_hls_via_helper(
    url: &str,
    final_dest: &Path,
    cancel_flag: &AtomicBool,
    progress_cb: Option<&ProgressCallback>,
) -> Result<PathBuf> {
    if let Some(binary) = find_yt_dlp(None) {
        return download_yt_dlp(&binary, url, final_dest, cancel_flag, progress_cb);
    }

    Err(anyhow!(
        "Downloading HLS streams requires yt-dlp or the macOS native AVFoundation bridge. Install yt-dlp via 'brew install yt-dlp'."
    ))
}
