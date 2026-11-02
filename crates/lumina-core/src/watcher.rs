//! Real-time filesystem watcher for the Lumina library.
//!
//! Wraps notify-debouncer-mini to turn OS filesystem events into catalog
//! mutations and EngineEvent notifications:
//!
//! * file added / modified  -> Scanner::inspect_media_file -> catalog upsert
//!   -> EngineEvent::BatchDiscovered
//! * folder added / moved in -> every media file under it, as above
//! * file or folder deleted / moved away -> rows marked missing (metadata
//!   kept) -> EngineEvent::ItemsMissing; a rename reconnects through the
//!   upsert (same inode) or a fold
//! * notify error (events were lost) -> the error hook, which rescans
//!
//! The watcher runs on its own thread so filesystem events never block the UI
//! thread. Dropping the last LibraryWatcher handle signals the thread to
//! stop and joins it, tearing down the debouncer and its OS watch handles.

use anyhow::Result;
use crossbeam_channel::Sender;
use notify_debouncer_mini::notify::RecommendedWatcher;
use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult, Debouncer};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::catalog::Catalog;
use crate::scanner::{file_signature, walk_media, Scanner};
use crate::EngineEvent;

/// Called when the OS watcher reports an error: events may have been lost.
pub(crate) type ErrorHook = Box<dyn Fn() + Send + Sync>;

/// Debounce window: coalesce bursts of filesystem events (create + write +
/// chmod, editor temp-file dance, ...) into a single inspection pass.
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(500);

/// Handle to the background library watcher.
///
/// Cloning the handle is cheap and shares the same underlying watcher. The
/// watcher thread is stopped and joined when the last cloned handle is
/// dropped.
#[derive(Clone)]
pub struct LibraryWatcher {
    inner: Arc<LibraryWatcherInner>,
}

struct LibraryWatcherInner {
    stop_tx: Mutex<Option<std::sync::mpsc::SyncSender<()>>>,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl LibraryWatcher {
    /// Starts a recursive watcher rooted at library_root.
    ///
    /// Fails if the OS watcher cannot be created or the root cannot be
    /// watched; in that case the caller can keep running without live updates
    /// (the initial scan still works).
    pub fn new(
        library_root: PathBuf,
        catalog: Catalog,
        event_tx: Sender<EngineEvent>,
        on_error: ErrorHook,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let (stop_tx, stop_rx) = std::sync::mpsc::sync_channel::<()>(0);
        // Startup handshake: report whether the OS watch was successfully
        // registered instead of silently running a dead watcher.
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Result<()>>(1);

        let thread_stop = stop.clone();
        let thread_handle = thread::Builder::new()
            .name("lumina-fs-watcher".to_string())
            .spawn(move || {
                watch_loop(
                    library_root,
                    catalog,
                    event_tx,
                    thread_stop,
                    stop_rx,
                    init_tx,
                    on_error,
                )
            })
            .map_err(|err| {
                anyhow::anyhow!("failed to spawn lumina filesystem watcher thread: {err}")
            })?;

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                inner: Arc::new(LibraryWatcherInner {
                    stop_tx: Mutex::new(Some(stop_tx)),
                    stop,
                    thread: Mutex::new(Some(thread_handle)),
                }),
            }),
            Ok(Err(err)) => {
                let _ = thread_handle.join();
                Err(err)
            }
            Err(_) => {
                let _ = thread_handle.join();
                Err(anyhow::anyhow!(
                    "lumina filesystem watcher thread exited before it was ready"
                ))
            }
        }
    }

    /// Requests shutdown and waits for the watcher thread to finish.
    pub fn stop(&self) {
        self.inner.shutdown();
    }
}

