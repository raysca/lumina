//! Direct HTTP/HTTPS video downloads using the system curl.

use anyhow::{anyhow, bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::{
    run_captured, sanitize_filename, spawn_group, url_extension, wait_child, DownloadPhase,
    NetworkDownloadProgress, NetworkMediaInfo, NetworkSourceKind, ProgressCallback,
    INSPECT_TIMEOUT,
};

/// The headers of the last response in a `curl -I -L` dump (after redirects).
#[derive(Debug, Default, PartialEq)]
struct FinalResponse {
    status: Option<u16>,
    content_length: Option<u64>,
    content_type: Option<String>,
    filename: Option<String>,
}

fn parse_final_response(dump: &str) -> FinalResponse {
    let mut out = FinalResponse::default();
    for line in dump.lines() {
        let line = line.trim();
        if line.starts_with("HTTP/") {
            // A new response (redirect hop): forget the previous one's headers.
            out = FinalResponse {
                status: line.split_whitespace().nth(1).and_then(|s| s.parse().ok()),
                ..Default::default()
            };
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => out.content_length = value.parse().ok(),
            "content-type" => out.content_type = Some(value.to_ascii_lowercase()),
            "content-disposition" => {
                out.filename = value
                    .split(';')
                    .filter_map(|part| part.trim().strip_prefix("filename="))
                    .map(|f| f.trim_matches(['"', '\'']).to_string())
                    .find(|f| !f.is_empty());
            }
            _ => {}
        }
    }
    out
}

/// Refuses responses that are not a downloadable video: an error status, or
/// a web page (a login wall or error page behind a `.mp4` link).
fn check_response(response: &FinalResponse) -> Result<()> {
    // Some servers refuse HEAD (403/405/501) but serve the file to GET.
    if let Some(status) = response.status.filter(|s| *s >= 400) {
        if !matches!(status, 403 | 405 | 501) {
            log::warn!("HEAD answered HTTP {status}");
            bail!("The website couldn't find this video. Check that the link still works.");
        }
    }
    if response
        .content_type
        .as_deref()
        .is_some_and(|t| t.starts_with("text/html"))
    {
        bail!("This link opens a web page, not a video file. Copy the video's own link instead.");
    }
    Ok(())
}

/// Inspects a direct video URL with an HTTP HEAD: size, file name, and
/// whether it is a web page rather than a video.
pub fn inspect_direct_url(url: &str, cancel: &AtomicBool) -> Result<NetworkMediaInfo> {
    let output = run_captured(
        Command::new("curl")
            .args(["-sS", "-I", "-L", "--max-time", "15", "--"])
            .arg(url),
        cancel,
        INSPECT_TIMEOUT,
    )?;
    if !output.status.success() {
        bail!(curl_error(output.status, &output.stderr));
    }
    let response = parse_final_response(&String::from_utf8_lossy(&output.stdout));
    check_response(&response)?;

    let path = url.split(['?', '#']).next().unwrap_or(url);
    let file_name = response
        .filename
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or("video").to_string());
    let extension = url_extension(&file_name)
        .or_else(|| url_extension(path))
        .unwrap_or("mp4");
    let stem = file_name
        .rsplit_once('.')
        .map(|(base, _)| base)
        .unwrap_or(&file_name);

    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind: NetworkSourceKind::DirectUrl,
        title: sanitize_filename(stem),
        duration_secs: None,
        uploader: None,
        expected_bytes: response.content_length,
        extension,
    })
}

/// A plain-language message for a failed curl run.
pub(crate) fn curl_error(status: ExitStatus, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr);
    let detail = detail.lines().last().unwrap_or("").trim();
    match status.code() {
        Some(6) => "Could not find the server. Check the address and your connection.".into(),
        Some(7) => "Could not connect to the server.".into(),
        Some(22) => {
            log::warn!("curl: {detail}");
            "The website refused the download. The link may have expired.".into()
        }
        Some(28) => "The server took too long to respond.".into(),
        Some(35 | 60) => "Could not make a secure connection to the server.".into(),
        // Tool details go to the log, not the dialog.
        _ => {
            log::warn!("curl failed ({status}): {detail}");
            "The download was interrupted. Check your connection and try again.".into()
        }
    }
}

