//! Locating and running yt-dlp (and the ffmpeg it merges with).

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use super::{
    run_captured, spawn_group, wait_child, DownloadPhase, NetworkDownloadProgress,
    NetworkMediaInfo, NetworkSourceKind, ProgressCallback, INSPECT_TIMEOUT,
};

/// Debug builds: overrides the yt-dlp lookup with one path (a missing path
/// means "not installed"); tests use it to stay off the network.
pub const YT_DLP_ENV: &str = "LUMINA_YT_DLP";

/// Homebrew prefixes; apps launched from Finder don't get them on `PATH`.
const BREW_BINS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin"];

/// Finds `name` next to the running executable (the app bundle's
/// `Contents/MacOS`, or `target/debug` after `make bundle-deps`), then in
/// Homebrew, then on `PATH`. Never relative to the working directory.
///
/// Inside a macOS `.app` — the signed, released artifact whose tools are
/// bundled and SHA-256 pinned (`scripts/fetch_yt_dlp.sh`, `fetch_ffmpeg.sh`)
/// — only the copy beside the executable is trusted, so a missing bundle can
/// never fall through to an unpinned PATH binary. Everywhere else (a local
/// `target/…` build, a Linux `.deb` install beside `/usr/bin`, a debug build)
/// keeps the Homebrew/PATH fallback that AGENTS.md §5.4 documents for a local
/// build.
fn find_tool(name: &str) -> Option<PathBuf> {
    find_tool_for(name, running_in_macos_app_bundle())
}

/// True when `exe` is the executable inside a macOS `.app` bundle. Compared
/// case-insensitively because Finder treats `.APP` the same as `.app`.
fn is_app_bundle_exe(exe: &Path) -> bool {
    exe.to_string_lossy()
        .to_ascii_lowercase()
        .contains(".app/contents/macos/")
}

/// True when this process is the executable inside a macOS `.app` bundle.
fn running_in_macos_app_bundle() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::env::current_exe()
            .map(|exe| is_app_bundle_exe(&exe))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn find_tool_for(name: &str, bundle_only: bool) -> Option<PathBuf> {
    let beside_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)));
    if bundle_only {
        return beside_exe.filter(|candidate| candidate.is_file());
    }
    let brew = BREW_BINS.iter().map(|dir| Path::new(dir).join(name));
    let on_path = std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .filter(|dir| dir.is_absolute())
                .map(|dir| dir.join(name))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    beside_exe
        .into_iter()
        .chain(brew)
        .chain(on_path)
        .find(|candidate| candidate.is_file())
}

/// True when this process is a build from this source tree, not an install:
/// a debug build (never distributed), or an executable under a cargo
/// `target/` directory (`make run-release`, a local `make bundle`).
pub(crate) fn is_source_build() -> bool {
    cfg!(debug_assertions)
        || std::env::current_exe()
            .map(|exe| exe_is_under_target(&exe))
            .unwrap_or(false)
}

/// `…/target/debug/…`, `…/target/<triple>/release/…` and a local
/// `…/target/release/bundle/osx/Lumina.app/…` all count; an installed
/// `/Applications/Lumina.app` or `/usr/bin/lumina` does not. The component
/// name is a heuristic: a path that happens to contain `target` (or a custom
/// `CARGO_TARGET_DIR`) only picks which of the two messages is shown.
fn exe_is_under_target(exe: &Path) -> bool {
    exe.components().any(|part| part.as_os_str() == "target")
}

/// The yt-dlp to run, honouring [`YT_DLP_ENV`].
pub fn find_yt_dlp() -> Option<PathBuf> {
    // Debug builds only: a release app must not run a binary named by the
    // environment.
    if let Some(path) = std::env::var_os(YT_DLP_ENV).filter(|_| cfg!(debug_assertions)) {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    find_tool("yt-dlp")
}

/// The ffmpeg yt-dlp should merge video and audio with, if installed.
pub fn find_ffmpeg() -> Option<PathBuf> {
    find_tool("ffmpeg")
}

/// A plain sentence for a failed yt-dlp run. The raw stderr goes to the log;
/// the user never sees tool or extractor names.
fn yt_dlp_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    log::warn!("yt-dlp failed: {}", text.trim());
    let line = text
        .lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("ERROR:"))
        .unwrap_or("")
        .trim();
    // "[youtube] abc123: Video unavailable" -> "Video unavailable"
    let message = match line.strip_prefix('[') {
        Some(rest) => rest
            .split_once("] ")
            .map(|(_, msg)| msg.split_once(": ").map_or(msg, |(_, m)| m))
            .unwrap_or(line),
        None => line,
    };
    let lower = message.to_lowercase();
    let friendly = if lower.contains("unsupported url") {
        "Lumina can't find a video on this page."
    } else if lower.contains("private video") {
        "This video is private."
    } else if lower.contains("sign in") || lower.contains("login") || lower.contains("cookies") {
        "This video needs you to sign in to the website, so Lumina can't download it."
    } else if lower.contains("not available in your country") || lower.contains("geo") {
        "This video isn't available in your region."
    } else if lower.contains("unavailable") || lower.contains("removed") || lower.contains("404") {
        "This video is unavailable. It may have been removed."
    } else if lower.contains("timed out")
        || lower.contains("unable to download")
        || lower.contains("connection")
    {
        "The website didn't respond. Check your connection and try again."
    } else {
        "Lumina couldn't download this video."
    };
    friendly.to_string()
}

