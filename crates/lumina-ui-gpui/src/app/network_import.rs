//! "Import Video from URL": the dialog's state. Submitting a link checks it
//! off the UI thread (helper present, not downloaded before, inspected);
//! pressing Import on the preview hands it to the download queue
//! (`app::downloads`), which does the download, file and catalog work.

use gpui::*;
use lumina_core::net_import::{
    classify_url, helper_missing, inspect_url, NetworkMediaInfo, MISSING_PART,
};
use lumina_core::PriorDownload;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::LuminaApp;

/// Inspects a link (title, length, size); stops when the flag is set.
/// Tests replace it so nothing touches the network.
pub(crate) type InspectFn =
    Arc<dyn Fn(&str, &AtomicBool) -> anyhow::Result<NetworkMediaInfo> + Send + Sync>;

/// The real inspector.
pub(crate) fn real_inspect() -> InspectFn {
    Arc::new(inspect_url)
}

/// What the background check found.
enum CheckResult {
    MissingHelper,
    Prior(PriorDownload),
    Inspected(anyhow::Result<NetworkMediaInfo>),
}

/// The stages of the Import Video from URL dialog.
#[derive(Debug, Clone)]
pub(crate) enum NetworkImportStage {
    /// URL text entry.
    Input { url: String, error: Option<String> },
    /// Checking and inspecting the link off the UI thread. Closing the
    /// dialog sets `cancel`; a result from another check is ignored.
    Checking {
        url: String,
        cancel: Arc<AtomicBool>,
    },
    /// Inspected: the preview card, waiting for Import.
    Ready { url: String, info: NetworkMediaInfo },
    /// Lumina downloaded this link before and it is still in the library.
    AlreadyDownloaded { url: String, prior: PriorDownload },
}

#[derive(Default)]
pub(crate) struct NetworkImportState {
    pub stage: Option<NetworkImportStage>,
    /// `None` until a library opens. Tests replace it.
    pub inspect: Option<InspectFn>,
}

impl LuminaApp {
    /// Opens the "Import Video from URL" modal. Downloads already in the
    /// queue keep running.
    pub fn open_network_import_dialog(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() {
            self.show_warning_toast("Open a library before importing", cx);
            return;
        }
        if self.import_blocked_by_modal() {
            self.show_warning_toast("Close the open dialog before importing", cx);
            return;
        }
        // ponytail: no clipboard read on open; macOS may prompt for every
        // programmatic paste. ⌘V pastes into the field.
        self.network_import.stage = Some(NetworkImportStage::Input {
            url: String::new(),
            error: None,
        });
        cx.notify();
    }

    /// Closes the modal (Cancel, Esc, a click outside). Queued downloads
    /// are not affected.
    pub fn close_network_import_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(NetworkImportStage::Checking { cancel, .. }) = &self.network_import.stage {
            cancel.store(true, Ordering::Relaxed);
        }
        self.network_import.stage = None;
        cx.notify();
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

    pub(crate) fn set_network_input_error(
        &mut self,
        url: String,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.network_import.stage = Some(NetworkImportStage::Input {
            url,
            error: Some(error),
        });
        cx.notify();
    }

    /// Download Again in the "already downloaded" prompt.
    pub fn download_again_from_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(NetworkImportStage::AlreadyDownloaded { url, .. }) =
            self.network_import.stage.take()
        else {
            return;
        };
        self.enqueue_download(url, true, None, cx);
        cx.notify();
    }

    /// Import on the preview: queues the link with its inspection.
    pub fn import_inspected_link(&mut self, cx: &mut Context<Self>) {
        if !matches!(
            self.network_import.stage,
            Some(NetworkImportStage::Ready { .. })
        ) {
            return;
        }
        let Some(NetworkImportStage::Ready { url, info }) = self.network_import.stage.take() else {
            return;
        };
        self.enqueue_download(url, false, Some(info), cx);
        cx.notify();
    }

    /// The dialog's Import on a typed link. In order: the link is checked;
    /// one already in the downloads list is pointed at; then, off the UI
    /// thread, an earlier download is looked up, a missing helper is
    /// reported, and the link is inspected for the preview.
    pub fn submit_network_url(&mut self, url: String, cx: &mut Context<Self>) {
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
        let kind = match classify_url(&url) {
            Ok(kind) => kind,
            Err(err) => {
                self.set_network_input_error(url, err.to_string(), cx);
                return;
            }
        };
        if let Some(id) = self.downloads.active_for(&url) {
            self.network_import.stage = None;
            self.reveal_download(id, cx);
            return;
        }
        let (Some(catalog), Some(inspect)) =
            (self.download_catalog(), self.network_import.inspect.clone())
        else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        self.network_import.stage = Some(NetworkImportStage::Checking {
            url: url.clone(),
            cancel: cancel.clone(),
        });
        cx.notify();

        let check_url = url.clone();
        let check_cancel = cancel.clone();
        // ponytail: inspecting holds an executor worker for up to the
        // inspect timeout; a dedicated thread if that ever starves others.
        let check = cx.background_executor().spawn(async move {
            // An earlier download is offered even when the helper is
            // missing: Show in Library needs no helper.
            match catalog.find_prior_download(&check_url) {
                Ok(Some(prior)) => return CheckResult::Prior(prior),
                Ok(None) => {}
                Err(err) => log::warn!("looking up earlier downloads of {check_url}: {err:#}"),
            }
            if helper_missing(kind) {
                return CheckResult::MissingHelper;
            }
            CheckResult::Inspected(inspect(&check_url, &check_cancel))
        });
        cx.spawn(async move |this, cx| {
            let result = check.await;
            let _ = this.update(cx, |app, cx| {
                // Closed, or a newer check took over: drop this result.
                let current = matches!(
                    &app.network_import.stage,
                    Some(NetworkImportStage::Checking { cancel: now, .. }) if Arc::ptr_eq(now, &cancel)
                );
                if !current {
                    return;
                }
                app.network_import.stage = Some(match result {
                    CheckResult::MissingHelper => NetworkImportStage::Input {
                        url,
                        error: Some(MISSING_PART.to_string()),
                    },
                    CheckResult::Prior(prior) => {
                        NetworkImportStage::AlreadyDownloaded { url, prior }
                    }
                    CheckResult::Inspected(Ok(info)) => NetworkImportStage::Ready { url, info },
                    CheckResult::Inspected(Err(err)) => {
                        log::warn!("inspecting {url}: {err:#}");
                        // ponytail: outer message only; inner causes can name
                        // the download tools, which UI copy must not show.
                        NetworkImportStage::Input {
                            url,
                            error: Some(err.to_string()),
                        }
                    }
                });
                cx.notify();
            });
        })
        .detach();
    }
}
