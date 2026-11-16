//! Preferences modal state and actions.

use std::path::{Path, PathBuf};

use gpui::*;

use crate::app::LuminaApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreferencesTab {
    #[default]
    General,
    Library,
    Downloads,
    Storage,
    About,
}

impl PreferencesTab {
    /// Sidebar order; ↑/↓ walk it.
    pub const ALL: [Self; 5] = [
        Self::General,
        Self::Library,
        Self::Downloads,
        Self::Storage,
        Self::About,
    ];
}

/// Thumbnail cache directory used by Lumina.
pub fn thumbnail_cache_dir() -> PathBuf {
    // Tests click Clear Cache; they must never touch the real cache.
    if cfg!(test) {
        return std::env::temp_dir().join(format!("lumina-test-thumbs-{}", std::process::id()));
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("lumina")
        .join("thumbnails")
}

/// Thumbnail cache footprint shown on the Storage tab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheUsage {
    pub bytes: u64,
    pub files: usize,
}

fn directory_usage(path: &Path) -> CacheUsage {
    let mut usage = CacheUsage::default();
    let Ok(entries) = std::fs::read_dir(path) else {
        return usage;
    };
    for entry in entries.filter_map(Result::ok) {
        match entry.metadata() {
            Ok(metadata) if metadata.is_file() => {
                usage.bytes += metadata.len();
                usage.files += 1;
            }
            Ok(metadata) if metadata.is_dir() => {
                let inner = directory_usage(&entry.path());
                usage.bytes += inner.bytes;
                usage.files += inner.files;
            }
            _ => {}
        }
    }
    usage
}

/// "just now", "2 min ago", "3 h ago", "2 d ago".
pub fn ago(elapsed: std::time::Duration) -> String {
    match elapsed.as_secs() {
        s if s < 60 => "just now".to_string(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86_400),
    }
}

/// LICENSE / THIRD_PARTY_NOTICES.md: the bundle's Resources, else the repo
/// root (dev builds).
pub fn bundled_doc(name: &str) -> Option<PathBuf> {
    let bundled = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.parent()?.join("Resources").join(name)));
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name);
    bundled
        .into_iter()
        .chain([repo])
        .find(|path| path.is_file())
}

fn reveal(path: &Path) {
    // Tests click these buttons; never open Finder from a test.
    if cfg!(test) {
        return;
    }
    if let Err(err) = std::process::Command::new("open").arg(path).spawn() {
        log::warn!("could not open {path:?}: {err:?}");
    }
}

impl LuminaApp {
    /// Settings › General › Default view on launch. A failed save puts the
    /// old choice back and shows the error banner.
    pub fn set_launch_view(&mut self, view: crate::config::LaunchView, cx: &mut Context<Self>) {
        let previous = self.config.launch_view;
        if previous == view {
            return;
        }
        self.config.launch_view = view;
        if !self.save_config() {
            self.config.launch_view = previous;
        }
        cx.notify();
    }

    /// The General TopBar's Reset to defaults and ⌘K Reset Settings to
    /// Defaults: settings (General) with the confirmation over them.
    pub fn prompt_reset_preferences(&mut self, cx: &mut Context<Self>) {
        if !self.preferences_open {
            self.open_preferences(cx);
        }
        // Still closed: an edit draft has the keys.
        if self.preferences_open {
            self.preferences_tab = PreferencesTab::General;
            self.preferences_reset_confirm = true;
        }
        cx.notify();
    }

    pub fn cancel_reset_preferences(&mut self, cx: &mut Context<Self>) {
        self.preferences_reset_confirm = false;
        cx.notify();
    }

    /// Puts the UI preferences back to their defaults, live and saved (see
    /// [`crate::config::AppConfig::reset_ui_preferences`]). A failed save
    /// puts every value back and shows the error banner.
    pub fn confirm_reset_preferences(&mut self, cx: &mut Context<Self>) {
        self.preferences_reset_confirm = false;
        let mut before = self.config.clone();
        self.layout_into(&mut before);
        let mut reset = before.clone();
        reset.reset_ui_preferences();
        self.apply_ui_preferences(reset, cx);
        if self.save_config() {
            self.set_last_action("Settings reset to defaults");
        } else {
            self.apply_ui_preferences(before, cx);
        }
        cx.notify();
    }

