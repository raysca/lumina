//! The URL download queue: up to [`MAX_RUNNING`] jobs at once, the rest
//! waiting in the order they were added. Rows live in the catalog
//! (`download_queue`) so they survive a restart; progress is memory only.
//!
//! Every catalog write runs off the UI thread. Writes for a finished job
//! happen on its own thread, against the catalog it started with, so a job
//! that outlives a library switch never touches the new library.

mod job;
mod lifecycle;
mod panel;

pub(crate) use job::{real_job, JobEvent, JobFn, JobOutcome, JobRequest};
pub(crate) use lifecycle::DownloadsConfirm;

use gpui::*;
use lumina_core::net_import::{classify_url, NetworkDownloadProgress, MISSING_PART};
use lumina_core::{Catalog, DownloadRow, DownloadState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::LuminaApp;

/// Downloads that run at the same time.
pub(crate) const MAX_RUNNING: usize = 3;

/// One row of the list, with what only the running app knows.
pub(crate) struct DownloadEntry {
    pub row: DownloadRow,
    pub progress: Option<NetworkDownloadProgress>,
    /// Download Again: skip the "already in your library" check.
    pub skip_prior_check: bool,
    cancel: Arc<AtomicBool>,
    /// A Retry/Resume write is on its way; repeat presses wait for it.
    requeueing: bool,
}

impl DownloadEntry {
    fn new(row: DownloadRow, skip_prior_check: bool) -> Self {
        Self {
            row,
            progress: None,
            skip_prior_check,
            cancel: Arc::new(AtomicBool::new(false)),
            requeueing: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(row: DownloadRow) -> Self {
        Self::new(row, false)
    }

    fn is_active(&self) -> bool {
        matches!(
            self.row.state,
            DownloadState::Waiting | DownloadState::Running | DownloadState::Paused
        )
    }

    fn matches_url(&self, url: &str) -> bool {
        self.row.url == url || self.row.canonical_url.as_deref() == Some(url)
    }
}

#[derive(Default)]
pub(crate) struct DownloadQueue {
    /// In queue order.
    pub entries: Vec<DownloadEntry>,
    /// The dropdown is showing.
    pub open: bool,
    /// A download failed since the list was last opened (the red dot).
    pub unseen_failure: bool,
    /// Row with keyboard focus in the dropdown.
    pub focused: Option<i64>,
    /// Row shown as "Already in your downloads".
    pub highlight: Option<i64>,
    /// Bumped when the library changes; results of older jobs are dropped.
    pub generation: u64,
    /// `None` until a library opens. Tests replace it.
    pub job: Option<JobFn>,
    /// Ids for rows whose insert hasn't returned yet (always negative).
    next_placeholder: i64,
    /// The "still running" prompt before quitting or switching library.
    pub confirm: Option<DownloadsConfirm>,
    /// The dropdown body, so keyboard focus can scroll into view.
    pub scroll: ScrollHandle,
    /// Set where the app would quit (the test platform's quit does nothing).
    #[cfg(test)]
    pub quit_requested: bool,
}

impl DownloadQueue {
    fn index_of(&self, id: i64) -> Option<usize> {
        self.entries.iter().position(|entry| entry.row.id == id)
    }

    fn active_for(&self, url: &str) -> Option<i64> {
        self.entries
            .iter()
            .find(|entry| entry.is_active() && entry.matches_url(url))
            .map(|entry| entry.row.id)
    }

    /// Switching libraries: stop every job and forget the old library's rows.
    pub(crate) fn reset(&mut self, job: Option<JobFn>) {
        for entry in &self.entries {
            entry.cancel.store(true, Ordering::Relaxed);
        }
        self.entries.clear();
        self.open = false;
        self.unseen_failure = false;
        self.focused = None;
        self.highlight = None;
        self.confirm = None;
        self.generation += 1;
        self.job = job;
    }
}

/// Closing the window (or a test ending) stops every job, so no thread keeps
/// downloading for a library nobody shows.
impl Drop for DownloadQueue {
    fn drop(&mut self) {
        for entry in &self.entries {
            entry.cancel.store(true, Ordering::Relaxed);
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// What the job's thread hands back to the UI.
struct Finished {
    row: DownloadRow,
    outcome: JobOutcome,
    /// The row couldn't be stored; the details are in the log.
    write_failed: bool,
}

impl LuminaApp {
    /// The open library's catalog. Background work holds this, never the
    /// engine: a pending task holding the engine keeps its event thread
    /// alive, and opening another library joins that thread.
    pub(super) fn download_catalog(&self) -> Option<Catalog> {
        self.grid_state
            .as_ref()
            .map(|state| state.engine.catalog().clone())
    }

    /// The toolbar badge: downloading, waiting and paused rows, so after a
    /// reopen the icon shows there is unfinished work.
    pub(crate) fn download_badge_count(&self) -> usize {
        self.downloads
            .entries
            .iter()
            .filter(|entry| entry.is_active())
            .count()
    }

    /// The dialog's Import: checks the link, then either queues it, points
    /// at the row already in the list, or asks about an earlier download.
    pub fn submit_network_url(&mut self, url: String, cx: &mut Context<Self>) {
        use crate::app::network_import::NetworkImportStage;
        if matches!(
            self.network_import.stage,
            Some(NetworkImportStage::Checking { .. })
        ) {
            return;
        }
        let url = url.trim().to_string();
        if url.is_empty() {
            self.set_network_input_error(url, "Enter a video link".to_string(), cx);
            return;
        }
        if let Err(err) = classify_url(&url) {
            self.set_network_input_error(url, err.to_string(), cx);
            return;
        }
        if let Some(id) = self.downloads.active_for(&url) {
            self.network_import.stage = None;
            self.downloads.open = true;
            self.downloads.highlight = Some(id);
            cx.notify();
            return;
        }
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        self.network_import.stage = Some(NetworkImportStage::Checking { url: url.clone() });
        cx.notify();

        let lookup_url = url.clone();
        let lookup = cx
            .background_executor()
            .spawn(async move { catalog.find_prior_download(&lookup_url) });
        cx.spawn(async move |this, cx| {
            let prior = lookup.await;
            let _ = this.update(cx, |app, cx| {
                // The dialog was closed or changed meanwhile.
                if !matches!(
                    &app.network_import.stage,
                    Some(NetworkImportStage::Checking { url: checking }) if *checking == url
                ) {
                    return;
                }
                match prior {
                    Ok(Some(prior)) => {
                        app.network_import.stage =
                            Some(NetworkImportStage::AlreadyDownloaded { url, prior });
                    }
                    other => {
                        if let Err(err) = other {
                            log::warn!("looking up earlier downloads of {url}: {err:#}");
                        }
                        app.network_import.stage = None;
                        app.enqueue_download(url, false, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Adds `url` to the end of the queue and starts it when a slot is free.
    pub fn enqueue_download(
        &mut self,
        url: String,
        skip_prior_check: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.downloads.active_for(&url) {
            self.downloads.open = true;
            self.downloads.highlight = Some(id);
            cx.notify();
            return;
        }
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        // A placeholder until the insert returns, so a second press of
        // Import sees this URL is already queued.
        self.downloads.next_placeholder -= 1;
        let placeholder = self.downloads.next_placeholder;
        self.downloads.entries.push(DownloadEntry::new(
            DownloadRow {
                id: placeholder,
                url: url.clone(),
                canonical_url: None,
                title: None,
                state: DownloadState::Waiting,
                error: None,
                can_retry: true,
                media_id: None,
                added_epoch_ms: now_ms(),
                finished_epoch_ms: None,
                in_library: false,
            },
            skip_prior_check,
        ));
        cx.notify();

        let generation = self.downloads.generation;
        let insert = cx.background_executor().spawn({
            let catalog = catalog.clone();
            async move { catalog.add_download(&url) }
        });
        cx.spawn(async move |this, cx| {
            let inserted = insert.await;
            let _ = this.update(cx, |app, cx| {
                if app.downloads.generation != generation {
                    return;
                }
                let Some(index) = app.downloads.index_of(placeholder) else {
                    // Removed before it was stored: remove the stored row too.
                    if let Ok(row) = inserted {
                        app.remove_stored_download(catalog, row.id, cx);
                    }
                    return;
                };
                match inserted {
                    Ok(row) => {
                        app.downloads.entries[index].row = row;
                        app.pump_downloads(cx);
                    }
                    Err(err) => {
                        log::error!("queueing a download: {err:#}");
                        app.downloads.entries.remove(index);
                        if app.downloads.entries.is_empty() {
                            app.close_downloads_panel(cx);
                        }
                        app.set_error("Couldn't add the download. Try again.", cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn remove_stored_download(&self, catalog: Catalog, id: i64, cx: &mut Context<Self>) {
        cx.background_executor()
            .spawn(async move {
                if let Err(err) = catalog.remove_download(id) {
                    log::warn!("removing download {id}: {err:#}");
                }
            })
            .detach();
    }

    /// Starts waiting rows, oldest first, until [`MAX_RUNNING`] run.
    pub(crate) fn pump_downloads(&mut self, cx: &mut Context<Self>) {
        let (Some(job), Some(catalog)) = (self.downloads.job.clone(), self.download_catalog())
        else {
            return;
        };
        let mut running = self
            .downloads
            .entries
            .iter()
            .filter(|entry| entry.row.state == DownloadState::Running)
            .count();
        for index in 0..self.downloads.entries.len() {
            if running >= MAX_RUNNING {
                break;
            }
            let entry = &self.downloads.entries[index];
            if entry.row.state != DownloadState::Waiting || entry.row.id < 0 {
                continue;
            }
            self.start_download(index, job.clone(), catalog.clone(), cx);
            running += 1;
        }
        cx.notify();
    }

    fn start_download(
        &mut self,
        index: usize,
        job: JobFn,
        catalog: Catalog,
        cx: &mut Context<Self>,
    ) {
        let entry = &mut self.downloads.entries[index];
        let Ok(kind) = classify_url(&entry.row.url) else {
            // Only checked URLs are queued; a stored row could predate that.
            entry.row.state = DownloadState::Failed;
            entry.row.error = Some("This link can't be downloaded.".to_string());
            entry.row.can_retry = false;
            return;
        };
        entry.row.state = DownloadState::Running;
        entry.progress = None;
        entry.cancel = Arc::new(AtomicBool::new(false));
        let request = JobRequest {
            row_id: entry.row.id,
            url: entry.row.url.clone(),
            kind,
            skip_prior_check: entry.skip_prior_check,
        };
        let cancel = entry.cancel.clone();
        let mut row = entry.row.clone();
        let generation = self.downloads.generation;
        let (events_tx, events_rx) = async_channel::unbounded::<JobEvent>();
        let (done_tx, done_rx) = async_channel::bounded::<Finished>(1);

        // A dedicated thread: a download can take hours and must not hold
        // a shared executor worker.
        let spawned = std::thread::Builder::new()
            .name(format!("lumina-download-{}", request.row_id))
            .spawn(move || {
                let mut write_failed = false;
                if let Err(err) = catalog.update_download(&row) {
                    log::error!("storing download {}: {err:#}", row.id);
                    write_failed = true;
                }
                let outcome = job(&request, &cancel, &events_tx);
                drop(events_tx);
                if apply_outcome(&mut row, &outcome) {
                    if let Err(err) = catalog.update_download(&row) {
                        log::error!("storing download {}: {err:#}", row.id);
                        write_failed = true;
                    }
                }
                let _ = done_tx.send_blocking(Finished {
                    row,
                    outcome,
                    write_failed,
                });
            });
        if let Err(err) = spawned {
            log::error!("spawning a download thread: {err:#}");
            let entry = &mut self.downloads.entries[index];
            entry.row.state = DownloadState::Waiting;
            self.set_error("Couldn't start the download. Try again.", cx);
            return;
        }

        let row_id = self.downloads.entries[index].row.id;
        cx.spawn(async move |this, cx| {
            while let Ok(first) = events_rx.recv().await {
                // Coalesce: one notify for everything queued so far.
                let mut batch = vec![first];
                while let Ok(more) = events_rx.try_recv() {
                    batch.push(more);
                }
                let _ = this.update(cx, |app, cx| {
                    app.apply_job_events(generation, row_id, batch, cx)
                });
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            let Ok(finished) = done_rx.recv().await else {
                return;
            };
            let _ = this.update(cx, |app, cx| app.finish_download(generation, finished, cx));
        })
        .detach();
    }

    fn apply_job_events(
        &mut self,
        generation: u64,
        row_id: i64,
        events: Vec<JobEvent>,
        cx: &mut Context<Self>,
    ) {
        if self.downloads.generation != generation {
            return;
        }
        let Some(index) = self.downloads.index_of(row_id) else {
            return;
        };
        let catalog = self.download_catalog();
        let entry = &mut self.downloads.entries[index];
        for event in events {
            match event {
                JobEvent::Inspected {
                    canonical_url,
                    title,
                } => {
                    entry.row.canonical_url = Some(canonical_url.clone());
                    entry.row.title = Some(title.clone());
                    let Some(catalog) = catalog.clone() else {
                        continue;
                    };
                    cx.background_executor()
                        .spawn(async move {
                            if let Err(err) =
                                catalog.set_download_title(row_id, &canonical_url, &title)
                            {
                                log::warn!("storing the title of download {row_id}: {err:#}");
                            }
                        })
                        .detach();
                }
                JobEvent::Progress(progress) => entry.progress = Some(progress),
            }
        }
        cx.notify();
    }

    fn finish_download(&mut self, generation: u64, finished: Finished, cx: &mut Context<Self>) {
        if self.downloads.generation != generation {
            return;
        }
        // Removed (✕) while it ran: the row stays gone, but a file that
        // made it into the catalog still shows in the library.
        let Some(index) = self.downloads.index_of(finished.row.id) else {
            if matches!(finished.outcome, JobOutcome::Imported { .. }) {
                self.refresh_album_counts();
                self.refresh_active_view(cx);
            }
            return;
        };
        if matches!(finished.outcome, JobOutcome::Cancelled) {
            return;
        }
        let entry = &mut self.downloads.entries[index];
        // The job's copy of the row predates its inspection.
        let mut row = finished.row;
        row.title = row.title.or(entry.row.title.take());
        row.canonical_url = row.canonical_url.or(entry.row.canonical_url.take());
        entry.row = row;
        entry.progress = None;
        match finished.outcome {
            JobOutcome::Imported {
                item,
                timeline_error,
            } => {
                self.show_toast(format!("Imported \"{}\"", item.file_name), cx);
                if timeline_error.is_some() {
                    self.set_error(
                        // The details are in the log (see `ingest_network_imported_file`).
                        format!(
                            "Imported \"{}\". The timeline will update after Lumina restarts.",
                            item.file_name
                        ),
                        cx,
                    );
                }
                self.refresh_album_counts();
                self.refresh_active_view(cx);
            }
            JobOutcome::Failed(_)
            | JobOutcome::MissingHelper
            | JobOutcome::NotCatalogued { .. } => {
                if !self.downloads.open {
                    self.downloads.unseen_failure = true;
                }
            }
            JobOutcome::AlreadyInLibrary(_) | JobOutcome::Cancelled => {}
        }
        if finished.write_failed {
            self.set_error(
                "Lumina couldn't save the download list. It may be out of date after a restart.",
                cx,
            );
        }
        self.pump_downloads(cx);
    }

    /// ✕ and ⌫: cancels a running download, or removes any row. The next
    /// waiting download moves up.
    pub fn cancel_or_remove_download(&mut self, row_id: i64, cx: &mut Context<Self>) {
        let Some(index) = self.downloads.index_of(row_id) else {
            return;
        };
        let entry = self.downloads.entries.remove(index);
        entry.cancel.store(true, Ordering::Relaxed);
        if self.downloads.highlight == Some(row_id) {
            self.downloads.highlight = None;
        }
        // No rows, no Downloads button to hang the list from: close it, as
        // Clear finished does.
        if self.downloads.entries.is_empty() {
            self.close_downloads_panel(cx);
        }
        // A placeholder is removed once its insert returns.
        if let (true, Some(catalog)) = (row_id > 0, self.download_catalog()) {
            let generation = self.downloads.generation;
            let removal = cx
                .background_executor()
                .spawn(async move { catalog.remove_download(row_id) });
            let mut entry = Some(entry);
            cx.spawn(async move |this, cx| {
                if let Err(err) = removal.await {
                    log::error!("removing download {row_id}: {err:#}");
                    let _ = this.update(cx, |app, cx| {
                        if app.downloads.generation != generation {
                            return;
                        }
                        if let Some(mut entry) = entry.take() {
                            // Its job was stopped: it can be resumed.
                            if entry.row.state == DownloadState::Running {
                                entry.row.state = DownloadState::Paused;
                            }
                            let at = index.min(app.downloads.entries.len());
                            app.downloads.entries.insert(at, entry);
                        }
                        app.set_error("Couldn't remove the download. Try again.", cx);
                    });
                }
            })
            .detach();
        }
        self.pump_downloads(cx);
    }

    /// Retry on a failed row: back to the end of the queue.
    pub fn retry_download(&mut self, row_id: i64, cx: &mut Context<Self>) {
        self.requeue_download(row_id, false, cx);
    }

    /// Download Again on a row found to be in the library already.
    pub fn download_again(&mut self, row_id: i64, cx: &mut Context<Self>) {
        self.requeue_download(row_id, true, cx);
    }

    pub(crate) fn requeue_download(
        &mut self,
        row_id: i64,
        skip_prior_check: bool,
        cx: &mut Context<Self>,
    ) {
        let (Some(index), Some(catalog)) =
            (self.downloads.index_of(row_id), self.download_catalog())
        else {
            return;
        };
        let entry = &mut self.downloads.entries[index];
        // Only finished-without-success rows start over; a running or
        // waiting one is already on its way.
        let restartable = matches!(
            entry.row.state,
            DownloadState::Failed | DownloadState::Paused | DownloadState::Duplicate
        );
        if entry.requeueing || !restartable {
            return;
        }
        entry.requeueing = true;
        let generation = self.downloads.generation;
        let requeue = cx
            .background_executor()
            .spawn(async move { catalog.requeue_download(row_id) });
        cx.spawn(async move |this, cx| {
            let requeued = requeue.await;
            let _ = this.update(cx, |app, cx| {
                if app.downloads.generation != generation {
                    return;
                }
                let Some(index) = app.downloads.index_of(row_id) else {
                    return;
                };
                app.downloads.entries[index].requeueing = false;
                match requeued {
                    Ok(row) => {
                        let mut entry = app.downloads.entries.remove(index);
                        entry.row = row;
                        entry.skip_prior_check = skip_prior_check;
                        // Placeholders (not stored yet) stay last.
                        let at = app
                            .downloads
                            .entries
                            .iter()
                            .position(|entry| entry.row.id < 0)
                            .unwrap_or(app.downloads.entries.len());
                        app.downloads.entries.insert(at, entry);
                        app.pump_downloads(cx);
                    }
                    Err(err) => {
                        log::error!("requeueing download {row_id}: {err:#}");
                        app.set_error("Couldn't restart the download. Try again.", cx);
                    }
                }
            });
        })
        .detach();
    }
}

/// Moves `row` to where `outcome` leaves it. False when nothing should be
/// stored (a cancelled job's row was removed or is resumed later).
fn apply_outcome(row: &mut DownloadRow, outcome: &JobOutcome) -> bool {
    match outcome {
        JobOutcome::Imported { item, .. } => {
            row.state = DownloadState::Done;
            row.media_id = Some(item.id);
            row.in_library = true;
            row.finished_epoch_ms = Some(now_ms());
        }
        JobOutcome::AlreadyInLibrary(prior) => {
            row.state = DownloadState::Duplicate;
            row.media_id = Some(prior.media_id);
            row.in_library = true;
            row.finished_epoch_ms = Some(now_ms());
        }
        JobOutcome::Failed(message) => {
            row.state = DownloadState::Failed;
            row.error = Some(message.clone());
            row.can_retry = true;
        }
        JobOutcome::MissingHelper => {
            row.state = DownloadState::Failed;
            row.error = Some(MISSING_PART.to_string());
            row.can_retry = false;
        }
        JobOutcome::NotCatalogued { path } => {
            row.state = DownloadState::Failed;
            row.error = Some(format!(
                "The video was saved as \"{}\" in your Imports folder but couldn't be added to Lumina. Rescan the library to add it.",
                path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
            ));
            row.can_retry = false;
        }
        JobOutcome::Cancelled => return false,
    }
    true
}
