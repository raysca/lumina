//! Actions behind the downloads dropdown: showing it, keyboard focus,
//! each row's main action, and Clear finished.

use gpui::*;
use lumina_core::{Catalog, DownloadState, MediaId, SystemAlbum};

use super::DownloadEntry;
use crate::icons::Icon;
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;

/// The Downloads screen's filter chips, left to right. Session only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DownloadFilter {
    #[default]
    All,
    Downloading,
    Queued,
    Paused,
    Failed,
    Completed,
}

impl DownloadFilter {
    pub(crate) const ALL: [DownloadFilter; 6] = [
        DownloadFilter::All,
        DownloadFilter::Downloading,
        DownloadFilter::Queued,
        DownloadFilter::Paused,
        DownloadFilter::Failed,
        DownloadFilter::Completed,
    ];

    /// The chip's selector suffix and i18n key suffix.
    pub(crate) fn key(self) -> &'static str {
        match self {
            DownloadFilter::All => "all",
            DownloadFilter::Downloading => "downloading",
            DownloadFilter::Queued => "queued",
            DownloadFilter::Paused => "paused",
            DownloadFilter::Failed => "failed",
            DownloadFilter::Completed => "completed",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            DownloadFilter::All => Icon::Layers,
            DownloadFilter::Downloading => Icon::ArrowDownToLine,
            DownloadFilter::Queued => Icon::ListOrdered,
            DownloadFilter::Paused => Icon::Pause,
            DownloadFilter::Failed => Icon::TriangleAlert,
            DownloadFilter::Completed => Icon::Check,
        }
    }

    pub(crate) fn matches(self, state: DownloadState) -> bool {
        match self {
            DownloadFilter::All => true,
            DownloadFilter::Downloading => state == DownloadState::Running,
            DownloadFilter::Queued => state == DownloadState::Waiting,
            DownloadFilter::Paused => state == DownloadState::Paused,
            DownloadFilter::Failed => state == DownloadState::Failed,
            DownloadFilter::Completed => {
                matches!(state, DownloadState::Done | DownloadState::Duplicate)
            }
        }
    }
}

/// Which list a section order is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DownloadSurface {
    /// The toolbar dropdown.
    Panel,
    /// The Downloads screen, under its filter chip.
    Screen,
}

/// The dropdown's sections, top to bottom, by label.
const PANEL_SECTIONS: [(&str, &[DownloadState]); 5] = [
    ("Downloading", &[DownloadState::Running]),
    ("Waiting", &[DownloadState::Waiting]),
    ("Paused", &[DownloadState::Paused]),
    ("Failed", &[DownloadState::Failed]),
    ("Earlier", &[DownloadState::Done, DownloadState::Duplicate]),
];

/// The screen's sections, top to bottom, by selector key.
const SCREEN_SECTIONS: [(&str, &[DownloadState]); 4] = [
    ("active", &[DownloadState::Running]),
    ("attention", &[DownloadState::Paused, DownloadState::Failed]),
    ("queued", &[DownloadState::Waiting]),
    (
        "completed",
        &[DownloadState::Done, DownloadState::Duplicate],
    ),
];

impl LuminaApp {
    /// The non-empty sections of `surface`, each label (Panel) or key
    /// (Screen) with its rows in queue order; the screen's completed rows
    /// newest first, and only the rows its filter keeps.
    pub(crate) fn download_sections(
        &self,
        surface: DownloadSurface,
    ) -> Vec<(&'static str, Vec<&DownloadEntry>)> {
        let (sections, filter): (&[(&str, &[DownloadState])], _) = match surface {
            DownloadSurface::Panel => (&PANEL_SECTIONS, DownloadFilter::All),
            DownloadSurface::Screen => (&SCREEN_SECTIONS, self.downloads.filter),
        };
        sections
            .iter()
            .map(|(label, states)| {
                let mut rows: Vec<&DownloadEntry> = self
                    .downloads
                    .entries
                    .iter()
                    .filter(|entry| states.contains(&entry.row.state))
                    .filter(|entry| filter.matches(entry.row.state))
                    .collect();
                if *label == "completed" {
                    rows.sort_by_key(|e| std::cmp::Reverse((e.row.finished_epoch_ms, e.row.id)));
                }
                (*label, rows)
            })
            .filter(|(_, rows)| !rows.is_empty())
            .collect()
    }