    /// Copies the live layout (panels, grid density, sort) into `cfg`, as a
    /// save writes it.
    pub(crate) fn layout_into(&self, cfg: &mut crate::config::AppConfig) {
        cfg.sidebar_width = self.sidebar_width;
        cfg.inspector_width = self.inspector_width;
        cfg.sidebar_visible = !self.sidebar_collapsed;
        cfg.inspector_visible = self.inspector_open;
        cfg.grid_columns = self.grid_columns.clamp(2, 8);
        cfg.card_fit_mode = self.card_fit_mode;
        cfg.sort_field = Some(self.sort.field_key().to_string());
        cfg.sort_direction = Some(self.sort.direction_key().to_string());
    }

    /// Makes `cfg` the config and the live layout follow it. A new sort
    /// reorders the grid, so the position-based selection is cleared.
    fn apply_ui_preferences(&mut self, cfg: crate::config::AppConfig, cx: &mut Context<Self>) {
        self.config = cfg;
        self.sidebar_width = self.config.sidebar_width;
        self.inspector_width = self.config.inspector_width;
        self.sidebar_collapsed = !self.config.sidebar_visible;
        // The View menu names what Show/Hide Sidebar will do next.
        cx.set_menus(crate::menus::build_menus(self.sidebar_collapsed));
        self.inspector_open = self.config.inspector_visible;
        self.grid_columns = crate::views::search_bar::ladder_columns(self.config.grid_columns);
        self.card_fit_mode = self.config.card_fit_mode;
        let sort = self.config.restored_sort();
        if sort != self.sort {
            self.sort = sort;
            self.clear_selection(cx);
        }
    }

    pub fn open_preferences(&mut self, cx: &mut Context<Self>) {
        self.open_preferences_at(PreferencesTab::General, cx);
    }

    pub fn open_about(&mut self, cx: &mut Context<Self>) {
        self.open_preferences_at(PreferencesTab::About, cx);
    }

    /// Opens settings on `tab` (⌘K Settings: Downloads, the downloads
    /// settings buttons). Refused while an edit draft is open.
    pub fn open_preferences_at(&mut self, tab: PreferencesTab, cx: &mut Context<Self>) {
        // Its sheet would take the keys over an open draft (Done/Cancel).
        if self.edits.session.is_some() {
            return;
        }
        self.preferences_open = true;
        self.preferences_tab = tab;
        self.preferences_cache_cleared = false;
        self.refresh_cache_usage();
        cx.notify();
    }

    pub fn close_preferences(&mut self, cx: &mut Context<Self>) {
        self.preferences_open = false;
        cx.notify();
    }

    pub fn set_preferences_tab(&mut self, tab: PreferencesTab, cx: &mut Context<Self>) {
        self.preferences_tab = tab;
        if tab == PreferencesTab::Storage {
            self.refresh_cache_usage();
        }
        cx.notify();
    }

