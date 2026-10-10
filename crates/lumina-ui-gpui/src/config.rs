use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub last_library_path: Option<PathBuf>,
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        base.join("lumina").join("config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(cfg) = serde_json::from_slice(&bytes) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }
}