/// Inspects a video page with `yt-dlp --dump-single-json`: title, duration,
/// uploader, size.
pub fn inspect_url_helper(
    binary: &Path,
    url: &str,
    kind: NetworkSourceKind,
    cancel: &AtomicBool,
) -> Result<NetworkMediaInfo> {
    let output = run_captured(
        Command::new(binary)
            // --ignore-config: a user's yt-dlp config (-o, -P, --exec,
            // --download-archive) must not redirect or skip the download.
            .args([
                "--ignore-config",
                "--socket-timeout",
                "15",
                "--dump-single-json",
            ])
            .args(["--no-playlist", "--skip-download", "--no-warnings", "--"])
            .arg(url),
        cancel,
        INSPECT_TIMEOUT,
    )?;
    if !output.status.success() {
        bail!(yt_dlp_error(&output.stderr));
    }
    let json: Value = serde_json::from_slice(&output.stdout)
        .context("Lumina couldn't read this video's details")?;
    let text = |key: &str| json.get(key).and_then(Value::as_str).map(str::to_string);

    Ok(NetworkMediaInfo {
        url: url.to_string(),
        kind,
        title: text("title").unwrap_or_else(|| "video".to_string()),
        duration_secs: json.get("duration").and_then(Value::as_f64),
        uploader: text("uploader").or_else(|| text("channel")),
        expected_bytes: json
            .get("filesize")
            .or_else(|| json.get("filesize_approx"))
            .and_then(Value::as_u64),
        extension: "mp4",
    })
}

/// One `--progress-template` line: `downloaded|total|speed|vcodec`.
const PROGRESS_TEMPLATE: &str = "download:%(progress.downloaded_bytes)s|%(progress.total_bytes,progress.total_bytes_estimate)s|%(progress.speed)s|%(info.vcodec)s";

fn parse_progress(line: &str) -> Option<NetworkDownloadProgress> {
    let mut parts = line.trim().split('|');
    let downloaded = parts.next()?.parse::<u64>().ok()?;
    let total = parts.next()?.parse::<f64>().ok().map(|t| t as u64);
    let speed = parts.next()?.parse::<f64>().map(|s| s as u64).unwrap_or(0);
    // A merged download fetches video then audio; say which is running.
    let message = match parts.next()? {
        "none" => "Downloading audio",
        _ => "Downloading video",
    };
    Some(NetworkDownloadProgress {
        phase: DownloadPhase::Downloading,
        bytes_downloaded: downloaded,
        total_bytes: total,
        speed_bytes_per_sec: speed,
        message: message.to_string(),
    })
}

