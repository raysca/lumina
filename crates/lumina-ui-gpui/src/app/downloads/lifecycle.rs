//! The list across quits and library switches: ask before cutting running
//! downloads short, bring stored rows back paused, resume on request.

use gpui::*;
use lumina_core::DownloadState;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::DownloadEntry;
use crate::LuminaApp;

/// What the "downloads are still running" prompt goes on to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DownloadsConfirm {
    Quit,
    OpenLibrary(PathBuf),
}

impl LuminaApp {
    /// Downloading and waiting rows: what Pause all would stop. Paused
    /// rows wait for the user.
    pub(crate) fn running_or_waiting_count(&self) -> usize {
        self.downloads
            .entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.row.state,
                    DownloadState::Waiting | DownloadState::Running
                )
            })
            .count()
    }

    /// What quitting or switching library would cut short: downloading and
    /// waiting rows, and paused rows whose job hasn't ended yet.
    pub(crate) fn downloads_cut_by_leaving(&self) -> usize {
        self.downloads
            .entries
            .iter()
            .filter(|entry| {
                entry.live_job.is_some()
                    || matches!(
                        entry.row.state,
                        DownloadState::Waiting | DownloadState::Running
                    )
            })
            .count()
    }

    /// True (and the prompt shows) when downloads would be cut short.
    pub(crate) fn ask_before_leaving(
        &mut self,
        then: DownloadsConfirm,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.downloads_cut_by_leaving() == 0 {
            return false;
        }
        self.downloads.open = false;
        self.downloads.confirm = Some(then);
        cx.notify();
        true
    }

    /// File > Quit and ⌘Q.
    pub(crate) fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.ask_before_leaving(DownloadsConfirm::Quit, cx) {
            return;
        }
        // Rows stay Running/Waiting on disk; the next open shows them paused.
        self.downloads.reset(None);
        // State changed since the last debounced session save would be lost.
        self.save_config();
        // Last: a failed Restart to Update stops here with the banner.
        if !self.install_update_on_quit() {
            cx.notify();
            return;
        }
        #[cfg(test)]
        {
            self.downloads.quit_requested = true;
        }
        cx.quit();
    }

    /// Quit / Open in the prompt, and ⏎.
    pub(crate) fn confirm_downloads_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(then) = self.downloads.confirm.take() else {
            return;
        };
        // Stops the jobs; with nothing running the actions below go ahead.
        self.downloads.reset(self.downloads.job.clone());
        match then {
            DownloadsConfirm::Quit => self.request_quit(cx),
            DownloadsConfirm::OpenLibrary(path) => self.open_library(path, cx),
        }
    }

    /// Cancel, Esc and the backdrop: the downloads keep going.
    pub(crate) fn dismiss_downloads_prompt(&mut self, cx: &mut Context<Self>) {
        self.downloads.confirm = None;
        // A cancelled Restart to Update: a later quit installs, but doesn't reopen.
        self.updates.relaunch = false;
        cx.notify();
    }

    /// After a library opens: stored rows, unfinished ones paused. Never
    /// starts a download, so opening a library touches no network.
    pub(crate) fn load_downloads(&mut self, cx: &mut Context<Self>) {
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        let generation = self.downloads.generation;
        let load = cx.background_executor().spawn(async move {
            catalog.recover_downloads()?;
            catalog.list_downloads()
        });
        cx.spawn(async move |this, cx| {
            let loaded = load.await;
            let _ = this.update(cx, |app, cx| {
                if app.downloads.generation != generation {
                    return;
                }
                app.downloads.loaded = true;
                match loaded {
                    Ok(rows) => {
                        // Rows added while this loaded stay after the stored ones.
                        let added = std::mem::take(&mut app.downloads.entries);
                        let mut entries: Vec<DownloadEntry> = rows
                            .into_iter()
                            .filter(|row| added.iter().all(|e| e.row.id != row.id))
                            .map(|row| DownloadEntry::new(row, false, None))
                            .collect();
                        entries.extend(added);
                        app.downloads.entries = entries;
                        cx.notify();
                    }
                    Err(err) => {
                        log::error!("loading the download list: {err:#}");
                        app.set_error("Couldn't load your downloads.", cx);
                    }
                }
            });
        })
        .detach();
    }

    /// Resume on a paused row: starts over at the end of the queue.
    pub fn resume_download(&mut self, row_id: i64, cx: &mut Context<Self>) {
        let Some(entry) = self.downloads.entries.iter().find(|e| e.row.id == row_id) else {
            return;
        };
        if entry.row.state == DownloadState::Paused {
            let skip = entry.skip_prior_check;
            self.requeue_download(row_id, skip, cx);
        }
    }

    /// Resume All: every paused row, in list order.
    pub fn resume_all_downloads(&mut self, cx: &mut Context<Self>) {
        let paused: Vec<(i64, bool)> = self
            .downloads
            .entries
            .iter()
            .filter(|entry| entry.row.state == DownloadState::Paused)
            .map(|entry| (entry.row.id, entry.skip_prior_check))
            .collect();
        self.requeue_downloads(paused, cx);
    }

    pub(crate) fn requeue_download(
        &mut self,
        row_id: i64,
        skip_prior_check: bool,
        cx: &mut Context<Self>,
    ) {
        self.requeue_downloads(vec![(row_id, skip_prior_check)], cx);
    }

    /// Starts `(row, skip_prior_check)` rows over at the end of the queue,
    /// in the order given: one background task writes them one after the
    /// other, so each takes the next queue position.
    pub(crate) fn requeue_downloads(&mut self, mut rows: Vec<(i64, bool)>, cx: &mut Context<Self>) {
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        rows.retain(|(row_id, _)| {
            let Some(index) = self.downloads.index_of(*row_id) else {
                return false;
            };
            let entry = &mut self.downloads.entries[index];
            // Only finished-without-success rows start over; a running or
            // waiting one is already on its way.
            let restartable = matches!(
                entry.row.state,
                DownloadState::Failed | DownloadState::Paused | DownloadState::Duplicate
            );
            if entry.requeueing || !restartable {
                return false;
            }
            entry.requeueing = true;
            true
        });
        if rows.is_empty() {
            return;
        }
        let generation = self.downloads.generation;
        let ids: Vec<i64> = rows.iter().map(|(row_id, _)| *row_id).collect();
        let requeue = cx.background_executor().spawn(async move {
            ids.into_iter()
                .map(|row_id| catalog.requeue_download(row_id))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let requeued = requeue.await;
            let _ = this.update(cx, |app, cx| {
                if app.downloads.generation != generation {
                    return;
                }
                let mut failed = false;
                for ((row_id, skip_prior_check), result) in rows.into_iter().zip(requeued) {
                    let Some(index) = app.downloads.index_of(row_id) else {
                        continue;
                    };
                    app.downloads.entries[index].requeueing = false;
                    match result {
                        // The old run saved a file it couldn't catalogue
                        // while this write was on its way: the row stays as
                        // it ended (the catalog keeps it too, see
                        // `Catalog::requeue_download`). Starting over would
                        // download it a second time.
                        Ok(row)
                            if [&row, &app.downloads.entries[index].row]
                                .iter()
                                .any(|r| r.state == DownloadState::Failed && !r.can_retry) => {}
                        Ok(row) => {
                            let mut entry = app.downloads.entries.remove(index);
                            entry.row = row;
                            entry.skip_prior_check = skip_prior_check;
                            // A new run's flag: the old run's ending (or a
                            // stop meant for it) never touches this one.
                            entry.cancel = Arc::new(AtomicBool::new(false));
                            // Placeholders (not stored yet) stay last.
                            let at = app
                                .downloads
                                .entries
                                .iter()
                                .position(|entry| entry.row.id < 0)
                                .unwrap_or(app.downloads.entries.len());
                            app.downloads.entries.insert(at, entry);
                        }
                        Err(err) => {
                            log::error!("requeueing download {row_id}: {err:#}");
                            failed = true;
                        }
                    }
                }
                if failed {
                    app.set_error("Couldn't restart the download. Try again.", cx);
                }
                app.pump_downloads(cx);
            });
        })
        .detach();
    }
}
