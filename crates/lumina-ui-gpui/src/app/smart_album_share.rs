//! Duplicate Smart Album, and Export / Import Smart Album Rules…: a smart
//! album's rules as a JSON file, to keep or to use in another library.
//!
//! The file read and write run on the background executor; the catalog
//! write goes through [`LuminaApp::album_write`].

use std::path::PathBuf;

use gpui::*;
use lumina_core::text::{clean_text, MAX_NAME_CHARS};
use lumina_core::{
    from_portable_json, to_portable_json, PortableSmartAlbum, SmartAlbum, MAX_PORTABLE_BYTES,
};

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;

#[cfg(test)]
thread_local! {
    /// Rules file dialogs the app asked to open on this test thread.
    pub(crate) static RULE_FILE_DIALOGS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

impl LuminaApp {
    /// The smart album the grid shows, if any.
    fn open_smart_album(&self) -> Option<SmartAlbum> {
        match self.selection {
            AlbumSelection::User(id) => self.user_smart_albums.iter().find(|a| a.id == id).cloned(),
            _ => None,
        }
    }

    /// Duplicate Smart Album: a copy of the open smart album's rules, which
    /// then opens. The copy starts unpinned.
    pub fn duplicate_selected_smart_album(&mut self, cx: &mut Context<Self>) {
        if self.albums_blocked(cx) {
            return;
        }
        let Some(album) = self.open_smart_album() else {
            self.show_toast(t("smart_album.open_first"), cx);
            return;
        };
        let title = clean_text(
            &t("smart_album.copy_title").replace("{title}", &album.title),
            MAX_NAME_CHARS,
        );
        let Some(copy) = self.album_write(
            "smart_album.duplicate_failed",
            |e| e.create_smart_album(&title, &album.icon_name, album.match_mode, &album.rules),
            cx,
        ) else {
            return;
        };
        self.refresh_album_counts();
        self.select_album(AlbumSelection::User(copy.id), cx);
        let message = t("smart_album.duplicated").replace("{title}", &copy.title);
        self.set_last_action(message.clone());
        self.show_toast(message, cx);
    }

    /// Export Smart Album Rules…: asks where to save the open smart album's
    /// rules, then [`Self::export_smart_album_rules_to`].
    pub fn export_selected_smart_album_rules(&mut self, cx: &mut Context<Self>) {
        let Some(album) = self.open_smart_album() else {
            self.show_toast(t("smart_album.open_first"), cx);
            return;
        };
        // Tests count the request instead of opening a real dialog.
        #[cfg(test)]
        {
            let _ = album;
            RULE_FILE_DIALOGS.with(|n| n.set(n.get() + 1));
        }
        #[cfg(not(test))]
        cx.spawn(async move |this, cx| {
            let name: String = album
                .title
                .chars()
                .map(|c| if matches!(c, '/' | ':') { '-' } else { c })
                .collect::<String>()
                // A leading dot would make a hidden file.
                .trim_start_matches('.')
                .to_string();
            let name = if name.is_empty() {
                t("smart_album.untitled")
            } else {
                name
            };
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title(t("smart_album.export_dialog"))
                .set_file_name(format!("{name}.json"))
                .add_filter("JSON", &["json"])
                .save_file()
                .await
            else {
                return;
            };
            let path = handle.path().to_path_buf();
            // Err only when the window has closed meanwhile.
            this.update(cx, |app, cx| {
                app.export_smart_album_rules_to(album.id, path, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Writes smart album `id`'s rules to `path` off the UI thread.
    pub fn export_smart_album_rules_to(&mut self, id: i64, path: PathBuf, cx: &mut Context<Self>) {
        let Some(album) = self.user_smart_albums.iter().find(|a| a.id == id).cloned() else {
            return;
        };
        let json = match to_portable_json(&album) {
            Ok(json) => json,
            Err(err) => return self.rule_file_failed("smart_album.export_failed", &err, cx),
        };
        let task = cx
            .background_executor()
            .spawn(async move { std::fs::write(&path, json) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |app, cx| match result {
                Ok(()) => {
                    let message = t("smart_album.exported").replace("{title}", &album.title);
                    app.show_toast(message, cx);
                }
                Err(err) => app.rule_file_failed("smart_album.export_failed", &err.into(), cx),
            })
            .ok();
        })
        .detach();
    }

    /// Import Smart Album Rules…: asks for a rules file, then
    /// [`Self::import_smart_album_rules_from`].
    pub fn import_smart_album_rules(&mut self, cx: &mut Context<Self>) {
        if self.albums_blocked(cx) {
            return;
        }
        #[cfg(test)]
        {
            let _ = cx;
            RULE_FILE_DIALOGS.with(|n| n.set(n.get() + 1));
        }
        #[cfg(not(test))]
        cx.spawn(async move |this, cx| {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title(t("smart_album.import_dialog"))
                .add_filter("JSON", &["json"])
                .pick_file()
                .await
            else {
                return;
            };
            let path = handle.path().to_path_buf();
            this.update(cx, |app, cx| app.import_smart_album_rules_from(path, cx))
                .ok();
        })
        .detach();
    }

    /// Reads and checks the rules file at `path` off the UI thread, then
    /// saves it as a new smart album and opens it. A file whose rules do not
    /// compile, or that comes from a newer build, saves nothing.
    pub fn import_smart_album_rules_from(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let task = cx.background_executor().spawn(async move {
            // Checked before reading: the file can be anything the user picks.
            if std::fs::metadata(&path)?.len() > MAX_PORTABLE_BYTES as u64 {
                anyhow::bail!("This file is too large to be a smart album rules file");
            }
            let json = std::fs::read_to_string(&path)?;
            from_portable_json(&json)
        });
        cx.spawn(async move |this, cx| {
            let parsed = task.await;
            this.update(cx, |app, cx| app.save_imported_smart_album(parsed, cx))
                .ok();
        })
        .detach();
    }

    fn save_imported_smart_album(
        &mut self,
        parsed: anyhow::Result<PortableSmartAlbum>,
        cx: &mut Context<Self>,
    ) {
        let portable = match parsed {
            Ok(portable) => portable,
            Err(err) => return self.rule_file_failed("smart_album.import_failed", &err, cx),
        };
        // A dialog may have opened while the file was read.
        if self.albums_blocked(cx) {
            return;
        }
        let title = match clean_text(&portable.title, MAX_NAME_CHARS) {
            title if title.is_empty() => t("smart_album.untitled"),
            title => title,
        };
        let icon = match clean_text(&portable.icon_name, MAX_NAME_CHARS) {
            icon if icon.is_empty() => "folder".to_string(),
            icon => icon,
        };
        let Some(album) = self.album_write(
            "smart_album.import_failed",
            |e| e.create_smart_album(&title, &icon, portable.match_mode, &portable.rules),
            cx,
        ) else {
            return;
        };
        self.refresh_album_counts();
        self.select_album(AlbumSelection::User(album.id), cx);
        let message = t("smart_album.imported").replace("{title}", &album.title);
        self.set_last_action(message.clone());
        self.show_toast(message, cx);
    }

    fn rule_file_failed(&mut self, action: &str, err: &anyhow::Error, cx: &mut Context<Self>) {
        let action = t(action);
        log::warn!("{action}: {err:?}");
        self.set_error(format!("{action}: {err:#}"), cx);
    }
}
