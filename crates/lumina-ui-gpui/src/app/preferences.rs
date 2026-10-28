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
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("lumina")
        .join("thumbnails")
}

fn directory_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            match entry.metadata() {
                Ok(metadata) if metadata.is_file() => metadata.len(),
                Ok(metadata) if metadata.is_dir() => directory_size(&path),
                _ => 0,
            }
        })
        .sum()
}

fn reveal(path: &Path) {
    if let Err(err) = std::process::Command::new("open").arg(path).spawn() {
        log::warn!("could not open {path:?}: {err:?}");
    }
}

impl LuminaApp {
    pub fn open_preferences(&mut self, cx: &mut Context<Self>) {
        self.preferences_open = true;
        self.preferences_tab = PreferencesTab::General;
        self.refresh_cache_usage();
        cx.notify();
    }

    pub fn open_about(&mut self, cx: &mut Context<Self>) {
        self.preferences_open = true;
        self.preferences_tab = PreferencesTab::About;
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
        self.preferences_cache_bytes = Some(directory_size(&thumbnail_cache_dir()));
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
                self.set_last_action("Thumbnail cache cleared");
            }
            Err(err) => self.set_error(format!("Could not clear thumbnail cache: {err}"), cx),
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
            state.engine.rescan();
        }
        self.last_action = None;
        cx.notify();
    }

    pub fn reveal_library(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.grid_state.as_ref() {
            reveal(state.engine.library_root());
        }
        cx.notify();
    }

    pub fn library_path_label(&self) -> String {
        self.grid_state
            .as_ref()
            .map(|state| state.engine.library_root().display().to_string())
            .or_else(|| {
                self.config
                    .last_library_path
                    .as_ref()
                    .map(|p| p.display().to_string())
            })
            .unwrap_or_else(|| "No library open".to_string())
    }
}
