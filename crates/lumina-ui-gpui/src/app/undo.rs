//! Undo/redo for catalog mutations.
//!
//! Only operations that are fully reversible in the catalog are recorded:
//! ratings, color labels, favorites, captions, tags, trash/restore, metadata
//! template applies, global keyword rename/merge/delete (exact rows, aliases
//! and smart-album rules), and smart album deletion.
//! Permanent deletion is intentionally excluded.

use anyhow::Result;
use gpui::*;
use lumina_core::{LuminaEngine, MediaId, SmartAlbum, TagChange, TagReplay, TemplateChange};

use crate::app::tag_undo::TagViewRefs;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

const MAX_UNDO: usize = 100;

use lumina_core::Coordinates;
/// One (id, before, after) row in a grouped batch location edit.
pub type BatchLocationChange = (MediaId, Coordinates, Coordinates);

/// Metadata field a grouped batch edit covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchField {
    Title,
    Caption,
}

#[derive(Debug, Clone)]
pub enum UndoAction {
    /// (id, before, after); one entry covers a whole batch.
    Rating { changes: Vec<(MediaId, u8, u8)> },
    /// (id, before, after); one entry covers a whole batch.
    ColorLabel { changes: Vec<(MediaId, u8, u8)> },
    /// (id, before, after)
    Favorite { changes: Vec<(MediaId, bool, bool)> },
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
    Date {
        id: MediaId,
        before: i64,
        after: i64,
    },
    Location {
        id: MediaId,
        before: (Option<f64>, Option<f64>),
        after: (Option<f64>, Option<f64>),
    },
    /// Tags were added (\`added = true\`) or removed.
    Tags {
        ids: Vec<MediaId>,
        tag: String,
        added: bool,
    },
    /// (id, before, after)
    Trashed { changes: Vec<(MediaId, bool, bool)> },
    /// One grouped undo entry for a batch title/caption edit: a single undo
    /// restores every touched item.
    BatchMetadata {
        field: BatchField,
        /// (id, before, after)
        changes: Vec<(MediaId, Option<String>, Option<String>)>,
    },
    /// One grouped undo entry for a batch date edit.
    /// (id, before_epoch_ms, after_epoch_ms)
    BatchDate { changes: Vec<(MediaId, i64, i64)> },
    /// One grouped undo entry for a batch location edit.
    /// (id, before, after)
    BatchLocation { changes: Vec<BatchLocationChange> },
    /// A deleted smart album; undo restores it under the same id.
    SmartAlbumDeleted { album: SmartAlbum },
    /// One metadata template apply: undo restores every item's rights and
    /// removes only the keywords the apply added.
    Template { change: TemplateChange },
    /// A global keyword rename, merge or delete (one entry for a bulk
    /// delete or a multi-tag merge): undo puts back the exact rows, aliases
    /// and smart-album rules.
    TagRows {
        label: &'static str,
        change: TagChange,
        /// The view before the op and right after it; see [`TagViewRefs`].
        view_before: TagViewRefs,
        view_after: TagViewRefs,
        /// True for a rename or move onto a new path: the tag view holds the
        /// same items on both sides, so grid positions stay valid.
        keeps_positions: bool,
    },
    /// A deleted metadata template (user config, not the catalog); undo
    /// puts it back at `index`.
    TemplateDeleted {
        index: usize,
        template: crate::app::templates::MetadataTemplate,
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
            UndoAction::Rating { changes } | UndoAction::ColorLabel { changes } => {
                changes.iter().map(|(id, _, _)| *id).collect()
            }
            UndoAction::Caption { id, .. }
            | UndoAction::Title { id, .. }
            | UndoAction::Date { id, .. }
            | UndoAction::Location { id, .. } => vec![*id],
            UndoAction::Favorite { changes } | UndoAction::Trashed { changes } => {
                changes.iter().map(|(id, _, _)| *id).collect()
            }
            UndoAction::BatchMetadata { changes, .. } => {
                changes.iter().map(|(id, _, _)| *id).collect()
            }
            UndoAction::BatchDate { changes } => changes.iter().map(|(id, _, _)| *id).collect(),
            UndoAction::BatchLocation { changes } => changes.iter().map(|(id, _, _)| *id).collect(),
            UndoAction::Tags { ids, .. } => ids.clone(),
            UndoAction::SmartAlbumDeleted { .. } | UndoAction::TemplateDeleted { .. } => Vec::new(),
            UndoAction::Template { change } => change.affected_ids(),
            UndoAction::TagRows { change, .. } => change.affected_ids(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            UndoAction::Rating { .. } => "rating",
            UndoAction::ColorLabel { .. } => "color label",
            UndoAction::Favorite { .. } => "favorite",
            UndoAction::Caption { .. } => "caption",
            UndoAction::Title { .. } => "title",
            UndoAction::Date { .. } => "date",
            UndoAction::Location { .. } => "location",
            UndoAction::Tags { .. } => "tag edit",
            UndoAction::Trashed { .. } => "trash",
            UndoAction::BatchMetadata { field, .. } => match field {
                BatchField::Title => "batch title",
                BatchField::Caption => "batch caption",
            },
            UndoAction::BatchDate { .. } => "batch date",
            UndoAction::BatchLocation { .. } => "batch location",
            UndoAction::SmartAlbumDeleted { .. } => "smart album delete",
            UndoAction::Template { .. } => "template",
            UndoAction::TemplateDeleted { .. } => "template delete",
            UndoAction::TagRows { label, .. } => label,
        }
    }

