//! Import by copy: drop or pick files and folders, review a plan, copy.
//! Planning and copying run off the UI thread; see `lumina_core::import`.

use gpui::*;
use lumina_core::import::{CopyImportReport, ImportPlan};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(not(test))]
use crate::app::types::UiMessage;
use crate::i18n;
use crate::LuminaApp;

/// The import confirmation dialog.
pub(crate) enum ImportDialog {
    /// Walking, reading and duplicate-checking `count` dropped or picked
    /// paths; `checked` is (files read, files found) once the walk is done.
    Planning {
        count: usize,
        checked: Option<(usize, usize)>,
    },
    /// Ready to copy; an empty plan offers only OK.
    Ready(ImportPlan),
}

/// Import state on [`LuminaApp`]: the dialog and a running copy.
#[derive(Default)]
pub(crate) struct ImportState {
    pub dialog: Option<ImportDialog>,
    /// Cancels planning when the dialog is dismissed.
    plan_cancel: Arc<AtomicBool>,
    /// Set while a copy runs; storing true stops it after the current file.
    running: Option<Arc<AtomicBool>>,
    /// (copied, total) for the status bar.
    pub progress: Option<(usize, usize)>,
    /// "Import into New Library…" opened this library: ask for the photos to
    /// copy in when its first scan completes. Cleared when another opens.
    pub pending_import_after_open: bool,
    /// The last import's failure line, kept until the banner is dismissed:
    /// a later error takes the banner's title, and this stays under it.
    pub failure: Option<String>,
}

impl ImportState {
    /// A copy in progress without a copy task, for status bar tests.
    #[cfg(test)]
    pub fn pretend_running(&mut self, done: usize, total: usize) {
        self.running = Some(Arc::default());
        self.progress = Some((done, total));
    }

    pub fn is_busy(&self) -> bool {
        self.dialog.is_some() || self.running.is_some()
    }

    /// Cancel Import was chosen and the current file is finishing.
    pub fn is_stopping(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|cancel| cancel.load(Ordering::Relaxed))
    }
}

impl LuminaApp {
    /// A dialog, sheet or menu that takes the keyboard is up.
    fn dialog_open(&self) -> bool {
        self.command_palette_open
            || self.shortcuts_overlay_open
            || self.preferences_open
            || self.delete_confirm_state.is_some()
            || self.album_draft.confirm_delete.is_some()
            || self.albums.confirm_delete.is_some()
            || self.albums.picker.is_some()
            || self.album_draft.modal_open
            || self.context_menu_state.is_some()
            || self.network_import.stage.is_some()
            || self.downloads.confirm.is_some()
            || self.templates.editor.is_some()
    }

    /// A dialog, sheet, menu or the viewer is up. The header's Sort and
    /// Downloads cards (and the Add filter menu) are deferred, so they would paint above it.
    fn modal_covers_header(&self) -> bool {
        self.dialog_open()
            || self.import.dialog.is_some()
            || self.is_fullscreen_viewer
            || self.compare.is_some()
            || self.slideshow.is_some()
    }

    /// Closes the Sort, Add filter and Downloads popovers (and the Albums
    /// overview's menus) once anything modal opens over
    /// them. Run from render, so every opener (menus, palette, actions,
    /// clicks) is covered without each one knowing about the header.
    pub(crate) fn close_header_popovers_under_modals(&mut self) {
        if self.modal_covers_header() {
            self.close_header_popovers();
        }
        // The Tags page menus never outlive the page (Back, a search, a
        // year) and give way to the sidebar search (⌘F).
        if !self.tags_page_shown() || self.search_focused {
            self.tag_manager.close_popovers();
        }
        // Likewise the Downloads screen's row menu.
        if !self.downloads_screen_shown() || self.search_focused {
            self.downloads.row_menu = None;
        }
    }

