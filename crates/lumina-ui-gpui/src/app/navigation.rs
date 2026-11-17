use gpui::*;
use lumina_core::MediaId;

use crate::app::history::{NavigationHistory, NavigationSnapshot};
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    /// Snapshot of the durable browsing destination used by Back/Forward.
    pub(crate) fn navigation_snapshot(&self) -> NavigationSnapshot {
        NavigationSnapshot {
            selection: self.selection.clone(),
            search_query: self.search_query.clone(),
            active_year: self.active_year,
            selected_media_id: self.selected_item.as_ref().map(|item| item.id),
            selected_index: self.selected_index,
            selection_anchor: self.selection_anchor,
            viewer_open: self.is_fullscreen_viewer,
        }
    }

    /// Records a destination change that the caller already applied. Every
    /// view, search and year change ends here (Back/Forward in
    /// `apply_navigation_snapshot`), so an empty state's Tab focus is
    /// dropped here: coming back to a view starts with no focused button.
    pub(crate) fn record_navigation_change(&mut self, departing: NavigationSnapshot) {
        self.active_view.empty_focus = None;
        if self.navigation_history.is_restoring() {
            return;
        }
        let arriving = self.navigation_snapshot();
        self.navigation_history.record(departing, arriving);
    }

    /// Re-seeds the trail after a library switch or startup restore. Those are
    /// jumps, not user navigation, so they never create a Back entry.
    pub(crate) fn reset_navigation_history(&mut self) {
        self.navigation_history = NavigationHistory::new(self.navigation_snapshot());
    }

    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        if self.compare_holds_view() {
            return;
        }
        self.navigation_history
            .sync_pending(self.navigation_snapshot());
        let Some(snapshot) = self.navigation_history.go_back() else {
            cx.notify();
            return;
        };
        self.apply_navigation_snapshot(snapshot, cx);
    }

    /// Redoes a Back. Syncing first means a destination reached since the
    /// Back (a typed search, say) drops the Forward entries, like a browser.
    pub fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        if self.compare_holds_view() {
            return;
        }
        self.navigation_history
            .sync_pending(self.navigation_snapshot());
        let Some(snapshot) = self.navigation_history.go_forward() else {
            cx.notify();
            return;
        };
        self.apply_navigation_snapshot(snapshot, cx);
    }

    /// Back would move: a recorded entry precedes the cursor, or the live
    /// state drifted (an unsynced typed search can always go Back).
    pub(crate) fn can_navigate_back(&self) -> bool {
        self.navigation_history.can_go_back()
            || self
                .navigation_history
                .has_pending_destination(&self.navigation_snapshot())
    }

    /// Forward would move: an entry is ahead and no unsynced destination
    /// would truncate it first.
    pub(crate) fn can_navigate_forward(&self) -> bool {
        self.navigation_history.can_go_forward()
            && !self
                .navigation_history
                .has_pending_destination(&self.navigation_snapshot())
    }

    fn apply_navigation_snapshot(&mut self, snapshot: NavigationSnapshot, cx: &mut Context<Self>) {
        // As in `record_navigation_change`.
        self.active_view.empty_focus = None;
        self.navigation_history.begin_restore();
        self.restore_navigation_snapshot(snapshot, cx);
        self.navigation_history.end_restore();
        // A fallback (removed album/tag/media) may have changed the applied
        // destination; keep the trail pointing at what the user actually sees.
        self.navigation_history
            .replace_current(self.navigation_snapshot());
        cx.notify();
    }

    fn restore_navigation_snapshot(
        &mut self,
        snapshot: NavigationSnapshot,
        cx: &mut Context<Self>,
    ) {
        let viewer_was_open = self.is_fullscreen_viewer;
        if viewer_was_open && !snapshot.viewer_open {
            self.close_viewer(cx);
        }

        // Back/Forward lands on Exact copies with a fresh Similar shots read
        // to come: the old list may hold items trashed meanwhile.
        self.stacks.reset();
        self.albums.overview.leave();
        self.cancel_tag_manager_prompts();
        self.selection = self.resolve_navigation_selection(snapshot.selection);
        self.search_query = snapshot.search_query;
        self.search_focused = false;
        self.active_year = snapshot.active_year.filter(|year| {
            self.timeline_year_buckets.is_empty()
                || self
                    .timeline_year_buckets
                    .iter()
                    .any(|(bucket, _)| bucket == year)
        });
        self.clear_selection(cx);

        if let Some(media_id) = snapshot.selected_media_id {
            if let Some(index) = self.navigation_index_for(media_id, snapshot.selected_index) {
                let total = self.get_active_count();
                if let Some(item) = self.get_active_slice(index..index + 1).into_iter().next() {
                    self.selected_indices.insert(index);
                    self.selected_index = Some(index);
                    self.selection_anchor = snapshot
                        .selection_anchor
                        .filter(|anchor| *anchor < total)
                        .or(Some(index));
                    self.select_item_internal(item, cx);
                    self.refresh_selected_tags();
                    self.update_batch_summary();
                    self.reveal_grid_selection();
                }
            }
        }

        if snapshot.viewer_open {
            if self.selected_item.is_some() {
                self.is_fullscreen_viewer = true;
                self.viewer_state.reset();
                // `select_item_internal` already started video only when the
                // viewer was open while selecting; cover the closed -> open jump.
                if !viewer_was_open {
                    self.sync_viewer_player(cx);
                }
            } else {
                // The saved media item is gone: never open a viewer with no item.
                self.is_fullscreen_viewer = false;
            }
        }

        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Falls back to All when a saved user album or tag no longer exists.
    fn resolve_navigation_selection(&self, selection: AlbumSelection) -> AlbumSelection {
        match selection {
            AlbumSelection::User(id)
                if !self.user_smart_albums.iter().any(|album| album.id == id) =>
            {
                AlbumSelection::AllLibrary
            }
            AlbumSelection::Album(id) if self.album_title(id).is_none() => {
                AlbumSelection::AllLibrary
            }
            AlbumSelection::Tag(tag)
                if !self.library_tag_counts.iter().any(|(name, _)| name == &tag) =>
            {
                AlbumSelection::AllLibrary
            }
            other => other,
        }
    }

    /// Resolves a saved media position without per-frame SQL, reusing the
    /// restore-time chunk walk when the saved index no longer matches.
    pub(crate) fn navigation_index_for(
        &self,
        id: MediaId,
        saved_index: Option<usize>,
    ) -> Option<usize> {
        let total = self.get_active_count();
        if total == 0 {
            return None;
        }
        if let Some(index) = saved_index.filter(|index| *index < total) {
            if self
                .get_active_slice(index..index + 1)
                .into_iter()
                .next()
                .map(|item| item.id)
                == Some(id)
            {
                return Some(index);
            }
        }
        self.find_active_index(id)
    }
}
