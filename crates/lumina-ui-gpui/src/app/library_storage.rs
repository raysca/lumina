//! Settings › Library's storage card: the library's bytes by kind, the
//! thumbnail cache and the library volume's capacity, read off the UI
//! thread and kept until the next refresh.

use std::path::PathBuf;

use gpui::*;
use lumina_core::StorageSummary;

use crate::app::preferences::{directory_usage, thumbnail_cache_dir, CacheUsage};
use crate::app::LuminaApp;

/// One reading of what the open library takes on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryStorage {
    /// The library it was read for; a reading for another is dropped.
    pub root: PathBuf,
    pub summary: StorageSummary,
    pub cache: CacheUsage,
    /// `(total, available)` bytes of the library's volume, when known.
    pub capacity: Option<(u64, u64)>,
}

impl LibraryStorage {
    /// Photos, videos and downloads: the library's own files.
    pub fn library_bytes(&self) -> u64 {
        self.summary.photo_bytes + self.summary.video_bytes + self.summary.download_bytes
    }
}

#[cfg(test)]
thread_local! {
    /// Tests set it to read the volume's capacity as unknown.
    pub(crate) static CAPACITY_UNKNOWN: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

impl LuminaApp {
    /// The last reading, if it is for the library open now. Settings shows
    /// nothing rather than another library's numbers.
    pub(crate) fn library_storage(&self) -> Option<&LibraryStorage> {
        let root = self.grid_state.as_ref()?.engine.library_root();
        self.preferences_storage
            .as_ref()
            .filter(|storage| storage.root == root)
    }

    /// Reads the library's storage in the background. The previous reading
    /// stays on screen until this one lands.
    pub(crate) fn refresh_library_storage(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let root = engine.library_root().to_path_buf();
        // Read on this thread: the test cache directory is per test thread.
        let cache_dir = thumbnail_cache_dir();
        #[cfg(test)]
        let probe_capacity = !CAPACITY_UNKNOWN.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let probe_capacity = true;
        let task = cx.background_executor().spawn(async move {
            LibraryStorage {
                summary: engine.storage_summary(),
                cache: directory_usage(&cache_dir),
                capacity: probe_capacity
                    .then(|| crate::storage::volume_capacity(&root))
                    .flatten(),
                root,
            }
        });
        cx.spawn(async move |this, cx| {
            let storage = task.await;
            // The app may be gone; then nothing is left to update.
            let _ = this.update(cx, |app, cx| {
                let current = app
                    .grid_state
                    .as_ref()
                    .is_some_and(|s| s.engine.library_root() == storage.root);
                if current {
                    app.preferences_storage = Some(storage);
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
