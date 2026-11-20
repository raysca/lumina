//! Network video import: downloads media from direct URLs, HLS (.m3u8), and
//! web video sites (YouTube and others, through the bundled yt-dlp).
//!
//! The user starts every request by pasting a URL; nothing runs in the
//! background (see `docs/network-import.md`). A download is staged in a hidden
//! `.<nanos>.lumina-import/` folder in `<library>/Imports/YYYY/YYYY-MM-DD/`
//! (the scanner and watcher skip dot-names), then linked under a free name
//! with [`crate::import::claim_name`], catalogued with its source URL, and
//! given a companion `.xmp` sidecar.

pub mod direct;
pub mod helper;
pub mod hls;

pub use helper::{find_ffmpeg, find_yt_dlp};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Local};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::import::{claim_name, IMPORTS_DIR, TEMP_SUFFIX};
use crate::scanner::Scanner;
use crate::{LuminaEngine, MediaItem};

/// Video containers a direct link may point at; all of them are ones the
/// scanner catalogues.
const DIRECT_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "mkv", "avi"];
/// Shown when a bundled helper is gone; names no tool (users never need to
/// know how a download works).
const MISSING_PART: &str =
    "A part of Lumina needed for downloads is missing. Reinstall Lumina to fix this.";
/// The message for a source build, which is rebuilt, not reinstalled. Names
/// the recipe, not the tools. Short enough for the download popover's
/// three-line clamp, where a failed row shows it.
const MISSING_PART_DEV: &str =
    "A part of Lumina needed for downloads is missing from this build. Run \"make bundle-deps\", then try again.";
/// On Linux the helper is a distribution package, so reinstalling Lumina
/// would not add it; point at the package manager instead.
const MISSING_PART_LINUX: &str =
    "A part of Lumina needed for downloads is missing. Install the download helper your package manager provides, then try again.";

/// The message for a missing helper, for the build that is running. Call it
/// only on the failure path: it asks where the executable lives.
pub fn missing_part() -> &'static str {
    missing_part_for(helper::is_source_build())
}

/// The message for a known build kind, so both can be tested without moving
/// the test binary.
fn missing_part_for(source_build: bool) -> &'static str {
    if source_build {
        MISSING_PART_DEV
    } else if cfg!(target_os = "linux") {
        MISSING_PART_LINUX
    } else {
        MISSING_PART
    }
}

/// Shown when the library folder can't be written; the path goes to the log.
const CANT_SAVE: &str =
    "Lumina couldn't save to your library folder. Check that it is connected and has free space.";
/// Longest a URL inspection may take before its tool is killed.
const INSPECT_TIMEOUT: Duration = Duration::from_secs(60);
/// Longest a file name stem may be, in characters.
const MAX_STEM_CHARS: usize = 120;

/// The classification of an incoming network media source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkSourceKind {
    /// Direct video file download over HTTP/HTTPS (.mp4, .mov, etc.).
    DirectUrl,
    /// HTTP Live Streaming playlist (.m3u8).
    HlsStream,
    /// YouTube video link (watch, shorts, youtu.be).
    YouTube,
    /// Another web video site supported by yt-dlp (e.g. Vimeo).
    GenericPlatform,
}

impl NetworkSourceKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::DirectUrl => "Video File",
            Self::HlsStream => "Video Stream",
            Self::YouTube => "YouTube",
            Self::GenericPlatform => "Web Video",
        }
    }

    /// Whether downloading this kind needs the yt-dlp helper.
    pub fn needs_yt_dlp(&self) -> bool {
        !matches!(self, Self::DirectUrl)
    }
}

