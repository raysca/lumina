use gpui::*;
use lumina_core::{DeletionTarget, MediaId, MediaType, SystemAlbum};

use crate::app::types::{DeleteConfirmState, DeleteIntent};
use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    pub fn trash_or_restore_selected(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.selected_item.clone() else {
            return;
        };
        let new_trashed = !item.is_trashed;
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_trashed(item.id, new_trashed)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                if let Some(ref mut current) = self.selected_item {
                    current.is_trashed = new_trashed;
                }
                self.push_undo(UndoAction::Trashed {
                    changes: vec![(item.id, item.is_trashed, new_trashed)],
                });
                self.refresh_album_counts();
                let closed_pip = new_trashed && self.close_pip_for_removed(&[item.id], cx);
                self.show_undo_toast(
                    match (new_trashed, closed_pip) {
                        (true, true) => "Moved to Trash · Picture-in-Picture closed",
                        (true, false) => "Moved to Trash",
                        _ => "Restored from Trash",
                    },
                    cx,
                );
                if let Some(idx) = self.selected_index {
                    self.select_index(idx, cx);
                } else {
                    cx.notify();
                }
            }
            Err(err) => self.set_error(format!("Could not update Trash: {err}"), cx),
        }
    }

    pub fn batch_trash_or_restore(&mut self, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let is_trash_view = self.selection == AlbumSelection::Trash;
        let new_trashed = !is_trash_view;
        let ids: Vec<MediaId> = items.iter().map(|i| i.id).collect();
        let changes: Vec<(MediaId, bool, bool)> = items
            .iter()
            .map(|item| (item.id, item.is_trashed, new_trashed))
            .collect();

        let result = if let Some(ref state) = self.grid_state {
            state.engine.batch_set_trashed(&ids, new_trashed)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not update Trash: {err}"), cx);
            return;
        }
        self.push_undo(UndoAction::Trashed { changes });
        let closed_pip = new_trashed && self.close_pip_for_removed(&ids, cx);
        let message = match (new_trashed, closed_pip) {
            (true, true) => format!(
                "Moved {} item(s) to Trash · Picture-in-Picture closed",
                ids.len()
            ),
            (true, false) => format!("Moved {} item(s) to Trash", ids.len()),
            _ => format!("Restored {} item(s)", ids.len()),
        };
        self.set_last_action(message.clone());
        self.refresh_album_counts();
        self.show_undo_toast(message, cx);
        self.clear_selection(cx);
    }

    pub fn prompt_empty_trash(&mut self, cx: &mut Context<Self>) {
        let count = self
            .grid_state
            .as_ref()
            .map(|s| {
                s.engine
                    .query_system_album_count(SystemAlbum::RecentlyDeleted)
            })
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        self.delete_confirm_state = Some(DeleteConfirmState {
            intent: DeleteIntent::EmptyTrash,
            target: DeletionTarget::SystemTrash,
            item_ids: Vec::new(),
            item_count: count,
            permanent_acknowledged: false,
            photos: 0,
            videos: 0,
            total_bytes: 0,
            preview_ids: Vec::new(),
        });
        cx.notify();
    }

    /// Every delete (⌫, menu, dock, inspector) asks first (v3). Move to
    /// Trash is preselected, so ⌫ then ↵ stays a two-key, undoable delete.
    pub fn prompt_delete_selected(&mut self, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let ids: Vec<MediaId> = items.iter().map(|i| i.id).collect();
        let videos = items
            .iter()
            .filter(|i| i.media_type == MediaType::Video)
            .count();
        self.delete_confirm_state = Some(DeleteConfirmState {
            intent: if self.selection == AlbumSelection::Trash {
                DeleteIntent::DeleteSelected
            } else {
                DeleteIntent::LibrarySelection
            },
            target: DeletionTarget::SystemTrash,
            item_count: ids.len(),
            preview_ids: ids.iter().take(3).copied().collect(),
            item_ids: ids,
            permanent_acknowledged: false,
            photos: items.len() - videos,
            videos,
            total_bytes: items.iter().map(|i| i.file_size).sum(),
        });
        cx.notify();
    }

    /// ↑/↓ in the dialog: previous/next option.
    pub fn step_delete_target(&mut self, delta: isize, cx: &mut Context<Self>) {
        const ORDER: [DeletionTarget; 3] = [
            DeletionTarget::SystemTrash,
            DeletionTarget::CatalogOnly,
            DeletionTarget::PermanentDisk,
        ];
        let Some(current) = self.delete_confirm_state.as_ref().map(|st| st.target) else {
            return;
        };
        let index = ORDER.iter().position(|t| *t == current).unwrap_or(0) as isize;
        let next = ORDER[(index + delta).clamp(0, ORDER.len() as isize - 1) as usize];
        self.set_delete_target(next, cx);
    }

    pub fn close_delete_modal(&mut self, cx: &mut Context<Self>) {
        self.delete_confirm_state = None;
        cx.notify();
    }

    pub fn set_delete_target(&mut self, target: DeletionTarget, cx: &mut Context<Self>) {
        if let Some(ref mut st) = self.delete_confirm_state {
            st.target = target;
            st.permanent_acknowledged = false;
            cx.notify();
        }
    }

    pub fn toggle_delete_permanent_acknowledged(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut st) = self.delete_confirm_state {
            st.permanent_acknowledged = !st.permanent_acknowledged;
            cx.notify();
        }
    }

    pub fn execute_confirmed_deletion(&mut self, cx: &mut Context<Self>) {
        // Permanent deletion needs the acknowledgement; until then Enter does
        // nothing and the dialog stays open.
        if self.delete_confirm_state.as_ref().is_some_and(|st| {
            st.target == DeletionTarget::PermanentDisk && !st.permanent_acknowledged
        }) {
            return;
        }
        // From the library, Move to Trash is the soft, undoable trash.
        // One item keeps the grid index so ⌫↵ walks forward through the grid.
        if let Some(st) = self.delete_confirm_state.as_ref().filter(|st| {
            st.intent == DeleteIntent::LibrarySelection && st.target == DeletionTarget::SystemTrash
        }) {
            let single = st.item_count == 1;
            self.delete_confirm_state = None;
            if single {
                self.trash_or_restore_selected(cx);
            } else {
                self.batch_trash_or_restore(cx);
            }
            return;
        }
        if let Some(st) = self.delete_confirm_state.take() {
            // Emptying the Trash removes a trashed video playing in PiP.
            let removed_pip: Vec<MediaId> = match st.intent {
                DeleteIntent::DeleteSelected | DeleteIntent::LibrarySelection => {
                    st.item_ids.clone()
                }
                DeleteIntent::EmptyTrash => self
                    .video
                    .item
                    .as_ref()
                    .filter(|_| self.video.in_pip)
                    .and_then(|item| {
                        let state = self.grid_state.as_ref()?;
                        state.engine.get_item_by_id(item.id)
                    })
                    .filter(|item| item.is_trashed)
                    .map(|item| vec![item.id])
                    .unwrap_or_default(),
            };
            if let Some(ref state) = self.grid_state {
                let result = match st.intent {
                    DeleteIntent::EmptyTrash => state.engine.empty_trash(st.target).map(|_| ()),
                    DeleteIntent::DeleteSelected | DeleteIntent::LibrarySelection => state
                        .engine
                        .permanently_delete_items(&st.item_ids, st.target)
                        .map(|_| ()),
                };
                if let Err(err) = result {
                    self.set_error(format!("Could not delete: {err:#}"), cx);
                    self.refresh_album_counts();
                    return;
                }
            }
            let closed_pip = self.close_pip_for_removed(&removed_pip, cx);
            let mut message = match st.intent {
                DeleteIntent::EmptyTrash => "Trash emptied".to_string(),
                _ if st.target == DeletionTarget::CatalogOnly => {
                    format!("Removed {} item(s) from Lumina", st.item_ids.len())
                }
                DeleteIntent::DeleteSelected | DeleteIntent::LibrarySelection => {
                    format!("Permanently deleted {} item(s)", st.item_ids.len())
                }
            };
            if closed_pip {
                message.push_str(" · Picture-in-Picture closed");
                self.show_toast(message.clone(), cx);
            }
            self.set_last_action(message);
            self.refresh_album_counts();
            self.clear_selection(cx);
        }
    }

    pub fn restore_all_trash(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self
            .grid_state
            .as_ref()
            .map(|state| state.engine.restore_all_trash())
        else {
            return;
        };
        match result {
            Ok(n) => self.show_toast(format!("Restored {n} item(s)"), cx),
            // Restoring everything is idempotent, so Retry is safe here.
            Err(err) => self.set_error_with_retry(
                format!("Could not restore items: {err}"),
                Self::restore_all_trash,
                cx,
            ),
        }
        self.refresh_album_counts();
        self.clear_selection(cx);
    }

    pub fn restore_selected_trash(&mut self, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let ids: Vec<MediaId> = items.iter().map(|i| i.id).collect();
        let changes: Vec<(MediaId, bool, bool)> =
            items.iter().map(|item| (item.id, true, false)).collect();
        let result = if let Some(ref state) = self.grid_state {
            state.engine.restore_trashed_items(&ids).map(|_| ())
        } else {
            Ok(())
        };
        if let Err(err) = result {
            // No Retry: it would act on whatever is selected by then.
            self.set_error(format!("Could not restore items: {err}"), cx);
            return;
        }
        self.push_undo(UndoAction::Trashed { changes });
        let message = format!("Restored {} item(s)", ids.len());
        self.set_last_action(message.clone());
        self.refresh_album_counts();
        self.show_undo_toast(message, cx);
        self.clear_selection(cx);
    }
}
