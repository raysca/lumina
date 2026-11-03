//! Import by copy: planning (walk, skips, duplicates, destination) and
//! copying (names, mtime, temp files, cataloguing, cancel, failures).

use lumina_core::{LuminaEngine, MediaItem};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A library (`media/`) and a separate source folder outside it.
struct Fixture {
    root: PathBuf,
    engine: Arc<LuminaEngine>,
    events: crossbeam_channel::Receiver<lumina_core::EngineEvent>,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "lumina_import_{name}_{}_{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::create_dir_all(root.join("source")).unwrap();
        let (engine, rx) = LuminaEngine::open_with_paths(
            root.join("media"),
            root.join("catalog.db"),
            Some(root.join("cache")),
        )
        .expect("open engine with isolated paths");
        // Wait for the initial (empty) scan so it cannot race the test.
        while !matches!(
            rx.recv_timeout(Duration::from_secs(15))
                .expect("scan completes"),
            lumina_core::EngineEvent::ScanComplete { .. }
        ) {}
        Self {
            root,
            engine,
            events: rx,
        }
    }

    fn media(&self) -> PathBuf {
        self.root.join("media")
    }

    fn source(&self) -> PathBuf {
        self.root.join("source")
    }

    fn plan(&self, paths: &[PathBuf]) -> lumina_core::import::ImportPlan {
        self.engine
            .plan_import(paths, &AtomicBool::new(false))
            .expect("plan succeeds")
    }

    fn import(&self, paths: &[PathBuf]) -> lumina_core::import::CopyImportReport {
        let plan = self.plan(paths);
        self.engine.run_import(plan, &AtomicBool::new(false))
    }

    /// Catalogues `path` now, as a scan would.
    fn catalogue(&self, path: &Path) -> MediaItem {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let item = lumina_core::scanner::Scanner::new(self.media(), tx)
            .inspect_media_file(path)
            .expect("readable media");
        self.engine
            .catalog()
            .insert_or_update_batch(&[item])
            .unwrap()
            .remove(0)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A small jpeg whose pixels (and so content hash) depend on `seed`.
fn write_jpeg(path: &Path, seed: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::RgbImage::from_fn(16, 16, |x, y| {
        image::Rgb([(x * 12) as u8, (y * 12) as u8, seed])
    })
    .save(path)
    .unwrap();
}

/// 2024-05-06 12:00 UTC: the same calendar day in every time zone it is
/// likely to run in.
fn may_sixth() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_714_996_800)
}

