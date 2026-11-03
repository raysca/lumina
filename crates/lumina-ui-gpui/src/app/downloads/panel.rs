//! Actions behind the downloads dropdown: showing it, keyboard focus,
//! each row's main action, and Clear finished.

use gpui::*;
use lumina_core::DownloadState;

use super::DownloadEntry;
use crate::LuminaApp;

/// The dropdown's sections, top to bottom.
pub(crate) const SECTIONS: [(&str, &[DownloadState]); 5] = [
    ("Downloading", &[DownloadState::Running]),
    ("Waiting", &[DownloadState::Waiting]),
    ("Paused", &[DownloadState::Paused]),
    ("Failed", &[DownloadState::Failed]),
    ("Done", &[DownloadState::Done, DownloadState::Duplicate]),
];

impl LuminaApp {
    /// Rows in the order the dropdown shows them.
    pub(crate) fn download_display_order(&self) -> Vec<&DownloadEntry> {
        SECTIONS
            .iter()
            .flat_map(|(_, states)| {
                self.downloads
                    .entries
                    .iter()
                    .filter(move |entry| states.contains(&entry.row.state))
            })
            .collect()
    }

    /// Bytes done over bytes expected, across running downloads whose size
    /// is known: the toolbar ring. `None` when no size is known.
    pub(crate) fn download_progress(&self) -> Option<f32> {
        let (done, total) = self
            .downloads
            .entries
            .iter()
            .filter(|entry| entry.row.state == DownloadState::Running)
            .filter_map(|entry| entry.progress.as_ref())
            .filter_map(|p| {
                p.total_bytes
                    .filter(|t| *t > 0)
                    .map(|t| (p.bytes_downloaded, t))
            })
            .fold((0u64, 0u64), |(d, t), (pd, pt)| (d + pd.min(pt), t + pt));
        (total > 0).then(|| done as f32 / total as f32)
    }

    /// ⌥⌘L, the toolbar button, View > Show Downloads and the palette.
    pub fn toggle_downloads_panel(&mut self, cx: &mut Context<Self>) {
        if self.downloads.open {
            self.close_downloads_panel(cx);
            return;
        }
        if self.downloads.entries.is_empty() {
            return;
        }
        self.downloads.open = true;
        self.downloads.unseen_failure = false;
        self.downloads.focused = self.download_display_order().first().map(|e| e.row.id);
        cx.notify();
    }

    pub(crate) fn close_downloads_panel(&mut self, cx: &mut Context<Self>) {
        self.downloads.open = false;
        self.downloads.focused = None;
        self.downloads.highlight = None;
        cx.notify();
    }

    /// ↑/↓ in the open dropdown.
    pub(crate) fn move_download_focus(&mut self, step: isize, cx: &mut Context<Self>) {
        let order: Vec<i64> = self
            .download_display_order()
            .iter()
            .map(|entry| entry.row.id)
            .collect();
        if order.is_empty() {
            return;
        }
        let next = match self
            .downloads
            .focused
            .and_then(|id| order.iter().position(|o| *o == id))
        {
            Some(at) => (at as isize + step).clamp(0, order.len() as isize - 1) as usize,
            None => 0,
        };
        self.downloads.focused = Some(order[next]);
        cx.notify();
    }

    /// ⏎ on a row and a click on its body: Retry on a failed row, Resume on
    /// a paused one.
    pub(crate) fn download_main_action(&mut self, row_id: i64, cx: &mut Context<Self>) {
        let Some(entry) = self.downloads.entries.iter().find(|e| e.row.id == row_id) else {
            return;
        };
        match entry.row.state {
            DownloadState::Failed if entry.row.can_retry => self.retry_download(row_id, cx),
            DownloadState::Paused => self.resume_download(row_id, cx),
            _ => {}
        }
    }

    /// Clear finished: removes Done, Failed and Duplicate rows. The media
    /// stays in the library.
    pub fn clear_finished_downloads(&mut self, cx: &mut Context<Self>) {
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        let finished = |entry: &DownloadEntry| {
            matches!(
                entry.row.state,
                DownloadState::Done | DownloadState::Failed | DownloadState::Duplicate
            )
        };
        let (cleared, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.downloads.entries)
            .into_iter()
            .partition(|entry| finished(entry));
        self.downloads.entries = kept;
        if cleared.is_empty() {
            return;
        }
        if self.downloads.entries.is_empty() {
            self.close_downloads_panel(cx);
        } else if !self
            .downloads
            .focused
            .is_some_and(|id| self.downloads.entries.iter().any(|e| e.row.id == id))
        {
            self.downloads.focused = self.download_display_order().first().map(|e| e.row.id);
        }
        cx.notify();

        let generation = self.downloads.generation;
        let clear = cx
            .background_executor()
            .spawn(async move { catalog.clear_finished_downloads() });
        let mut cleared = Some(cleared);
        cx.spawn(async move |this, cx| {
            if let Err(err) = clear.await {
                log::error!("clearing finished downloads: {err:#}");
                let _ = this.update(cx, |app, cx| {
                    if app.downloads.generation != generation {
                        return;
                    }
                    // Put them back: the stored list still has them.
                    app.downloads
                        .entries
                        .extend(cleared.take().unwrap_or_default());
                    app.downloads.entries.sort_by_key(|entry| {
                        (entry.row.id < 0, entry.row.added_epoch_ms, entry.row.id)
                    });
                    app.set_error("Couldn't clear finished downloads. Try again.", cx);
                });
            }
        })
        .detach();
    }
}
