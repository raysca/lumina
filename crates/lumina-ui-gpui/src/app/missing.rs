//! Missing originals: Locate… (reconnects an item to its file inside the
//! library) and the viewer's offline card state (drive name, Keep preview, focus).

use gpui::{Context, Keystroke};
use lumina_core::{MediaId, RelinkOutcome};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[cfg(not(test))]
use crate::app::types::UiMessage;
use crate::app::LuminaApp;

#[cfg(test)]
thread_local! {
    /// Items the app opened the Locate picker for on this test thread.
    pub(crate) static LOCATE_REQUESTS: std::cell::RefCell<Vec<MediaId>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The viewer's "Original not available" card (Design 10).
#[derive(Default)]
pub(crate) struct MissingCardState {
    /// Items whose card "Keep preview" dismissed for this session; the
    /// bar stays.
    pub kept: HashSet<MediaId>,
    /// Tab focus: 0 Locate file…, 1 Keep preview. Cleared on every item
    /// change and when the viewer closes.
    pub focus: Option<usize>,
    /// The selected missing item and its disconnected drive, worked out
    /// once when it is selected, never per frame.
    pub drive: Option<(MediaId, Option<String>)>,
}

/// The drive name of a path under `/Volumes/<name>` when `<name>` is not
/// mounted, so the viewer can say "“<name>” is disconnected". It lists the
/// local `/Volumes` folder by name only and never enters a mounted volume,
/// so a stalled network share cannot block it.
pub(crate) fn disconnected_drive(path: &Path) -> Option<String> {
    let mut parts = path.components();
    let (Some(Component::RootDir), Some(Component::Normal(volumes)), Some(Component::Normal(name))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if volumes != "Volumes" {
        return None;
    }
    // Unreadable /Volumes: say nothing rather than guess.
    let mounted = std::fs::read_dir("/Volumes")
        .ok()?
        .flatten()
        .any(|entry| entry.file_name() == name);
    (!mounted).then(|| name.to_string_lossy().into_owned())
}

impl LuminaApp {
    /// Works out the selected missing item's drive once, and drops the
    /// button focus when the item changed. Called on every selection and
    /// when the missing set is refreshed; closing the viewer drops focus too.
    pub(crate) fn sync_missing_card(&mut self) {
        let drive = self
            .selected_item
            .as_ref()
            .filter(|item| self.missing_items.contains(&item.id))
            .map(|item| (item.id, disconnected_drive(&item.path)));
        let same_item = matches!((&drive, &self.missing_card.drive),
            (Some((new, _)), Some((old, _))) if new == old);
        if !same_item {
            self.missing_card.focus = None;
        }
        self.missing_card.drive = drive;
    }

    /// The disconnected drive of the missing item `id`, if any.
    pub(crate) fn missing_drive(&self, id: MediaId) -> Option<&str> {
        match &self.missing_card.drive {
            Some((cached, drive)) if *cached == id => drive.as_deref(),
            _ => None,
        }
    }

    /// The viewer item is missing and its card is not dismissed. Edit mode
    /// and a slideshow hide it (the bar stays): it would cover the crop
    /// handles or the presentation.
    pub(crate) fn missing_card_shown(&self) -> bool {
        self.is_fullscreen_viewer
            && self.edits.session.is_none()
            && self.slideshow.is_none()
            && self.selected_item.as_ref().is_some_and(|item| {
                self.missing_items.contains(&item.id) && !self.missing_card.kept.contains(&item.id)
            })
    }

    /// "Keep preview": hides the card for this item until the app quits.
    pub(crate) fn keep_missing_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = self.selected_item.as_ref() {
            self.missing_card.kept.insert(item.id);
        }
        self.missing_card.focus = None;
        cx.notify();
    }

    /// The card's keys: Tab moves between its two buttons, ↵ presses the
    /// focused one. With no button focused ↵ keeps closing the viewer, and
    /// Esc is untouched.
    pub(crate) fn handle_missing_card_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.missing_card_shown() {
            return false;
        }
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt {
            return false;
        }
        match (keystroke.key.as_str(), self.missing_card.focus) {
            ("tab", focus) => {
                // Two buttons: either direction toggles.
                self.missing_card.focus = Some(match focus {
                    Some(index) => 1 - index,
                    None if m.shift => 1,
                    None => 0,
                });
                cx.notify();
            }
            ("enter" | "return", Some(0)) if !m.shift => self.locate_missing_selected(cx),
            ("enter" | "return", Some(_)) if !m.shift => self.keep_missing_preview(cx),
            _ => return false,
        }
        true
    }

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
        {
            let _ = (start, tx);
            LOCATE_REQUESTS.with(|r| r.borrow_mut().push(id));
        }
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