    /// Rows in the order `surface` shows them.
    pub(crate) fn download_display_order(&self, surface: DownloadSurface) -> Vec<&DownloadEntry> {
        self.download_sections(surface)
            .into_iter()
            .flat_map(|(_, rows)| rows)
            .collect()
    }

    /// Rows the screen's filter keeps.
    pub(crate) fn download_filter_count(&self, filter: DownloadFilter) -> usize {
        self.downloads
            .entries
            .iter()
            .filter(|entry| filter.matches(entry.row.state))
            .count()
    }

    /// A chip click, and ←/→ on the screen (`step`, stopping at the ends).
    pub(crate) fn set_download_filter(&mut self, filter: DownloadFilter, cx: &mut Context<Self>) {
        self.downloads.filter = filter;
        cx.notify();
    }

    pub(crate) fn move_download_filter(&mut self, step: isize, cx: &mut Context<Self>) {
        let all = DownloadFilter::ALL;
        let at = all
            .iter()
            .position(|f| *f == self.downloads.filter)
            .unwrap_or(0);
        let next = (at as isize + step).clamp(0, all.len() as isize - 1) as usize;
        self.set_download_filter(all[next], cx);
    }

    /// Whether any row is in one of `states`: the palette's download entries.
    pub(crate) fn any_download_in(&self, states: &[DownloadState]) -> bool {
        self.downloads
            .entries
            .iter()
            .any(|entry| states.contains(&entry.row.state))
    }

    /// Bytes a second across running downloads; 0 when none reports one.
    pub(crate) fn download_speed(&self) -> u64 {
        self.downloads
            .entries
            .iter()
            .filter(|entry| entry.row.state == DownloadState::Running)
            .filter_map(|entry| entry.progress.as_ref())
            .map(|progress| progress.speed_bytes_per_sec)
            .sum()
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
        self.downloads.focused = self
            .download_display_order(DownloadSurface::Panel)
            .first()
            .map(|e| e.row.id);
        cx.notify();
    }

    pub(crate) fn close_downloads_panel(&mut self, cx: &mut Context<Self>) {
        self.downloads.open = false;
        self.downloads.focused = None;
        self.downloads.highlight = None;
        cx.notify();
    }

    /// ↑/↓ in the open dropdown (`Panel`) or on the Downloads screen.
    pub(crate) fn move_download_focus(
        &mut self,
        step: isize,
        surface: DownloadSurface,
        cx: &mut Context<Self>,
    ) {
        let order: Vec<i64> = self
            .download_display_order(surface)
            .iter()
            .map(|entry| entry.row.id)
            .collect();
        if order.is_empty() {
            return;
        }
        let focused = match surface {
            DownloadSurface::Panel => self.downloads.focused,
            DownloadSurface::Screen => self.downloads.screen_focused,
        };
        let next = match focused.and_then(|id| order.iter().position(|o| *o == id)) {
            Some(at) => (at as isize + step).clamp(0, order.len() as isize - 1) as usize,
            None => 0,
        };
        let id = order[next];
        // Both lists show each non-empty section's header, then its rows.
        let mut child = 0;
        let mut target = None;
        for (_, rows) in self.download_sections(surface) {
            if let Some(at) = rows.iter().position(|entry| entry.row.id == id) {
                target = Some(child + 1 + at);
                break;
            }
            child += 1 + rows.len();
        }
        let scroll = match surface {
            DownloadSurface::Panel => {
                self.downloads.focused = Some(id);
                &self.downloads.scroll
            }
            DownloadSurface::Screen => {
                self.downloads.screen_focused = Some(id);
                &self.downloads.screen_scroll
            }
        };
        if let Some(target) = target {
            scroll.scroll_to_item(target);
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
            self.downloads.focused = self
                .download_display_order(DownloadSurface::Panel)
                .first()
                .map(|e| e.row.id);
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
