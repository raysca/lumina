//! "Import Video from URL": the dialog's state. Pressing Import hands the
//! link to the download queue (`app::downloads`), which does the network,
//! file and catalog work off the UI thread.

use gpui::*;
use lumina_core::PriorDownload;

use crate::LuminaApp;

/// The stages of the Import Video from URL dialog.
#[derive(Debug, Clone)]
pub(crate) enum NetworkImportStage {
    /// URL text entry.
    Input { url: String, error: Option<String> },
    /// Looking the link up among earlier downloads, off the UI thread.
    Checking { url: String },
    /// Lumina downloaded this link before and it is still in the library.
    AlreadyDownloaded { url: String, prior: PriorDownload },
}

#[derive(Default)]
pub(crate) struct NetworkImportState {
    pub stage: Option<NetworkImportStage>,
}

impl LuminaApp {
    /// Opens the "Import Video from URL" modal. Downloads already in the
    /// queue keep running.
    pub fn open_network_import_dialog(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() {
            self.show_toast("Open a library before importing", cx);
            return;
        }
        if self.import_blocked_by_modal() {
            self.show_toast("Close the open dialog before importing", cx);
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
        self.enqueue_download(url, true, cx);
        cx.notify();
    }
}
