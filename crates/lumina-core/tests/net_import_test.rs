//! Network import downloads, end to end against `file://` URLs and a fake
//! yt-dlp, so nothing touches the network.

use lumina_core::net_import::{
    direct::inspect_direct_url, helper::YT_DLP_ENV, NetworkDownloadProgress, NetworkMediaInfo,
    NetworkSourceKind, ProgressCallback,
};
use lumina_core::{EngineEvent, LuminaEngine, Scanner};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

struct Fixture {
    root: PathBuf,
    engine: Arc<LuminaEngine>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "lumina_net_{name}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let library = root.join("library");
        std::fs::create_dir_all(&library).unwrap();
        let (engine, rx) = LuminaEngine::open_with_paths(
            library,
            root.join("catalog.db"),
            Some(root.join("cache")),
        )
        .unwrap();
        let scanned =
            std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_secs(10)).ok())
                .any(|event| matches!(event, EngineEvent::ScanComplete { .. }));
        assert!(scanned, "the initial scan must finish");
        Self { root, engine }
    }

    /// A served "video" outside the library, as a `file://` URL.
    fn source(&self, name: &str, bytes: &[u8]) -> String {
        let dir = self.root.join("server");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        format!("file://{}", path.display())
    }

    fn download(&self, info: &NetworkMediaInfo, cancel: bool) -> anyhow::Result<PathBuf> {
        let cancel = AtomicBool::new(cancel);
        let seen = Arc::new(Mutex::new(Vec::<NetworkDownloadProgress>::new()));
        let sink = seen.clone();
        let progress: ProgressCallback = Box::new(move |p| sink.lock().unwrap().push(p));
        self.engine.download_network_media(info, &cancel, &progress)
    }

    fn staging_dirs(&self) -> Vec<PathBuf> {
        walkdir::WalkDir::new(self.root.join("library"))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".lumina-import"))
            .map(|e| e.path().to_path_buf())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn direct_info(url: &str, title: &str) -> NetworkMediaInfo {
    let mut info = inspect_direct_url(url, &AtomicBool::new(false)).unwrap();
    info.title = title.to_string();
    info
}

#[test]
fn direct_download_never_replaces_an_existing_file() {
    let fx = Fixture::new("collide");
    let url = fx.source("clip.mov", b"new download");
    let info = direct_info(&url, "Holiday");
    assert_eq!(info.extension, "mov", "a .mov link keeps its container");
    assert_eq!(info.expected_bytes, Some(12));

    let existing = fx.engine.network_import_dir().join("Holiday.mov");
    std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
    std::fs::write(&existing, b"the user's own file").unwrap();

    let path = fx.download(&info, false).unwrap();
    assert_eq!(path.file_name().unwrap(), "Holiday 2.mov");
    assert_eq!(std::fs::read(&path).unwrap(), b"new download");
    assert_eq!(
        std::fs::read(&existing).unwrap(),
        b"the user's own file",
        "a download must never overwrite a file already in the library"
    );
    assert!(
        fx.staging_dirs().is_empty(),
        "staging is removed after success"
    );
}

#[test]
fn download_does_not_take_a_missing_items_path() {
    let fx = Fixture::new("missing_row");
    let dir = fx.engine.network_import_dir();
    let claimed = dir.join("Talk.mp4");
    // A catalogued item whose file is gone still owns its path.
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&claimed, b"old video").unwrap();
    let (tx, _rx) = crossbeam_channel::unbounded();
    let item = Scanner::new(fx.root.join("library"), tx)
        .inspect_media_file(&claimed)
        .unwrap();
    fx.engine.catalog().insert_or_update_batch(&[item]).unwrap();
    std::fs::remove_file(&claimed).unwrap();

    let info = direct_info(&fx.source("talk.mp4", b"video"), "Talk");
    let path = fx.download(&info, false).unwrap();
    assert_eq!(
        path.file_name().unwrap(),
        "Talk 2.mp4",
        "a new download must not inherit a missing item's row and metadata"
    );
}

#[test]
fn cancelled_download_leaves_nothing_behind() {
    let fx = Fixture::new("cancel");
    let info = direct_info(&fx.source("a.mp4", b"video"), "A");
    let err = fx.download(&info, true).unwrap_err();
    assert!(err.to_string().contains("cancelled"), "got: {err:#}");
    assert!(
        fx.staging_dirs().is_empty(),
        "a cancel removes its staging folder"
    );
    assert!(!fx.engine.network_import_dir().join("A.mp4").exists());
}

#[test]
fn web_page_behind_a_video_link_is_refused_after_download() {
    let fx = Fixture::new("html_body");
    // file:// has no Content-Type, like a server that refuses HEAD.
    let info = direct_info(
        &fx.source("watch.mp4", b"  <!DOCTYPE html><html>login</html>"),
        "Watch",
    );
    let err = fx.download(&info, false).unwrap_err();
    assert!(
        err.to_string().contains("web page"),
        "an HTML body must not be imported as a video, got: {err:#}"
    );
    assert!(!fx.engine.network_import_dir().join("Watch.mp4").exists());
    assert!(fx.staging_dirs().is_empty());
}