impl LibraryWatcherInner {
    fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.stop_tx.lock() {
            drop(guard.take());
        }
        if let Ok(mut guard) = self.thread.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for LibraryWatcherInner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Entry point for the watcher thread: registers the recursive watch, reports
/// startup success, then parks until shutdown.
fn watch_loop(
    library_root: PathBuf,
    catalog: Catalog,
    event_tx: Sender<EngineEvent>,
    stop: Arc<AtomicBool>,
    stop_rx: std::sync::mpsc::Receiver<()>,
    init_tx: std::sync::mpsc::SyncSender<Result<()>>,
    on_error: ErrorHook,
) {
    // notify reports paths using the form of the watched root. Canonicalize
    // where possible (e.g. macOS /var -> /private/var) and remember both
    // forms so we can map event paths back to the exact strings stored by the
    // scanner.
    let watch_root = library_root
        .canonicalize()
        .unwrap_or_else(|_| library_root.clone());

    let debouncer = match build_debouncer(
        &watch_root,
        watch_root.clone(),
        library_root,
        catalog,
        event_tx,
        stop.clone(),
        on_error,
    ) {
        Ok(debouncer) => debouncer,
        Err(err) => {
            let _ = init_tx.send(Err(err));
            return;
        }
    };

    if init_tx.send(Ok(())).is_err() {
        // The engine was dropped while we were starting up.
        return;
    }

    // Park until shutdown is signaled instead of polling
    let _ = stop_rx.recv();

    // Dropping the debouncer stops its internal thread and releases the OS
    // watch handles (fsevents / inotify / kqueue).
    drop(debouncer);
}

/// Creates the debounced watcher and registers the recursive watch.
fn build_debouncer(
    watch_root: &Path,
    event_root: PathBuf,
    library_root: PathBuf,
    catalog: Catalog,
    event_tx: Sender<EngineEvent>,
    stop: Arc<AtomicBool>,
    on_error: ErrorHook,
) -> Result<Debouncer<RecommendedWatcher>> {
    // The scanner is only used for header/EXIF inspection here; it never
    // walks the tree from this instance.
    let scanner = Scanner::new(library_root.clone(), event_tx.clone());
    let handler_stop = stop;

    let mut debouncer = new_debouncer(DEBOUNCE_WINDOW, move |result: DebounceEventResult| {
        // Ignore any events that arrive after shutdown has been requested.
        if handler_stop.load(Ordering::SeqCst) {
            return;
        }
        match result {
            Ok(events) => {
                let paths = events
                    .iter()
                    .map(|event| normalize_event_path(&event.path, &event_root, &library_root))
                    .collect();
                handle_batch(paths, &library_root, &scanner, &catalog, &event_tx);
            }
            Err(err) => {
                log::warn!("watcher notify error, rescanning: {err:?}");
                on_error();
            }
        }
    })?;

    debouncer
        .watcher()
        .watch(watch_root, RecursiveMode::Recursive)
        .map_err(|err| {
            anyhow::anyhow!(
                "failed to watch library root {:?}: {err}",
                watch_root.display()
            )
        })?;

    Ok(debouncer)
}

/// Maps a path reported by the OS watcher back into the path form used by the
/// scanner / catalog. This keeps add and delete events consistent even when
/// the watched root had to be canonicalized (symlinked library paths).
fn normalize_event_path(path: &Path, event_root: &Path, library_root: &Path) -> PathBuf {
    match path.strip_prefix(event_root) {
        Ok(relative) => library_root.join(relative),
        Err(_) => path.to_path_buf(),
    }
}

/// Applies one debounced batch of changed paths to the catalog. The batch
/// carries no event kinds and no order, so gone paths are handled first: a
/// rename whose two halves share a batch is then missing before its new path
/// is ingested, and the upsert reconnects it by inode.
pub(crate) fn handle_batch(
    paths: Vec<PathBuf>,
    library_root: &Path,
    scanner: &Scanner,
    catalog: &Catalog,
    event_tx: &Sender<EngineEvent>,
) {
    // Only paths inside the library count. FSEvents reports an unmount as a
    // removal of the mount point, an ancestor of the root, which must not
    // mark the library missing.
    let mut paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| match path.strip_prefix(library_root) {
            Ok(relative) => {
                relative.components().next().is_some() && !has_hidden_component(relative)
            }
            Err(_) => false,
        })
        .collect();
    paths.sort();
    paths.dedup();
    if paths.is_empty() || !library_root.is_dir() {
        return;
    }