    /// Plain field resets, no notify: called from render.
    pub(crate) fn close_header_popovers(&mut self) {
        self.sort_menu_open = false;
        self.filter_menu = None;
        self.albums.overview.sort_menu = None;
        self.albums.overview.card_menu = None;
        self.albums.page.menu = None;
        self.tag_manager.close_popovers();
        self.downloads.row_menu = None;
        if self.downloads.open {
            self.downloads.open = false;
            self.downloads.focused = None;
            self.downloads.highlight = None;
        }
    }

    /// Another dialog or menu is up. The import dialog would open under the
    /// ones drawn above it, and ↵ would go to the ones whose keys come first
    /// (a delete confirmation), so imports wait until it is closed.
    pub(crate) fn import_blocked_by_modal(&self) -> bool {
        self.dialog_open()
            || self.sort_menu_open
            || self.filter_menu.is_some()
            || self.downloads.open
            || self.rate_popover_shown()
            || self.albums.overview.sort_menu.is_some()
            || self.albums.overview.card_menu.is_some()
            || self.albums.page.menu.is_some()
            || self.downloads.row_menu.is_some()
    }

    /// "Import…": a folder picker allowing several folders. (Loose files can
    /// be dropped; rfd cannot offer files and folders in one panel.)
    pub fn pick_import_folders(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() {
            self.show_warning_toast("Open a library before importing", cx);
            return;
        }
        // Say so before the picker, not after the folders are chosen.
        if self.import.is_busy() {
            self.show_warning_toast("An import is already running", cx);
            return;
        }
        if self.import_blocked_by_modal() {
            self.show_warning_toast("Close the open dialog before importing", cx);
            return;
        }
        // Tests count the request instead of opening a real dialog.
        #[cfg(test)]
        IMPORT_PICKS.with(|n| n.set(n.get() + 1));
        #[cfg(not(test))]
        {
            let tx = self.ui_tx.clone();
            cx.spawn(async move |_this, _cx| {
                if let Some(handles) = rfd::AsyncFileDialog::new()
                    .set_title("Import into Library")
                    .pick_folders()
                    .await
                {
                    let paths = handles.iter().map(|h| h.path().to_path_buf()).collect();
                    let _ = tx.send_blocking(UiMessage::ImportPicked(paths));
                }
            })
            .detach();
        }
    }

    /// "Import into New Library…" (welcome ⇧⌘I): choose or create the new
    /// library's folder; it opens, and its first scan then asks for sources.
    pub fn pick_new_library_for_import(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        {
            NEW_LIBRARY_PICKS.with(|n| n.set(n.get() + 1));
            let _ = cx;
        }
        #[cfg(not(test))]
        {
            let tx = self.ui_tx.clone();
            cx.spawn(async move |_this, _cx| {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title("Choose or Create a Folder for the New Library")
                    .pick_folder()
                    .await
                {
                    let _ =
                        tx.send_blocking(UiMessage::NewLibraryPicked(handle.path().to_path_buf()));
                }
            })
            .detach();
        }
    }

