//! Recent libraries (welcome screen and ⌘K) and the startup library choice.
//!
//! The list lives in `AppConfig::recent_libraries`. Forgetting an entry only
//! edits that list: the folder and its catalog are never touched.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use gpui::*;

use crate::app::LuminaApp;
use crate::config::AppConfig;
use crate::i18n::t;

/// How many libraries the list keeps.
pub(crate) const RECENT_LIBRARY_LIMIT: usize = 8;

/// Welcome-screen state for the recent libraries.
#[derive(Debug, Default)]
pub(crate) struct WelcomeState {
    /// The library that could not be found (at startup or when chosen).
    pub(crate) missing: Option<PathBuf>,
    /// Retry is checking the missing library off the UI thread.
    pub(crate) retrying: bool,
    /// Why the last Retry could not open the missing library. Only a real
    /// error: a folder that is simply not there has none.
    pub(crate) missing_error: Option<String>,
    /// Tab focus in the missing-library state: 0 Retry, 1 Choose.
    pub(crate) missing_focus: Option<usize>,
    /// Counts library opens, Retry starts and Backs; an open, a Retry or a
    /// recent check applies only if it has not moved since it started.
    pub(crate) retry_generation: u64,
    /// The ticket and folder of the library open in flight; see
    /// [`Self::opening`].
    pub(crate) opening: Option<(u64, PathBuf)>,
    /// The recent the keyboard has focused (↑/↓), as a list index.
    pub(crate) focused_recent: Option<usize>,
    /// Recents whose folder is not there. `None` = not checked yet, shown
    /// as available.
    pub(crate) unavailable: Option<HashSet<PathBuf>>,
    /// The sample library is being written (a second request is ignored).
    pub(crate) generating_sample: bool,
    /// The list the last availability check was started for.
    checked: Option<Vec<PathBuf>>,
    /// The scrolling welcome body, and the recents list inside it (row `i`
    /// is its child `i + 1`, after the heading): the rows' last-frame
    /// bounds, to scroll the focused one into view.
    pub(crate) body_scroll: ScrollHandle,
    pub(crate) recents_scroll: ScrollHandle,
    /// The focus the body was last scrolled for, so it scrolls once per
    /// ↑/↓ and a wheel scroll is left alone.
    scrolled_for: std::cell::Cell<Option<usize>>,
}

impl WelcomeState {
    /// Shows (or with `None` leaves) the unreachable-library state, dropping
    /// any Retry still in flight: its result will no longer apply.
    pub(crate) fn set_missing(&mut self, missing: Option<PathBuf>) {
        self.missing = missing;
        self.retrying = false;
        self.missing_error = None;
        self.missing_focus = None;
    }

    /// A library open is in flight and nothing has superseded it: the
    /// welcome body hides so it does not flash up at launch.
    pub(crate) fn opening(&self) -> bool {
        self.opening_path().is_some()
    }

    /// The folder of the open in flight that nothing has superseded.
    pub(crate) fn opening_path(&self) -> Option<&Path> {
        let (ticket, path) = self.opening.as_ref()?;
        (*ticket == self.retry_generation).then_some(path.as_path())
    }
}

/// Puts `path` (canonicalised, so `/var` and `/private/var` are one entry)
/// at the front, dropping an older copy and anything past the limit.
pub(crate) fn push_recent(list: &mut Vec<PathBuf>, path: &Path) {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    // Entries were canonicalised when pushed; comparing needs no IO.
    list.retain(|p| *p != path);
    list.insert(0, path);
    list.truncate(RECENT_LIBRARY_LIMIT);
}

/// Forgets one entry. It touches only the list, never the folder or catalog.
pub(crate) fn remove_recent(list: &mut Vec<PathBuf>, index: usize) {
    if index < list.len() {
        list.remove(index);
    }
}

/// What to open at launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StartupLibrary {
    Open(PathBuf),
    /// The last library's folder is gone; the welcome screen says so.
    Missing(PathBuf),
    None,
}

/// The last library, if its folder is still there (one `is_dir`, as before).
pub(crate) fn startup_library(cfg: &AppConfig) -> StartupLibrary {
    match &cfg.last_library_path {
        Some(path) if path.is_dir() => StartupLibrary::Open(path.clone()),
        Some(path) => StartupLibrary::Missing(path.clone()),
        None => StartupLibrary::None,
    }
}

