//! Actions behind the downloads dropdown: showing it, keyboard focus,
//! each row's main action, and Clear finished.

use gpui::*;
use lumina_core::{Catalog, DownloadState, MediaId, SystemAlbum};

use super::DownloadEntry;
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;

/// The dropdown's sections, top to bottom.
const SECTIONS: [(&str, &[DownloadState]); 5] = [
    ("Downloading", &[DownloadState::Running]),
    ("Waiting", &[DownloadState::Waiting]),
    ("Paused", &[DownloadState::Paused]),
    ("Failed", &[DownloadState::Failed]),
    ("Done", &[DownloadState::Done, DownloadState::Duplicate]),
];

impl LuminaApp {
    /// The dropdown's non-empty sections, each label with its rows.
    pub(crate) fn download_sections(&self) -> Vec<(&'static str, Vec<&DownloadEntry>)> {
        SECTIONS
            .iter()
            .map(|(label, states)| {
                let rows = self
                    .downloads
                    .entries
                    .iter()
                    .filter(|entry| states.contains(&entry.row.state))
                    .collect();
                (*label, rows)
            })
            .filter(|(_, rows): &(_, Vec<_>)| !rows.is_empty())
            .collect()
    }

    /// Rows in the order the dropdown shows them.
    pub(crate) fn download_display_order(&self) -> Vec<&DownloadEntry> {
        self.download_sections()
            .into_iter()
            .flat_map(|(_, rows)| rows)
            .collect()
    }

    /// Whether any row is in one of `states`: the palette's download entries.
    pub(crate) fn any_download_in(&self, states: &[DownloadState]) -> bool {
        self.downloads
            .entries
            .iter()
            .any(|entry| states.contains(&entry.row.state))
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
        let id = order[next];
        self.downloads.focused = Some(id);
        // The body lists each non-empty section's label, then its rows.
        let mut child = 0;
        let mut target = None;
        for (_, rows) in self.download_sections() {
            if let Some(at) = rows.iter().position(|entry| entry.row.id == id) {
                target = Some(child + 1 + at);
                break;
            }
            child += 1 + rows.len();
        }
        if let Some(target) = target {
            self.downloads.scroll.scroll_to_item(target);
        }
        cx.notify();
    }

    /// ⏎ on a row and a click on its body: Retry on a failed row, Pause on
    /// a running one (⏎ only; the click skips it), Resume on a paused one,
    /// Show in Library on a finished one.
    pub(crate) fn download_main_action(&mut self, row_id: i64, cx: &mut Context<Self>) {
        let Some(entry) = self.downloads.entries.iter().find(|e| e.row.id == row_id) else {
            return;
        };
        match (entry.row.state, entry.row.media_id) {
            (DownloadState::Failed, _) if entry.row.can_retry => self.retry_download(row_id, cx),
            (DownloadState::Paused, _) => self.resume_download(row_id, cx),
            (DownloadState::Running, _) => self.pause_download(row_id, cx),
            (DownloadState::Done | DownloadState::Duplicate, Some(id)) if entry.row.in_library => {
                // Trashed or hidden since it finished: say so on the row.
                let shown = self.show_download_in_library(id, cx);
                if !shown {
                    if let Some(index) = self.downloads.index_of(row_id) {
                        self.downloads.entries[index].row.in_library = false;
                    }
                }
            }
            _ => {}
        }
    }

    /// Show in Library: the Downloaded album with the item selected. False
    /// when the item is no longer in that album.
    pub fn show_download_in_library(&mut self, media_id: MediaId, cx: &mut Context<Self>) -> bool {
        self.close_downloads_panel(cx);
        self.close_network_import_dialog(cx);
        self.select_album(AlbumSelection::System(SystemAlbum::Downloaded), cx);
        // ponytail: walks the album like the startup restore does; an
        // index-of-id query if Downloaded ever gets huge.
        if let Some(index) = self.find_active_index(media_id) {
            if let Some(item) = self.get_active_slice(index..index + 1).pop() {
                self.select_item_at(item, index, cx);
                self.reveal_grid_selection();
                return true;
            }
        }
        false
    }

    /// Clear finished (dropdown, ⌘K): removes Done, Failed and Duplicate
    /// rows. The media stays in the library.
    pub fn clear_finished_downloads(&mut self, cx: &mut Context<Self>) {
        self.clear_downloads(
            &[
                DownloadState::Done,
                DownloadState::Failed,
                DownloadState::Duplicate,
            ],
            Catalog::clear_finished_downloads,
            cx,
        );
    }

    /// Clear completed (the Downloads screen): removes Done and Duplicate
    /// rows. Failed rows stay, since they hold the link for Retry.
    pub fn clear_completed_downloads(&mut self, cx: &mut Context<Self>) {
        self.clear_downloads(
            &[DownloadState::Done, DownloadState::Duplicate],
            Catalog::clear_completed_downloads,
            cx,
        );
    }

    /// Removes rows in `states` at once, then from the catalog with `clear`
    /// (which must delete the same states) off the UI thread. A failed write
    /// puts them back and shows the banner.
    fn clear_downloads(
        &mut self,
        states: &[DownloadState],
        clear: fn(&Catalog) -> anyhow::Result<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        let (cleared, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.downloads.entries)
            .into_iter()
            .partition(|entry| states.contains(&entry.row.state));
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
            .spawn(async move { clear(&catalog) });
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
