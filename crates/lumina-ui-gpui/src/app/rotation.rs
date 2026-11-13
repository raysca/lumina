//! Quarter-turn rotation of the selection (R / ⇧R, ⌘K, Image menu, context
//! menu) and the texture eviction every display-only edit needs.

use gpui::*;
use lumina_core::{MediaId, MediaType};

use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;
use crate::views::viewer_state::ViewerRotation;

impl LuminaApp {
    /// Rotates the item open in the viewer, or every still in the grid
    /// selection, one quarter turn, as one undo step.
    ///
    /// The original file is never rewritten: only the catalog's
    /// `rotation_quarters` column changes, and the decoder applies the turn
    /// when frames are requested. Each item turns from its own angle, so a
    /// mixed selection stays mixed. Videos are skipped because the player
    /// surface is not rotated; a selection of only videos says so.
    pub fn rotate_selection(&mut self, direction: ViewerRotation, cx: &mut Context<Self>) {
        if self.media_edit_refused(("rotating", "rotate"), cx) {
            return;
        }
        // In edit mode Image > Rotate and ⌘K turn the draft, not the catalog.
        if self.edits.session.is_some() {
            self.rotate_edit_draft(direction, cx);
            return;
        }
        let items = if self.is_fullscreen_viewer {
            self.selected_item.iter().cloned().collect()
        } else {
            self.get_selected_items()
        };
        if items.is_empty() {
            self.set_last_action("Select an item to rotate");
            cx.notify();
            return;
        }
        let ids: Vec<MediaId> = items
            .iter()
            .filter(|item| item.media_type != MediaType::Video)
            .map(|item| item.id)
            .collect();
        if ids.is_empty() {
            self.set_last_action("Rotation is not supported for video");
            cx.notify();
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before rotating", cx);
            return;
        };
        let current = match engine.get_rotations(&ids) {
            Ok(current) => current,
            Err(err) => {
                self.set_error(format!("Could not read rotation: {err:#}"), cx);
                return;
            }
        };
        // Ids gone from the catalog are left out, so undo never writes them.
        let changes: Vec<(MediaId, u8, u8)> = ids
            .iter()
            .filter_map(|id| current.get(id).map(|&q| (*id, q, direction.apply_to(q))))
            .collect();
        if changes.is_empty() {
            cx.notify();
            return;
        }
        let writes: Vec<(MediaId, u8)> = changes.iter().map(|&(id, _, a)| (id, a)).collect();
        if let Err(err) = engine.set_rotations(&writes) {
            self.set_error(format!("Could not rotate: {err:#}"), cx);
            return;
        }
        let changed: Vec<MediaId> = writes.iter().map(|&(id, _)| id).collect();
        self.after_edits_written(&changed, cx);
        self.push_undo(UndoAction::Rotation { changes });
        let side = match direction {
            ViewerRotation::Left => "left",
            ViewerRotation::Right => "right",
        };
        let message = match changed.len() {
            1 => format!("Rotated {side}"),
            n => format!("Rotated {n} items {side}"),
        };
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    /// True (with a toast) when an edit to the selection must not run now:
    /// behind a sheet or dialog, while a field has focus, or in Recently
    /// Deleted. Menu actions skip the keymap's modal shield, so every edit
    /// action checks this itself. `verb` is ("rotating", "rotate").
    pub(crate) fn media_edit_refused(
        &mut self,
        (doing, verb): (&str, &str),
        cx: &mut Context<Self>,
    ) -> bool {
        // The import dialog is not in `import_blocked_by_modal`: its own
        // steps call that check.
        if self.import_blocked_by_modal() || self.import.dialog.is_some() {
            self.show_warning_toast(format!("Close the open dialog before {doing}"), cx);
            return true;
        }
        if self.text_target().is_some() {
            self.show_warning_toast(format!("Finish editing the field before {doing}"), cx);
            return true;
        }
        // The context menu hides Rotate here; keys, ⌘K and the menu match it.
        if self.selection == AlbumSelection::Trash {
            self.show_warning_toast(
                format!("Restore items from Recently Deleted to {verb} them"),
                cx,
            );
            return true;
        }
        false
    }

    /// Drops what was rendered for `ids` so the grid and viewer decode them
    /// again after a display-only edit (rotation, and later edits and
    /// external saves): the GPU texture cache is keyed by id alone. The grid
    /// re-requests the visible thumbnails when it next paints.
    pub(crate) fn evict_rendered(&mut self, ids: &[MediaId]) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let mut stale = Vec::new();
        {
            let mut in_flight = state.in_flight.lock();
            let mut textures = state.texture_cache.lock();
            for id in ids {
                in_flight.remove(id);
                stale.extend(textures.pop(id));
            }
        }
        for image in stale {
            self.retire_image(image);
        }
        // The open item's full-size frame is re-requested through the normal
        // background path, so the decoder, not the UI, transforms it.
        if let Some(open) = self
            .selected_item
            .as_ref()
            .map(|item| item.id)
            .filter(|id| ids.contains(id))
        {
            self.set_viewer_image(None);
            if let Some(state) = self.grid_state.as_ref() {
                self.request_viewer_frame(state, open);
            }
        }
    }
}
