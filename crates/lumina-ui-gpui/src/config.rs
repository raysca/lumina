use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::app::types::LayoutMode;
use crate::views::sidebar::AlbumSelection;
use lumina_core::{SortDirection, SortField, SortSpec, SystemAlbum};

/// Stable identifier for the sidebar view that was active when the config was saved.
///
/// Persisted as a short token instead of serializing AlbumSelection directly so
/// config files stay stable when the UI enum changes. Only views reachable from the
/// sidebar get a dedicated token; other system albums fall back to All.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastView {
    All,
    Favorites,
    Videos,
    Screenshots,
    Places,
    Trash,
    User(i64),
    Tag(String),
}

impl LastView {
    /// Encodes the view as a config token.
    pub fn token(&self) -> String {
        match self {
            LastView::All => "all".to_string(),
            LastView::Favorites => "favorites".to_string(),
            LastView::Videos => "videos".to_string(),
            LastView::Screenshots => "screenshots".to_string(),
            LastView::Places => "places".to_string(),
            LastView::Trash => "trash".to_string(),
            LastView::User(id) => format!("user:{id}"),
            LastView::Tag(tag) => format!("tag:{tag}"),
        }
    }

    /// Decodes a token. Missing, unknown, or malformed tokens fall back to All.
    pub fn from_token(token: &str) -> Self {
        match token {
            "all" | "" => LastView::All,
            "favorites" => LastView::Favorites,
            "videos" => LastView::Videos,
            "screenshots" => LastView::Screenshots,
            "places" => LastView::Places,
            "trash" => LastView::Trash,
            other => {
                if let Some(raw_id) = other.strip_prefix("user:") {
                    return match raw_id.parse::<i64>() {
                        Ok(id) if id > 0 => LastView::User(id),
                        _ => LastView::All,
                    };
                }
                if let Some(tag) = other.strip_prefix("tag:") {
                    if !tag.is_empty() {
                        return LastView::Tag(tag.to_string());
                    }
                }
                LastView::All
            }
        }
    }

    /// Maps a live sidebar selection to its persisted token form.
    pub fn from_selection(selection: &AlbumSelection) -> Self {
        match selection {
            AlbumSelection::AllLibrary => LastView::All,
            AlbumSelection::System(SystemAlbum::Favorites) => LastView::Favorites,
            AlbumSelection::System(SystemAlbum::Videos) => LastView::Videos,
            AlbumSelection::System(SystemAlbum::Screenshots) => LastView::Screenshots,
            AlbumSelection::System(SystemAlbum::Places) => LastView::Places,
            AlbumSelection::System(SystemAlbum::RecentlyDeleted) | AlbumSelection::Trash => {
                LastView::Trash
            }
            // Views not exposed by the sidebar have no persisted token yet.
            AlbumSelection::System(_) => LastView::All,
            AlbumSelection::User(id) => LastView::User(*id),
            AlbumSelection::Tag(tag) => LastView::Tag(tag.clone()),
        }
    }

    /// Expands the token back into a sidebar selection.
    ///
    /// User albums must be validated against the live library before applying;
    /// a removed album falls back to All in the app layer.
    pub fn to_selection(&self) -> AlbumSelection {
        match self {
            LastView::All => AlbumSelection::AllLibrary,
            LastView::Favorites => AlbumSelection::System(SystemAlbum::Favorites),
            LastView::Videos => AlbumSelection::System(SystemAlbum::Videos),
            LastView::Screenshots => AlbumSelection::System(SystemAlbum::Screenshots),
            LastView::Places => AlbumSelection::System(SystemAlbum::Places),
            LastView::Trash => AlbumSelection::Trash,
            LastView::User(id) => AlbumSelection::User(*id),
            LastView::Tag(tag) => AlbumSelection::Tag(tag.clone()),
        }
    }
}

