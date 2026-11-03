//! Network video import state and asynchronous actions.

use gpui::*;
use lumina_core::net_import::helper::{find_yt_dlp, inspect_url_helper};
use lumina_core::net_import::{
    classify_url, DownloadPhase, NetworkDownloadProgress, NetworkMediaInfo, NetworkSourceKind,
    ProgressCallback,
};
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
        tags: Vec<String>,
        is_favorite: bool,
    },
    /// Downloading stream in background with live progress.
    Downloading {
        title: String,
        progress: NetworkDownloadProgress,
    },
    /// Required helper binary (yt-dlp) is missing.
    MissingHelper { url: String, message: String },
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

        // Auto-detect URL from system clipboard
        let mut prefill_url = String::new();
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                let trimmed = text.trim();
                if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                    prefill_url = trimmed.to_string();
                }
            }
        }

        self.network_import.cancel_flag = Arc::new(AtomicBool::new(false));
        self.network_import.stage = Some(NetworkImportStage::Input {
            url: prefill_url,
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

    /// Updates the URL input buffer (used in tests).
    #[cfg(test)]
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

    /// Inspects the candidate URL, fetching title, duration, and metadata off the UI thread.
    pub fn inspect_network_url(&mut self, url: String, cx: &mut Context<Self>) {
        let trimmed = url.trim().to_string();
        if trimmed.is_empty() {
            if let Some(NetworkImportStage::Input { ref mut error, .. }) = self.network_import.stage
            {
                *error = Some("Please enter a valid URL".to_string());
                cx.notify();
            }
            return;
        }

        let kind = match classify_url(&trimmed) {
            Ok(k) => k,
            Err(e) => {
                if let Some(NetworkImportStage::Input { ref mut error, .. }) =
                    self.network_import.stage
                {
                    *error = Some(e.to_string());
                    cx.notify();
                }
                return;
            }
        };

        // If it requires yt-dlp, verify presence before inspecting
        if matches!(
            kind,
            NetworkSourceKind::YouTube | NetworkSourceKind::GenericPlatform
        ) && find_yt_dlp(None).is_none()
        {
            self.network_import.stage = Some(NetworkImportStage::MissingHelper {
                url: trimmed,
                message: "YouTube imports require the 'yt-dlp' tool. Install it with 'brew install yt-dlp'.".to_string(),
            });
            cx.notify();
            return;
        }

        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };

        self.network_import.stage = Some(NetworkImportStage::Inspecting {
            url: trimmed.clone(),
        });
        cx.notify();

        let cancel = Arc::new(AtomicBool::new(false));
        self.network_import.cancel_flag = cancel.clone();
        let query_url = trimmed.clone();
        let fallback_url = trimmed.clone();

        let inspect_task = cx.background_executor().spawn(async move {
            match kind {
                NetworkSourceKind::DirectUrl => {
                    lumina_core::net_import::direct::inspect_direct_url(&query_url)
                }
                NetworkSourceKind::HlsStream => {
                    lumina_core::net_import::hls::inspect_hls_url(&query_url)
                }
                NetworkSourceKind::YouTube | NetworkSourceKind::GenericPlatform => {
                    let binary = find_yt_dlp(None)
                        .ok_or_else(|| anyhow::anyhow!("yt-dlp is required to inspect this URL"))?;
                    inspect_url_helper(&binary, &query_url)
                }
            }
        });

        cx.spawn(async move |this, cx| {
            let result = inspect_task.await;
            let _ = this.update(cx, |app, cx| {
                if cancel.load(Ordering::Relaxed) {
                    return;
                }

                match result {
                    Ok(info) => {
                        let dest = engine.destination_for_network_import(&info.title, "mp4");
                        app.network_import.stage = Some(NetworkImportStage::Ready {
                            info,
                            target_path: dest,
                            tags: Vec::new(),
                            is_favorite: false,
                        });
                    }
                    Err(err) => {
                        app.network_import.stage = Some(NetworkImportStage::Input {
                            url: fallback_url,
                            error: Some(format!("Could not inspect stream: {err:#}")),
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Starts downloading the inspected media stream.
    pub fn confirm_network_download(&mut self, cx: &mut Context<Self>) {
        let Some(NetworkImportStage::Ready {
            info,
            target_path,
            tags,
            is_favorite,
            ..
        }) = self.network_import.stage.clone()
        else {
            return;
        };

        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.network_import.cancel_flag = cancel_flag.clone();

        self.network_import.stage = Some(NetworkImportStage::Downloading {
            title: info.title.clone(),
            progress: NetworkDownloadProgress {
                phase: DownloadPhase::Connecting,
                bytes_downloaded: 0,
                total_bytes: info.expected_bytes,
                speed_bytes_per_sec: 0,
                eta_secs: None,
                message: "Connecting...".to_string(),
            },
        });
        cx.notify();

        let (progress_tx, progress_rx) = async_channel::unbounded::<NetworkDownloadProgress>();
        let p_tx = progress_tx.clone();
        let cb: ProgressCallback = Box::new(move |p| {
            let _ = p_tx.send_blocking(p);
        });

        let cancel_child = cancel_flag.clone();
        let dest = target_path.clone();
        let media_url = info.url.clone();
        let media_kind = info.kind;

        // Background download executor
        let download_task = cx.background_executor().spawn(async move {
            match media_kind {
                NetworkSourceKind::DirectUrl => {
                    lumina_core::net_import::direct::download_direct_url(
                        &media_url,
                        &dest,
                        &cancel_child,
                        Some(&cb),
                    )
                }
                NetworkSourceKind::HlsStream => {
                    // Try macOS native AVFoundation HLS bridge first
                    let hls_cancel = cancel_child.clone();
                    let cb_hls = cb;
                    let result = crate::player::macos::export_hls_stream_macos(
                        &media_url,
                        &dest,
                        &hls_cancel,
                        move |fraction| {
                            let pct = (fraction * 100.0) as u64;
                            cb_hls(NetworkDownloadProgress {
                                phase: DownloadPhase::Downloading,
                                bytes_downloaded: pct,
                                total_bytes: Some(100),
                                speed_bytes_per_sec: 0,
                                eta_secs: None,
                                message: format!("Exporting HLS stream: {pct}%"),
                            });
                        },
                    );
                    match result {
                        Ok(()) => Ok(dest),
                        Err(_) => {
                            // Fallback to yt-dlp helper if native export had issues
                            lumina_core::net_import::hls::download_hls_via_helper(
                                &media_url,
                                &dest,
                                &cancel_child,
                                None,
                            )
                        }
                    }
                }
                NetworkSourceKind::YouTube | NetworkSourceKind::GenericPlatform => {
                    let binary = find_yt_dlp(None)
                        .ok_or_else(|| anyhow::anyhow!("yt-dlp helper not found on system"))?;
                    lumina_core::net_import::helper::download_yt_dlp(
                        &binary,
                        &media_url,
                        &dest,
                        &cancel_child,
                        Some(&cb),
                    )
                }
            }
        });

        // UI progress listener loop
        let this_progress = cx.spawn({
            let rx = progress_rx.clone();
            async move |this, cx| {
                while let Ok(progress) = rx.recv().await {
                    let _ = this.update(cx, |app, cx| {
                        if let Some(NetworkImportStage::Downloading {
                            progress: ref mut cur_p,
                            ..
                        }) = app.network_import.stage
                        {
                            *cur_p = progress;
                            cx.notify();
                        }
                    });
                }
            }
        });
        this_progress.detach();

        // Completion handler
        let title_clone = info.title.clone();
        let uploader_clone = info.uploader.clone();
        let tags_clone = tags.clone();

        cx.spawn(async move |this, cx| {
            let result = download_task.await;
            let _ = this.update(cx, |app, cx| {
                if cancel_flag.load(Ordering::Relaxed) {
                    if let Ok(ref final_path) = result {
                        let _ = std::fs::remove_file(final_path);
                    }
                    app.network_import.stage = None;
                    cx.notify();
                    return;
                }

                match result {
                    Ok(final_path) => {
                        // Ingest media into catalog and sidecar
                        match engine.ingest_network_imported_file(
                            &final_path,
                            &info.url,
                            uploader_clone.as_deref(),
                            Some(&title_clone),
                            &tags_clone,
                            is_favorite,
                        ) {
                            Ok(item) => {
                                app.show_toast(format!("Imported \"{}\"", item.file_name), cx);
                                app.network_import.stage = None;
                                // Refresh catalog view and select newly imported item
                                app.refresh_after_import(cx);
                            }
                            Err(e) => {
                                app.set_error(format!("Ingestion failed: {e:#}"), cx);
                                app.network_import.stage = None;
                            }
                        }
                    }
                    Err(e) => {
                        app.set_error(format!("Download failed: {e:#}"), cx);
                        app.network_import.stage = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Helper to refresh grid and album counts after successful network import.
    fn refresh_after_import(&mut self, cx: &mut Context<Self>) {
        self.refresh_album_counts();
        self.refresh_active_view(cx);
        cx.notify();
    }
}
