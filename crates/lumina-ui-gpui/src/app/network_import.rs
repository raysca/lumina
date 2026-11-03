//! Network video import state and asynchronous actions. All network, file
//! and catalog work runs off the UI thread; see `lumina_core::net_import`.

use gpui::*;
use lumina_core::net_import::{
    classify_url, helper_missing, inspect_url, DownloadPhase, NetworkDownloadProgress,
    NetworkMediaInfo, ProgressCallback,
};
use lumina_core::MediaItem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::LuminaApp;

/// The stages of the Network Import modal flow.
#[derive(Debug, Clone)]
pub(crate) enum NetworkImportStage {
    /// URL text entry.
    Input { url: String, error: Option<String> },
    /// Fetching stream info / metadata off the UI thread.
    Inspecting { url: String },
    /// Inspected and ready to confirm download options.
    Ready {
        info: NetworkMediaInfo,
        target_path: PathBuf,
        is_favorite: bool,
        /// A library file already downloaded from this URL.
        already_imported: Option<PathBuf>,
    },
    /// Downloading in the background with live progress.
    Downloading {
        title: String,
        progress: NetworkDownloadProgress,
        /// Esc was pressed once; a second press stops the download.
        confirm_cancel: bool,
    },
    /// A helper this URL needs is not installed.
    MissingHelper { url: String },
}

#[derive(Default)]
pub(crate) struct NetworkImportState {
    pub stage: Option<NetworkImportStage>,
    pub cancel_flag: Arc<AtomicBool>,
}

impl NetworkImportState {
    pub fn is_busy(&self) -> bool {
        matches!(
            self.stage,
            Some(NetworkImportStage::Inspecting { .. })
                | Some(NetworkImportStage::Downloading { .. })
        )
    }
}

/// How a download finished, decided on the download thread.
enum Outcome {
    /// Catalogued; the timeline refresh may still have failed.
    Imported {
        item: Box<MediaItem>,
        timeline_error: Option<String>,
    },
    Cancelled,
    Failed(String),
    /// On disk but not in the catalog: kept, so the download is not lost.
    NotCatalogued {
        path: PathBuf,
    },
}

impl LuminaApp {
    /// Opens the "Import Video from URL" modal.
    pub fn open_network_import_dialog(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() {
            self.show_toast("Open a library before importing", cx);
            return;
        }
        if self.import.is_busy() || self.network_import.is_busy() {
            self.show_toast("An import is already running", cx);
            return;
        }
        if self.import_blocked_by_modal() {
            self.show_toast("Close the open dialog before importing", cx);
            return;
        }
        // ponytail: no clipboard read on open; macOS may prompt for every
        // programmatic paste. ⌘V pastes into the field.
        self.network_import.cancel_flag = Arc::new(AtomicBool::new(false));
        self.network_import.stage = Some(NetworkImportStage::Input {
            url: String::new(),
            error: None,
        });
        cx.notify();
    }

    /// Closes the modal, cancelling any active background inspection or download.
    pub fn close_network_import_dialog(&mut self, cx: &mut Context<Self>) {
        self.network_import
            .cancel_flag
            .store(true, Ordering::Relaxed);
        self.network_import.stage = None;
        cx.notify();
    }

    /// Esc and backdrop clicks: close, except that a running download asks
    /// for a second Esc so one stray key or click can't throw it away.
    pub fn dismiss_network_import(&mut self, from_keyboard: bool, cx: &mut Context<Self>) {
        if let Some(NetworkImportStage::Downloading {
            ref mut confirm_cancel,
            ..
        }) = self.network_import.stage
        {
            if from_keyboard && *confirm_cancel {
                self.close_network_import_dialog(cx);
            } else if from_keyboard {
                *confirm_cancel = true;
                cx.notify();
            }
            return;
        }
        self.close_network_import_dialog(cx);
    }

    /// Updates the URL input buffer.
    pub fn set_network_import_url(&mut self, url: String, cx: &mut Context<Self>) {
        if let Some(NetworkImportStage::Input {
            url: ref mut current,
            ref mut error,
        }) = self.network_import.stage
        {
            *current = url;
            *error = None;
            cx.notify();
        }
    }

    /// Toggles the favorite flag in the Ready stage.
    pub fn toggle_network_import_favorite(&mut self, cx: &mut Context<Self>) {
        if let Some(NetworkImportStage::Ready {
            ref mut is_favorite,
            ..
        }) = self.network_import.stage
        {
            *is_favorite = !*is_favorite;
            cx.notify();
        }
    }

    fn set_network_input_error(&mut self, url: String, error: String, cx: &mut Context<Self>) {
        self.network_import.stage = Some(NetworkImportStage::Input {
            url,
            error: Some(error),
        });
        cx.notify();
    }

    /// Inspects the candidate URL, fetching title, duration, and metadata off the UI thread.
    pub fn inspect_network_url(&mut self, url: String, cx: &mut Context<Self>) {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() {
            self.set_network_input_error(trimmed, "Enter a video link".to_string(), cx);
            return;
        }
        let kind = match classify_url(&trimmed) {
            Ok(kind) => kind,
            Err(e) => {
                self.set_network_input_error(trimmed, e.to_string(), cx);
                return;
            }
        };
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };

        self.network_import.stage = Some(NetworkImportStage::Inspecting {
            url: trimmed.clone(),
        });
        let cancel = Arc::new(AtomicBool::new(false));
        self.network_import.cancel_flag = cancel.clone();
        cx.notify();