/// One-shot session context captured at startup and applied after the initial scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredSession {
    pub view: LastView,
    pub search_query: String,
    pub active_year: Option<i32>,
    pub selected_media_id: Option<u64>,
}

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
    #[serde(default)]
    pub inspector_tab: Option<String>,
    #[serde(default)]
    pub window_x: Option<f32>,
    #[serde(default)]
    pub window_y: Option<f32>,
    #[serde(default)]
    pub window_width: Option<f32>,
    #[serde(default)]
    pub window_height: Option<f32>,
    #[serde(default)]
    pub sort_field: Option<String>,
    #[serde(default)]
    pub sort_direction: Option<String>,
    /// Last sidebar view token; see LastView.
    #[serde(default)]
    pub last_view: Option<String>,
    /// Search query active when the config was saved.
    #[serde(default)]
    pub last_search_query: Option<String>,
    /// Timeline year filter active when the config was saved.
    #[serde(default)]
    pub last_active_year: Option<i32>,
    /// Database id of the selected media item when the config was saved.
    #[serde(default)]
    pub last_selected_media_id: Option<u64>,
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
            inspector_tab: None,
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            sort_field: None,
            sort_direction: None,
            last_view: None,
            last_search_query: None,
            last_active_year: None,
            last_selected_media_id: None,
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

    /// Restores the last inspector tab, defaulting to Info.
    pub fn restored_inspector_tab(&self) -> crate::views::inspector::InspectorTab {
        crate::views::inspector::InspectorTab::from_config_key(self.inspector_tab.as_deref())
    }

    /// Returns persisted window geometry when it looks usable.
    pub fn restored_window_bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let (x, y, width, height) = (
            self.window_x?,
            self.window_y?,
            self.window_width?,
            self.window_height?,
        );
        if width >= 960.0 && height >= 600.0 && width.is_finite() && height.is_finite() {
            Some((x, y, width, height))
        } else {
            None
        }
    }

    /// Maps the persisted layout string back to a [`LayoutMode`], defaulting to Browser.
    /// Restores the persisted grid sort, defaulting to date taken descending.
    pub fn restored_sort(&self) -> SortSpec {
        let field = match self.sort_field.as_deref() {
            Some("name") => SortField::FileName,
            Some("size") => SortField::FileSize,
            Some("rating") => SortField::Rating,
            _ => SortField::DateTaken,
        };
        let direction = match self.sort_direction.as_deref() {
            Some("asc") => SortDirection::Ascending,
            _ => SortDirection::Descending,
        };
        SortSpec { field, direction }
    }

    pub fn restored_layout_mode(&self) -> LayoutMode {
        match self.layout_mode.as_str() {
            "split" => LayoutMode::Split,
            "viewer" => LayoutMode::Viewer,
            _ => LayoutMode::Browser,
        }
    }

    /// Builds the one-shot session context applied after the startup library scan.
    ///
    /// Malformed view tokens already decode to All; callers still validate user
    /// smart albums against the opened library before applying the result.
    pub fn restored_session(&self) -> RestoredSession {
        let view = match self.last_view.as_deref() {
            Some(token) => LastView::from_token(token),
            None => LastView::All,
        };
        RestoredSession {
            view,
            search_query: self.last_search_query.clone().unwrap_or_default(),
            active_year: self.last_active_year,
            selected_media_id: self.last_selected_media_id,
        }
    }

    /// Forgets the saved session so a newly opened library starts from All.
    pub fn clear_session_state(&mut self) {
        self.last_view = None;
        self.last_search_query = None;
        self.last_active_year = None;
        self.last_selected_media_id = None;
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
            inspector_tab: Some("location".to_string()),
            window_x: Some(120.0),
            window_y: Some(80.0),
            window_width: Some(1440.0),
            window_height: Some(900.0),
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.layout_mode, "viewer");
        assert_eq!(loaded.grid_columns, 7);
        assert!(!loaded.card_fit_mode);
        assert_eq!(loaded.restored_layout_mode(), LayoutMode::Viewer);
        assert_eq!(
            loaded.restored_inspector_tab(),
            crate::views::inspector::InspectorTab::Location
        );
        assert_eq!(loaded.restored_inspector_tab().config_key(), "location");
        assert_eq!(
            loaded.restored_window_bounds(),
            Some((120.0, 80.0, 1440.0, 900.0))
        );

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
    fn test_restored_window_bounds_rejects_unusable_geometry() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.restored_window_bounds(), None);

        cfg.window_x = Some(10.0);
        cfg.window_y = Some(10.0);
        cfg.window_width = Some(400.0);
        cfg.window_height = Some(200.0);
        assert_eq!(cfg.restored_window_bounds(), None);

        cfg.window_width = Some(1200.0);
        cfg.window_height = Some(800.0);
        assert!(cfg.restored_window_bounds().is_some());
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

    #[test]
    fn test_sort_preferences_roundtrip() {
        let path = temp_config_path("sort");
        let cfg = AppConfig {
            sort_field: Some("name".to_string()),
            sort_direction: Some("asc".to_string()),
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.restored_sort(),
            SortSpec {
                field: SortField::FileName,
                direction: SortDirection::Ascending,
            }
        );
        assert_eq!(
            AppConfig::default().restored_sort(),
            SortSpec::default(),
            "missing keys fall back to date taken descending"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_session_state_roundtrip() {
        let path = temp_config_path("session");
        let cfg = AppConfig {
            last_view: Some(LastView::Tag("travel".to_string()).token()),
            last_search_query: Some("camera:sony".to_string()),
            last_active_year: Some(2024),
            last_selected_media_id: Some(42),
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.restored_session(),
            RestoredSession {
                view: LastView::Tag("travel".to_string()),
                search_query: "camera:sony".to_string(),
                active_year: Some(2024),
                selected_media_id: Some(42),
            },
            "a saved session must round-trip through the config file"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_invalid_view_token_falls_back_to_all() {
        for token in [
            "", "bogus", "user:", "user:abc", "user:-1", "user:0", "tag:",
        ] {
            let cfg = AppConfig {
                last_view: Some(token.to_string()),
                ..AppConfig::default()
            };
            assert_eq!(
                cfg.restored_session().view,
                LastView::All,
                "token {token:?} must fall back to All"
            );
        }
        assert_eq!(
            AppConfig::default().restored_session().view,
            LastView::All,
            "a missing token must fall back to All"
        );
    }

    #[test]
    fn test_user_and_tag_view_tokens_roundtrip() {
        assert_eq!(LastView::User(42).token(), "user:42");
        assert_eq!(LastView::from_token("user:42"), LastView::User(42));

        assert_eq!(LastView::Tag("family".to_string()).token(), "tag:family");
        assert_eq!(
            LastView::from_token("tag:family"),
            LastView::Tag("family".to_string())
        );

        let tricky_tag = LastView::Tag("trip: 2024 / family".to_string());
        assert_eq!(
            LastView::from_token(&tricky_tag.token()),
            tricky_tag,
            "tags may contain separators and spaces"
        );

        assert_eq!(
            LastView::from_selection(&AlbumSelection::User(7)),
            LastView::User(7)
        );
        assert_eq!(
            LastView::from_selection(&AlbumSelection::Tag("travel".to_string())),
            LastView::Tag("travel".to_string())
        );
        assert_eq!(
            LastView::from_selection(&AlbumSelection::Trash),
            LastView::Trash
        );
        assert_eq!(
            LastView::from_selection(&AlbumSelection::AllLibrary),
            LastView::All
        );
    }
}