fn set_mtime(path: &Path, time: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn plan_follows_scanner_rules_and_skips_library_files() {
    let fx = Fixture::new("plan_rules");
    write_jpeg(&fx.source().join("a.jpg"), 1);
    write_jpeg(&fx.source().join(".hidden/b.jpg"), 2);
    std::fs::write(fx.source().join("notes.txt"), b"not media").unwrap();
    let inside = fx.media().join("inside.jpg");
    write_jpeg(&inside, 3);

    let plan = fx.plan(&[fx.source(), inside]);

    assert_eq!(plan.files.len(), 1, "only a.jpg is copied");
    assert_eq!(plan.files[0].source, fx.source().join("a.jpg"));
    assert_eq!(plan.unsupported, 1, "notes.txt is counted, not copied");
    assert_eq!(
        plan.in_library, 1,
        "a file already in the library folder is not copied again"
    );
    assert_eq!(plan.duplicates, 0);
}

#[test]
fn plan_skips_library_and_in_batch_duplicates() {
    let fx = Fixture::new("plan_dups");
    let existing = fx.media().join("existing.jpg");
    write_jpeg(&existing, 1);
    fx.catalogue(&existing);
    write_jpeg(&fx.source().join("copy_of_existing.jpg"), 1);
    write_jpeg(&fx.source().join("new.jpg"), 2);
    write_jpeg(&fx.source().join("sub/new_again.jpg"), 2);

    let plan = fx.plan(&[fx.source()]);

    assert_eq!(
        plan.duplicates, 2,
        "one copy of a library item, one repeat within the import"
    );
    assert_eq!(plan.files.len(), 1, "one of the two new.jpg copies is kept");
}

#[test]
fn import_copies_by_date_keeps_mtime_and_catalogues() {
    let fx = Fixture::new("copy");
    let source = fx.source().join("a.jpg");
    write_jpeg(&source, 1);
    set_mtime(&source, may_sixth());

    let report = fx.import(&[fx.source()]);

    assert_eq!(report.copied, 1);
    assert!(report.failed.is_empty() && !report.cancelled);
    let day = fx.media().join("Imports/2024/2024-05-06");
    assert_eq!(
        names_in(&day),
        vec!["a.jpg"],
        "the copy lands by date taken and no temp file is left"
    );
    let dest = day.join("a.jpg");
    assert_eq!(
        std::fs::metadata(&dest).unwrap().modified().unwrap(),
        may_sixth(),
        "the copy keeps the source's modified time"
    );
    assert!(source.exists(), "the original is not changed");
    assert!(
        fx.engine
            .get_item_slice(0..10)
            .iter()
            .any(|item| item.path == dest),
        "the copy is catalogued"
    );

    let again = fx.plan(&[fx.source()]);
    assert!(
        again.files.is_empty() && again.duplicates == 1,
        "importing the same folder twice copies nothing"
    );
}

#[test]
fn name_clash_with_a_file_or_a_missing_row_gets_a_suffix() {
    let fx = Fixture::new("clash");
    let day = fx.media().join("Imports/2024/2024-05-06");
    write_jpeg(&day.join("a.jpg"), 1);
    let gone = day.join("a 2.jpg");
    write_jpeg(&gone, 2);
    fx.catalogue(&gone);
    std::fs::remove_file(&gone).unwrap();
    fx.engine.catalog().mark_missing_under(&gone).unwrap();
    let source = fx.source().join("a.jpg");
    write_jpeg(&source, 3);
    set_mtime(&source, may_sixth());

    let report = fx.import(&[source]);

    assert_eq!(report.copied, 1);
    assert!(
        day.join("a 3.jpg").exists() && !gone.exists(),
        "a file on disk and a missing row's path are both taken, got {:?}",
        names_in(&day)
    );
}

#[test]
fn cancel_stops_copying() {
    let fx = Fixture::new("cancel");
    write_jpeg(&fx.source().join("a.jpg"), 1);
    write_jpeg(&fx.source().join("b.jpg"), 2);
    let plan = fx.plan(&[fx.source()]);
    assert_eq!(plan.files.len(), 2);

    while fx.events.try_recv().is_ok() {}
    let report = fx.engine.run_import(plan, &AtomicBool::new(true));

    assert!(report.cancelled);
    assert_eq!(report.copied, 0);
    let last = fx.events.try_iter().find_map(|event| match event {
        lumina_core::EngineEvent::ImportProgress { done, total } => Some((done, total)),
        _ => None,
    });
    assert_eq!(
        last,
        Some((0, 2)),
        "a stopped copy does not report itself complete"
    );
}

#[test]
// A source gone before copying fails in `fs::copy`, before any temp exists;
// removal of a temp after a later failure is not exercised here.
fn failed_copy_is_reported_and_leaves_nothing() {
    let fx = Fixture::new("fail");
    let source = fx.source().join("a.jpg");
    write_jpeg(&source, 1);
    set_mtime(&source, may_sixth());
    let plan = fx.plan(std::slice::from_ref(&source));
    std::fs::remove_file(&source).unwrap();

    let report = fx.engine.run_import(plan, &AtomicBool::new(false));

    assert_eq!(report.copied, 0);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].0, source);
    assert!(
        names_in(&fx.media().join("Imports/2024/2024-05-06")).is_empty(),
        "a failed copy leaves nothing behind"
    );
}

#[test]
fn stale_temp_files_are_cleaned() {
    let fx = Fixture::new("stale");
    let stale = fx
        .media()
        .join("Imports/2020/2020-01-01/.a.jpg.lumina-import");
    std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
    std::fs::write(&stale, b"half a copy").unwrap();

    fx.plan(&[fx.source()]);

    assert!(!stale.exists(), "a crash's half-done copy is removed");
}

#[test]
fn stale_download_folders_are_cleaned() {
    let fx = Fixture::new("stale_dir");
    let stale = fx
        .media()
        .join("Imports/2020/2020-01-01/.123.lumina-import");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("video.mp4.part"), b"half a download").unwrap();

    fx.plan(&[fx.source()]);

    assert!(
        !stale.exists(),
        "a crash's half-done network download folder is removed"
    );
}

#[test]
fn checking_and_copying_report_progress() {
    use lumina_core::EngineEvent;
    let fx = Fixture::new("progress");
    for seed in 0..3 {
        write_jpeg(&fx.source().join(format!("{seed}.jpg")), seed);
    }
    while fx.events.try_recv().is_ok() {}

    let report = fx.import(&[fx.source()]);
    assert_eq!(report.copied, 3);

    let events: Vec<EngineEvent> = fx.events.try_iter().collect();
    let checking: Vec<(usize, usize)> = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::ImportChecking { checked, total } => Some((*checked, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(checking.first(), Some(&(0, 3)), "checking starts at zero");
    assert_eq!(
        checking.last(),
        Some(&(3, 3)),
        "and ends with every file read"
    );
    let copying = events.iter().rev().find_map(|event| match event {
        EngineEvent::ImportProgress { done, total } => Some((*done, *total)),
        _ => None,
    });
    assert_eq!(copying, Some((3, 3)), "the copy ends with a full count");
}