/// Downloads `url` with yt-dlp into the staging folder `dir`. yt-dlp names
/// the file (`video.<ext>`); the caller picks it up from `dir`.
pub fn download_yt_dlp(
    binary: &Path,
    url: &str,
    dir: &Path,
    cancel: &AtomicBool,
    progress: &ProgressCallback,
) -> Result<()> {
    progress(NetworkDownloadProgress::new(
        DownloadPhase::Connecting,
        "Reading the video page",
    ));
    let mut command = Command::new(binary);
    command
        .args([
            "--ignore-config",
            "--socket-timeout",
            "30",
            "--newline",
            "--no-playlist",
        ])
        .args(["--progress-template", PROGRESS_TEMPLATE])
        .arg("-o")
        // `%(ext)s`: yt-dlp appends its own extension to a merged file, so
        // a fixed output name would not be where the file ends up.
        .arg(dir.join("video.%(ext)s"));
    match find_ffmpeg() {
        Some(ffmpeg) => {
            command
                .args(["-f", "bv*[ext=mp4]+ba[ext=m4a]/bv*+ba/b"])
                // Best quality up to H.264: many Macs can't play AV1, and
                // macOS doesn't play VP9 in MP4.
                .args(["-S", "vcodec:h264,res,acodec:aac"])
                .args(["--merge-output-format", "mp4", "--ffmpeg-location"])
                .arg(ffmpeg);
        }
        // Without ffmpeg yt-dlp cannot merge: ask for single-file formats so
        // the result has both picture and sound.
        None => {
            command.args(["-f", "b[ext=mp4]/b"]);
        }
    }
    let (mut child, _reaper) = spawn_group(
        command
            .arg("--")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?;

    let (tx, rx) = crossbeam_channel::unbounded();
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        }
    });
    let stderr = super::drain(child.stderr.take());

    let status = wait_child(&mut child, cancel, None, || {
        for line in rx.try_iter() {
            if let Some(p) = parse_progress(&line) {
                progress(p);
            }
        }
    })?;
    let _ = reader.join();
    let stderr = stderr.join().unwrap_or_default();
    if !status.success() {
        return Err(anyhow!(yt_dlp_error(&stderr)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_lines_parse_and_label_the_stream() {
        let p = parse_progress("1048576|4194304.0|52428.5|avc1.4d401f").unwrap();
        assert_eq!(p.bytes_downloaded, 1048576);
        assert_eq!(p.total_bytes, Some(4194304));
        assert_eq!(p.message, "Downloading video");
        let audio = parse_progress("10|NA|NA|none").unwrap();
        assert_eq!(audio.total_bytes, None, "an unknown total stays unknown");
        assert_eq!(audio.message, "Downloading audio");
        assert!(parse_progress("[download] Destination: x").is_none());
    }

    #[test]
    fn yt_dlp_errors_are_plain_language() {
        let stderr = b"WARNING: something\nERROR: [youtube] abc: Video unavailable\n";
        assert_eq!(
            yt_dlp_error(stderr),
            "This video is unavailable. It may have been removed."
        );
        let unknown = b"ERROR: [generic] Unsupported URL: https://example.com\n";
        let message = yt_dlp_error(unknown);
        assert!(
            !message.contains("generic") && !message.contains("yt-dlp"),
            "tool and extractor names stay out of user messages: {message}"
        );
    }

    /// A `.app` bundle must never exec a tool from Homebrew or PATH: only the
    /// bundled, SHA-256-pinned copy beside the executable is trusted. A local
    /// build (not inside a `.app`) may still use PATH.
    #[test]
    fn bundle_lookup_ignores_homebrew_and_path() {
        let on_path = if cfg!(windows) { "cmd.exe" } else { "sh" };
        // The test binary's directory does not contain a shell, so a local
        // build finds it on PATH and a bundle refuses it.
        assert!(
            find_tool_for(on_path, false).is_some(),
            "a local build may use a tool from PATH"
        );
        assert!(
            find_tool_for(on_path, true).is_none(),
            "a bundle must not run an unpinned PATH tool"
        );
    }

    #[test]
    fn app_bundle_detection_matches_only_the_bundle_layout() {
        assert!(is_app_bundle_exe(Path::new(
            "/Applications/Lumina.app/Contents/MacOS/lumina-ui-gpui"
        )));
        assert!(
            is_app_bundle_exe(Path::new(
                "/Applications/Lumina.APP/Contents/MacOS/lumina-ui-gpui"
            )),
            "the extension is case-insensitive, like Finder"
        );
        assert!(!is_app_bundle_exe(Path::new(
            "/Users/me/src/lumina/target/debug/lumina-ui-gpui"
        )));
        assert!(!is_app_bundle_exe(Path::new("/usr/bin/lumina")));
    }

    /// A local test bundle built with `LUMINA_BUNDLE_DEPS=0` sits under
    /// `target/` and is still a source build (rebuilt, not reinstalled); an
    /// installed app or a Linux package is not.
    #[test]
    fn source_build_detection_covers_local_bundles() {
        assert!(exe_is_under_target(Path::new(
            "/Users/me/src/lumina/target/debug/lumina-ui-gpui"
        )));
        assert!(exe_is_under_target(Path::new(
            "/Users/me/src/lumina/target/aarch64-apple-darwin/debug/lumina-ui-gpui"
        )));
        assert!(exe_is_under_target(Path::new(
            "/Users/me/src/lumina/target/release/bundle/osx/Lumina.app/Contents/MacOS/lumina-ui-gpui"
        )));
        assert!(!exe_is_under_target(Path::new(
            "/Applications/Lumina.app/Contents/MacOS/lumina-ui-gpui"
        )));
        assert!(!exe_is_under_target(Path::new("/usr/bin/lumina")));
    }
}
