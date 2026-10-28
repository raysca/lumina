use gpui::*;
use lumina_core::{DeletionTarget, MediaId, SystemAlbum};

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
        self.set_last_action(if new_trashed {
            format!("Moved {} item(s) to Trash", ids.len())
        } else {
            format!("Restored {} item(s)", ids.len())
        });
        self.refresh_album_counts();
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
        });
        cx.notify();
    }

    pub fn prompt_delete_selected(&mut self, cx: &mut Context<Self>) {
        let items = self.get_selected_items();
        if items.is_empty() {
            return;
        }
        let ids: Vec<MediaId> = items.iter().map(|i| i.id).collect();
        let count = ids.len();
        self.delete_confirm_state = Some(DeleteConfirmState {
            intent: DeleteIntent::DeleteSelected,
            target: DeletionTarget::SystemTrash,
            item_ids: ids,
            item_count: count,
            permanent_acknowledged: false,
        });
        cx.notify();
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
        if let Some(st) = self.delete_confirm_state.take() {
            if st.target == DeletionTarget::PermanentDisk && !st.permanent_acknowledged {
                return;
            }
            if let Some(ref state) = self.grid_state {
                match st.intent {
                    DeleteIntent::EmptyTrash => {
                        let _ = state.engine.empty_trash(st.target);
                    }
                    DeleteIntent::DeleteSelected => {
                        let _ = state
                            .engine
                            .permanently_delete_items(&st.item_ids, st.target);
                    }
                }
            }
            self.set_last_action(match st.intent {
                DeleteIntent::EmptyTrash => "Trash emptied".to_string(),
                DeleteIntent::DeleteSelected => {
                    format!("Permanently deleted {} item(s)", st.item_ids.len())
                }
            });
            self.refresh_album_counts();
            self.clear_selection(cx);
        }
    }

    pub fn restore_all_trash(&mut self, cx: &mut Context<Self>) {
        if let Some(ref state) = self.grid_state {
            let _ = state.engine.restore_all_trash();
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
            self.set_error(format!("Could not restore items: {err}"), cx);
            return;
        }
        self.push_undo(UndoAction::Trashed { changes });
        self.set_last_action(format!("Restored {} item(s)", ids.len()));
        self.refresh_album_counts();
        self.clear_selection(cx);
    }
}
