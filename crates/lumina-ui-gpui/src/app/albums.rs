//! Hand-picked albums: the sidebar list, New Album (⌘N), rename and delete.
//!
//! Every catalog write goes through [`LuminaApp::album_write`], which shows a
//! failure in the error banner and returns `None`, so callers change local
//! state only after the catalog took the write.

use std::collections::HashMap;

use gpui::*;
use lumina_core::text::{clean_text, MAX_NAME_CHARS};
use lumina_core::{Album, LuminaEngine, MediaId};

use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;

/// The sidebar's albums and the inline edits on them.
#[derive(Default)]
pub struct AlbumsState {
    /// Every album, oldest first (the catalog's order).
    pub list: Vec<Album>,
    /// Active member count per album id.
    pub counts: HashMap<i64, usize>,
    /// The album whose name is being edited in the sidebar.
    pub renaming: Option<i64>,
    pub rename_buffer: String,
    /// The album whose delete confirmation is open.
    pub confirm_delete: Option<i64>,
}

#[cfg(test)]
thread_local! {
    /// Makes [`LuminaApp::album_write`] fail without writing on this test
    /// thread: the UI crate has no SQL handle to break.
    pub(crate) static ALBUM_WRITE_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

impl LuminaApp {
    /// Runs one album write. On failure the banner says `t(action)` and
    /// why, and the result is `None`; with no library open it is `None` too.
    fn album_write<T>(
        &mut self,
        action: &str,
        write: impl FnOnce(&LuminaEngine) -> anyhow::Result<T>,
        cx: &mut Context<Self>,
    ) -> Option<T> {
        let engine = self.grid_state.as_ref()?.engine.clone();
        #[cfg(test)]
        let result = if ALBUM_WRITE_FAILS.with(std::cell::Cell::get) {
            Err(anyhow::anyhow!("database is locked"))
        } else {
            write(&engine)
        };
        #[cfg(not(test))]
        let result = write(&engine);
        match result {
            Ok(value) => Some(value),
            Err(err) => {
                let action = t(action);
                log::warn!("{action}: {err:?}");
                self.set_error(format!("{action}: {err}"), cx);
                None
            }
        }
    }

    /// Reloads the album list and counts; part of `refresh_album_counts`.
    pub(crate) fn refresh_albums(&mut self, engine: &LuminaEngine) {
        let rows = engine.list_albums_with_counts();
        self.albums.counts = rows.iter().map(|(album, n)| (album.id, *n)).collect();
        self.albums.list = rows.into_iter().map(|(album, _)| album).collect();
    }

    pub(crate) fn album_title(&self, id: i64) -> Option<&str> {
        self.albums
            .list
            .iter()
            .find(|album| album.id == id)
            .map(|album| album.title.as_str())
    }

    /// True (with a toast) while album actions must wait: the File menu
    /// reaches the app in edit mode, and opening the name field would close
    /// the viewer and drop the unsaved draft; under a sheet the field would
    /// take keys while hidden.
    fn albums_blocked(&mut self, cx: &mut Context<Self>) -> bool {
        if self.edits.session.is_some() {
            self.show_toast(t("album.finish_edit_first"), cx);
            return true;
        }
        if self.import_blocked_by_modal() {
            self.show_toast(t("album.close_dialog_first"), cx);
            return true;
        }
        false
    }

    /// New Album (⌘N): always empty, since one selected item is the normal
    /// grid state. It starts in rename mode.
    pub fn new_album(&mut self, cx: &mut Context<Self>) {
        if self.albums_blocked(cx) {
            return;
        }
        self.create_album_with(&[], cx);
    }

    /// New Album with Selection: an album holding the selected items.
    pub fn new_album_with_selection(&mut self, cx: &mut Context<Self>) {
        if self.albums_blocked(cx) {
            return;
        }
        let ids: Vec<MediaId> = self.get_selected_items().iter().map(|i| i.id).collect();
        if ids.is_empty() {
            self.show_toast(t("album.select_first"), cx);
            return;
        }
        self.create_album_with(&ids, cx);
    }

    fn create_album_with(&mut self, ids: &[MediaId], cx: &mut Context<Self>) {
        let title = t("album.untitled");
        let Some(album) = self.album_write(
            "album.create_failed",
            |e| e.create_album_with(&title, ids),
            cx,
        ) else {
            return;
        };
        self.refresh_album_counts();
        self.start_album_rename(album.id, cx);
    }

    /// Opens the sidebar name field for `id`, showing the sidebar (and
    /// leaving the viewer) so the field is on screen.
    pub fn start_album_rename(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.albums_blocked(cx) {
            return;
        }
        let Some(title) = self.album_title(id).map(str::to_string) else {
            return;
        };
        self.albums.rename_buffer = title;
        self.albums.renaming = Some(id);
        // The name field takes the keys, not a search left focused.
        self.search_focused = false;
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        if self.sidebar_collapsed {
            self.toggle_sidebar(cx);
        }
        cx.notify();
    }

    /// Saves the typed name. An emptied or unchanged name renames nothing.
    pub fn commit_album_rename(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.albums.renaming.take() else {
            return;
        };
        let typed = std::mem::take(&mut self.albums.rename_buffer);
        let after = clean_text(&typed, MAX_NAME_CHARS);
        let Some(before) = self.album_title(id).map(str::to_string) else {
            cx.notify();
            return;
        };
        if after.is_empty() || after == before {
            cx.notify();
            return;
        }
        match self.album_write("album.rename_failed", |e| e.rename_album(id, &after), cx) {
            Some(true) => {
                let message = t("album.renamed").replace("{title}", &after);
                self.push_undo(UndoAction::AlbumRenamed { id, before, after });
                self.refresh_album_counts();
                self.set_last_action(message.clone());
                self.show_undo_toast(message, cx);
            }
            // Deleted meanwhile: the list catches up.
            Some(false) => self.refresh_album_counts(),
            // The field stays open with the typed name, to try again.
            None => {
                self.albums.renaming = Some(id);
                self.albums.rename_buffer = typed;
            }
        }
        cx.notify();
    }

    pub fn cancel_album_rename(&mut self, cx: &mut Context<Self>) {
        self.albums.renaming = None;
        self.albums.rename_buffer.clear();
        cx.notify();
    }

    /// Rename Album… in the palette: the open album.
    pub fn rename_selected_album(&mut self, cx: &mut Context<Self>) {
        if let AlbumSelection::Album(id) = self.selection {
            self.start_album_rename(id, cx);
        }
    }

    /// Asks before deleting; see [`Self::confirm_delete_album`].
    pub fn prompt_delete_album(&mut self, id: i64, cx: &mut Context<Self>) {
        if self.albums_blocked(cx) {
            return;
        }
        self.albums.confirm_delete = Some(id);
        cx.notify();
    }

    /// Delete Album… in the palette: asks about the open album.
    pub fn delete_selected_album(&mut self, cx: &mut Context<Self>) {
        if let AlbumSelection::Album(id) = self.selection {
            self.prompt_delete_album(id, cx);
        }
    }

    pub fn cancel_delete_album(&mut self, cx: &mut Context<Self>) {
        self.albums.confirm_delete = None;
        cx.notify();
    }

    /// Deletes the album the confirmation asks about. Its media stay; ⌘Z
    /// restores it under the same id with its members.
    pub fn confirm_delete_album(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.albums.confirm_delete.take() else {
            return;
        };
        match self.album_write("album.delete_failed", |e| e.delete_album(id), cx) {
            Some(Some((album, members))) => {
                if self.selection == AlbumSelection::Album(id) {
                    self.selection = AlbumSelection::AllLibrary;
                    self.clear_selection(cx);
                }
                self.refresh_album_counts();
                self.push_undo(UndoAction::AlbumDeleted { album, members });
                self.set_last_action(t("album.deleted"));
                self.show_undo_toast(t("album.deleted"), cx);
            }
            Some(None) => self.refresh_album_counts(),
            None => {}
        }
        cx.notify();
    }
}
