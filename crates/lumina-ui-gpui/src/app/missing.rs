//! Locate…: reconnects a missing item to its file inside the library.

use gpui::Context;
use lumina_core::{MediaId, RelinkOutcome};
use std::path::PathBuf;

#[cfg(not(test))]
use crate::app::types::UiMessage;
use crate::app::LuminaApp;

impl LuminaApp {
    /// Opens a file picker for the one selected missing item, starting in
    /// the folder it was last seen in.
    pub fn locate_missing_selected(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let selected = self.get_selected_ids();
        let Some(item) = self
            .selected_item
            .clone()
            .filter(|item| selected == [item.id] && self.missing_items.contains(&item.id))
        else {
            self.show_toast("Select one missing item to locate it", cx);
            return;
        };
        let root = state.engine.library_root().to_path_buf();
        // The dialog falls back on its own when the folder is gone too.
        let start = item.path.parent().map(PathBuf::from).unwrap_or(root);
        let tx = self.ui_tx.clone();
        let id = item.id;
        #[cfg(test)]
        let _ = (start, tx, id);
        #[cfg(not(test))]
        cx.spawn(async move |_this, _cx| {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title(format!("Locate “{}”", item.file_name))
                .set_directory(&start)
                .pick_file()
                .await
            {
                let _ = tx.send_blocking(UiMessage::LocatePicked(id, handle.path().to_path_buf()));
            }
        })
        .detach();
    }

    /// Reconnects `id` to `path` off the UI thread (it may hash the file).
    pub fn apply_locate(&mut self, id: MediaId, path: PathBuf, cx: &mut Context<Self>) {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { engine.relink(id, &path) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(outcome) => {
                        if let RelinkOutcome::Folded(removed) = outcome {
                            if app.selected_item.as_ref().is_some_and(|i| i.id == removed) {
                                app.clear_selection(cx);
                            }
                        }
                        if let Some(engine) = app.grid_state.as_ref().map(|s| s.engine.clone()) {
                            if app.selected_item.as_ref().is_some_and(|i| i.id == id) {
                                app.selected_item = engine.get_item_by_id(id);
                            }
                        }
                        app.refresh_album_counts();
                        app.show_toast("File reconnected", cx);
                    }
                    Err(err) => app.set_error(err.to_string(), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