    // Only a confirmed absence is gone; a path that cannot be checked
    // (permissions, I/O error) is left alone rather than marked missing.
    let (mut gone, mut present) = (Vec::new(), Vec::new());
    for path in paths {
        match path.try_exists() {
            Ok(false) => gone.push(path),
            Ok(true) => present.push(path),
            Err(err) => log::warn!("watcher could not check {}: {err}", path.display()),
        }
    }

    let mut missing = Vec::new();
    for path in &gone {
        match catalog.mark_missing_under(path) {
            Ok(change) => {
                missing.extend(change.missing);
                for (kept, removed) in change.folded {
                    let _ = event_tx.send(EngineEvent::ItemRemoved(removed));
                    // The kept row has a new path.
                    if let Ok(Some(item)) = catalog.get_item_by_id(kept) {
                        let _ = event_tx.send(EngineEvent::BatchDiscovered { items: vec![item] });
                    }
                }
            }
            Err(err) => log::warn!("watcher failed to mark {path:?} missing: {err:?}"),
        }
    }
    if !missing.is_empty() {
        let _ = event_tx.send(EngineEvent::ItemsMissing(missing));
    }

    // A folder moved or copied in arrives as one event for the folder.
    let mut files = Vec::new();
    for path in present {
        if path.is_dir() {
            files.extend(walk_media(&path));
        } else if path.is_file() && crate::scanner::is_supported_media(&path) {
            files.push(path);
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        return;
    }

    // Files already catalogued as they are need no new read. An import
    // catalogues its copies itself; this keeps their events from re-reading
    // each file and refreshing the UI once per file.
    let known = match catalog.present_signatures(&files) {
        Ok(known) => known,
        Err(err) => {
            log::warn!("watcher failed to read catalogued files: {err:?}");
            Default::default()
        }
    };
    let items: Vec<_> = files
        .iter()
        .filter(|path| known.get(*path) != file_signature(path).as_ref())
        .filter_map(|path| scanner.inspect_media_file(path))
        .collect();
    if items.is_empty() {
        return;
    }
    match catalog.insert_or_update_batch(&items) {
        Ok(inserted) => {
            let _ = event_tx.send(EngineEvent::BatchDiscovered { items: inserted });
        }
        Err(err) => log::warn!("watcher failed to save {} files: {err:?}", items.len()),
    }
}

/// Returns true when any component of the path is a dot-file/dot-directory.
/// The scanner skips hidden entries, so the watcher must not resurrect them.
fn has_hidden_component(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::files::tests::TempDir;
    use crossbeam_channel::{unbounded, Receiver};

    fn write_jpeg(path: &Path) {
        image::RgbImage::from_fn(8, 8, |x, y| image::Rgb([x as u8 * 30, y as u8 * 30, 90]))
            .save(path)
            .expect("jpeg must be writable");
    }

    struct Fixture {
        dir: TempDir,
        root: PathBuf,
        catalog: Catalog,
        scanner: Scanner,
        events: Receiver<EngineEvent>,
        tx: Sender<EngineEvent>,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = TempDir::new(name);
            let root = dir.0.join("library");
            std::fs::create_dir_all(&root).unwrap();
            let (tx, events) = unbounded();
            Self {
                scanner: Scanner::new(root.clone(), tx.clone()),
                catalog: Catalog::open_in_memory().unwrap(),
                dir,
                root,
                events,
                tx,
            }
        }

        fn batch(&self, paths: Vec<PathBuf>) {
            handle_batch(paths, &self.root, &self.scanner, &self.catalog, &self.tx);
        }

        fn add(&self, rel: &str) -> crate::MediaId {
            let path = self.root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            write_jpeg(&path);
            self.batch(vec![path.clone()]);
            self.events.try_iter().for_each(drop);
            self.catalog
                .get_file_signatures()
                .unwrap()
                .contains_key(&path)
                .then_some(())
                .expect("the file must be catalogued");
            self.catalog
                .get_item_slice(0, 100)
                .unwrap()
                .into_iter()
                .find(|i| i.path == path)
                .unwrap()
                .id
        }
    }

