//! One queued download from start to finish: inspect the link, check it
//! isn't already in the library, download, catalogue. Runs on its own
//! thread; the scheduler in `super` decides when.

use lumina_core::net_import::{
    helper_missing, inspect_url, NetworkDownloadProgress, NetworkSourceKind, ProgressCallback,
};
use lumina_core::{LuminaEngine, MediaItem, PriorDownload};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What a job is asked to download.
#[derive(Debug, Clone)]
pub(crate) struct JobRequest {
    pub row_id: i64,
    pub url: String,
    pub kind: NetworkSourceKind,
    /// Download Again: the user already saw it is in the library.
    pub skip_prior_check: bool,
}

/// Reported while a job runs.
#[derive(Debug, Clone)]
pub(crate) enum JobEvent {
    Inspected {
        canonical_url: String,
        title: String,
    },
    Progress(NetworkDownloadProgress),
}

/// How a job ended.
#[derive(Debug)]
pub(crate) enum JobOutcome {
    /// Catalogued; the timeline refresh may still have failed.
    Imported {
        item: Box<MediaItem>,
        timeline_error: Option<String>,
    },
    /// The inspected address is already in the library.
    AlreadyInLibrary(PriorDownload),
    Cancelled,
    /// The top message, written for people; the chain is logged.
    Failed(String),
    /// A bundled helper is missing: Retry can't fix it.
    MissingHelper,
    /// On disk but not in the catalog: kept, so the download is not lost.
    NotCatalogued {
        path: PathBuf,
    },
}

/// Runs one job. Tests replace it so nothing touches the network.
pub(crate) type JobFn = Arc<
    dyn Fn(&JobRequest, &AtomicBool, &async_channel::Sender<JobEvent>) -> JobOutcome + Send + Sync,
>;

/// The real job: the existing `net_import` steps, in order.
pub(crate) fn real_job(engine: Arc<LuminaEngine>) -> JobFn {
    Arc::new(move |request, cancel, events| {
        if helper_missing(request.kind) {
            return JobOutcome::MissingHelper;
        }
        let failed = |err: anyhow::Error| {
            if cancel.load(Ordering::Relaxed) {
                return JobOutcome::Cancelled;
            }
            log::warn!("downloading {}: {err:#}", request.url);
            JobOutcome::Failed(err.to_string())
        };
        let info = match inspect_url(&request.url, cancel) {
            Ok(info) => info,
            Err(err) => return failed(err),
        };
        let _ = events.send_blocking(JobEvent::Inspected {
            canonical_url: info.url.clone(),
            title: info.title.clone(),
        });
        if !request.skip_prior_check {
            match engine.find_prior_download(&info.url) {
                Ok(Some(prior)) => return JobOutcome::AlreadyInLibrary(prior),
                Ok(None) => {}
                Err(err) => log::warn!("looking up earlier downloads of {}: {err:#}", info.url),
            }
        }
        let progress_events = events.clone();
        let progress: ProgressCallback = Box::new(move |progress| {
            let _ = progress_events.send_blocking(JobEvent::Progress(progress));
        });
        let path = match engine.download_network_media(&info, cancel, &progress) {
            Ok(path) => path,
            Err(err) => return failed(err),
        };
        match engine.ingest_network_imported_file(&path, &info, false) {
            Ok(ingest) => JobOutcome::Imported {
                item: Box::new(ingest.item),
                timeline_error: ingest.timeline_error,
            },
            Err(err) => {
                log::error!("cataloguing {}: {err:#}", path.display());
                JobOutcome::NotCatalogued { path }
            }
        }
    })
}
