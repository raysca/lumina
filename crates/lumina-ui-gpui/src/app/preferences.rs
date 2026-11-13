//! Preferences modal state and actions.

use std::path::{Path, PathBuf};

use gpui::*;

use crate::app::LuminaApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreferencesTab {
    #[default]
    General,
    Storage,
    About,
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
    pub fn open_preferences(&mut self, cx: &mut Context<Self>) {
        // Its sheet would take the keys over an open draft (Done/Cancel).
        if self.edits.session.is_some() {
            return;
        }
        self.preferences_open = true;
        self.preferences_tab = PreferencesTab::General;
        self.preferences_cache_cleared = false;
        self.refresh_cache_usage();
        cx.notify();
    }

    pub fn open_about(&mut self, cx: &mut Context<Self>) {
        // About is a Preferences tab: the same sheet, the same rule.
        if self.edits.session.is_some() {
            return;
        }
        self.preferences_open = true;
        self.preferences_tab = PreferencesTab::About;
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
