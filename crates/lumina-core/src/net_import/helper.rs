//! Subprocess runner and detector for yt-dlp.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    sanitize_filename, DownloadPhase, NetworkDownloadProgress, NetworkMediaInfo, NetworkSourceKind,
    ProgressCallback,
};

/// Common locations where `yt-dlp` may be installed on macOS.
const CANDIDATE_PATHS: &[&str] = &[
    "/opt/homebrew/bin/yt-dlp",
    "/usr/local/bin/yt-dlp",
    "/usr/bin/yt-dlp",
];

/// Finds a valid `yt-dlp` executable on the host system.
pub fn find_yt_dlp(custom_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(custom) = custom_path {
        if custom.is_file() {
            return Some(custom.to_path_buf());
        }
    }

    for candidate in CANDIDATE_PATHS {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let user_local = home.join(".local/bin/yt-dlp");
        if user_local.is_file() {
            return Some(user_local);
        }
    }

    // Check system PATH without spawning a subprocess
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("yt-dlp");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Inspects a video URL using `yt-dlp --dump-single-json` to extract title, duration, uploader.
pub fn inspect_url_helper(binary: &Path, url: &str) -> Result<NetworkMediaInfo> {
    let output = Command::new(binary)
        .arg("--socket-timeout")
        .arg("15")
        .arg("--dump-single-json")
        .arg("--no-playlist")
        .arg("--skip-download")
        .arg("--no-warnings")
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run yt-dlp to inspect video")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("yt-dlp error: {}", stderr.trim()));
    }

    let json: Value = serde_json::from_slice(&output.stdout)
        .context("Failed to parse yt-dlp metadata JSON output")?;

    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .map(sanitize_filename)
        .unwrap_or_else(|| "video".to_string());

    let duration_secs = json.get("duration").and_then(|v| v.as_f64());
    let uploader = json
        .get("uploader")
        .or_else(|| json.get("channel"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let thumbnail_url = json
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let expected_bytes = json
        .get("filesize")
        .or_else(|| json.get("filesize_approx"))
        .and_then(|v| v.as_u64());

    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind: NetworkSourceKind::YouTube,
        title,
        duration_secs,
        uploader,
        thumbnail_url,
        expected_bytes,
    })
}

/// Downloads a video via yt-dlp with line-by-line progress reporting.
pub fn download_yt_dlp(
    binary: &Path,
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
    let temp_name = format!(".{nanos}.mp4.lumina-import");
    let temp_path = final_dest
        .parent()
        .map(|p| p.join(&temp_name))
        .unwrap_or_else(|| PathBuf::from(&temp_name));

    let template = "%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.speed)s|%(progress.eta)s";

    let mut child = Command::new(binary)
        .arg("--socket-timeout")
        .arg("30")
        .arg("--newline")
        .arg("--progress-template")
        .arg(template)
        .arg("--no-playlist")
        .arg("-f")
        .arg("bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b")
        .arg("--merge-output-format")
        .arg("mp4")
        .arg("-o")
        .arg(&temp_path)
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn yt-dlp")?;

    if let Some(cb) = progress_cb {
        cb(NetworkDownloadProgress {
            phase: DownloadPhase::Connecting,
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
            eta_secs: None,
            message: "Initializing yt-dlp stream extraction...".to_string(),
        });
    }

    let stdout = child.stdout.take();
    let (tx, rx) = crossbeam_channel::unbounded();

    let reader_thread = std::thread::spawn(move || {
        if let Some(out) = stdout {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        }
    });

    // Drain stderr concurrently to prevent pipe buffer deadlocks (Blocker 1.4)
    let stderr = child.stderr.take();
    let (err_tx, err_rx) = crossbeam_channel::unbounded();
    let stderr_thread = std::thread::spawn(move || {
        if let Some(err) = stderr {
            let reader = BufReader::new(err);
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim().to_string();
                if !trimmed.is_empty() {
                    let _ = err_tx.send(trimmed);
                }
            }
        }
    });

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&temp_path);
            return Err(anyhow!("Download cancelled by user"));
        }

        // Drain any progress lines
        while let Ok(line) = rx.try_recv() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() == 4 {
                let downloaded = parts[0].trim().parse::<u64>().unwrap_or(0);
                let total = parts[1].trim().parse::<u64>().ok();
                let speed = parts[2]
                    .trim()
                    .parse::<f64>()
                    .map(|s| s as u64)
                    .unwrap_or(0);
                let eta = parts[3].trim().parse::<u64>().ok();

                if let Some(cb) = progress_cb {
                    cb(NetworkDownloadProgress {
                        phase: DownloadPhase::Downloading,
                        bytes_downloaded: downloaded,
                        total_bytes: total,
                        speed_bytes_per_sec: speed,
                        eta_secs: eta,
                        message: format!(
                            "Downloading: {} MB{}",
                            downloaded / (1024 * 1024),
                            total.map_or(String::new(), |t| format!(" / {} MB", t / (1024 * 1024)))
                        ),
                    });
                }
            }
        }

        match child.try_wait()? {
            Some(status) => {
                let _ = reader_thread.join();
                let _ = stderr_thread.join();
                if !status.success() {
                    let _ = fs::remove_file(&temp_path);
                    let err_lines: Vec<String> = err_rx.try_iter().collect();
                    let err_msg = if !err_lines.is_empty() {
                        err_lines.join(" ")
                    } else {
                        format!("yt-dlp failed with status: {status}")
                    };
                    return Err(anyhow!("{err_msg}"));
                }
                break;
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }

    if let Some(cb) = progress_cb {
        cb(NetworkDownloadProgress {
            phase: DownloadPhase::Finalizing,
            bytes_downloaded: fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0),
            total_bytes: None,
            speed_bytes_per_sec: 0,
            eta_secs: None,
            message: "Finalizing video container...".to_string(),
        });
    }

    fs::rename(&temp_path, final_dest).with_context(|| {
        format!(
            "Failed to rename yt-dlp download from {:?} to {:?}",
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
