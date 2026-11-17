//! Restoring the saved session (view, search, year, selection, scroll) after
//! the first scan.

use gpui::*;
use lumina_core::{Album, MediaId, SmartAlbum};

use crate::app::mosaic_layout::PendingScroll;
use crate::app::mosaic_nav::restore_offset;
use crate::app::LuminaApp;
use crate::config::LastView;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    /// Applies the one-shot session captured at startup.
    ///
    /// Called exactly once, after the initial scan completes so the library's
    /// user albums and counts are known. The view/search/year are restored even
    /// when the saved media item has disappeared; in that case no item is selected.
    ///
    /// `user_moved`: the live state left the startup default (album, typed
    /// search, year, card, viewer) before the scan finished. Their choice
    /// then wins: the restore is dropped and their Back trail stays.
    pub(super) fn apply_restored_session(&mut self, user_moved: bool, cx: &mut Context<Self>) {
        if self
            .grid_state
            .as_ref()
            .map(|state| state.is_scanning)
            .unwrap_or(true)
        {
            return;
        }
        let Some(restored) = self.pending_restore.take() else {
            return;
        };
        // One snapshot check covers every path, including a typed search
        // that never records a navigation change.
        if user_moved {
            self.save_config();
            return;
        }

        // ponytail: a queue still loading counts as not empty, so the
        // screen restores and shows its empty state; the load is one small
        // query started before the scan, so it has nearly always landed.
        let downloads_empty = self.downloads.loaded && self.downloads.entries.is_empty();
        self.selection = resolve_restored_view(
            restored.view,
            &self.user_smart_albums,
            &self.albums.list,
            downloads_empty,
        );
        self.search_query = restored.search_query;
        // A session saved before D11 can hold an album with a query. The
        // trail is reset below, so no Back step is needed.
        if !self.search_query.trim().is_empty() {
            self.selection = AlbumSelection::AllLibrary;
        }
        self.search_focused = false;
        self.active_year = restored.active_year;
        // A session saved before years were scoped to the library.
        if self.active_year.is_some() {
            self.selection = AlbumSelection::AllLibrary;
        }

        // The startup open already cleared these, but restoring must never
        // inherit stale position-based state if a scan lands late.
        self.selected_item = None;
        self.selected_index = None;
        self.selected_indices.clear();
        self.selection_anchor = None;
        self.selected_item_tags.clear();
        self.selected_tag_counts.clear();
        self.batch_summary = Default::default();
        self.set_viewer_image(None);

        if let Some(id) = restored.selected_media_id {
            let media_id = MediaId(id);
            if let Some(index) = self.find_active_index(media_id) {
                if let Some(item) = self.get_active_slice(index..index + 1).into_iter().next() {
                    self.selected_indices.insert(index);
                    self.selection_anchor = Some(index);
                    self.selected_index = Some(index);
                    self.select_item_internal(item, cx);
                    self.refresh_selected_tags();
                    self.update_batch_summary();
                }
            }
        }
        self.restore_grid_scroll(restored.scroll_anchor_id.map(MediaId), cx);
        // The startup restore is a jump, not user navigation: seed the trail
        // with the restored destination so Back cannot return to pre-restore state.
        self.reset_navigation_history();
        // Persist the applied session once so a startup window-bounds save cannot
        // leave an empty session on disk before the initial scan finishes.
        self.save_config();
        cx.notify();
    }

    /// Scrolls the restored grid so the saved anchor item's row is on top.
    ///
    /// GPUI keeps one deferred scroll, so revealing the selection afterwards
    /// would replace the anchor scroll: the selection is revealed only when
    /// the anchor is gone or the selected row falls outside the anchored
    /// viewport. In Mosaic the hidden Grid list is left alone: the anchor
    /// (else the selection) goes on top now when the view's layout is
    /// there, else once it lands, with the same selection reveal.
    fn restore_grid_scroll(&mut self, anchor: Option<MediaId>, cx: &mut Context<Self>) {
        if self.mosaic_shown() {
            let item = anchor
                .and_then(|id| self.find_active_index(id))
                .or(self.selected_index);
            match (item, self.mosaic_layout(), self.grid_state.as_ref()) {
                (Some(item), Some(layout), Some(state)) if !layout.rows.is_empty() => {
                    let viewport = f32::from(state.mosaic.viewport_bounds().size.height);
                    let offset = restore_offset(&layout, item, self.selected_index, viewport);
                    state.mosaic.scroll_to(offset);
                    cx.notify();
                }
                _ => {
                    self.mosaic.pending_scroll = item.map(|item| PendingScroll {
                        key: self.view_key(),
                        item,
                        reveal: self.selected_index,
                    })
                }
            }
            return;
        }
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let cols = self.grid_cols();
        let Some(anchor_row) = anchor
            .and_then(|id| self.find_active_index(id))
            .map(|index| index / cols)
        else {
            self.reveal_grid_selection();
            return;
        };
        state
            .scroll_handle
            .scroll_to_item_strict(anchor_row, gpui::ScrollStrategy::Top);
        let viewport = f32::from(
            state
                .scroll_handle
                .0
                .borrow()
                .base_handle
                .bounds()
                .size
                .height,
        );
        // Zero before the first layout: the anchor wins then.
        let visible_rows = (viewport / self.row_height()) as usize;
        let selected_row = self.selected_index.map(|index| index / cols);
        if selected_row.is_some_and(|row| {
            visible_rows > 0 && !(anchor_row..anchor_row + visible_rows).contains(&row)
        }) {
            self.reveal_grid_selection();
        }
    }

    /// Finds a media item's position in the active result set without per-frame SQL.
    ///
    /// The core engine has no index-of-id query for filtered views, so this walks
    /// the active slices once in bounded chunks during restore only.
    pub(crate) fn find_active_index(&self, id: MediaId) -> Option<usize> {
        const CHUNK: usize = 512;
        let total = self.get_active_count();
        let mut offset = 0;
        while offset < total {
            let end = (offset + CHUNK).min(total);
            let slice = self.get_active_slice(offset..end);
            if slice.is_empty() {
                break;
            }
            if let Some(position) = slice.iter().position(|item| item.id == id) {
                return Some(offset + position);
            }
            offset = end;
        }
        None
    }
}