    #[test]
    fn unmount_event_at_ancestor_marks_nothing() {
        let f = Fixture::new("watch_unmount");
        f.add("a.jpg");
        f.batch(vec![f.dir.0.clone(), f.root.clone()]);
        assert!(
            f.catalog.get_missing_ids().unwrap().is_empty(),
            "events at or above the root must not mark the library missing"
        );
    }

    #[test]
    fn batch_with_root_gone_marks_nothing() {
        let f = Fixture::new("watch_root_gone");
        let a = f.root.join("a.jpg");
        f.add("a.jpg");
        std::fs::remove_dir_all(&f.root).unwrap();
        f.batch(vec![a]);
        assert!(
            f.catalog.get_missing_ids().unwrap().is_empty(),
            "a vanished library folder must not mark its files missing"
        );
    }

    #[test]
    fn unchanged_file_event_is_skipped() {
        let f = Fixture::new("watch_unchanged");
        f.add("a.jpg");
        f.batch(vec![f.root.join("a.jpg")]);
        assert!(
            f.events.try_iter().next().is_none(),
            "a file catalogued as it is needs no re-read or event"
        );
    }

    #[test]
    fn rename_in_one_batch_reconnects() {
        let f = Fixture::new("watch_rename");
        let id = f.add("a.jpg");
        let (old, new) = (f.root.join("a.jpg"), f.root.join("sub/b.jpg"));
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::rename(&old, &new).unwrap();
        // The new path first: the batch carries no order.
        f.batch(vec![new.clone(), old]);
        let item = f.catalog.get_item_by_id(id).unwrap().unwrap();
        assert_eq!(item.path, new, "the row follows its file");
        assert_eq!(f.catalog.total_count().unwrap(), 1);
        assert!(f.catalog.get_missing_ids().unwrap().is_empty());
    }

    #[test]
    fn folder_move_reconnects_all_files() {
        let f = Fixture::new("watch_folder_move");
        let a = f.add("trip/a.jpg");
        let b = f.add("trip/day 2/b.jpg");
        std::fs::rename(f.root.join("trip"), f.root.join("2024 trip")).unwrap();
        // A folder move arrives as one event for each folder.
        f.batch(vec![f.root.join("trip"), f.root.join("2024 trip")]);
        assert_eq!(f.catalog.total_count().unwrap(), 2, "no new rows");
        assert!(f.catalog.get_missing_ids().unwrap().is_empty());
        let a = f.catalog.get_item_by_id(a).unwrap().unwrap();
        let b = f.catalog.get_item_by_id(b).unwrap().unwrap();
        assert_eq!(a.path, f.root.join("2024 trip/a.jpg"));
        assert_eq!(b.path, f.root.join("2024 trip/day 2/b.jpg"));
    }

    #[test]
    fn deleted_folder_marks_files_missing() {
        let f = Fixture::new("watch_folder_delete");
        let a = f.add("trip/a.jpg");
        f.add("trip 2/b.jpg");
        std::fs::remove_dir_all(f.root.join("trip")).unwrap();
        f.batch(vec![f.root.join("trip")]);
        assert_eq!(
            f.catalog.get_missing_ids().unwrap(),
            [a].into_iter().collect(),
            "only the deleted folder's file is missing"
        );
        assert!(f
            .events
            .try_iter()
            .any(|e| matches!(e, EngineEvent::ItemsMissing(ids) if ids == vec![a])));
    }
}