    /// Applies one side. Only a keyword change can leave anything alone
    /// (rows, aliases or albums changed since); every other action reports
    /// nothing skipped.
    fn apply(&self, engine: &LuminaEngine, before: bool) -> Result<TagReplay> {
        let done = match self {
            UndoAction::Rating { changes } => engine.set_ratings(&pick(changes, before)),
            UndoAction::ColorLabel { changes } => engine.set_color_labels(&pick(changes, before)),
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
            UndoAction::Date {
                id,
                before: b,
                after: a,
            } => engine.set_timestamp(*id, if before { *b } else { *a }),
            UndoAction::Location {
                id,
                before: b,
                after: a,
            } => {
                let (lat, lon) = if before { b } else { a };
                engine.set_location(*id, *lat, *lon)
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
            UndoAction::BatchMetadata { field, changes } => {
                let values: Vec<(MediaId, Option<&str>)> = changes
                    .iter()
                    .map(|(id, b, a)| (*id, if before { b } else { a }.as_deref()))
                    .collect();
                match field {
                    BatchField::Title => engine.set_titles(&values),
                    BatchField::Caption => engine.set_captions(
                        &values
                            .iter()
                            .map(|&(id, caption)| (id, caption.unwrap_or("")))
                            .collect::<Vec<_>>(),
                    ),
                }
            }
            UndoAction::BatchDate { changes } => engine.set_timestamps(&pick(changes, before)),
            UndoAction::BatchLocation { changes } => engine.set_locations(&pick(changes, before)),
            UndoAction::SmartAlbumDeleted { album } => {
                if before {
                    engine.restore_smart_album(album)
                } else {
                    engine.delete_smart_album(album.id).map(|_| ())
                }
            }
            UndoAction::Template { change } => engine.replay_template_change(change, before),
            // Config, not the catalog: `apply_history_action` handles it.
            UndoAction::TemplateDeleted { .. } => Ok(()),
            UndoAction::TagRows { change, .. } => return engine.apply_tag_rows(change, before),
        };
        done.map(|()| TagReplay::default())
    }
}

/// The toast's note on what a replay left alone, or `""`.
fn replay_note(skipped: TagReplay) -> String {
    let kept = |n: usize, one: &str, many: &str| match n {
        0 => String::new(),
        1 => format!("; 1 {one} since was kept"),
        n => format!("; {n} {many} since were kept"),
    };
    kept(skipped.aliases, "alias changed", "aliases changed")
        + &kept(skipped.albums, "smart album edited", "smart albums edited")
}

/// The `before` or `after` side of (id, before, after) rows.
fn pick<T: Copy>(changes: &[(MediaId, T, T)], before: bool) -> Vec<(MediaId, T)> {
    changes
        .iter()
        .map(|&(id, b, a)| (id, if before { b } else { a }))
        .collect()
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
        if let Some(note) = self.apply_history_action(&action, true, cx) {
            let label = action.label();
            let affected = action.affected_ids();
            self.undo.redo_stack.push(action);
            self.refresh_after_history(cx);
            self.sync_metadata_sidecars(&affected, cx);
            self.set_last_action(format!("Undid {label}"));
            self.announce_history(format!("Undid {label}{note}"), false, cx);
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
        if let Some(note) = self.apply_history_action(&action, false, cx) {
            let label = action.label();
            let affected = action.affected_ids();
            self.undo.undo_stack.push(action);
            self.refresh_after_history(cx);
            self.sync_metadata_sidecars(&affected, cx);
            self.set_last_action(format!("Redid {label}"));
            // Redo pushes the action back onto the undo stack, so the toast can
            // offer Undo to reverse it again.
            self.announce_history(format!("Redid {label}{note}"), true, cx);
        } else {
            self.undo.redo_stack.push(action);
        }
        cx.notify();
    }

    /// Mirrors a successful history step into the toast channel so keyboard
    /// undo/redo gets the same feedback as other reversible actions.
    fn announce_history(&mut self, message: String, undo_available: bool, cx: &mut Context<Self>) {
        if undo_available {
            self.show_undo_toast(message, cx);
        } else {
            self.show_toast(message, cx);
        }
    }

    fn apply_history_action(
        &mut self,
        action: &UndoAction,
        before: bool,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if let UndoAction::TemplateDeleted { index, template } = action {
            return self
                .replay_template_delete(*index, template, before, cx)
                .then(String::new);
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before undoing", cx);
            return None;
        };
        match action.apply(&engine, before) {
            Ok(skipped) => {
                // Redoing a smart album delete must not leave the view on it.
                if let (UndoAction::SmartAlbumDeleted { album }, false) = (action, before) {
                    if self.selection == AlbumSelection::User(album.id) {
                        self.selection = AlbumSelection::AllLibrary;
                        self.clear_selection(cx);
                    }
                }
                // Get Info caches rights per item; reload them.
                if matches!(action, UndoAction::Template { .. }) {
                    self.info_details = None;
                }
                // The tag manager's detail grid lists the focused keyword's
                // items; counts and aliases reload in refresh_after_history.
                if let UndoAction::TagRows {
                    view_before,
                    view_after,
                    keeps_positions,
                    ..
                } = action
                {
                    let (from, to) = if before {
                        (view_after, view_before)
                    } else {
                        (view_before, view_after)
                    };
                    self.restore_tag_view_refs(from, to, *keeps_positions, cx);
                    self.load_tag_manager_items();
                }
                Some(replay_note(skipped))
            }
            Err(err) => {
                self.set_error(format!("History action failed: {err}"), cx);
                None
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
                self.set_selected_from_catalog(item);
            }
        }
        // Covers both a single item and a multi-selection (per-tag counts).
        self.refresh_selected_tags();
        // Batch labels and mixed-value summaries read the refreshed maps.
        self.update_batch_summary();
        self.refresh_album_counts();
        cx.notify();
    }
}
