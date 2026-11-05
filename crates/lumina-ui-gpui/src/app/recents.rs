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
    /// The recent the keyboard has focused (↑/↓), as a list index.
    pub(crate) focused_recent: Option<usize>,
    /// Recents whose folder is not there. `None` = not checked yet, shown
    /// as available.
    pub(crate) unavailable: Option<HashSet<PathBuf>>,
    /// The sample library is being written (a second request is ignored).
    pub(crate) generating_sample: bool,
    /// The list the last availability check was started for.
    checked: Option<Vec<PathBuf>>,
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

/// The welcome notice for a library that is not where it was.
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
            StartupLibrary::Missing(path) => self.welcome.missing = Some(path),
            StartupLibrary::None => {}
        }
    }

    /// Opens a recent library (welcome row, ⌘K), re-checking its one folder
    /// first. A missing folder shows the notice instead of opening.
    // ponytail: this one `is_dir` runs on the UI thread, like the
    // synchronous `LuminaEngine::open` right after it; an unmounted network
    // volume can stall here. Move both to the background executor together.
    pub(crate) fn open_recent_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            if let Some(unavailable) = self.welcome.unavailable.as_mut() {
                unavailable.insert(path.clone());
            }
            if self.grid_state.is_some() {
                // The welcome notice is not on screen; say it in the banner.
                self.set_error(missing_library_message(&path), cx);
            }
            self.welcome.missing = Some(path);
            cx.notify();
            return;
        }
        // Through open_library, so a running download asks first.
        self.open_library(path, cx);
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
            self.welcome.focused_recent = self
                .welcome
                .focused_recent
                .filter(|_| len > 0)
                .map(|i| i.min(len - 1));
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
                self.set_error(format!("Could not save settings: {err}"), cx);
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
        use crate::views::onboarding::{RecentRow, WelcomeProps};
        let recents = self
            .config
            .recent_libraries
            .iter()
            .enumerate()
            .map(|(i, path)| RecentRow {
                name: library_name(path),
                path: path.display().to_string(),
                unavailable: self.recent_unavailable(path),
                focused: self.welcome.focused_recent == Some(i),
                on_open: Box::new(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.open_recent_at(i, cx)),
                ),
                on_remove: Box::new(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_recent_at(i, cx)),
                ),
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
            missing_notice: self.welcome.missing.as_deref().map(missing_library_message),
        }
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