        let query_url = trimmed.clone();
        let task_cancel = cancel.clone();
        let inspect_task = cx.background_executor().spawn(async move {
            // The tool lookup stats files: keep it off the UI thread too.
            if helper_missing(kind) {
                return anyhow::Ok(Err(()));
            }
            let info = inspect_url(&query_url, &task_cancel)?;
            let target = engine.preview_network_destination(&info);
            let already = engine
                .find_prior_download(&info.url)
                .map(|prior| prior.map(|prior| prior.path))
                .unwrap_or_else(|err| {
                    log::warn!("looking up earlier imports of {}: {err:#}", info.url);
                    None
                });
            anyhow::Ok(Ok((info, target, already)))
        });

        cx.spawn(async move |this, cx| {
            let result = inspect_task.await;
            let _ = this.update(cx, |app, cx| {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                match result {
                    Ok(Err(())) => {
                        app.network_import.stage =
                            Some(NetworkImportStage::MissingHelper { url: trimmed });
                        cx.notify();
                    }
                    Ok(Ok((info, target_path, already_imported))) => {
                        app.network_import.stage = Some(NetworkImportStage::Ready {
                            info,
                            target_path,
                            is_favorite: false,
                            already_imported,
                        });
                        cx.notify();
                    }
                    // The top message is written for people; the chain is for the log.
                    Err(err) => {
                        log::warn!("inspecting {trimmed}: {err:#}");
                        app.set_network_input_error(trimmed, err.to_string(), cx)
                    }
                }
            });
        })
        .detach();
    }

    /// Starts downloading the inspected media stream.
    pub fn confirm_network_download(&mut self, cx: &mut Context<Self>) {
        let Some(NetworkImportStage::Ready {
            info, is_favorite, ..
        }) = self.network_import.stage.clone()
        else {
            return;
        };
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };

        let cancel = Arc::new(AtomicBool::new(false));
        self.network_import.cancel_flag = cancel.clone();
        let mut connecting = NetworkDownloadProgress::new(DownloadPhase::Connecting, "Connecting");
        connecting.total_bytes = info.expected_bytes;
        self.network_import.stage = Some(NetworkImportStage::Downloading {
            title: info.title.clone(),
            progress: connecting,
            confirm_cancel: false,
        });
        cx.notify();

        let (progress_tx, progress_rx) = async_channel::unbounded::<NetworkDownloadProgress>();
        let (done_tx, done_rx) = async_channel::bounded::<Outcome>(1);
        let progress: ProgressCallback = Box::new(move |p| {
            let _ = progress_tx.send_blocking(p);
        });

        // A dedicated thread: a download can take hours and must not hold a
        // shared executor worker.
        let spawned = std::thread::Builder::new()
            .name("lumina-net-import".to_string())
            .spawn(move || {
                let outcome = match engine.download_network_media(&info, &cancel, &progress) {
                    Err(_) if cancel.load(Ordering::Relaxed) => Outcome::Cancelled,
                    Err(e) => {
                        log::warn!("network import of {} failed: {e:#}", info.url);
                        Outcome::Failed(e.to_string())
                    }
                    Ok(path) => {
                        match engine.ingest_network_imported_file(&path, &info, is_favorite) {
                            Ok(ingest) => Outcome::Imported {
                                item: Box::new(ingest.item),
                                timeline_error: ingest.timeline_error,
                            },
                            Err(e) => {
                                log::error!("cataloguing {}: {e:#}", path.display());
                                Outcome::NotCatalogued { path }
                            }
                        }
                    }
                };
                let _ = done_tx.send_blocking(outcome);
            });
        if let Err(err) = spawned {
            self.network_import.stage = None;
            log::error!("spawning the download thread: {err}");
            self.set_error("Couldn't start the download. Try again.".to_string(), cx);
            return;
        }

        cx.spawn(async move |this, cx| {
            while let Ok(progress) = progress_rx.recv().await {
                let _ = this.update(cx, |app, cx| {
                    if let Some(NetworkImportStage::Downloading {
                        progress: ref mut current,
                        ..
                    }) = app.network_import.stage
                    {
                        *current = progress;
                        cx.notify();
                    }
                });
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            let Ok(outcome) = done_rx.recv().await else {
                return;
            };
            let _ = this.update(cx, |app, cx| {
                if matches!(
                    app.network_import.stage,
                    Some(NetworkImportStage::Downloading { .. })
                ) {
                    app.network_import.stage = None;
                }
                match outcome {
                    Outcome::Imported {
                        item,
                        timeline_error,
                    } => {
                        app.show_toast(format!("Imported \"{}\"", item.file_name), cx);
                        if timeline_error.is_some() {
                            app.set_error(
                                // The details are in the log (see `ingest_network_imported_file`).
                                format!("Imported \"{}\". The timeline will update after Lumina restarts.", item.file_name),
                                cx,
                            );
                        }
                        app.refresh_album_counts();
                        app.refresh_active_view(cx);
                    }
                    Outcome::Cancelled => {}
                    Outcome::Failed(error) => {
                        app.set_error(format!("Couldn't import the video. {error}"), cx)
                    }
                    // The details are in the log; the file is kept so nothing is lost.
                    Outcome::NotCatalogued { path } => app.set_error(
                        format!(
                            "The video was saved as \"{}\" in your Imports folder but couldn't be added to Lumina. Rescan the library to add it.",
                            path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()
                        ),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