/// The folder's own name, for rows and messages.
pub(crate) fn library_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The banner text for a recent that is not where it was, chosen while
/// another library is open (the welcome state is not on screen then).
pub(crate) fn missing_library_message(path: &Path) -> String {
    t("welcome.missing_library")
        .replace("{name}", &library_name(path))
        .replace("{path}", &path.display().to_string())
}

impl LuminaApp {
    /// Opens the startup library, or notes that it is missing.
    pub(crate) fn apply_startup_library(
        &mut self,
        startup: StartupLibrary,
        cx: &mut Context<Self>,
    ) {
        match startup {
            // Startup keeps the pending restore; a user-chosen folder clears it.
            StartupLibrary::Open(path) => self.open_library_inner(path, cx),
            StartupLibrary::Missing(path) => {
                self.welcome.set_missing(Some(path.clone()));
                // The folder may have moved with its volume (an external
                // drive mounted elsewhere): look before asking the user.
                self.try_relocate_library(path, cx);
            }
            StartupLibrary::None => {}
        }
    }

    /// Looks for a missing library's volume and, when it is mounted somewhere
    /// else, updates the saved paths and opens it from there. The catalog
    /// follows the volume, so nothing is rescanned or lost.
    pub(crate) fn try_relocate_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // A later open, Back or Retry supersedes this relocation.
        let ticket = (self.library_generation, self.welcome.retry_generation);
        let task = cx.background_executor().spawn({
            let path = path.clone();
            async move { lumina_core::library::find_library_relocation(&path) }
        });
        cx.spawn(async move |this, cx| {
            let relocation = match task.await {
                Ok(Some(relocation)) => relocation,
                Ok(None) => return,
                Err(err) => {
                    log::warn!("could not look for the missing library at {path:?}: {err:#}");
                    return;
                }
            };
            let _ = this.update(cx, |app, cx| {
                // The catalog is remapped but not moved yet, so a superseded
                // result leaves the old layout untouched.
                if (app.library_generation, app.welcome.retry_generation) != ticket {
                    return;
                }
                let found = relocation.library_root.clone();
                if let Err(err) = relocation.commit() {
                    log::error!("could not move the relocated catalog: {err:#}");
                    app.set_error(format!("Could not open the moved library: {err:#}"), cx);
                    return;
                }
                for recent in &mut app.config.recent_libraries {
                    if recent == &path {
                        *recent = found.clone();
                    }
                }
                if app.config.last_library_path.as_ref() == Some(&path) {
                    app.config.last_library_path = Some(found.clone());
                }
                if let Err(err) = app.try_save_config() {
                    // The catalog already moved; open it for this session and
                    // let the next save (or launch) settle the paths.
                    log::warn!("could not save the relocated library path: {err:#}");
                    app.set_error(format!("Could not save settings: {err:#}"), cx);
                }
                app.welcome.set_missing(None);
                app.open_library_inner(found, cx);
            });
        })
        .detach();
    }

    /// Opens a recent library (welcome row, ⌘K), re-checking its one folder
    /// first on the background executor (an unmounted network volume can
    /// stall a `stat` for seconds). A missing folder shows the notice
    /// instead of opening.
    pub(crate) fn open_recent_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // Back, a Retry or another library supersedes this check.
        let ticket = (self.library_generation, self.welcome.retry_generation);
        let task = cx.background_executor().spawn({
            let path = path.clone();
            async move { path.is_dir() }
        });
        cx.spawn(async move |this, cx| {
            let present = task.await;
            // The app may be gone; then nothing is left to update.
            let _ = this.update(cx, |app, cx| {
                // Another library opened meanwhile (a double click, ⌘O), or
                // Back or Retry was pressed: this late answer must not apply.
                if (app.library_generation, app.welcome.retry_generation) != ticket {
                    return;
                }
                if present {
                    // Through open_library, so a running download asks first.
                    app.open_library(path, cx);
                    return;
                }
                if let Some(unavailable) = app.welcome.unavailable.as_mut() {
                    unavailable.insert(path.clone());
                }
                if app.grid_state.is_some() {
                    // The welcome notice is not on screen; say it in the banner.
                    app.set_error(missing_library_message(&path), cx);
                }
                app.welcome.set_missing(Some(path.clone()));
                // It may be the same volume at a new mount point.
                app.try_relocate_library(path, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Retry in the unreachable-library state. The folder check and the
    /// engine open run on the background executor (an unmounted network
    /// volume can stall either for seconds); the result is applied here.
    pub(crate) fn retry_missing_library(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.welcome.missing.clone() else {
            return;
        };
        if self.welcome.retrying {
            return;
        }
        self.welcome.retrying = true;
        self.welcome.retry_generation += 1;
        let generation = self.welcome.retry_generation;
        cx.notify();
        let task = cx.background_executor().spawn({
            let path = path.clone();
            async move {
                path.is_dir()
                    .then(|| crate::app::library::open_engine(&path))
            }
        });
        cx.spawn(async move |this, cx| {
            let opened = task.await;
            // The app may be gone; then nothing is left to update.
            let _ = this.update(cx, |app, cx| {
                // Back, another library (Choose, ⌘O, a recent) or a newer
                // Retry superseded this one: its result must not apply.
                let current = app.welcome.retrying && app.welcome.retry_generation == generation;
                if current {
                    app.welcome.retrying = false;
                    cx.notify();
                }
                if !current
                    || app.grid_state.is_some()
                    || app.welcome.missing.as_ref() != Some(&path)
                {
                    // The stale engine started its initial scan on open.
                    if let Some(Ok((engine, _))) = &opened {
                        engine.cancel_scan();
                    }
                    return;
                }
                match opened {
                    // Still not there: the state stays, nothing has changed.
                    None => {
                        app.welcome.missing_error = None;
                        // Before asking the user, look for the volume.
                        app.try_relocate_library(path.clone(), cx);
                    }
                    Some(Err(err)) => {
                        log::warn!("retry could not open library at {path:?}: {err:#}");
                        app.welcome.missing_error = Some(format!("{err:#}"));
                    }
                    Some(Ok((engine, rx))) => {
                        // Only the startup library's saved view applies to it.
                        if app.config.last_library_path.as_ref() != Some(&path) {
                            app.pending_restore = None;
                            app.config.clear_session_state();
                        }
                        app.finish_open_library(path, engine, rx, cx);
                    }
                }
            });
        })
        .detach();
    }

    /// Back (Esc) from the unreachable-library state to the welcome screen
    /// and its recent libraries. A Retry still in flight is dropped.
    pub(crate) fn leave_missing_library(&mut self, cx: &mut Context<Self>) {
        self.welcome.set_missing(None);
        // A recent's folder check still in flight is dropped too.
        self.welcome.retry_generation += 1;
        cx.notify();
    }

    /// Opens the recent at `index`, if there is one.
    pub(crate) fn open_recent_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(path) = self.config.recent_libraries.get(index).cloned() {
            self.welcome.focused_recent = Some(index);
            self.open_recent_library(path, cx);
        }
    }

    /// ↑/↓ on the welcome screen; the first press focuses an end.
    pub(crate) fn move_recent_focus(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.config.recent_libraries.len();
        if len == 0 {
            return;
        }
        let next = match self.welcome.focused_recent {
            None if delta < 0 => len - 1,
            None => 0,
            Some(i) => (i as isize + delta).clamp(0, len as isize - 1) as usize,
        };
        self.welcome.focused_recent = Some(next);
        cx.notify();
    }

    /// Forgets the recent at `index` (⌫ or ×). On a save error the list is
    /// put back and the banner says why (AGENTS §2).
    pub(crate) fn remove_recent_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let before = self.config.recent_libraries.clone();
        remove_recent(&mut self.config.recent_libraries, index);
        if self.save_recents_or_revert(before, cx) {
            let len = self.config.recent_libraries.len();
            // A row above the focus moves it up with its library; the
            // focused row itself hands the focus to the next one.
            self.welcome.focused_recent = self
                .welcome
                .focused_recent
                .filter(|_| len > 0)
                .map(|i| if index < i { i - 1 } else { i.min(len - 1) });
        }
        cx.notify();
    }

    /// ⌘K Clear Recent Libraries. Only the list changes.
    pub(crate) fn clear_recents(&mut self, cx: &mut Context<Self>) {
        let before = std::mem::take(&mut self.config.recent_libraries);
        if self.save_recents_or_revert(before, cx) {
            self.welcome.focused_recent = None;
        }
        cx.notify();
    }

    /// Saves a list change; on failure restores `before` and reports it.
    fn save_recents_or_revert(&mut self, before: Vec<PathBuf>, cx: &mut Context<Self>) -> bool {
        match self.try_save_config() {
            Ok(()) => true,
            Err(err) => {
                self.config.recent_libraries = before;
                self.set_error(format!("Could not save settings: {err:#}"), cx);
                false
            }
        }
    }

    /// Checks, off the UI thread, which recents are not there (an
    /// unmounted network volume can block a `stat` for seconds). Runs when
    /// the welcome screen first shows and again when the list changes.
    pub(crate) fn check_recent_availability(&mut self, cx: &mut Context<Self>) {
        let paths = self.config.recent_libraries.clone();
        if self.welcome.checked.as_ref() == Some(&paths) {
            return;
        }
        self.welcome.checked = Some(paths.clone());
        let task = cx.background_executor().spawn({
            let paths = paths.clone();
            async move {
                paths
                    .into_iter()
                    .filter(|path| !path.is_dir())
                    .collect::<HashSet<_>>()
            }
        });
        cx.spawn(async move |this, cx| {
            let unavailable = task.await;
            // The app may be gone; then nothing is left to update.
            let _ = this.update(cx, |app, cx| {
                // A newer check for a changed list wins.
                if app.welcome.checked.as_ref() == Some(&paths) {
                    app.welcome.unavailable = Some(unavailable);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The welcome screen's content and handlers.
    pub(crate) fn welcome_props(
        &self,
        cx: &mut Context<Self>,
    ) -> crate::views::onboarding::WelcomeProps {
        use crate::views::missing_library::MissingLibraryProps;
        use crate::views::onboarding::{RecentRow, WelcomeProps};
        self.scroll_focused_recent_into_view();
        let recents =
            self.config
                .recent_libraries
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    let path = path.clone();
                    RecentRow {
                        name: library_name(&path),
                        path: path.display().to_string(),
                        unavailable: self.recent_unavailable(&path),
                        focused: self.welcome.focused_recent == Some(i),
                        on_open: Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.open_recent_at(i, cx)
                        })),
                        on_remove: Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.remove_recent_at(i, cx)
                        })),
                        on_delete: Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.prompt_delete_library(path.clone(), cx)
                        })),
                    }
                })
                .collect();
        WelcomeProps {
            on_open: Box::new(
                cx.listener(|this, _: &ClickEvent, _, cx| this.pick_folder_dialog(cx)),
            ),
            on_import_new: Box::new(
                cx.listener(|this, _: &ClickEvent, _, cx| this.pick_new_library_for_import(cx)),
            ),
            on_pick_desktop_pics: Box::new(
                cx.listener(|this, _: &ClickEvent, _, cx| this.open_pictures_library(cx)),
            ),
            on_sample: Box::new(
                cx.listener(|this, _: &ClickEvent, _, cx| this.open_sample_library(cx)),
            ),
            sample_busy: self.welcome.generating_sample,
            has_desktop_pics: self.has_desktop_pics,
            recents,
            recents_scroll: self.welcome.recents_scroll.clone(),
            missing: self.welcome.missing.as_deref().map(|path| {
                let body = if self.welcome.missing_error.is_some() {
                    t("welcome.missing_body_failed")
                } else if path.starts_with("/Volumes") {
                    t("welcome.missing_body_volume")
                } else {
                    t("welcome.missing_body_folder")
                };
                MissingLibraryProps {
                    path: path.display().to_string(),
                    body,
                    checking: self.welcome.retrying,
                    error: self.welcome.missing_error.clone(),
                    focus: self.welcome.missing_focus,
                    on_retry: Box::new(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.retry_missing_library(cx)),
                    ),
                    on_choose: Box::new(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.pick_folder_dialog(cx)),
                    ),
                    on_back: Box::new(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.leave_missing_library(cx)),
                    ),
                }
            }),
        }
    }

    /// Scrolls the welcome body the least amount that shows the focused
    /// recent (nearest edge), once per focus change. Runs at render from
    /// the last frame's bounds, which match the body's current offset (the
    /// offset changes here or by the wheel, never between two ↑/↓).
    fn scroll_focused_recent_into_view(&self) {
        let focused = self.welcome.focused_recent;
        if self.welcome.scrolled_for.replace(focused) == focused {
            return;
        }
        let Some(row) = focused.and_then(|i| self.welcome.recents_scroll.bounds_for_item(i + 1))
        else {
            return;
        };
        let body = &self.welcome.body_scroll;
        let viewport = body.bounds();
        let mut offset = body.offset();
        if row.top() < viewport.top() {
            offset.y += viewport.top() - row.top();
        } else if row.bottom() > viewport.bottom() {
            offset.y -= row.bottom() - viewport.bottom();
        }
        // Prepaint clamps the offset to the content.
        body.set_offset(offset);
    }

    /// Whether a recent is known to be missing.
    pub(crate) fn recent_unavailable(&self, path: &Path) -> bool {
        self.welcome
            .unavailable
            .as_ref()
            .is_some_and(|set| set.contains(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina_recents_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[core::prelude::v1::test]
    fn recent_libraries_dedupe_cap_and_order() {
        let root = temp("order");
        let dirs: Vec<PathBuf> = (0..10)
            .map(|i| {
                let dir = root.join(format!("lib{i}"));
                std::fs::create_dir_all(&dir).unwrap();
                dir
            })
            .collect();
        let mut list = Vec::new();
        for dir in &dirs {
            push_recent(&mut list, dir);
        }
        assert_eq!(list.len(), RECENT_LIBRARY_LIMIT, "the list is capped");
        assert_eq!(
            list[0],
            dirs[9].canonicalize().unwrap(),
            "the newest is first"
        );
        assert_eq!(
            list[7],
            dirs[2].canonicalize().unwrap(),
            "the oldest fell off"
        );

        // Two spellings of one folder (as /var is /private/var on macOS):
        // a symlink and its target are one library.
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&dirs[5], &alias).unwrap();
        push_recent(&mut list, &dirs[5]);
        push_recent(&mut list, &alias);
        // temp_dir() is under /var on macOS; this is its /private/var form.
        push_recent(&mut list, &dirs[5].canonicalize().unwrap());
        assert_eq!(list.len(), RECENT_LIBRARY_LIMIT, "a reopen adds no copy");
        assert_eq!(
            list[0],
            dirs[5].canonicalize().unwrap(),
            "a reopen moves it first"
        );
        assert_eq!(
            list.iter().filter(|p| **p == list[0]).count(),
            1,
            "/var and /private/var are the same library"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[core::prelude::v1::test]
    fn remove_recent_keeps_catalog_and_folder() {
        let dir = temp("remove");
        let catalog = dir.join("catalog_0000.db");
        std::fs::write(&catalog, b"catalog").unwrap();
        let mut list = Vec::new();
        push_recent(&mut list, &dir);
        remove_recent(&mut list, 0);
        remove_recent(&mut list, 5);
        assert!(list.is_empty(), "the entry is forgotten");
        assert!(dir.is_dir(), "removing a recent never deletes its folder");
        assert_eq!(
            std::fs::read(&catalog).unwrap(),
            b"catalog",
            "removing a recent never touches its catalog"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[core::prelude::v1::test]
    fn startup_library_reports_missing_folder() {
        let dir = temp("startup");
        let mut cfg = AppConfig::default();
        assert_eq!(startup_library(&cfg), StartupLibrary::None);
        cfg.last_library_path = Some(dir.clone());
        assert_eq!(startup_library(&cfg), StartupLibrary::Open(dir.clone()));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            startup_library(&cfg),
            StartupLibrary::Missing(dir.clone()),
            "a gone folder is reported, not skipped silently"
        );
        assert!(missing_library_message(&dir).contains(&library_name(&dir)));
    }

    #[core::prelude::v1::test]
    fn old_config_without_recents_loads() {
        let cfg: AppConfig = serde_json::from_str(r#"{"last_library_path":"/Photos"}"#)
            .expect("a config from before recents must still parse");
        assert_eq!(cfg.last_library_path.as_deref(), Some(Path::new("/Photos")));
        assert!(cfg.recent_libraries.is_empty());
    }
}