/// Classifies a raw URL string into a [`NetworkSourceKind`].
pub fn classify_url(raw_url: &str) -> Result<NetworkSourceKind> {
    let trimmed = raw_url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        bail!("Enter a web address that starts with http:// or https://");
    }

    let lower = trimmed.to_lowercase();
    if lower.contains("youtube.com/watch")
        || lower.contains("youtu.be/")
        || lower.contains("youtube.com/shorts/")
        || lower.contains("youtube.com/live/")
    {
        return Ok(NetworkSourceKind::YouTube);
    }
    let path = lower.split(['?', '#']).next().unwrap_or("");
    if path.ends_with(".m3u8") {
        return Ok(NetworkSourceKind::HlsStream);
    }
    if url_extension(path).is_some() {
        return Ok(NetworkSourceKind::DirectUrl);
    }
    Ok(NetworkSourceKind::GenericPlatform)
}

/// The supported video extension at the end of a URL or file name, if any.
fn url_extension(path: &str) -> Option<&'static str> {
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let ext = path.rsplit_once('.')?.1.to_lowercase();
    DIRECT_EXTENSIONS
        .iter()
        .copied()
        .find(|known| *known == ext)
}

/// Sanitizes a title into a file name stem: no path separators or characters
/// Finder and SMB reject, at most [`MAX_STEM_CHARS`] characters.
pub fn sanitize_filename(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('_'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let trimmed = out.trim().trim_matches('.');
    // Truncate by characters: a byte cut could split a multi-byte character.
    let short: String = trimmed.chars().take(MAX_STEM_CHARS).collect();
    let short = short.trim().trim_end_matches('.');
    if short.is_empty() {
        "video".to_string()
    } else {
        short.to_string()
    }
}

/// Information inspected about a remote media source prior to download.
#[derive(Debug, Clone)]
pub struct NetworkMediaInfo {
    pub url: String,
    pub kind: NetworkSourceKind,
    /// The title as published; the file name is a sanitized copy.
    pub title: String,
    pub duration_secs: Option<f64>,
    pub uploader: Option<String>,
    pub expected_bytes: Option<u64>,
    /// Container of a direct download; yt-dlp downloads take the extension
    /// of the file yt-dlp produces.
    pub extension: &'static str,
}

/// Current phase of a download task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPhase {
    Connecting,
    Downloading,
    Finalizing,
}

/// Real-time progress report emitted during download execution.
#[derive(Debug, Clone)]
pub struct NetworkDownloadProgress {
    pub phase: DownloadPhase,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: u64,
    /// What is happening, e.g. "Downloading audio"; the UI adds the sizes.
    pub message: String,
}

impl NetworkDownloadProgress {
    pub fn new(phase: DownloadPhase, message: impl Into<String>) -> Self {
        Self {
            phase,
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: 0,
            message: message.into(),
        }
    }

    pub fn percent(&self) -> Option<f32> {
        let total = self.total_bytes.filter(|t| *t > 0)?;
        Some(((self.bytes_downloaded as f64 / total as f64) * 100.0) as f32)
    }
}

/// A catalogued network download.
#[derive(Debug)]
pub struct NetworkIngest {
    pub item: MediaItem,
    /// The item is catalogued, but the timeline could not be refreshed.
    pub timeline_error: Option<String>,
}

/// Callback type for receiving progress updates.
pub type ProgressCallback = Box<dyn Fn(NetworkDownloadProgress) + Send + Sync>;

/// Whether a bundled helper `kind` needs is missing (a damaged install):
/// yt-dlp for web video and streams, plus ffmpeg for streams (without it
/// yt-dlp leaves MPEG-TS data in a `.mp4` file). Web video still works
/// without ffmpeg, in a single-file format. Looks at the disk; call it off
/// the UI thread.
pub fn helper_missing(kind: NetworkSourceKind) -> bool {
    (kind.needs_yt_dlp() && find_yt_dlp().is_none())
        || (kind == NetworkSourceKind::HlsStream && find_ffmpeg().is_none())
}