/// Points the yt-dlp lookup at `script` (debug builds honour the override).
/// Every test in this binary that sets it uses a script of its own fixture;
/// they are serialized by `YT_DLP_LOCK`.
#[cfg(unix)]
fn with_fake_yt_dlp<T>(fx: &Fixture, body: &str, run: impl FnOnce() -> T) -> T {
    use std::os::unix::fs::PermissionsExt;
    static YT_DLP_LOCK: Mutex<()> = Mutex::new(());
    let _guard = YT_DLP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let bin = fx.root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("yt-dlp");
    std::fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var(YT_DLP_ENV, &script);
    let result = run();
    std::env::remove_var(YT_DLP_ENV);
    result
}

/// Mirrors real yt-dlp: `%(ext)s` in the -o template becomes the merged
/// container's extension, and a template without it still gets `.mp4`
/// appended (yt-dlp's `correct_ext`), so a fixed output name is never where
/// the merged file lands.
const MERGING_YT_DLP: &str = "out=''\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = -o ]; then out=\"$2\"; shift; fi\n  shift\ndone\n\
case \"$out\" in *'%(ext)s'*) out=$(echo \"$out\" | sed 's/%(ext)s/mp4/') ;; *) out=\"$out.mp4\" ;; esac\n\
echo '5|5|1.0|avc1'\nprintf merged > \"$out\"\n";

fn youtube_info(title: &str) -> NetworkMediaInfo {
    NetworkMediaInfo {
        url: "https://www.youtube.com/watch?v=abc".to_string(),
        kind: NetworkSourceKind::YouTube,
        title: title.to_string(),
        duration_secs: None,
        uploader: Some("Channel".to_string()),
        expected_bytes: None,
        extension: "mp4",
    }
}

/// yt-dlp names a merged file itself; a fixed output name made every
/// merged YouTube download fail to rename.
#[cfg(unix)]
#[test]
fn yt_dlp_download_is_found_by_its_real_extension() {
    let fx = Fixture::new("ytdlp");
    let info = youtube_info("日本語のタイトル: Part 1");
    let path = with_fake_yt_dlp(&fx, MERGING_YT_DLP, || fx.download(&info, false)).unwrap();
    assert_eq!(path.file_name().unwrap(), "日本語のタイトル_ Part 1.mp4");
    assert_eq!(std::fs::read(&path).unwrap(), b"merged");
    assert!(fx.staging_dirs().is_empty());

    let item = fx
        .engine
        .ingest_network_imported_file(&path, &info, true)
        .unwrap()
        .item;
    assert_eq!(
        item.title.as_deref(),
        Some("日本語のタイトル: Part 1"),
        "the catalog keeps the published title, not the file-safe one"
    );
    assert!(item.is_favorite);
    let source = fx.engine.get_media_source(item.id).unwrap().unwrap();
    assert_eq!(source.0, info.url);
    assert_eq!(source.1.as_deref(), Some("Channel"));
    assert_eq!(
        fx.engine.find_media_by_source_url(&info.url).unwrap(),
        Some(path),
        "a second import of the same URL can be detected"
    );
}

/// The bundled yt-dlp is a launcher that runs Python in a child process;
/// cancelling must stop the child too, not just the launcher.
#[cfg(unix)]
#[test]
fn cancel_stops_the_helpers_child_processes() {
    let fx = Fixture::new("ytdlp_group");
    let pid_file = fx.root.join("child.pid");
    let body = format!("sleep 30 &\necho $! > '{}'\nwait\n", pid_file.display());
    let info = youtube_info("Long");
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let pid_wait = pid_file.clone();
    let stopper = std::thread::spawn(move || {
        while !pid_wait.exists() {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        flag.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    let result = with_fake_yt_dlp(&fx, &body, || {
        let progress: ProgressCallback = Box::new(|_| {});
        fx.engine.download_network_media(&info, &cancel, &progress)
    });
    stopper.join().unwrap();
    assert!(result.is_err(), "a cancelled download fails");

    let child: u32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    // Allow up to 2 s for the killed child to be reaped by launchd.
    let alive_now = || {
        std::process::Command::new("kill")
            .args(["-0", &child.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut alive = alive_now();
    while alive && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        alive = alive_now();
    }
    assert!(
        !alive,
        "the helper's child process must not outlive a cancel"
    );
    assert!(fx.staging_dirs().is_empty());
}

/// The watcher can catalogue the new file before ingest; the user's
/// favorite and the published title must still apply.
#[cfg(unix)]
#[test]
fn ingest_applies_favorite_when_the_watcher_got_there_first() {
    let fx = Fixture::new("watcher_race");
    let info = youtube_info("Concert: Live");
    let path = with_fake_yt_dlp(&fx, MERGING_YT_DLP, || fx.download(&info, false)).unwrap();
    let (tx, _rx) = crossbeam_channel::unbounded();
    let early = Scanner::new(fx.root.join("library"), tx)
        .inspect_media_file(&path)
        .unwrap();
    fx.engine
        .catalog()
        .insert_or_update_batch(&[early])
        .unwrap();

    let item = fx
        .engine
        .ingest_network_imported_file(&path, &info, true)
        .unwrap()
        .item;
    let row = fx
        .engine
        .catalog()
        .get_item_by_id(item.id)
        .unwrap()
        .unwrap();
    assert!(row.is_favorite, "Add to Favorites must survive the race");
    assert_eq!(row.title.as_deref(), Some("Concert: Live"));
}
