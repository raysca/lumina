//! The URL download queue: up to the Simultaneous downloads setting
//! (`AppConfig::download_concurrency`) jobs at once, the rest waiting in
//! the order they were added. Lowering the setting never cancels a running
//! job; it only holds back new starts. Rows live in the catalog
//! (`download_queue`) so they survive a restart; progress is memory only.
//!
//! Every catalog write runs off the UI thread. Writes for a finished job
//! happen on its own thread, against the catalog it started with, so a job
//! that outlives a library switch never touches the new library.

mod job;
mod lifecycle;
mod panel;
mod pause;
mod screen;
mod speed;

pub(crate) use job::{real_job, JobEvent, JobFn, JobOutcome, JobRequest};
pub(crate) use lifecycle::DownloadsConfirm;
pub(crate) use panel::{DownloadFilter, DownloadSurface};
pub(crate) use screen::row_menu_items;
pub(crate) use speed::{SpeedHistory, SAMPLES as SPEED_SAMPLES};

use gpui::*;
use lumina_core::net_import::{
    classify_url, missing_part, NetworkDownloadProgress, NetworkMediaInfo,
};
use lumina_core::{Catalog, DownloadRow, DownloadState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::LuminaApp;

/// A job silent this long has stalled: its last speed no longer shows.
/// Nothing samples between job events, so a stalled job would otherwise
/// keep its last speed (and time left) on screen.
const STALE_SPEED: Duration = Duration::from_secs(5);

/// One row of the list, with what only the running app knows.
pub(crate) struct DownloadEntry {
    pub row: DownloadRow,
    pub progress: Option<NetworkDownloadProgress>,
    /// Clears the speed [`STALE_SPEED`] after the last progress event.
    /// Replaced by each event and dropped with the row or its progress, so
    /// it never fires for a removed, finished or restarted run.
    stale_wait: Option<Task<()>>,
    /// Download Again: skip the "already in your library" check.
    pub skip_prior_check: bool,
    /// The dialog's inspection, handed to the first run of the job.
    info: Option<NetworkMediaInfo>,
    cancel: Arc<AtomicBool>,
    /// The cancel flag of this row's job thread while one is alive. A
    /// paused (or resumed) row keeps it until that job really ends: it
    /// still holds a slot, and the row's next run waits for it.
    live_job: Option<Arc<AtomicBool>>,
    /// A Retry/Resume write is on its way; repeat presses wait for it.
    requeueing: bool,
}

impl DownloadEntry {
    fn new(row: DownloadRow, skip_prior_check: bool, info: Option<NetworkMediaInfo>) -> Self {
        Self {
            row,
            progress: None,
            stale_wait: None,
            skip_prior_check,
            info,
            cancel: Arc::new(AtomicBool::new(false)),
            live_job: None,
            requeueing: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(row: DownloadRow) -> Self {
        Self::new(row, false, None)
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
    /// The stored rows have loaded (or failed to) since the library opened.
    pub loaded: bool,
    /// The Downloads screen's chip. Session only, never saved.
    pub filter: DownloadFilter,
    /// Combined speed, one sample a second, for the throughput bars.
    pub speed: SpeedHistory,
    /// Ids for rows whose insert hasn't returned yet (always negative).
    next_placeholder: i64,
    /// The "still running" prompt before quitting or switching library.
    pub confirm: Option<DownloadsConfirm>,
    /// The dropdown body, so keyboard focus can scroll into view.
    pub scroll: ScrollHandle,
    /// Row with keyboard focus on the Downloads screen, and its list.
    pub screen_focused: Option<i64>,
    pub screen_scroll: ScrollHandle,
    /// A completed row's "…" menu: the row and the highlighted item.
    pub row_menu: Option<(i64, usize)>,
    /// A Move up is being stored; another waits until it lands.
    pub moving: bool,
    /// Set where the app would quit (the test platform's quit does nothing).
    #[cfg(test)]
    pub quit_requested: bool,
}

impl DownloadQueue {
    fn index_of(&self, id: i64) -> Option<usize> {
        self.entries.iter().position(|entry| entry.row.id == id)
    }

    pub(crate) fn active_for(&self, url: &str) -> Option<i64> {
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
        self.loaded = false;
        self.filter = DownloadFilter::All;
        self.speed = SpeedHistory::default();
        self.screen_focused = None;
        self.row_menu = None;
        self.moving = false;
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
    /// The run's cancel flag: tells this run from a later one of the row.
    cancel: Arc<AtomicBool>,
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

    /// Removes and returns the queued rows matching `take`, leaving the rest.
    ///
    /// Every clear action is "partition the queue, then delete the taken rows
    /// from the catalog"; sharing the partition keeps the two halves of that
    /// contract in one place. Which states are taken stays at each call site.
    pub(crate) fn take_download_entries(
        &mut self,
        mut take: impl FnMut(&DownloadEntry) -> bool,
    ) -> Vec<DownloadEntry> {
        let (taken, kept) = std::mem::take(&mut self.downloads.entries)
            .into_iter()
            .partition(|entry| take(entry));
        self.downloads.entries = kept;
        taken
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

    /// Adds `url` to the end of the queue and starts it when a slot is free.
    /// `info` is the dialog's inspection of the link, if it made one.
    pub fn enqueue_download(
        &mut self,
        url: String,
        skip_prior_check: bool,
        info: Option<NetworkMediaInfo>,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.downloads.active_for(&url) {
            self.reveal_download(id, cx);
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
            info,
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

    /// Starts waiting rows, oldest first, until the Simultaneous downloads
    /// setting's number run.
    pub(crate) fn pump_downloads(&mut self, cx: &mut Context<Self>) {
        let (Some(job), Some(catalog)) = (self.downloads.job.clone(), self.download_catalog())
        else {
            return;
        };
        // Running rows, and paused or resumed rows whose job hasn't ended.
        let mut running = self
            .downloads
            .entries
            .iter()
            .filter(|entry| entry.live_job.is_some())
            .count();
        for index in 0..self.downloads.entries.len() {
            if running >= self.config.download_concurrency {
                break;
            }
            let entry = &self.downloads.entries[index];
            if entry.row.state != DownloadState::Waiting
                || entry.row.id < 0
                || entry.live_job.is_some()
            {
                continue;
            }
            self.start_download(index, job.clone(), catalog.clone(), cx);
            running += 1;
        }
        // The last one ended: a later run starts with no old bars.
        if running == 0 {
            self.downloads.speed = SpeedHistory::default();
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
        entry.stale_wait = None;
        entry.cancel = Arc::new(AtomicBool::new(false));
        let request = JobRequest {
            row_id: entry.row.id,
            url: entry.row.url.clone(),
            kind,
            skip_prior_check: entry.skip_prior_check,
            // Only the first run: Retry and Resume inspect again.
            info: entry.info.take(),
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
                    cancel,
                });
            });
        if let Err(err) = spawned {
            log::error!("spawning a download thread: {err:#}");
            let entry = &mut self.downloads.entries[index];
            entry.row.state = DownloadState::Waiting;
            self.set_error("Couldn't start the download. Try again.", cx);
            return;
        }

        let entry = &mut self.downloads.entries[index];
        entry.live_job = Some(entry.cancel.clone());
        let run = entry.cancel.clone();
        let row_id = entry.row.id;
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
                // The thread died without reporting (a panic): it holds no
                // slot any more.
                let _ = this.update(cx, |app, cx| {
                    if app.downloads.generation == generation {
                        app.end_live_job(row_id, &run);
                        app.pump_downloads(cx);
                    }
                });
                return;
            };
            let _ = this.update(cx, |app, cx| app.finish_download(generation, finished, cx));
        })
        .detach();
    }

    pub(crate) fn apply_job_events(
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
                JobEvent::Progress(progress) => {
                    entry.progress = Some(progress);
                    entry.stale_wait = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(STALE_SPEED).await;
                        let _ = this.update(cx, |app, cx| app.clear_stalled_speed(row_id, cx));
                    }));
                }
            }
        }
        // Progress events are the clock: no timer runs while nothing does.
        let combined = self.download_speed();
        self.downloads.speed.record(Instant::now(), combined);
        cx.notify();
    }

    /// The job of `row_id` reported nothing for [`STALE_SPEED`]: its speed
    /// goes, and the screen redraws without it.
    fn clear_stalled_speed(&mut self, row_id: i64, cx: &mut Context<Self>) {
        let Some(index) = self.downloads.index_of(row_id) else {
            return;
        };
        let entry = &mut self.downloads.entries[index];
        if let Some(progress) = entry.progress.as_mut() {
            if progress.speed_bytes_per_sec > 0 {
                progress.speed_bytes_per_sec = 0;
                // The bars drop with the speed, or the last one stays up.
                let combined = self.download_speed();
                self.downloads.speed.record(Instant::now(), combined);
                cx.notify();
            }
        }
    }

    fn finish_download(&mut self, generation: u64, finished: Finished, cx: &mut Context<Self>) {
        if self.downloads.generation != generation {
            return;
        }
        self.end_live_job(finished.row.id, &finished.cancel);
        // Removed (✕) while it ran, or paused and resumed so a later run
        // owns the row (a step that ignores the cancel flag can end this
        // run late): the row is left alone, but a file that made it into
        // the catalog still shows in the library. This run's stored result
        // is overwritten by the later run's own writes when it ends; until
        // then it says Done for a file that really was catalogued.
        // A file saved but not catalogued is kept on the row with its
        // explanation: a later run can't have started (it waits for this
        // one), and starting over would download it a second time.
        let current = self.downloads.index_of(finished.row.id).filter(|&at| {
            Arc::ptr_eq(&self.downloads.entries[at].cancel, &finished.cancel)
                || matches!(finished.outcome, JobOutcome::NotCatalogued { .. })
        });
        let Some(index) = current else {
            if matches!(finished.outcome, JobOutcome::Imported { .. }) {
                self.refresh_album_counts();
                self.refresh_active_view(cx);
            }
            self.pump_downloads(cx);
            return;
        };
        let entry = &mut self.downloads.entries[index];
        if matches!(finished.outcome, JobOutcome::Cancelled) {
            // Paused while it ran: store it now that the job can write no
            // more. With a Resume write in flight, that write stores the row.
            // A Resume pressed after this still converges: its run's thread
            // stores Running first, and a row left waiting reopens paused.
            if entry.row.state == DownloadState::Paused && !entry.requeueing {
                let row = entry.row.clone();
                self.store_paused(vec![row], false, cx);
            }
            self.pump_downloads(cx);
            return;
        }
        // The job's copy of the row predates its inspection.
        let mut row = finished.row;
        row.title = row.title.or(entry.row.title.take());
        row.canonical_url = row.canonical_url.or(entry.row.canonical_url.take());
        entry.row = row;
        entry.progress = None;
        entry.stale_wait = None;
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

    /// The run with cancel flag `run` of row `row_id` has ended: its slot
    /// frees, unless a later run already holds the row.
    fn end_live_job(&mut self, row_id: i64, run: &Arc<AtomicBool>) {
        if let Some(at) = self.downloads.index_of(row_id) {
            let entry = &mut self.downloads.entries[at];
            if entry
                .live_job
                .as_ref()
                .is_some_and(|live| Arc::ptr_eq(live, run))
            {
                entry.live_job = None;
            }
        }
    }

    /// ✕ and ⌫: cancels a running download, or removes any row. The next
    /// waiting download moves up.
    pub fn cancel_or_remove_download(&mut self, row_id: i64, cx: &mut Context<Self>) {
        let Some(index) = self.downloads.index_of(row_id) else {
            return;
        };
        // The row leaves the list, so its slot frees at once, even while a
        // paused row's job is still finishing.
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
                            // Its job may have ended while the row was out
                            // of the list, unseen: it holds no slot.
                            entry.live_job = None;
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
            row.error = Some(missing_part().to_string());
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
    // "last tried 14:05" on a failed row; Done rows show when they ended.
    if row.state == DownloadState::Failed {
        row.finished_epoch_ms = Some(now_ms());
    }
    true
}
