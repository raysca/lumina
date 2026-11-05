//! The list across quits and library switches: ask before cutting running
//! downloads short, bring stored rows back paused, resume on request.

use gpui::*;
use lumina_core::DownloadState;
use std::path::PathBuf;

use super::DownloadEntry;
use crate::LuminaApp;

/// What the "downloads are still running" prompt goes on to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DownloadsConfirm {
    Quit,
    OpenLibrary(PathBuf),
}

impl LuminaApp {
    /// Downloading and waiting rows: what quitting or switching library
    /// would cut short. Paused rows wait for the user.
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

    /// True (and the prompt shows) when downloads would be cut short.
    pub(crate) fn ask_before_leaving(
        &mut self,
        then: DownloadsConfirm,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.running_or_waiting_count() == 0 {
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
                match loaded {
                    Ok(rows) => {
                        // Rows added while this loaded stay after the stored ones.
                        let added = std::mem::take(&mut app.downloads.entries);
                        let mut entries: Vec<DownloadEntry> = rows
                            .into_iter()
                            .filter(|row| added.iter().all(|e| e.row.id != row.id))
                            .map(|row| DownloadEntry::new(row, false))
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
        let paused: Vec<i64> = self
            .downloads
            .entries
            .iter()
            .filter(|entry| entry.row.state == DownloadState::Paused)
            .map(|entry| entry.row.id)
            .collect();
        // ponytail: one write each; the catalog lock keeps them apart, but
        // they may land out of order. One batched write if order matters.
        for row_id in paused {
            self.resume_download(row_id, cx);
        }
    }
}
