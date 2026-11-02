//! Real-time filesystem watcher for the Lumina library.
//!
//! Wraps notify-debouncer-mini to turn OS filesystem events into catalog
//! mutations and EngineEvent notifications:
//!
//! * file added / modified  -> Scanner::inspect_media_file -> catalog upsert
//!   -> EngineEvent::BatchDiscovered
//! * file deleted / renamed away -> catalog removal -> EngineEvent::ItemRemoved
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
use crate::scanner::Scanner;
use crate::EngineEvent;

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
                for event in events {
                    let path = normalize_event_path(&event.path, &event_root, &library_root);
                    handle_event(&path, &scanner, &catalog, &event_tx);
                }
            }
            Err(err) => log::warn!("watcher notify error: {err:?}"),
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

/// Applies one filesystem event to the catalog and forwards the matching
/// engine event. Events for directories, hidden entries and non-media
/// extensions are ignored, mirroring the scanner's filtering rules.
fn handle_event(path: &Path, scanner: &Scanner, catalog: &Catalog, event_tx: &Sender<EngineEvent>) {
    if has_hidden_component(path) {
        return;
    }

    if path.exists() {
        if !path.is_file() {
            return;
        }
        let Some(item) = scanner.inspect_media_file(path) else {
            // Unsupported extension or unreadable file.
            return;
        };
        match catalog.insert_or_update_batch(std::slice::from_ref(&item)) {
            Ok(inserted) if !inserted.is_empty() => {
                let _ = event_tx.send(EngineEvent::BatchDiscovered { items: inserted });
            }
            Ok(_) => {}
            Err(err) => log::warn!("watcher failed to upsert {path:?}: {err:?}"),
        }
        return;
    }

    // The path no longer exists: it was deleted, moved or renamed away.
    match catalog.remove_item_by_path(path) {
        Ok(Some(id)) => {
            let _ = event_tx.send(EngineEvent::ItemRemoved(id));
        }
        Ok(None) => {}
        Err(err) => log::warn!("watcher failed to remove {path:?}: {err:?}"),
    }
}

/// Returns true when any component of the path is a dot-file/dot-directory.
/// The scanner skips hidden entries, so the watcher must not resurrect them.
fn has_hidden_component(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
}
