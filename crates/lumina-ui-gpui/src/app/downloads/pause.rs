//! Pause and Pause all. Pausing a running download stops its job (the
//! cancel flag) and keeps the row; Resume starts over, never mid-file.

use gpui::*;
use lumina_core::{DownloadRow, DownloadState};
use std::sync::atomic::Ordering;

use crate::LuminaApp;

impl LuminaApp {
    /// Pause on a running or waiting row. A running row's job stops and the
    /// next waiting row takes its slot; the row is stored Paused once the
    /// job has ended (see `finish_download`), so the job's own writes can't
    /// land after it.
    pub fn pause_download(&mut self, row_id: i64, cx: &mut Context<Self>) {
        if let Some(row) = self.pause_in_memory(row_id) {
            self.store_paused(vec![row], true, cx);
        }
        self.pump_downloads(cx);
    }

    /// Pause All: every running and waiting row.
    pub fn pause_all_downloads(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<i64> = self
            .downloads
            .entries
            .iter()
            .map(|entry| entry.row.id)
            .collect();
        let waiting: Vec<DownloadRow> = ids
            .into_iter()
            .filter_map(|id| self.pause_in_memory(id))
            .collect();
        if !waiting.is_empty() {
            self.store_paused(waiting, true, cx);
        }
        cx.notify();
    }

    /// Marks the row Paused and stops its job. Returns a waiting row, which
    /// has no job to store it and must be stored here.
    fn pause_in_memory(&mut self, row_id: i64) -> Option<DownloadRow> {
        let index = self.downloads.index_of(row_id)?;
        let entry = &mut self.downloads.entries[index];
        // ponytail: a placeholder (insert not back yet) isn't paused; it
        // lives for one write. Remember the intent if that ever matters.
        if entry.row.id < 0 {
            return None;
        }
        match entry.row.state {
            DownloadState::Running => {
                entry.cancel.store(true, Ordering::Relaxed);
                entry.row.state = DownloadState::Paused;
                entry.progress = None;
                None
            }
            DownloadState::Waiting => {
                entry.row.state = DownloadState::Paused;
                Some(entry.row.clone())
            }
            _ => None,
        }
    }

    /// Stores paused rows off the UI thread, and shows the banner when that
    /// fails. With `revert` (rows that were waiting) a failed row goes back
    /// to waiting. Without it (a stopped job) the row stays paused, which is
    /// true; the stored Running row comes back paused at the next open.
    pub(super) fn store_paused(
        &mut self,
        rows: Vec<DownloadRow>,
        revert: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        let generation = self.downloads.generation;
        let write = cx.background_executor().spawn(async move {
            rows.into_iter()
                .filter(|row| {
                    let failed = catalog.update_download(row);
                    if let Err(err) = &failed {
                        log::error!("storing paused download {}: {err:#}", row.id);
                    }
                    failed.is_err()
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let failed = write.await;
            if failed.is_empty() {
                return;
            }
            let _ = this.update(cx, |app, cx| {
                if app.downloads.generation != generation {
                    return;
                }
                if !revert {
                    app.set_error(
                        "Lumina couldn't save the download list. It may be out of date after a restart.",
                        cx,
                    );
                    return;
                }
                for row in failed {
                    let Some(index) = app.downloads.index_of(row.id) else {
                        continue;
                    };
                    let entry = &mut app.downloads.entries[index];
                    // Still paused, and no Resume on its way.
                    if entry.row.state == DownloadState::Paused && !entry.requeueing {
                        entry.row.state = DownloadState::Waiting;
                    }
                }
                app.set_error("Couldn't pause the download. Try again.", cx);
                app.pump_downloads(cx);
            });
        })
        .detach();
    }
}
