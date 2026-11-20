use gpui::*;
use lumina_core::{EngineEvent, ExportPreset, LuminaEngine, MediaItem, SystemAlbum};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::views::grid::GridState;
use crate::views::sidebar::AlbumSelection;

/// Opens the engine for a library folder: the IO half of opening a library,
/// safe to run on a background thread (it touches no app state).
pub(crate) fn open_engine(
    path: &Path,
) -> anyhow::Result<(
    Arc<LuminaEngine>,
    impl IntoIterator<Item = EngineEvent> + Send + 'static,
)> {
    // Tests must never write the user's real config or catalogs: they keep
    // the catalog inside the (temporary) library folder instead.
    #[cfg(not(test))]
    return LuminaEngine::open(path.to_path_buf());
    #[cfg(test)]
    {
        let opened = LuminaEngine::open_with_paths(
            path.to_path_buf(),
            path.join("catalog.db"),
            Some(path.join("cache")),
        )?;
        OPENED_ENGINES
            .lock()
            .push((path.to_path_buf(), Arc::downgrade(&opened.0)));
        Ok(opened)
    }
}

/// Every engine [`open_engine`] opened, by library path, so a test can
/// check that one it never installed was dropped. A static, not a thread
/// local: the open runs on the background executor.
#[cfg(test)]
pub(crate) static OPENED_ENGINES: parking_lot::Mutex<
    Vec<(PathBuf, std::sync::Weak<LuminaEngine>)>,
> = parking_lot::Mutex::new(Vec::new());

