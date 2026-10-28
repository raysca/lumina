//! Undo/redo for catalog mutations.
//!
//! Only operations that are fully reversible in the catalog are recorded:
//! ratings, color labels, favorites, captions, tags, and trash/restore.
//! Permanent deletion and global tag rename/merge are intentionally excluded.

use anyhow::Result;
use gpui::*;
use lumina_core::{LuminaEngine, MediaId};

use crate::app::LuminaApp;

const MAX_UNDO: usize = 100;

#[derive(Debug, Clone)]
pub enum UndoAction {
    Rating {
        id: MediaId,
        before: u8,
        after: u8,
    },
    ColorLabel {
        id: MediaId,
        before: u8,
        after: u8,
    },
    /// (id, before, after)
    Favorite {
        changes: Vec<(MediaId, bool, bool)>,
    },
    Caption {
        id: MediaId,
        before: Option<String>,
        after: Option<String>,
    },
    Title {
        id: MediaId,
        before: Option<String>,
        after: Option<String>,
    },
    /// Tags were added (\`added = true\`) or removed.
    Tags {
        ids: Vec<MediaId>,
        tag: String,
        added: bool,
    },
    /// (id, before, after)
    Trashed {
        changes: Vec<(MediaId, bool, bool)>,
    },
}

#[derive(Default)]
pub struct UndoState {
    pub undo_stack: Vec<UndoAction>,
    pub redo_stack: Vec<UndoAction>,
}

impl UndoAction {
    /// IDs whose XMP sidecar should be refreshed after applying this action.
    pub fn affected_ids(&self) -> Vec<MediaId> {
        match self {
            UndoAction::Rating { id, .. } | UndoAction::ColorLabel { id, .. } => vec![*id],
            UndoAction::Caption { id, .. } | UndoAction::Title { id, .. } => vec![*id],
            UndoAction::Favorite { changes } | UndoAction::Trashed { changes } => {
                changes.iter().map(|(id, _, _)| *id).collect()
            }
            UndoAction::Tags { ids, .. } => ids.clone(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            UndoAction::Rating { .. } => "rating",
            UndoAction::ColorLabel { .. } => "color label",
            UndoAction::Favorite { .. } => "favorite",
            UndoAction::Caption { .. } => "caption",
            UndoAction::Title { .. } => "title",
            UndoAction::Tags { .. } => "tag edit",
            UndoAction::Trashed { .. } => "trash",
        }
    }

    fn apply_before(&self, engine: &LuminaEngine) -> Result<()> {
        self.apply(engine, true)
    }

    fn apply_after(&self, engine: &LuminaEngine) -> Result<()> {
        self.apply(engine, false)
    }

    fn apply(&self, engine: &LuminaEngine, before: bool) -> Result<()> {
        match self {
            UndoAction::Rating {
                id,
                before: b,
                after: a,
            } => engine.set_rating(*id, if before { *b } else { *a }),
            UndoAction::ColorLabel {
                id,
                before: b,
                after: a,
            } => engine.set_color_label(*id, if before { *b } else { *a }),
            UndoAction::Favorite { changes } => set_bool_changes(changes, before, |ids, value| {
                engine.batch_set_favorite(ids, value)
            }),
            UndoAction::Caption {
                id,
                before: b,
                after: a,
            } => {
                let value = if before { b } else { a };
                engine.set_caption(*id, value.as_deref().unwrap_or(""))
            }
            UndoAction::Title {
                id,
                before: b,
                after: a,
            } => {
                let value = if before { b } else { a };
                engine.set_title(*id, value.as_deref())
            }
            UndoAction::Tags { ids, tag, added } => {
                let tags = std::slice::from_ref(tag);
                let present = if before { !*added } else { *added };
                if present {
                    engine.add_tags_batch(ids, tags)
                } else {
                    engine.remove_tags_batch(ids, tags)
                }
            }
            UndoAction::Trashed { changes } => set_bool_changes(changes, before, |ids, value| {
                engine.batch_set_trashed(ids, value)
            }),
        }
    }
}

fn set_bool_changes(
    changes: &[(MediaId, bool, bool)],
    before: bool,
    mut apply: impl FnMut(&[MediaId], bool) -> Result<()>,
) -> Result<()> {
    let mut true_ids = Vec::new();
    let mut false_ids = Vec::new();
    for (id, b, a) in changes {
        let value = if before { *b } else { *a };
        if value {
            true_ids.push(*id);
        } else {
            false_ids.push(*id);
        }
    }
    if !false_ids.is_empty() {
        apply(&false_ids, false)?;
    }
    if !true_ids.is_empty() {
        apply(&true_ids, true)?;
    }
    Ok(())
}

impl LuminaApp {
    pub fn push_undo(&mut self, action: UndoAction) {
        if self.undo.undo_stack.len() >= MAX_UNDO {
            self.undo.undo_stack.remove(0);
        }
        self.undo.undo_stack.push(action);
        self.undo.redo_stack.clear();
    }

    pub fn undo_last(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.undo.undo_stack.pop() else {
            self.set_last_action("Nothing to undo");
            cx.notify();
            return;
        };
        if self.apply_history_action(&action, true, cx) {
            let label = action.label();
            let affected = action.affected_ids();
            self.undo.redo_stack.push(action);
            self.refresh_after_history(cx);
            self.sync_metadata_sidecars(&affected);
            self.set_last_action(format!("Undid {label}"));
        } else {
            self.undo.undo_stack.push(action);
        }
        cx.notify();
    }

    pub fn redo_last(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.undo.redo_stack.pop() else {
            self.set_last_action("Nothing to redo");
            cx.notify();
            return;
        };
        if self.apply_history_action(&action, false, cx) {
            let label = action.label();
            let affected = action.affected_ids();
            self.undo.undo_stack.push(action);
            self.refresh_after_history(cx);
            self.sync_metadata_sidecars(&affected);
            self.set_last_action(format!("Redid {label}"));
        } else {
            self.undo.redo_stack.push(action);
        }
        cx.notify();
    }

    fn apply_history_action(
        &mut self,
        action: &UndoAction,
        before: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before undoing", cx);
            return false;
        };
        let result = if before {
            action.apply_before(&engine)
        } else {
            action.apply_after(&engine)
        };
        match result {
            Ok(()) => true,
            Err(err) => {
                self.set_error(format!("History action failed: {err}"), cx);
                false
            }
        }
    }

    pub(crate) fn refresh_after_history(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.grid_state.as_ref() {
            self.item_ratings = state.engine.get_ratings_map();
            self.item_color_labels = state.engine.get_color_labels_map();
            self.item_captions = state.engine.get_captions_map();
            let updated_selected = self
                .selected_item
                .as_ref()
                .and_then(|item| state.engine.get_item_by_id(item.id));
            if let Some(item) = updated_selected {
                self.selected_item_tags = state.engine.get_tags_for_item(item.id);
                self.selected_item = Some(item);
            }
        }
        self.refresh_album_counts();
        cx.notify();
    }
}
