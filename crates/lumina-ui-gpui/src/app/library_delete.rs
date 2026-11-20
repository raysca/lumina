//! Removing a library from Lumina: its catalog and cached thumbnails, never
//! its media. The folder stays exactly where the user keeps it.

use std::path::{Path, PathBuf};

use gpui::*;
use lumina_core::library;

use crate::app::LuminaApp;

/// The library a confirmation modal is asking about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDeleteState {
    pub path: PathBuf,
}

/// Two spellings of one folder (a symlink, `/var` and `/private/var`) are one
/// library, as [`crate::app::recents::push_recent`] already treats them.
fn same_library(a: &Path, b: &Path) -> bool {
    let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical(a) == canonical(b)
}

impl LuminaApp {
    /// Asks before removing the open library; the palette entry and the
    /// welcome screen's row both come here.
    pub(crate) fn prompt_delete_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.library_delete.is_some() {
            return;
        }
        self.library_delete = Some(LibraryDeleteState { path });
        cx.notify();
    }

    pub(crate) fn cancel_delete_library(&mut self, cx: &mut Context<Self>) {
        self.library_delete = None;
        cx.notify();
    }

    /// Removes the library the modal named: config first (so a failed save
    /// cannot leave a catalog deleted but the library still listed), then the
    /// catalog and thumbnails off the UI thread. The media files are never
    /// touched.
    pub(crate) fn confirm_delete_library(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.library_delete.take() else {
            return;
        };
        let path = state.path;
        let name = crate::app::recents::library_name(&path);

        let open = self
            .grid_state
            .as_ref()
            .is_some_and(|grid| same_library(grid.engine.library_root(), &path));
        let (catalog_db, cache_root) = if open {
            let engine = &self.grid_state.as_ref().expect("open library").engine;
            let Some(db) = engine.catalog_path() else {
                self.set_error("This library has no catalog file to remove.", cx);
                return;
            };
            (db.to_path_buf(), engine.cache_root().to_path_buf())
        } else {
            #[cfg(test)]
            if let Some(dir) = self.catalog_dir_override.as_ref() {
                (
                    lumina_core::library::catalog_path_in(dir, &path),
                    self.cache_dir_override
                        .clone()
                        .unwrap_or_else(lumina_core::cache::DiskCache::default_dir),
                )
            } else {
                (
                    library::catalog_path_for(&path),
                    lumina_core::cache::DiskCache::default_dir(),
                )
            }
            #[cfg(not(test))]
            (
                library::catalog_path_for(&path),
                lumina_core::cache::DiskCache::default_dir(),
            )
        };

        // Every place the removed library is named in the saved session.
        let before = self.config.clone();
        self.config
            .recent_libraries
            .retain(|recent| !same_library(recent, &path));
        if self
            .config
            .last_library_path
            .as_ref()
            .is_some_and(|last| same_library(last, &path))
        {
            self.config.last_library_path = None;
            self.config.clear_session_state();
        }
        if self
            .config
            .resume_library
            .as_ref()
            .is_some_and(|owner| same_library(owner, &path))
        {
            self.config.resume_library = None;
            self.config.resume_positions = Default::default();
        }
        if let Err(err) = self.try_save_config() {
            self.config = before;
            self.set_error(format!("Could not save settings: {err:#}"), cx);
            cx.notify();
            return;
        }

        let join = if open {
            let join = self.close_library_to_welcome(cx);
            // Settings was covering the library; show the welcome page it
            // returns to instead of an empty Library tab.
            if self.preferences_open {
                self.close_preferences(cx);
            }
            join
        } else {
            None
        };

        let task = cx.background_executor().spawn(async move {
            // The engine may still be held by a task that is shutting down
            // (a download, a decode). Joining here, off the UI thread, both
            // waits for it and keeps the catalog open until it is gone.
            if let Some(handle) = join {
                let _ = handle.join();
            }
            library::remove_library_at(&catalog_db, &cache_root)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| match result {
                Ok(removal) => {
                    let message = if removal.removed_catalog {
                        crate::i18n::t("library.removed")
                    } else {
                        crate::i18n::t("library.removed_already")
                    };
                    app.show_toast(
                        message
                            .replace("{name}", &name)
                            .replace("{path}", &path.display().to_string()),
                        cx,
                    );
                }
                Err(err) => {
                    log::error!("failed to remove library at {path:?}: {err:#}");
                    app.set_error(format!("Could not remove the library: {err:#}"), cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Closes the open library and shows the welcome screen again, with the
    /// same reset a library switch does so no id or texture from it survives.
    /// Returns the event reader thread to join once the engine has no holders
    /// left: joining it here would wait on the UI thread for the very tasks
    /// this reset is cancelling.
    fn close_library_to_welcome(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<std::thread::JoinHandle<()>> {
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        self.close_pip(cx);
        self.close_compare(cx);
        self.drop_slideshow(cx);
        self.abandon_import(cx);
        self.close_network_import_dialog(cx);
        self.discard_edit_draft();

        // The download job holds an engine clone; drop it before the engine
        // is expected to go away, or the event reader never sees its channel
        // close.
        self.downloads.reset(None);
        if let Some(old) = self.grid_state.take() {
            old.engine.cancel_scan();
            let mut cache = old.texture_cache.lock();
            while let Some((_, thumb)) = cache.pop_lru() {
                self.retire_image(thumb.image);
            }
        }
        self.library_generation += 1;
        let join = self.event_receiver_thread.take();

        self.first_scan = None;
        self.pending_restore = None;
        self.clear_selection_fields();
        self.selection = crate::views::sidebar::AlbumSelection::AllLibrary;
        self.stacks.reset();
        self.map = Default::default();
        self.album_draft = Default::default();
        self.albums = Default::default();
        self.delete_confirm_state = None;
        self.context_menu_state = None;
        self.cancel_tag_manager_prompts();
        self.undo = Default::default();
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.edits.reset_for_library();
        self.reset_navigation_history();
        self.welcome.set_missing(None);
        self.welcome.focused_recent = None;
        cx.notify();
        join
    }
}
