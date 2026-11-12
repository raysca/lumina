//! Edit in External Editor (⌘E, in place) and Edit a Copy (⌥⌘E) from the
//! keyboard, the command palette, the Image menu and the context menu.

use gpui::*;
use lumina_core::MediaItem;

use crate::app::viewer_actions::{
    EXTERNAL_EDITOR_FILE_MISSING_MESSAGE, EXTERNAL_EDITOR_INVALID_MESSAGE,
    EXTERNAL_EDITOR_UNSET_MESSAGE,
};
use crate::app::LuminaApp;

/// Banner shown when Edit a Copy could not copy or catalogue the file.
pub(crate) const EDIT_COPY_FAILED_MESSAGE: &str =
    "Could not make a copy to edit; the original is unchanged.";

impl LuminaApp {
    /// ⌘E / ⌥⌘E, the palette and the Image menu: the one selected item, in
    /// the grid or the viewer, opened in the configured editor (or a copy of
    /// it when `copy` is set). With several selected, the status bar asks for one.
    pub fn edit_selected_externally(&mut self, copy: bool, cx: &mut Context<Self>) {
        if !self.is_fullscreen_viewer && self.selected_indices.len() > 1 {
            self.set_last_action("Select one item to edit in the external editor");
            cx.notify();
            return;
        }
        let Some(item) = self.selected_item.clone() else {
            self.set_last_action("Select an item to edit in the external editor");
            cx.notify();
            return;
        };
        self.edit_item_externally(item, copy, cx);
    }

    /// Opens `item` in the external editor, or a fresh copy of it next to
    /// the original (`name (edited).ext`, with the original's metadata and
    /// none of Lumina's edits). With no usable editor set, says so and opens
    /// Preferences on the editor row; no copy is made. Refused behind sheets,
    /// in a text field and in Recently Deleted.
    pub fn edit_item_externally(&mut self, item: MediaItem, copy: bool, cx: &mut Context<Self>) {
        // Edit mode needs no check here: it absorbs ⌘E, and the menu items
        // and the palette are disabled while a draft is open.
        if self.media_edit_refused(("opening the external editor", "edit"), cx) {
            return;
        }
        if self.config.validated_external_editor().is_none() {
            let message = if self.config.external_editor.is_none() {
                EXTERNAL_EDITOR_UNSET_MESSAGE
            } else {
                EXTERNAL_EDITOR_INVALID_MESSAGE
            };
            self.set_error(message, cx);
            self.open_preferences(cx);
            return;
        }
        if !copy {
            self.open_item_in_external_editor(&item.path, cx);
            return;
        }
        if !item.path.is_file() {
            self.set_error(EXTERNAL_EDITOR_FILE_MISSING_MESSAGE, cx);
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            return;
        };
        self.set_last_action("Copying for the external editor…");
        cx.notify();
        // A video can be gigabytes: copy off the UI thread.
        let id = item.id;
        let started = std::sync::Arc::downgrade(&engine);
        let task = cx
            .background_executor()
            .spawn(async move { engine.duplicate_for_edit(id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(copy) if this.engine_is(&started) => this.open_edit_copy(copy, cx),
                // Another library opened while copying: the copy belongs to
                // the old one, so nothing here is selected or refreshed. The
                // file exists and was asked for, so the editor still opens it.
                // Deliberately no sidecar sync here: the copy has none.
                Ok(copy) => this.open_item_in_external_editor(&copy.path, cx),
                Err(err) => {
                    log::error!("Edit a Copy failed for item {}: {err:?}", id.0);
                    this.set_error(format!("{EDIT_COPY_FAILED_MESSAGE} Reason: {err}."), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// True while the library whose engine `started` points at is open.
    fn engine_is(&self, started: &std::sync::Weak<lumina_core::LuminaEngine>) -> bool {
        self.grid_state
            .as_ref()
            .is_some_and(|state| std::sync::Arc::downgrade(&state.engine).ptr_eq(started))
    }

    /// Shows the new copy (selected in the view when it is there) and opens
    /// it in the editor.
    fn open_edit_copy(&mut self, copy: MediaItem, cx: &mut Context<Self>) {
        // Counting it now keeps the watcher's batch for the new file from
        // reading as a structural change that clears the selection.
        if let Some(state) = self.grid_state.as_mut() {
            state.total_count = state.engine.total_count();
        }
        // The Albums overview holds no media: show the copy in All Media,
        // as PiP's expand does, so no hidden selection is left on the page.
        if self.albums_overview_shown() {
            self.select_album(crate::views::sidebar::AlbumSelection::AllLibrary, cx);
        }
        // Counts first (the view lookup below reads them), and the ratings,
        // labels and captions maps the new selection shows.
        self.refresh_after_history(cx);
        match self.find_active_index(copy.id) {
            Some(index) => {
                let item = self
                    .get_active_slice(index..index + 1)
                    .pop()
                    .unwrap_or_else(|| copy.clone());
                self.select_item_at(item, index, cx);
                self.reveal_grid_selection();
            }
            // Not in this view (an album the copy is not in): select it alone.
            None => {
                self.selected_indices.clear();
                self.selected_index = None;
                self.select_item(copy.clone(), cx);
            }
        }
        self.sync_metadata_sidecars(&[copy.id], cx);
        self.set_last_action(format!("Made {} to edit", copy.file_name));
        self.open_item_in_external_editor(&copy.path, cx);
    }
}

/// Test builds never launch an editor: they record each command's program
/// and arguments here instead, so tests can assert what would have run.
#[cfg(test)]
pub(crate) mod test_launches {
    use std::cell::RefCell;
    use std::process::Command;

    thread_local! {
        static LAUNCHES: RefCell<Vec<Vec<String>>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn record(command: &Command) {
        let mut line = vec![command.get_program().to_string_lossy().to_string()];
        line.extend(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().to_string()),
        );
        LAUNCHES.with(|launches| launches.borrow_mut().push(line));
    }

    /// Every command recorded on this thread since the last call.
    pub(crate) fn take() -> Vec<Vec<String>> {
        LAUNCHES.with(|launches| std::mem::take(&mut *launches.borrow_mut()))
    }
}