/// Inspects a URL off the UI thread: title, size, uploader. Killing the tool
/// when `cancel` is set or after [`INSPECT_TIMEOUT`].
pub fn inspect_url(url: &str, cancel: &AtomicBool) -> Result<NetworkMediaInfo> {
    match classify_url(url)? {
        NetworkSourceKind::DirectUrl => direct::inspect_direct_url(url, cancel),
        NetworkSourceKind::HlsStream => hls::inspect_hls_url(url, cancel),
        kind @ (NetworkSourceKind::YouTube | NetworkSourceKind::GenericPlatform) => {
            let binary = find_yt_dlp().ok_or_else(|| anyhow!(missing_part()))?;
            helper::inspect_url_helper(&binary, url, kind, cancel)
        }
    }
}

/// A hidden staging folder for one download, removed when dropped (after the
/// result has been linked under its real name, or on any failure).
struct StagingDir(PathBuf);

impl StagingDir {
    fn new(parent: &Path) -> Result<Self> {
        fs::create_dir_all(parent)
            .inspect_err(|err| log::error!("creating {}: {err}", parent.display()))
            .context(CANT_SAVE)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = parent.join(format!(".{nanos}{TEMP_SUFFIX}"));
        fs::create_dir(&path)
            .inspect_err(|err| log::error!("creating {}: {err}", path.display()))
            .context(CANT_SAVE)?;
        Ok(Self(path))
    }
}

impl Drop for StagingDir {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_dir_all(&self.0) {
            if err.kind() != io::ErrorKind::NotFound {
                log::warn!("could not remove {}: {err}", self.0.display());
            }
        }
    }
}

/// The finished download in `dir`: its largest file, skipping yt-dlp's
/// partial-download and resume files.
fn produced_file(dir: &Path) -> Result<PathBuf> {
    find_produced(dir)
        .inspect_err(|err| log::error!("reading {}: {err}", dir.display()))
        .context(CANT_SAVE)?
        .ok_or_else(|| anyhow!("The download finished without a video. Try again."))
}

fn find_produced(dir: &Path) -> io::Result<Option<PathBuf>> {
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.file_type()?.is_file() || name.ends_with(".part") || name.ends_with(".ytdl") {
            continue;
        }
        let len = entry.metadata()?.len();
        if best.as_ref().is_none_or(|(size, _)| len > *size) {
            best = Some((len, path));
        }
    }
    Ok(best.map(|(_, path)| path))
}

