//! Putting the selection in an album (the ⇧⌘A picker, the context menu,
//! the palette) and taking it out of the open album (⌫), with undo.
//!
//! Writes go through [`LuminaApp::album_write`]; local state changes only
//! after the catalog took the write.

use gpui::*;
use lumina_core::{Album, MediaId};

use crate::app::albums::AlbumPicker;
use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;

/// "Added 1 item to “Trip”" / "Added 3 items to “Trip”".
fn count_message(key: &str, n: usize, album: &str) -> String {
    let key = if n == 1 {
        format!("{key}_one")
    } else {
        format!("{key}_other")
    };
    t(&key)
        .replace("{n}", &n.to_string())
        .replace("{album}", album)
}

impl LuminaApp {
    fn selected_media_ids(&self) -> Vec<MediaId> {
        self.get_selected_items().iter().map(|i| i.id).collect()
    }

    /// Add to Album… (⇧⌘A): the picker for the current selection.
    pub fn open_album_picker(&mut self, cx: &mut Context<Self>) {
        // The context menu that offers it is not a dialog to wait for.
        self.close_context_menu(cx);
        if self.albums_blocked(cx) {
            return;
        }
        let ids = self.selected_media_ids();
        if ids.is_empty() {
            self.show_warning_toast(t("album.select_first"), cx);
            return;
        }
        // The filter takes the keys, not a search left focused.
        self.search_focused = false;
        self.albums.picker = Some(AlbumPicker {
            ids,
            filter: String::new(),
            highlight: 0,
            scroll: ScrollHandle::new(),
        });
        cx.notify();
    }

    pub fn close_album_picker(&mut self, cx: &mut Context<Self>) {
        self.albums.picker = None;
        cx.notify();
    }

    /// The albums the picker's filter matches, in sidebar order. The row
    /// after them is New Album….
    pub(crate) fn album_picker_rows(&self) -> Vec<&Album> {
        let filter = self
            .albums
            .picker
            .as_ref()
            .map(|p| p.filter.trim().to_lowercase())
            .unwrap_or_default();
        self.albums
            .list
            .iter()
            .filter(|album| album.title.to_lowercase().contains(&filter))
            .collect()
    }

    /// ↑/↓ in the picker.
    pub fn move_album_picker(&mut self, delta: isize, cx: &mut Context<Self>) {
        let last = self.album_picker_rows().len();
        if let Some(picker) = self.albums.picker.as_mut() {
            picker.highlight = picker.highlight.saturating_add_signed(delta).min(last);
            picker.scroll.scroll_to_item(picker.highlight);
        }
        cx.notify();
    }

    /// ↵ in the picker: the highlighted album, or New Album….
    pub fn activate_album_picker(&mut self, cx: &mut Context<Self>) {
        let Some(highlight) = self.albums.picker.as_ref().map(|p| p.highlight) else {
            return;
        };
        match self.album_picker_rows().get(highlight).map(|a| a.id) {
            Some(id) => self.add_picked_to_album(id, cx),
            None => self.picker_new_album(cx),
        }
    }

    /// Adds the picker's items to album `id` and closes the picker.
    pub fn add_picked_to_album(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(picker) = self.albums.picker.take() else {
            return;
        };
        cx.notify();
        self.add_to_album(id, &picker.ids, cx);
    }

    /// The picker's New Album… row: an album holding its items, in rename
    /// mode, as New Album with Selection makes, named after the filter text
    /// when there is some.
    pub fn picker_new_album(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = self.albums.picker.take() else {
            return;
        };
        // A typed name that matched no album is the name the user wanted.
        let typed = picker.filter.trim();
        let title = if typed.is_empty() {
            t("album.untitled")
        } else {
            typed.to_string()
        };
        self.create_album_with(title, &picker.ids, cx);
        cx.notify();
    }

    fn add_to_album(&mut self, id: i64, ids: &[MediaId], cx: &mut Context<Self>) {
        let Some(title) = self.album_title(id).map(str::to_string) else {
            return;
        };
        // Only new members are recorded, so undo leaves earlier ones in.
        let Some(added) = self.album_write("album.add_failed", |e| e.add_new_to_album(id, ids), cx)
        else {
            return;
        };
        if added.is_empty() {
            self.show_warning_toast(t("album.already_in").replace("{album}", &title), cx);
            return;
        }
        let message = count_message("album.added", added.len(), &title);
        self.push_undo(UndoAction::AlbumMembership {
            album: id,
            ids: added,
            added: true,
        });
        self.refresh_album_counts();
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
    }

    /// ⌫ in an album (and Remove from Album): takes the selection out of the
    /// open album. Nothing is trashed; the media stay in the library.
    pub fn remove_selection_from_album(&mut self, cx: &mut Context<Self>) {
        let AlbumSelection::Album(id) = self.selection else {
            return;
        };
        self.close_context_menu(cx);
        if self.albums_blocked(cx) {
            return;
        }
        let ids = self.selected_media_ids();
        let Some(title) = self.album_title(id).map(str::to_string) else {
            return;
        };
        if ids.is_empty() {
            return;
        }
        // Only real members are recorded: a selection can hold an item that
        // left the album meanwhile, and undo must not add it.
        let Some(removed) = self.album_write(
            "album.remove_failed",
            |e| e.remove_members_from_album(id, &ids),
            cx,
        ) else {
            return;
        };
        if removed.is_empty() {
            self.refresh_album_counts();
            self.show_warning_toast(t("album.not_in").replace("{album}", &title), cx);
            return;
        }
        let message = count_message("album.removed", removed.len(), &title);
        self.push_undo(UndoAction::AlbumMembership {
            album: id,
            ids: removed,
            added: false,
        });
        self.refresh_album_counts();
        // The removed items leave this view, so positions shift.
        self.clear_selection(cx);
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
    }
}