    /// ↑/↓ in settings: the previous or next category, stopping at the ends.
    pub fn step_preferences_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let all = PreferencesTab::ALL;
        let at = all.iter().position(|t| *t == self.preferences_tab);
        let next = at
            .unwrap_or(0)
            .saturating_add_signed(delta)
            .min(all.len() - 1);
        self.set_preferences_tab(all[next], cx);
    }

    pub fn refresh_cache_usage(&mut self) {
        self.preferences_cache = Some(directory_usage(&thumbnail_cache_dir()));
    }

    pub fn clear_thumbnail_cache(&mut self, cx: &mut Context<Self>) {
        let dir = thumbnail_cache_dir();
        let result = std::fs::remove_dir_all(&dir)
            .or_else(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(err)
                }
            })
            .and_then(|()| std::fs::create_dir_all(&dir));
        match result {
            Ok(()) => {
                self.refresh_cache_usage();
                self.preferences_cache_cleared = true;
                self.set_last_action("Thumbnail cache cleared");
            }
            Err(err) => {
                self.preferences_cache_cleared = false;
                self.set_error_with_retry(
                    format!("Could not clear thumbnail cache: {err:#}"),
                    Self::clear_thumbnail_cache,
                    cx,
                )
            }
        }
        cx.notify();
    }

    pub fn reveal_cache_directory(&mut self, cx: &mut Context<Self>) {
        let dir = thumbnail_cache_dir();
        let _ = std::fs::create_dir_all(&dir);
        reveal(&dir);
        cx.notify();
    }

    pub fn reveal_logs(&mut self, cx: &mut Context<Self>) {
        let path = crate::logging::log_file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
            reveal(parent);
        }
        cx.notify();
    }

    pub fn reveal_config(&mut self, cx: &mut Context<Self>) {
        let path = crate::config::AppConfig::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
            reveal(parent);
        }
        cx.notify();
    }

    pub fn rescan_library(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.grid_state.as_mut() {
            state.is_scanning = true;
            state.scan_progress = 0;
            // Rescan is the retry for thumbnails that failed to decode.
            state.clear_all_failed();
            // A first scan that failed sends no ScanComplete; the user's own
            // rescan must not then pop up the new-library import picker.
            self.import.pending_import_after_open = false;
            state.engine.rescan();
        }
        self.last_action = None;
        cx.notify();
    }

    /// About → MIT License / Acknowledgements.
    pub fn open_bundled_doc(&mut self, name: &str, cx: &mut Context<Self>) {
        match bundled_doc(name) {
            Some(path) => reveal(&path),
            None => self.set_error(format!("{name} is missing from this build."), cx),
        }
        cx.notify();
    }

    pub fn library_path_label(&self) -> String {
        self.grid_state
            .as_ref()
            .map(|state| crate::views::preferences::tilde(state.engine.library_root()))
            .or_else(|| {
                self.config
                    .last_library_path
                    .as_deref()
                    .map(crate::views::preferences::tilde)
            })
            .unwrap_or_else(|| "No library open".to_string())
    }

    /// Label for the configured external editor; "Not set" when unset or when
    /// the saved value fails use-time validation.
    pub fn external_editor_label(&self) -> String {
        self.config
            .validated_external_editor()
            .map(str::to_string)
            .unwrap_or_else(|| "Not set".to_string())
    }

    /// Persists the external editor choice and saves the config.
    ///
    /// The raw value is stored so a temporarily unavailable app can be
    /// re-validated when the action runs; empty values clear the setting.
    pub fn set_external_editor(&mut self, editor: Option<String>, cx: &mut Context<Self>) {
        self.config.external_editor = editor
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        self.save_config();
        cx.notify();
    }

    /// Opens the native macOS application picker for the external editor.
    ///
    /// Test builds compile the dialog out so no test can open a real dialog;
    /// tests call `Self::set_external_editor` directly instead.
    pub fn choose_external_editor(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        {
            let _ = (self, cx);
        }
        #[cfg(not(test))]
        {
            cx.spawn(async move |this, async_cx| {
                let mut dialog = rfd::AsyncFileDialog::new()
                    .set_title("Choose External Editor")
                    .add_filter("Applications", &["app"]);
                if Path::new("/Applications").is_dir() {
                    dialog = dialog.set_directory("/Applications");
                }
                let Some(handle) = dialog.pick_file().await else {
                    return;
                };
                let path = handle.path();
                let Some(editor) = path.to_str().map(str::to_string) else {
                    log::warn!("chosen external editor path is not valid UTF-8: {path:?}");
                    return;
                };
                let _ = this.update(async_cx, |this, cx| {
                    this.set_external_editor(Some(editor), cx);
                });
            })
            .detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn ago_reads_minutes_hours_and_days() {
        use std::time::Duration;
        assert_eq!(ago(Duration::from_secs(5)), "just now");
        assert_eq!(ago(Duration::from_secs(125)), "2 min ago");
        assert_eq!(ago(Duration::from_secs(3 * 3600 + 5)), "3 h ago");
        assert_eq!(ago(Duration::from_secs(2 * 86_400)), "2 d ago");
    }
}
