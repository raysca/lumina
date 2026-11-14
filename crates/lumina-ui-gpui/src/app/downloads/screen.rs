//! Actions only the Downloads screen offers: Move up, Retry all, Clear
//! queue, the completed row's "…" menu, and the screen's keys.

use gpui::*;
use lumina_core::DownloadState;

use super::{DownloadEntry, DownloadSurface};
use crate::LuminaApp;

/// The "…" menu's items for a row, as selector keys, top to bottom.
pub(crate) fn row_menu_items(entry: &DownloadEntry) -> Vec<&'static str> {
    let row = &entry.row;
    let finished = matches!(row.state, DownloadState::Done | DownloadState::Duplicate);
    let mut items = Vec::new();
    if finished && row.in_library && row.media_id.is_some() {
        items.push("show");
    }
    if row.state == DownloadState::Duplicate {
        items.push("again");
    }
    items.push("remove");
    items
}

impl LuminaApp {
    /// Move up (⌥⌘↑ and the row's arrow): a waiting row trades places with
    /// the waiting row before it, here at once and in the catalog off the
    /// UI thread. A failed write swaps them back and shows the banner.
    /// One move at a time: a second press while the first is being stored
    /// does nothing, so a revert never undoes the wrong swap.
    pub fn move_download_up(&mut self, row_id: i64, cx: &mut Context<Self>) {
        if self.downloads.moving {
            return;
        }
        let waiting = |e: &DownloadEntry| e.row.state == DownloadState::Waiting && e.row.id > 0;
        let entries = &self.downloads.entries;
        let Some(at) = entries
            .iter()
            .position(|e| e.row.id == row_id && waiting(e))
        else {
            return;
        };
        let Some(before) = entries[..at].iter().rposition(waiting) else {
            return;
        };
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        let other = entries[before].row.id;
        swap_places(&mut self.downloads.entries, before, at);
        self.downloads.moving = true;
        cx.notify();

        let generation = self.downloads.generation;
        let swap = cx
            .background_executor()
            .spawn(async move { catalog.swap_download_order(row_id, other) });
        cx.spawn(async move |this, cx| {
            let result = swap.await;
            let _ = this.update(cx, |app, cx| {
                // A library switch reset the queue (and the flag) already.
                if app.downloads.generation != generation {
                    return;
                }
                app.downloads.moving = false;
                if let Err(err) = result {
                    log::error!("moving download {row_id} up: {err:#}");
                    let (a, b) = (
                        app.downloads.index_of(row_id),
                        app.downloads.index_of(other),
                    );
                    if let (Some(a), Some(b)) = (a, b) {
                        swap_places(&mut app.downloads.entries, a.min(b), a.max(b));
                    }
                    app.set_error("Couldn't move the download. Try again.", cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Retry all (the Needs attention header, ⌘K Retry Failed Downloads):
    /// every failure that can be retried, back to the queue in list order.
    pub fn retry_all_downloads(&mut self, cx: &mut Context<Self>) {
        let failed: Vec<(i64, bool)> = self
            .downloads
            .entries
            .iter()
            .filter(|e| e.row.state == DownloadState::Failed && e.row.can_retry)
            .map(|e| (e.row.id, e.skip_prior_check))
            .collect();
        self.requeue_downloads(failed, cx);
    }

    /// Clear queue (the Queued header): removes every waiting row. Running,
    /// paused, failed and finished rows stay. A failed write puts the rows
    /// back and shows the banner.
    pub fn clear_queue(&mut self, cx: &mut Context<Self>) {
        let Some(catalog) = self.download_catalog() else {
            return;
        };
        let (cleared, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.downloads.entries)
            .into_iter()
            .partition(|e| e.row.state == DownloadState::Waiting);
        self.downloads.entries = kept;
        if cleared.is_empty() {
            return;
        }
        if self.downloads.entries.is_empty() {
            self.close_downloads_panel(cx);
        }
        cx.notify();
        // A placeholder is removed once its insert returns.
        let ids: Vec<i64> = cleared
            .iter()
            .map(|e| e.row.id)
            .filter(|id| *id > 0)
            .collect();
        let generation = self.downloads.generation;
        let removal = cx.background_executor().spawn(async move {
            ids.into_iter()
                .filter(|&id| match catalog.remove_download(id) {
                    Ok(()) => false,
                    Err(err) => {
                        log::error!("removing download {id}: {err:#}");
                        true
                    }
                })
                .collect::<Vec<i64>>()
        });
        let mut cleared = Some(cleared);
        cx.spawn(async move |this, cx| {
            let failed = removal.await;
            if failed.is_empty() {
                return;
            }
            let _ = this.update(cx, |app, cx| {
                if app.downloads.generation != generation {
                    return;
                }
                // Still stored: put them back where they were.
                let back = cleared.take().unwrap_or_default();
                app.downloads
                    .entries
                    .extend(back.into_iter().filter(|e| failed.contains(&e.row.id)));
                app.downloads
                    .entries
                    .sort_by_key(|e| (e.row.id < 0, e.row.added_epoch_ms, e.row.id));
                app.set_error("Couldn't clear the queue. Try again.", cx);
                app.pump_downloads(cx);
            });
        })
        .detach();
    }

    /// The "…" button: opens the row's menu on its first item, or closes it.
    pub(crate) fn toggle_download_row_menu(&mut self, row_id: i64, cx: &mut Context<Self>) {
        self.downloads.row_menu = match self.downloads.row_menu {
            Some((open, _)) if open == row_id => None,
            _ => Some((row_id, 0)),
        };
        self.downloads.screen_focused = Some(row_id);
        cx.notify();
    }

    pub(crate) fn close_download_row_menu(&mut self, cx: &mut Context<Self>) {
        self.downloads.row_menu = None;
        cx.notify();
    }

    /// Runs `item` (a [`row_menu_items`] key) on the row. The menu closes
    /// first.
    pub(crate) fn run_download_row_menu(
        &mut self,
        row_id: i64,
        item: &str,
        cx: &mut Context<Self>,
    ) {
        self.downloads.row_menu = None;
        match item {
            "show" => self.download_main_action(row_id, cx),
            "again" => self.download_again(row_id, cx),
            "remove" => self.cancel_or_remove_download(row_id, cx),
            _ => {}
        }
        cx.notify();
    }

    /// Menu keys: ↑/↓ move, ↵ or Space runs, Esc closes. The caller (the
    /// modal shield) absorbs every other key, so ⌫ never acts behind it.
    pub(crate) fn handle_download_row_menu_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some((row_id, at)) = self.downloads.row_menu else {
            return;
        };
        let Some(items) = self
            .downloads
            .entries
            .iter()
            .find(|e| e.row.id == row_id)
            .map(row_menu_items)
        else {
            self.close_download_row_menu(cx);
            return;
        };
        match key {
            "up" => self.downloads.row_menu = Some((row_id, at.saturating_sub(1))),
            "down" => self.downloads.row_menu = Some((row_id, (at + 1).min(items.len() - 1))),
            "enter" | "return" | "space" => {
                let item = items[at.min(items.len() - 1)];
                self.run_download_row_menu(row_id, item, cx);
            }
            "escape" => self.downloads.row_menu = None,
            _ => {}
        }
        cx.notify();
    }

    /// The Downloads screen's keys: ←/→ move between the filter chips, ↑/↓
    /// between rows, ↵ runs the row's main action, ⌘↵ Download Again, ⌫
    /// removes, ⌥⌘↑ moves a queued row up, Esc drops the focus. Text
    /// fields and modals (the "…" menu among them) take keys first.
    pub(crate) fn handle_downloads_screen_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.downloads_screen_shown() || self.is_fullscreen_viewer || self.search_focused {
            return false;
        }
        let m = &keystroke.modifiers;
        if m.control || m.shift || m.function {
            return false;
        }
        // Keys act only on a row the filter shows.
        let focused = self.downloads.screen_focused.filter(|id| {
            self.download_display_order(DownloadSurface::Screen)
                .iter()
                .any(|e| e.row.id == *id)
        });
        match (keystroke.key.as_str(), m.platform, m.alt) {
            ("up", true, true) => {
                if let Some(id) = focused {
                    self.move_download_up(id, cx);
                }
            }
            (_, _, true) => return false,
            ("enter" | "return", true, _) => {
                if let Some(id) = focused {
                    self.download_again(id, cx);
                }
            }
            (_, true, _) => return false,
            ("left", ..) => self.move_download_filter(-1, cx),
            ("right", ..) => self.move_download_filter(1, cx),
            ("up", ..) => self.move_download_focus(-1, DownloadSurface::Screen, cx),
            ("down", ..) => self.move_download_focus(1, DownloadSurface::Screen, cx),
            ("enter" | "return", ..) => {
                if let Some(id) = focused {
                    self.download_main_action(id, cx);
                }
            }
            ("backspace" | "delete", ..) => {
                if let Some(id) = focused {
                    // Focus moves to the row that takes its place.
                    let order: Vec<i64> = self
                        .download_display_order(DownloadSurface::Screen)
                        .iter()
                        .map(|e| e.row.id)
                        .collect();
                    let at = order.iter().position(|o| *o == id).unwrap_or(0);
                    self.downloads.screen_focused = order
                        .get(at + 1)
                        .or(at.checked_sub(1).and_then(|b| order.get(b)))
                        .copied();
                    self.cancel_or_remove_download(id, cx);
                }
            }
            ("escape", ..) if self.downloads.screen_focused.is_some() => {
                self.downloads.screen_focused = None;
                cx.notify();
            }
            _ => return false,
        }
        true
    }
}

/// Swaps two entries' places in the list and in the queue order.
fn swap_places(entries: &mut [DownloadEntry], before: usize, at: usize) {
    let added = entries[before].row.added_epoch_ms;
    entries[before].row.added_epoch_ms = entries[at].row.added_epoch_ms;
    entries[at].row.added_epoch_ms = added;
    entries.swap(before, at);
}
