use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::app::types::LayoutMode;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub last_library_path: Option<PathBuf>,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    #[serde(default = "default_inspector_width")]
    pub inspector_width: f32,
    #[serde(default = "default_sidebar_visible")]
    pub sidebar_visible: bool,
    #[serde(default = "default_inspector_visible")]
    pub inspector_visible: bool,
    #[serde(default = "default_layout_mode")]
    pub layout_mode: String,
    #[serde(default = "default_grid_columns")]
    pub grid_columns: usize,
    #[serde(default = "default_card_fit_mode")]
    pub card_fit_mode: bool,
}

fn default_sidebar_width() -> f32 {
    230.0
}

fn default_inspector_width() -> f32 {
    310.0
}

fn default_sidebar_visible() -> bool {
    true
}

fn default_inspector_visible() -> bool {
    false
}

fn default_layout_mode() -> String {
    "browser".to_string()
}

fn default_grid_columns() -> usize {
    4
}

fn default_card_fit_mode() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_library_path: None,
            sidebar_width: default_sidebar_width(),
            inspector_width: default_inspector_width(),
            sidebar_visible: default_sidebar_visible(),
            inspector_visible: default_inspector_visible(),
            layout_mode: default_layout_mode(),
            grid_columns: default_grid_columns(),
            card_fit_mode: default_card_fit_mode(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        base.join("lumina").join("config.json")
    }

    #[cfg_attr(test, allow(dead_code))] // production startup path only
    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    pub fn load_from(path: &Path) -> Self {
        if path.exists() {
            if let Ok(bytes) = fs::read(path) {
                if let Ok(cfg) = serde_json::from_slice(&bytes) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }

    /// Maps the persisted layout string back to a [`LayoutMode`], defaulting to Browser.
    pub fn restored_layout_mode(&self) -> LayoutMode {
        match self.layout_mode.as_str() {
            "split" => LayoutMode::Split,
            "viewer" => LayoutMode::Viewer,
            _ => LayoutMode::Browser,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina_cfg_test_{}_{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("config.json")
    }

    #[test]
    fn test_layout_mode_config_roundtrip_and_restore() {
        let path = temp_config_path("layout");

        let cfg = AppConfig {
            layout_mode: "viewer".to_string(),
            grid_columns: 7,
            card_fit_mode: false,
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.layout_mode, "viewer");
        assert_eq!(loaded.grid_columns, 7);
        assert!(!loaded.card_fit_mode);
        assert_eq!(loaded.restored_layout_mode(), LayoutMode::Viewer);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_restored_layout_mode_mapping_and_fallback() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.restored_layout_mode(), LayoutMode::Browser);

        cfg.layout_mode = "split".to_string();
        assert_eq!(cfg.restored_layout_mode(), LayoutMode::Split);

        cfg.layout_mode = "viewer".to_string();
        assert_eq!(cfg.restored_layout_mode(), LayoutMode::Viewer);

        cfg.layout_mode = "not-a-mode".to_string();
        assert_eq!(cfg.restored_layout_mode(), LayoutMode::Browser);
    }

    #[test]
    fn test_missing_config_file_falls_back_to_defaults() {
        let path = std::env::temp_dir().join("lumina_cfg_test_does_not_exist_987654321.json");
        let _ = std::fs::remove_file(&path);
        let cfg = AppConfig::load_from(&path);
        assert_eq!(cfg.layout_mode, "browser");
        assert_eq!(cfg.grid_columns, 4);
        assert!(cfg.card_fit_mode);
    }
}