impl LuminaApp {
    pub fn open_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.refuse_library_open(&path, cx) {
            return;
        }
        if self.ask_before_leaving(
            crate::app::downloads::DownloadsConfirm::OpenLibrary(path.clone()),
            cx,
        ) {
            return;
        }
        // A folder chosen by the user is a fresh session: drop the startup
        // restore context so another library's view or selection is never applied.
        self.pending_restore = None;
        self.config.clear_session_state();
        self.open_library_inner(path, cx);
    }

    /// Clears every id, texture, draft and trail that belongs to the library
    /// being left.
    ///
    /// Media and album ids restart per library, so an undo entry, dialog or
    /// import left open would act on the new library's rows (a Delete could
    /// trash unrelated files). Both the switch and the delete paths call this
    /// one reset so a new field cannot be missed on one of them.
    pub(crate) fn reset_library_state(&mut self, cx: &mut Context<Self>) {
        // Includes the tag-load request, so an in-flight load for the old
        // library cannot land on the new one.
        self.clear_selection_fields();
        self.selection = AlbumSelection::AllLibrary;
        self.stacks.reset();
        // The map's points and clusters belong to the old library.
        self.map = Default::default();
        self.album_draft = Default::default();
        self.albums = Default::default();
        self.delete_confirm_state = None;
        self.context_menu_state = None;
        self.cancel_tag_manager_prompts();
        // Undo entries hold ids from the old library.
        self.undo = Default::default();
        // An import, and a link being checked or previewed, belong to the
        // library they were started in.
        self.abandon_import(cx);
        self.close_network_import_dialog(cx);
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.discard_edit_draft();
        self.edits.reset_for_library();
        // A new library is a jump, not user navigation: start a fresh trail.
        self.reset_navigation_history();
    }

    /// Opens a library without touching the pending startup restore. The
    /// engine opens on the background executor (catalog migrations, an
    /// unmounted volume); the current library stays until it is installed.
    /// A later open, a Retry or Back supersedes this one.
    pub(crate) fn open_library_inner(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.refuse_library_open(&path, cx) {
            return;
        }
        if let Some(ref state) = self.grid_state {
            state.engine.cancel_scan();
        }
        // PiP plays media from the library being closed.
        self.close_pip(cx);
        self.close_compare(cx);
        self.drop_slideshow(cx);
        // A copy into the library being left stops now: finishing while the
        // new one opens, it would report into the new library.
        self.abandon_import(cx);

        self.welcome.retry_generation += 1;
        let ticket = self.welcome.retry_generation;
        self.welcome.opening = Some((ticket, path.clone()));
        cx.notify();
        let task = cx.background_executor().spawn({
            let path = path.clone();
            async move { open_engine(&path) }
        });
        cx.spawn(async move |this, cx| {
            let opened = task.await;
            // The app may be gone; then nothing is left to update.
            let _ = this.update(cx, |app, cx| {
                if app.welcome.retry_generation != ticket {
                    // The stale engine started its initial scan on open.
                    if let Ok((engine, _)) = &opened {
                        engine.cancel_scan();
                    }
                    return;
                }
                app.welcome.opening = None;
                cx.notify();
                let import = app.import.import_after_open.take() == Some(ticket);
                match opened {
                    Ok((engine, rx)) => {
                        app.finish_open_library(path, engine, rx, cx);
                        app.import.pending_import_after_open = import;
                    }
                    Err(e) => {
                        log::error!("failed to open library at {:?}: {e:?}", path);
                        app.set_error(format!("Could not open library: {e:#}"), cx);
                    }
                }
            });
        })
        .detach();
    }

    /// True when an open of `path` must not start: the same folder is
    /// already opening (two engines would share one catalog), or Merge all
    /// is running (it would land in the old engine with no undo entry; a
    /// toast says to wait).
    fn refuse_library_open(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        self.welcome.opening_path() == Some(path) || self.refuse_during_merge_all(cx)
    }

    /// The UI-thread half of opening a library: installs an engine that
    /// [`open_engine`] opened (here or on the background executor).
    pub(crate) fn finish_open_library(
        &mut self,
        path: PathBuf,
        engine: Arc<LuminaEngine>,
        rx: impl IntoIterator<Item = EngineEvent> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        // Here, not earlier: a failed open keeps the current library and its
        // viewer. Not later: the old library's state tracks which video the
        // resume point belongs to.
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        // Media ids restart in every library, so another library's resume
        // points would start the wrong videos and be pruned as missing.
        // ponytail: one library's points at a time; switching back starts
        // over. Key the map by library if people switch often.
        // Canonical, as the engine names catalogs: the same library opened
        // from Recents and from the folder picker is one library.
        let canonical = |p: &PathBuf| p.canonicalize().unwrap_or_else(|_| p.clone());
        let owner = self.config.resume_library.as_ref();
        let owner = owner.or(self.config.last_library_path.as_ref());
        if owner.map(canonical) != Some(canonical(&path)) {
            self.config.resume_positions = Default::default();
        }
        self.config.resume_library = Some(path.clone());
        // Only a library that opened becomes the startup library and a recent.
        self.config.last_library_path = Some(path.clone());
        crate::app::recents::push_recent(&mut self.config.recent_libraries, &path);
        self.welcome.set_missing(None);
        self.welcome.focused_recent = None;
        #[cfg(not(test))]
        if let Err(err) = self.config.save() {
            self.set_error(format!("Could not save settings: {err:#}"), cx);
        }

        // Share the config's resume map so playback positions recorded in the
        // grid are what the next config save writes to disk.
        // An empty catalog gets the "Building your library" screen.
        self.first_scan =
            (engine.total_count() == 0).then(crate::app::first_scan::FirstScanState::new);
        // Downloads belong to the library they were started in.
        self.downloads
            .reset(Some(crate::app::downloads::real_job(engine.clone())));
        self.network_import.inspect = Some(crate::app::network_import::real_inspect());
        let grid_state =
            GridState::with_resume_positions(engine, self.config.resume_positions.clone());
        if let Some(old) = self.grid_state.replace(grid_state) {
            let mut cache = old.texture_cache.lock();
            while let Some((_, thumb)) = cache.pop_lru() {
                self.retire_image(thumb.image);
            }
        }
        self.load_downloads(cx);
        self.reset_library_state(cx);

        // Drop and join the previous event-receiver thread to prevent orphaned threads.
        if let Some(handle) = self.event_receiver_thread.take() {
            let _ = handle.join();
        }

        // The old library's cancelled scan still queues events (a final
        // ScanComplete); the generation keeps them off the new library.
        self.library_generation += 1;
        let generation = self.library_generation;
        let tx = self.ui_tx.clone();
        self.event_receiver_thread = std::thread::Builder::new()
            .name("lumina-event-receiver".to_string())
            .spawn(move || {
                for event in rx {
                    let _ = tx.send_blocking(UiMessage::LibraryEvent { generation, event });
                }
            })
            .ok();

        self.refresh_album_counts();
        self.reload_edited_ids(cx);
        cx.notify();
    }

    pub fn pick_folder_dialog(&mut self, cx: &mut Context<Self>) {
        let current_dir = self
            .grid_state
            .as_ref()
            .map(|s| s.engine.library_root().to_path_buf())
            .or_else(|| self.config.last_library_path.clone());

        let tx = self.ui_tx.clone();
        // Tests count the request instead of opening a real dialog.
        #[cfg(test)]
        {
            let _ = (current_dir, tx, cx);
            FOLDER_PICKS.with(|n| n.set(n.get() + 1));
        }
        #[cfg(not(test))]
        cx.spawn(async move |_this, _cx| {
            let mut dialog = rfd::AsyncFileDialog::new().set_title("Choose Photos Library Folder");
            if let Some(ref dir) = current_dir {
                dialog = dialog.set_directory(dir);
            }
            if let Some(handle) = dialog.pick_folder().await {
                let _ = tx.send_blocking(UiMessage::FolderPicked(handle.path().to_path_buf()));
            }
        })
        .detach();
    }

    /// Opens a folder picker for exporting the current selection as `preset`.
    pub fn export_pick_folder(&mut self, preset: ExportPreset, cx: &mut Context<Self>) {
        // Refuse before the picker, so the user does not choose a folder
        // only to be told an export is already running.
        if self.jobs.export.is_some() {
            self.show_warning_toast(crate::i18n::t("jobs.export_running"), cx);
            return;
        }
        let selected = self.get_selected_ids();
        if selected.is_empty() {
            self.set_error("Select at least one item to export", cx);
            return;
        }
        // Tests record the request instead of opening a real dialog.
        #[cfg(test)]
        EXPORT_PICKS.with(|picks| picks.borrow_mut().push(preset));
        #[cfg(not(test))]
        {
            let tx = self.ui_tx.clone();
            cx.spawn(async move |_this, _cx| {
                if let Some(handle) = rfd::AsyncFileDialog::new()
                    .set_title(format!("Export {} item(s)", selected.len()))
                    .pick_folder()
                    .await
                {
                    let _ = tx.send_blocking(UiMessage::ExportFolderPicked(
                        handle.path().to_path_buf(),
                        preset,
                    ));
                }
            })
            .detach();
        }
    }

    /// The view's count from the sidebar's cached counts, without querying.
    pub(crate) fn cached_active_count(&self) -> Option<usize> {
        if !self.search_query.is_empty() {
            return None;
        }
        if let Some(year) = self.active_year {
            return self
                .timeline_year_buckets
                .iter()
                .find(|(y, _)| *y == year)
                .map(|(_, count)| *count);
        }
        match &self.selection {
            AlbumSelection::AllLibrary => {
                (self.total_library_count > 0).then_some(self.total_library_count)
            }
            AlbumSelection::System(album) => self.system_album_counts.get(album).copied(),
            AlbumSelection::User(id) => self.user_album_counts.get(id).copied(),
            AlbumSelection::Album(id) => self.albums.counts.get(id).copied(),
            AlbumSelection::Trash => self
                .system_album_counts
                .get(&SystemAlbum::RecentlyDeleted)
                .copied(),
            AlbumSelection::Tag(tag) => self
                .library_tag_counts
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, count)| *count),
            // Cached by refresh_album_counts.
            AlbumSelection::Untagged => Some(self.untagged_count),
            AlbumSelection::Duplicates => Some(self.duplicate_items_count),
            AlbumSelection::AlbumsOverview | AlbumSelection::Tags | AlbumSelection::Downloads => {
                Some(0)
            }
        }
    }

    /// The view's count for an action: cached counts first, then the active
    /// view cache, and only then a query. Rendering uses
    /// [`Self::displayed_count`], which never queries.
    pub fn get_active_count(&self) -> usize {
        let Some(state) = self.grid_state.as_ref() else {
            return 0;
        };
        self.cached_active_count()
            .or_else(|| self.fresh_view_count())
            .unwrap_or_else(|| self.view_query().count(&state.engine))
    }

    /// The view's items at `range` for an action: from the active view cache
    /// when it is current, else a query. Rendering uses
    /// [`Self::displayed_rows`], which never queries.
    pub fn get_active_slice(&self, range: std::ops::Range<usize>) -> Vec<MediaItem> {
        let Some(state) = self.grid_state.as_ref() else {
            return Vec::new();
        };
        if range.is_empty() {
            return Vec::new();
        }
        self.fresh_view_rows(&range)
            .unwrap_or_else(|| self.view_query().slice(&state.engine, range))
    }

    pub fn select_album(&mut self, selection: AlbumSelection, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.selection = selection;
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.stacks.reset();
        // A page's focus and menus never outlive the visit (↵ would fire them).
        self.albums.overview.leave();
        self.cancel_tag_manager_prompts();
        if self.selection == AlbumSelection::Duplicates {
            self.refresh_duplicates();
        }
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn toggle_shortcuts_overlay(&mut self, cx: &mut Context<Self>) {
        // Settings covers the window: the sheet would open unseen under it
        // and take its keys.
        if self.preferences_open && !self.shortcuts_overlay_open {
            return;
        }
        self.shortcuts_overlay_open = !self.shortcuts_overlay_open;
        self.shortcuts_scroll.set_offset(Point::default());
        cx.notify();
    }

    /// Scrolls the shortcut sheet by `dy` pixels (positive is down), clamped
    /// to its content.
    pub fn scroll_shortcuts(&mut self, dy: f32, cx: &mut Context<Self>) {
        let handle = &self.shortcuts_scroll;
        let max = handle.max_offset().y;
        let mut offset = handle.offset();
        offset.y = (offset.y - px(dy)).clamp(-max, px(0.0));
        handle.set_offset(offset);
        cx.notify();
    }

    pub fn close_shortcuts_overlay(&mut self, cx: &mut Context<Self>) {
        if self.shortcuts_overlay_open {
            self.shortcuts_overlay_open = false;
            cx.notify();
        }
    }

    /// Records a short user-facing confirmation for the status bar.
    pub fn set_last_action(&mut self, message: impl Into<String>) {
        self.last_action = Some(message.into());
    }

    /// Keeps XMP sidecars current after metadata edits, off the UI thread:
    /// a large batch writes thousands of files. Failures are logged but do
    /// not roll back the SQLite write. Sidecars another app wrote are left
    /// alone; the status bar says how many (the catalog keeps the edit).
    pub fn sync_metadata_sidecars(&self, ids: &[lumina_core::MediaId], cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            return;
        };
        let ids = ids.to_vec();
        let task = cx
            .background_executor()
            .spawn(async move { engine.write_metadata_sidecars(&ids) });
        cx.spawn(async move |this, cx| {
            let skipped = match task.await {
                Ok(report) => report.skipped_foreign.len(),
                Err(err) => {
                    log::warn!("could not update XMP sidecars: {err:?}");
                    return;
                }
            };
            if skipped == 0 {
                return;
            }
            // The app may be gone; then there is no status bar to update.
            let _ = this.update(cx, |app, cx| {
                app.set_last_action(if skipped == 1 {
                    "1 XMP sidecar was left unchanged: another app wrote it, or Lumina cannot read it".to_string()
                } else {
                    format!("{skipped} XMP sidecars were left unchanged: another app wrote them, or Lumina cannot read them")
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// Focuses the sidebar search field with its suggestions open, showing
    /// the sidebar first when it is collapsed to the rail (which has no
    /// field). Returns false when the sidebar could not be shown.
    pub fn focus_search_field(&mut self, cx: &mut Context<Self>) -> bool {
        if self.sidebar_collapsed {
            self.toggle_sidebar(cx);
            // A failed save puts the rail back: no field to focus.
            if self.sidebar_collapsed {
                return false;
            }
        }
        self.enter_search_field();
        cx.notify();
        true
    }

    /// Drops keyboard focus from the sidebar search field.
    pub fn blur_search(&mut self, cx: &mut Context<Self>) {
        if self.search_focused {
            self.search_focused = false;
            // The dropdown is only open while the field is focused; clear the
            // dismissal flag so the next focus starts with suggestions again.
            self.search_suggestions.dismissed = false;
            self.search_suggestions.selected = None;
            self.query_bar.chip = None;
            self.top_matches.highlight = None;
            cx.notify();
        }
    }

    /// Clears the sidebar search field's query.
    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.search_query.clear();
        self.search_focused = false;
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn set_search_query(&mut self, query: &str, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.search_query = query.to_string();
        // Before clearing the year: the helper snapshots the album as left.
        self.scope_search_to_library();
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn set_active_year(&mut self, year: Option<i32>, cx: &mut Context<Self>) {
        let departing = self.navigation_snapshot();
        self.active_year = year;
        // A year shows the whole library (it overrides the selection in
        // `ViewQuery`), so the sidebar must say Library, as a search does.
        // Back still returns to the album through `departing`.
        if year.is_some() {
            self.selection = AlbumSelection::AllLibrary;
            self.stacks.reset();
        }
        self.search_query.clear();
        self.search_focused = false;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
    }

    pub fn refresh_album_counts(&mut self) {
        if let Some(ref state) = self.grid_state {
            let engine = &state.engine;
            self.total_library_count = engine.total_count();
            let mut sys = HashMap::new();
            for album in lumina_core::SystemAlbum::all() {
                sys.insert(*album, engine.query_system_album_count(*album));
            }
            self.system_album_counts = sys;

            let albums = engine.list_smart_albums();
            let mut usr = HashMap::new();
            for a in &albums {
                usr.insert(a.id, engine.query_smart_album_count(a));
            }
            self.user_album_counts = usr;
            self.user_smart_albums = albums;
            let engine = engine.clone();
            self.refresh_albums(&engine);
            self.library_tag_counts = engine.query_tag_counts();
            self.untagged_count = engine.query_untagged_count();
            self.duplicate_items_count = engine.query_duplicate_count();
            if self.selection == AlbumSelection::Duplicates {
                self.set_duplicate_groups(engine.scan_duplicate_groups());
            }
            self.item_ratings = engine.get_ratings_map();
            self.item_color_labels = engine.get_color_labels_map();
            self.item_flags = engine.get_flags_map();
            self.item_captions = engine.get_captions_map();
            self.missing_items = engine.get_missing_ids();
            self.sync_missing_card();
            let months = engine.get_timeline_buckets("month");
            self.month_header.counts = months
                .iter()
                .filter_map(|b| {
                    let (year, month) = b.bucket_key.split_once('-')?;
                    Some(((year.parse().ok()?, month.parse().ok()?), b.item_count))
                })
                .collect();
            self.rail.set_counts(&self.month_header.counts);
            let buckets = engine.get_timeline_buckets("year");
            self.levels.set_covers(&buckets, &months);
            self.timeline_year_buckets = buckets
                .iter()
                .filter_map(|b| {
                    let yr = b.bucket_key.parse::<i32>().ok()?;
                    Some((yr, b.item_count))
                })
                .collect();
        }
        // Aliases change with tag renames, so they reload with the counts.
        self.refresh_keyword_aliases();

        // Tag mutations, scan removals, and undo/redo all refresh counts here.
        // If the active tag view lost its last item, drop the stale
        // position-based selection instead of leaving it pointing nowhere.
        if matches!(
            self.selection,
            AlbumSelection::Tag(_) | AlbumSelection::Untagged
        ) && self.get_active_count() == 0
            && (self.selected_item.is_some() || !self.selected_indices.is_empty())
        {
            self.clear_selection_fields();
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Folder pickers the app asked to open on this test thread.
    pub(crate) static FOLDER_PICKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Export folder pickers asked for on this thread, by preset.
    pub(crate) static EXPORT_PICKS: std::cell::RefCell<Vec<ExportPreset>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
