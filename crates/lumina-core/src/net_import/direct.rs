//! Direct HTTP/HTTPS video stream downloader using system curl.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::{
    sanitize_filename, DownloadPhase, NetworkDownloadProgress, NetworkMediaInfo, NetworkSourceKind,
    ProgressCallback,
};

/// Inspects a direct video URL using HTTP HEAD to fetch content length and filename.
pub fn inspect_direct_url(url: &str) -> Result<NetworkMediaInfo> {
    let output = Command::new("curl")
        .arg("-s")
        .arg("-I")
        .arg("-L")
        .arg("--max-time")
        .arg("10")
        .arg(url)
        .output()
        .context("Failed to execute curl for inspecting URL")?;

    if !output.status.success() {
        return Err(anyhow!("Network request failed or URL unreachable"));
    }

    let headers = String::from_utf8_lossy(&output.stdout);
    let mut expected_bytes = None;
    let mut disposition_filename = None;

    for line in headers.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = line.split(':').nth(1) {
                if let Ok(b) = val.trim().parse::<u64>() {
                    expected_bytes = Some(b);
                }
            }
        } else if lower.starts_with("content-disposition:") {
            if let Some(pos) = line.find("filename=") {
                let filename_part = &line[pos + 9..];
                let clean = filename_part.trim().trim_matches('"').trim_matches('\'');
                if !clean.is_empty() {
                    disposition_filename = Some(clean.to_string());
                }
            }
        }
    }

    // Determine candidate title from disposition or URL path
    let title = disposition_filename.unwrap_or_else(|| {
        let path_part = url.split('?').next().unwrap_or(url);
        let slug = path_part.rsplit('/').next().unwrap_or("video");
        let without_ext = slug.rsplit_once('.').map(|(base, _)| base).unwrap_or(slug);
        sanitize_filename(without_ext)
    });

    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind: NetworkSourceKind::DirectUrl,
        title,
        duration_secs: None,
        uploader: None,
        thumbnail_url: None,
        expected_bytes,
    })
}

/// Downloads a direct video stream via curl to a temp file, reporting progress.
pub fn download_direct_url(
    url: &str,
    final_dest: &Path,
    cancel_flag: &AtomicBool,
    progress_cb: Option<&ProgressCallback>,
) -> Result<PathBuf> {
    if let Some(parent) = final_dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Use .lumina-import suffix so interrupted files match clean_stale_temp_files
    let temp_name = format!(".{nanos}.tmp.lumina-import");
    let temp_path = final_dest
        .parent()
        .map(|p| p.join(&temp_name))
        .unwrap_or_else(|| PathBuf::from(&temp_name));

    // Spawn curl with line-buffered progress meter
    // `-#` or `--progress-bar` outputs to stderr
    let mut child = Command::new("curl")
        .arg("-f")
        .arg("-L")
        .arg("-C")
        .arg("-")
        .arg("-#")
        .arg("-o")
        .arg(&temp_path)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn curl for downloading")?;

    let start_time = Instant::now();
    let mut last_reported = Instant::now();

    if let Some(cb) = progress_cb {
        cb(NetworkDownloadProgress {
            phase: DownloadPhase::Connecting,
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
            eta_secs: None,
            message: "Connecting to server...".to_string(),
        });
    }

    let stderr = child.stderr.take();
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(err) = stderr {
            let mut reader = BufReader::new(err);
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let trimmed = line.trim().to_string();
                if !trimmed.is_empty() {
                    buffer.push(trimmed);
                }
                line.clear();
            }
        }
        buffer
    });

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&temp_path);
            return Err(anyhow!("Download cancelled by user"));
        }

        match child.try_wait()? {
            Some(status) => {
                let err_lines = reader_thread.join().unwrap_or_default();
                if !status.success() {
                    let _ = fs::remove_file(&temp_path);
                    let err_msg = if !err_lines.is_empty() {
                        err_lines.join(" ")
                    } else {
                        format!("curl exited with error status: {status}")
                    };
                    return Err(anyhow!("{err_msg}"));
                }
                break;
            }
            None => {
                // Periodically sample downloaded file size for reliable progress
                if last_reported.elapsed().as_millis() >= 100 {
                    let current_bytes = fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
                    let elapsed_secs = start_time.elapsed().as_secs_f64().max(0.001);
                    let speed = (current_bytes as f64 / elapsed_secs) as u64;

                    if let Some(cb) = progress_cb {
                        cb(NetworkDownloadProgress {
                            phase: DownloadPhase::Downloading,
                            bytes_downloaded: current_bytes,
                            total_bytes: None,
                            speed_bytes_per_sec: speed,
                            eta_secs: None,
                            message: format!("Downloading: {} bytes", current_bytes),
                        });
                    }
                    last_reported = Instant::now();
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }

    // Rename temp file to final destination
    if let Some(cb) = progress_cb {
        cb(NetworkDownloadProgress {
            phase: DownloadPhase::Finalizing,
            bytes_downloaded: fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0),
            total_bytes: None,
            speed_bytes_per_sec: 0,
            eta_secs: None,
            message: "Finalizing downloaded media...".to_string(),
        });
    }

    fs::rename(&temp_path, final_dest).with_context(|| {
        format!(
            "Failed to rename temp download {:?} to {:?}",
            temp_path, final_dest
        )
    })?;

    if let Some(cb) = progress_cb {
        cb(NetworkDownloadProgress {
            phase: DownloadPhase::Complete,
            bytes_downloaded: fs::metadata(final_dest).map(|m| m.len()).unwrap_or(0),
            total_bytes: None,
            speed_bytes_per_sec: 0,
            eta_secs: None,
            message: "Download complete".to_string(),
        });
    }

    Ok(final_dest.to_path_buf())
}