    /// Plans importing `paths` (dropped or picked) and opens the dialog.
    pub fn begin_import(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            self.show_warning_toast("Open a library before importing", cx);
            return;
        };
        if self.import.is_busy() {
            self.show_warning_toast("An import is already running", cx);
            return;
        }
        if self.import_blocked_by_modal() {
            self.show_warning_toast("Close the open dialog before importing", cx);
            return;
        }
        if paths.is_empty() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.import.plan_cancel = cancel.clone();
        self.import.dialog = Some(ImportDialog::Planning {
            count: paths.len(),
            checked: None,
        });
        let mine = cancel.clone();
        let task = cx
            .background_executor()
            .spawn(async move { engine.plan_import(&paths, &cancel) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                // Dismissed while planning, or superseded by a newer import.
                if mine.load(Ordering::Relaxed) || !Arc::ptr_eq(&app.import.plan_cancel, &mine) {
                    return;
                }
                match result {
                    Ok(plan) => app.import.dialog = Some(ImportDialog::Ready(plan)),
                    Err(err) => {
                        app.import.dialog = None;
                        app.set_error(format!("Could not read the files to import: {err:#}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Esc / Cancel / backdrop: closes the dialog, stopping any planning.
    pub fn cancel_import_dialog(&mut self, cx: &mut Context<Self>) {
        self.import.plan_cancel.store(true, Ordering::Relaxed);
        self.import.dialog = None;
        cx.notify();
    }

    /// ↵ / Import: copies the plan in the background. On an empty plan (OK)
    /// it only closes.
    pub fn confirm_import(&mut self, cx: &mut Context<Self>) {
        let Some(ImportDialog::Ready(plan)) = self.import.dialog.take() else {
            // Planning: nothing to confirm yet.
            return;
        };
        cx.notify();
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        if plan.files.is_empty() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.import.running = Some(cancel.clone());
        self.import.progress = Some((0, plan.files.len()));
        let mine = cancel.clone();
        let task = cx
            .background_executor()
            .spawn(async move { engine.run_import(plan, &cancel) });
        cx.spawn(async move |this, cx| {
            let report = task.await;
            let _ = this.update(cx, |app, cx| {
                // A library switch stopped it; the new library shows nothing.
                if app
                    .import
                    .running
                    .as_ref()
                    .is_some_and(|r| Arc::ptr_eq(r, &mine))
                {
                    app.finish_import(report, cx);
                }
            });
        })
        .detach();
    }

    /// A library switch: closes the dialog and stops a running copy after the
    /// current file (the old engine's event thread is joined next, and the
    /// copy holds that engine). Copies made so far stay in the old library.
    pub(crate) fn abandon_import(&mut self, cx: &mut Context<Self>) {
        self.import.pending_import_after_open = false;
        self.cancel_import_dialog(cx);
        if let Some(cancel) = self.import.running.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.import.progress = None;
    }

    /// "Cancel Import": stops after the current file; copies so far stay.
    pub fn cancel_running_import(&mut self, cx: &mut Context<Self>) {
        match &self.import.running {
            Some(cancel) => {
                cancel.store(true, Ordering::Relaxed);
                cx.notify();
            }
            None => self.show_warning_toast("No import is running", cx),
        }
    }

    pub(crate) fn finish_import(&mut self, report: CopyImportReport, cx: &mut Context<Self>) {
        let total = self
            .import
            .progress
            .map_or(report.copied + report.failed.len(), |(_, total)| total);
        self.import.running = None;
        self.import.progress = None;
        self.refresh_album_counts();
        let copied = |key: &str| {
            i18n::t(&i18n::plural_key(key, report.copied))
                .replace("{n}", &report.copied.to_string())
        };
        if report.cancelled {
            self.show_toast(copied("import.cancelled"), cx);
        } else if report.failed.is_empty() {
            self.show_toast(copied("import.imported"), cx);
        }
        // ponytail: failures only in the banner (it says X of Y); the
        // failed-file panel is a separate plan.
        if !report.failed.is_empty() {
            let details: Vec<String> = report
                .failed
                .iter()
                .take(3)
                .map(|(path, err)| {
                    let name = path.file_name().unwrap_or(path.as_os_str());
                    format!("{}: {err:#}", name.to_string_lossy())
                })
                .collect();
            let more = report.failed.len() - details.len();
            let more = if more > 0 {
                i18n::t("import.failed_more").replace("{n}", &i18n::count(more))
            } else {
                String::new()
            };
            let failure = format!(
                "{}: {}{more}",
                i18n::import_failed(report.copied, total, report.failed.len()),
                details.join("; ")
            );
            self.import.failure = Some(failure.clone());
            self.set_error(failure, cx);
        }
        cx.notify();
    }
}

#[cfg(test)]
thread_local! {
    /// Import pickers the app asked to open on this test thread.
    pub(crate) static IMPORT_PICKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// "Import into New Library…" folder pickers asked for on this thread.
    pub(crate) static NEW_LIBRARY_PICKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