/// Output of a finished helper process.
pub(crate) struct ToolOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs `command` to completion and captures its output, killing it when
/// `cancel` is set or `timeout` passes (AGENTS.md: native tools never block
/// a worker forever).
pub(crate) fn run_captured(
    command: &mut Command,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<ToolOutput> {
    let (mut child, _reaper) = spawn_group(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?;
    // Drain both pipes on their own threads so a full pipe never stalls the child.
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait_child(&mut child, cancel, Some(Instant::now() + timeout), || {})?;
    Ok(ToolOutput {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Spawns `command` as the leader of a new process group, tracked so the
/// whole group dies at exit (see `decoder::live_children`).
pub(crate) fn spawn_group(command: &mut Command) -> Result<(Child, impl Sized)> {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(command, 0);
    let child = command.spawn().with_context(|| {
        // Log the tool; the user only needs to know the download didn't start.
        log::error!("could not start {:?}", command.get_program());
        "Could not start the download"
    })?;
    let reaper = crate::decoder::live_children::track_group(child.id());
    Ok((child, reaper))
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    })
}

/// Polls `child` (a group leader from [`spawn_group`]) until it exits,
/// calling `tick` every 50 ms. Stops its whole group and fails when `cancel`
/// is set or `deadline` passes.
pub(crate) fn wait_child(
    child: &mut Child,
    cancel: &AtomicBool,
    deadline: Option<Instant>,
    mut tick: impl FnMut(),
) -> Result<ExitStatus> {
    loop {
        let timed_out = deadline.is_some_and(|d| Instant::now() >= d);
        if cancel.load(Ordering::Relaxed) || timed_out {
            crate::decoder::live_children::stop_group(child);
            if timed_out {
                bail!("The server took too long to respond");
            }
            bail!("Download cancelled");
        }
        if let Some(status) = child
            .try_wait()
            .context("The download stopped unexpectedly. Try again.")?
        {
            return Ok(status);
        }
        tick();
        std::thread::sleep(Duration::from_millis(50));
    }
}

impl LuminaEngine {
    /// Today's network import folder: `<library>/Imports/YYYY/YYYY-MM-DD/`.
    pub fn network_import_dir(&self) -> PathBuf {
        let now = Local::now();
        self.library_root
            .join(IMPORTS_DIR)
            .join(format!("{:04}", now.year()))
            .join(format!(
                "{:04}-{:02}-{:02}",
                now.year(),
                now.month(),
                now.day()
            ))
    }

    /// Downloads `info` into today's import folder and returns the final
    /// path. Blocks; run it off the UI thread. Never replaces an existing
    /// file or a catalogued (even missing) path.
    pub fn download_network_media(
        &self,
        info: &NetworkMediaInfo,
        cancel: &AtomicBool,
        progress: &ProgressCallback,
    ) -> Result<PathBuf> {
        let dir = self.network_import_dir();
        let staging = StagingDir::new(&dir)?;

        match info.kind {
            NetworkSourceKind::DirectUrl => {
                let target = staging.0.join(format!("download.{}", info.extension));
                direct::download_direct_url(
                    &info.url,
                    &target,
                    info.expected_bytes,
                    cancel,
                    progress,
                )?;
            }
            NetworkSourceKind::HlsStream
            | NetworkSourceKind::YouTube
            | NetworkSourceKind::GenericPlatform => {
                if helper_missing(info.kind) {
                    bail!(missing_part());
                }
                let binary = find_yt_dlp().ok_or_else(|| anyhow!(missing_part()))?;
                helper::download_yt_dlp(&binary, &info.url, &staging.0, cancel, progress)?;
            }
        }

        let produced = produced_file(&staging.0)?;
        if Scanner::classify_media_extension(&produced).is_none() {
            log::warn!("network import produced unsupported {}", produced.display());
            bail!("This video comes in a format Lumina can't show yet.");
        }
        let ext = produced
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| "mp4".to_string());
        progress(NetworkDownloadProgress::new(
            DownloadPhase::Finalizing,
            "Adding to library",
        ));
        // A missing row still owns its path: a new file there would inherit
        // that item's metadata.
        let taken = |path: &Path| {
            self.catalog
                .id_at_path(path)
                .map(|id| id.is_some())
                .map_err(|err| io::Error::other(format!("{err:#}")))
        };
        let name = format!("{}.{ext}", sanitize_filename(&info.title));
        claim_name(&produced, &dir, &name, &taken)
            .inspect_err(|err| log::error!("saving into {}: {err}", dir.display()))
            .context(CANT_SAVE)
    }

    /// Catalogues a downloaded file with its source attribution (one
    /// transaction) and writes a companion `.xmp` sidecar. Blocks; run it off
    /// the UI thread. An error means the file is not catalogued; a failed
    /// timeline refresh after the commit is reported in the result instead.
    pub fn ingest_network_imported_file(
        &self,
        final_path: &Path,
        info: &NetworkMediaInfo,
        is_favorite: bool,
    ) -> Result<NetworkIngest> {
        let scanner = Scanner::new(self.library_root.clone(), self.event_tx.clone());
        let mut item = scanner.inspect_media_file(final_path).ok_or_else(|| {
            anyhow!(
                "{} is not a media file Lumina can read",
                final_path.display()
            )
        })?;
        let title = info.title.trim();
        if !title.is_empty() {
            item.title = Some(title.to_string());
        }
        item.is_favorite = is_favorite;

        let inserted =
            self.catalog
                .insert_with_source(&item, &info.url, info.uploader.as_deref())?;
        // Keep timeline buckets fresh for zoom levels (AGENTS.md 5.3)
        let timeline_error = self.catalog.compute_timeline_buckets().err().map(|err| {
            log::error!("refreshing the timeline after a network import: {err:#}");
            format!("{err:#}")
        });

        let sidecar = crate::xmp::SidecarMetadata {
            title: inserted.title.clone(),
            caption: None,
            rating: 0,
            color_label: 0,
            tags: Vec::new(),
            source_url: Some(info.url.clone()),
            credit: info.uploader.clone(),
            ..Default::default()
        };
        if let Err(e) = crate::xmp::write_library_sidecar(final_path, &sidecar) {
            log::warn!(
                "Failed to write companion XMP sidecar for {:?}: {e:#}",
                final_path
            );
        }
        Ok(NetworkIngest {
            item: inserted,
            timeline_error,
        })
    }

    /// The library item Lumina already downloaded from `url`, if any.
    pub fn find_prior_download(&self, url: &str) -> Result<Option<crate::PriorDownload>> {
        self.catalog.find_prior_download(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packaged app is reinstalled; a source build is told the recipe that
    /// adds the pinned helpers, and neither message names a tool. The test
    /// binary is a source build, which pins the wiring, not just the table.
    #[test]
    fn missing_helper_message_matches_the_build() {
        assert_eq!(missing_part(), MISSING_PART_DEV, "tests run from target/");
        assert_eq!(missing_part_for(true), MISSING_PART_DEV);
        assert_eq!(
            missing_part_for(false),
            if cfg!(target_os = "linux") {
                MISSING_PART_LINUX
            } else {
                MISSING_PART
            }
        );
        assert!(
            MISSING_PART.contains("Reinstall"),
            "an installed app cannot fetch a helper at run time"
        );
        assert!(
            !MISSING_PART_DEV.contains("Reinstall"),
            "a source build is not reinstalled: {MISSING_PART_DEV}"
        );
        assert!(
            MISSING_PART_LINUX.contains("package manager"),
            "a Linux install gets its helper from the distribution"
        );
        for message in [MISSING_PART, MISSING_PART_DEV, MISSING_PART_LINUX] {
            let lower = message.to_lowercase();
            for word in [
                "yt-dlp", "ytdlp", "ffmpeg", "curl", "homebrew", "hls", "m3u8",
            ] {
                assert!(
                    !lower.contains(word),
                    "UI copy must not name download tools or formats: {message}"
                );
            }
        }
    }

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
            classify_url("https://example.com/clip.MOV?sig=1").unwrap(),
            NetworkSourceKind::DirectUrl
        );
        assert_eq!(
            classify_url("https://vimeo.com/12345678").unwrap(),
            NetworkSourceKind::GenericPlatform
        );
        assert_eq!(
            classify_url("https://example.com/?next=a.mp4.html").unwrap(),
            NetworkSourceKind::GenericPlatform,
            "an extension in the query string is not a direct link"
        );
        assert!(classify_url("not-a-url").is_err());
    }

    #[test]
    fn test_url_extension_keeps_container() {
        assert_eq!(url_extension("https://a.b/x.mkv?t=1"), Some("mkv"));
        assert_eq!(url_extension("https://a.b/x.MOV"), Some("mov"));
        assert_eq!(url_extension("https://a.b/x.webm"), None);
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

    #[test]
    fn test_sanitize_filename_truncates_on_char_boundary() {
        // 1 + 45 * 3 bytes: byte 120 falls inside a character.
        let title = format!("a{}", "日".repeat(45));
        let out = sanitize_filename(&title);
        assert_eq!(
            out.chars().count(),
            MAX_STEM_CHARS.min(46),
            "a long multi-byte title must be cut by characters, not bytes"
        );
        let emoji = "🎬".repeat(200);
        assert_eq!(sanitize_filename(&emoji).chars().count(), MAX_STEM_CHARS);
    }

    #[test]
    fn test_produced_file_skips_partials() {
        let dir = std::env::temp_dir().join(format!("lumina-produced-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("video.mp4.part"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("video.mp4"), vec![0u8; 10]).unwrap();
        assert_eq!(
            produced_file(&dir).unwrap(),
            dir.join("video.mp4"),
            "a .part file is never the finished download"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