/// Downloads `url` to `target` (inside a staging folder) with curl,
/// reporting progress from the file's growing size.
pub fn download_direct_url(
    url: &str,
    target: &Path,
    expected_bytes: Option<u64>,
    cancel: &AtomicBool,
    progress: &ProgressCallback,
) -> Result<()> {
    progress(NetworkDownloadProgress::new(
        DownloadPhase::Connecting,
        "Connecting to server",
    ));
    let mut command = Command::new("curl");
    command
        .args(["-sS", "-f", "-L", "--connect-timeout", "30"])
        // Give up on a connection that stalls for a minute.
        .args(["--speed-limit", "1", "--speed-time", "60"])
        .arg("-o")
        .arg(target)
        .arg("--")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let (mut child, _reaper) = spawn_group(&mut command)?;
    let stderr = super::drain(child.stderr.take());

    let start = Instant::now();
    let mut last = Instant::now();
    let status = wait_child(&mut child, cancel, None, || {
        if last.elapsed() < Duration::from_millis(100) {
            return;
        }
        last = Instant::now();
        let bytes = fs::metadata(target).map(|m| m.len()).unwrap_or(0);
        let secs = start.elapsed().as_secs_f64().max(0.001);
        progress(NetworkDownloadProgress {
            phase: DownloadPhase::Downloading,
            bytes_downloaded: bytes,
            total_bytes: expected_bytes,
            speed_bytes_per_sec: (bytes as f64 / secs) as u64,
            message: "Downloading".to_string(),
        });
    })?;
    let stderr = stderr.join().unwrap_or_default();
    if !status.success() {
        return Err(anyhow!(curl_error(status, &stderr)));
    }
    // A server that refused HEAD can still answer GET with a web page.
    if starts_like_html(target).context("Could not read the download")? {
        bail!("This link opens a web page, not a video file. Copy the video's own link instead.");
    }
    Ok(())
}

/// Whether a file begins like an HTML page; no video container starts with `<`.
fn starts_like_html(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut head = [0u8; 512];
    let n = fs::File::open(path)?.read(&mut head)?;
    let text = String::from_utf8_lossy(&head[..n])
        .trim_start()
        .to_ascii_lowercase();
    Ok(["<!doctype", "<html", "<head", "<body"]
        .iter()
        .any(|tag| text.starts_with(tag)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_response_uses_last_hop() {
        let dump = "HTTP/1.1 302 Found\r\nLocation: /x\r\nContent-Type: text/html\r\n\r\n\
                    HTTP/2 200\r\ncontent-type: video/mp4\r\ncontent-length: 1234\r\n\
                    content-disposition: attachment; filename=\"Clip.mov\"\r\n\r\n";
        assert_eq!(
            parse_final_response(dump),
            FinalResponse {
                status: Some(200),
                content_length: Some(1234),
                content_type: Some("video/mp4".into()),
                filename: Some("Clip.mov".into()),
            },
            "headers of a redirect hop must not leak into the final response"
        );
    }

    #[test]
    fn web_pages_and_errors_are_refused() {
        let page = FinalResponse {
            status: Some(200),
            content_type: Some("text/html; charset=utf-8".into()),
            ..Default::default()
        };
        assert!(
            check_response(&page).is_err(),
            "an HTML page behind a .mp4 link must not be imported as a video"
        );
        let gone = FinalResponse {
            status: Some(404),
            ..Default::default()
        };
        assert!(check_response(&gone).is_err());
        let no_head = FinalResponse {
            status: Some(405),
            ..Default::default()
        };
        assert!(
            check_response(&no_head).is_ok(),
            "servers that refuse HEAD still download"
        );
    }
}