/// Maps the persisted view onto the current library, falling back to All when
/// the saved smart album or album no longer exists, or the saved Downloads
/// screen has no downloads to show (`downloads_empty`).
pub(crate) fn resolve_restored_view(
    view: LastView,
    user_albums: &[SmartAlbum],
    albums: &[Album],
    downloads_empty: bool,
) -> AlbumSelection {
    match view {
        LastView::Downloads if downloads_empty => AlbumSelection::AllLibrary,
        LastView::User(id) if !user_albums.iter().any(|album| album.id == id) => {
            AlbumSelection::AllLibrary
        }
        LastView::Album(id) if !albums.iter().any(|album| album.id == id) => {
            AlbumSelection::AllLibrary
        }
        view => view.to_selection(),
    }
}

#[cfg(test)]
mod restore_tests {
    use super::*;
    use crate::app::types::UiMessage;
    use crate::views::grid::GridState;
    use lumina_core::MatchMode;
    use lumina_core::{EngineEvent, LuminaEngine, SystemAlbum};

    fn user_album(id: i64) -> SmartAlbum {
        let mut album = SmartAlbum::new(
            "Trips".to_string(),
            "folder".to_string(),
            MatchMode::All,
            Vec::new(),
        );
        album.id = id;
        album
    }

    #[core::prelude::v1::test]
    fn removed_user_album_falls_back_to_all() {
        assert_eq!(
            resolve_restored_view(LastView::User(7), &[], &[], false),
            AlbumSelection::AllLibrary,
            "a removed smart album must not restore a phantom view"
        );
        assert_eq!(
            resolve_restored_view(LastView::User(7), &[user_album(7)], &[], false),
            AlbumSelection::User(7),
            "an existing smart album restores its view"
        );
    }

    #[core::prelude::v1::test]
    fn tag_view_restores_without_a_user_album() {
        assert_eq!(
            resolve_restored_view(LastView::Tag("travel".to_string()), &[], &[], false),
            AlbumSelection::Tag("travel".to_string()),
            "tag views do not depend on smart album ids"
        );
    }

    fn scanned_engine(prefix: &str) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
        let temp_dir = std::env::temp_dir().join(format!(
            "lumina_restore_{}_{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        for index in 0..3 {
            std::fs::write(
                temp_dir.join(format!("restore_{index:02}.jpg")),
                b"test-jpeg-content",
            )
            .unwrap();
        }
        let (engine, rx) = LuminaEngine::open_with_paths(
            temp_dir.clone(),
            temp_dir.join("catalog.db"),
            Some(temp_dir.join("cache")),
        )
        .unwrap();
        while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
            if let EngineEvent::ScanComplete { .. } = event {
                break;
            }
        }
        (temp_dir, engine)
    }

    #[core::prelude::v1::test]
    fn restores_selection_after_scan_complete_and_survives_a_missing_item() {
        use crate::config::RestoredSession;

        let (temp_dir, engine) = scanned_engine("selection");
        let mut cx = crate::tests::TestApp::single();
        let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

        view.update(cx, |app, cx| {
            app.grid_state = Some(GridState::new(engine.clone()));
            app.refresh_album_counts();
            let items = app.get_active_slice(0..3);
            assert_eq!(items.len(), 3, "the scan must produce three items");
            let target_index = 1;
            let target = items[target_index].clone();
            app.pending_restore = Some(RestoredSession {
                view: LastView::All,
                search_query: String::new(),
                active_year: None,
                selected_media_id: Some(target.id.0),
                scroll_anchor_id: None,
            });

            let total = app
                .grid_state
                .as_ref()
                .map(|state| state.total_count)
                .unwrap_or(0);
            app.handle_ui_message(
                UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
                cx,
            );

            assert!(app.pending_restore.is_none(), "restore must run once");
            assert_eq!(
                app.selected_item.as_ref().map(|item| item.id),
                Some(target.id),
                "the saved media item must be selected after the scan"
            );
            assert_eq!(app.selected_index, Some(target_index));
            assert_eq!(app.selection_anchor, Some(target_index));
            assert!(app.selected_indices.contains(&target_index));
            assert_eq!(app.batch_summary.count, 1);
            let expected_row = target_index / app.grid_cols();
            assert_eq!(
                app.grid_state
                    .as_ref()
                    .map(|state| state.scroll_handle.logical_scroll_top_index()),
                Some(expected_row),
                "the restored row must be revealed"
            );

            // A saved item that no longer exists must keep the view but select nothing.
            // Back to the untouched startup state first: a selection made
            // before the restore would (rightly) cancel it.
            app.clear_selection(cx);
            app.pending_restore = Some(RestoredSession {
                view: LastView::Favorites,
                search_query: String::new(),
                active_year: None,
                selected_media_id: Some(u64::MAX),
                scroll_anchor_id: None,
            });
            app.handle_ui_message(
                UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
                cx,
            );
            assert!(app.pending_restore.is_none());
            assert_eq!(
                app.selection,
                AlbumSelection::System(SystemAlbum::Favorites),
                "the saved view must survive a missing media item"
            );
            assert!(
                app.selected_item.is_none(),
                "no stale item may stay selected"
            );
            assert!(app.selected_indices.is_empty());
        });

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
