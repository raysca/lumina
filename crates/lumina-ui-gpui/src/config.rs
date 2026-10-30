use anyhow::Result;
use parking_lot::Mutex;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::views::sidebar::AlbumSelection;
use lumina_core::{MediaId, SortDirection, SortField, SortSpec, SystemAlbum};

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
            // Untagged has no persisted token yet; it restores as All rather
            // than inventing a session format for a derived view.
            AlbumSelection::Untagged => LastView::All,
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

/// Maximum number of per-video resume positions kept in the config.
///
/// The map is a convenience cache, not user data: evicting the oldest entries
/// only loses a resume point, so a small fixed bound is enough to keep the
/// config file from growing without limit.
pub const MAX_RESUME_POSITIONS: usize = 200;

/// Positions closer than this to the start are treated as "not started".
const MIN_RESUME_SECONDS: f64 = 1.0;

/// Positions within this many seconds of the end restart from zero, so a
/// finished video never reopens on its last frame.
const RESUME_END_MARGIN_SECONDS: f64 = 2.0;

/// Chooses where a video player should start from a saved resume position.
///
/// Returns zero (play from the start) when there is no saved position, the
/// value or duration is not finite, the saved position is effectively at the
/// start, or it is within RESUME_END_MARGIN_SECONDS of the end.
pub fn resume_start_time(saved_seconds: Option<f64>, duration_secs: f64) -> f64 {
    let Some(saved) = saved_seconds else {
        return 0.0;
    };
    if !saved.is_finite() || !duration_secs.is_finite() {
        return 0.0;
    }
    if duration_secs <= 0.0 || saved < MIN_RESUME_SECONDS {
        return 0.0;
    }
    if saved >= duration_secs - RESUME_END_MARGIN_SECONDS {
        return 0.0;
    }
    saved
}

fn is_resumable_position(seconds: f64, duration_secs: f64) -> bool {
    if !seconds.is_finite() || seconds < MIN_RESUME_SECONDS {
        return false;
    }
    // Without a duration the near-end check cannot run; keep the positive
    // position so opening the video can still choose it once metadata loads.
    if !duration_secs.is_finite() || duration_secs <= 0.0 {
        return true;
    }
    seconds < duration_secs - RESUME_END_MARGIN_SECONDS
}

#[derive(Debug, Default)]
struct ResumeStore {
    entries: BTreeMap<u64, ResumeEntry>,
    /// Monotonic in-memory stamp used to evict the least recently updated
    /// positions. Loaded entries are stamped in media-id order so pruning an
    /// oversized hand-edited file is deterministic.
    next_recency: u64,
}

#[derive(Debug, Clone, Copy)]
struct ResumeEntry {
    seconds: f64,
    recency: u64,
}

/// Bounded map of per-video resume positions (seconds), keyed by media id.
///
/// The handle is shared by AppConfig and the live grid state so save_config
/// serializes the latest playback position without adding another flat field
/// to the app. All mutations are validated and capped. Tests build their own
/// instance and never reach the user config path.
#[derive(Debug, Clone, Default)]
pub struct ResumePositions {
    inner: Arc<Mutex<ResumeStore>>,
}

impl ResumePositions {
    /// Returns the saved position for a media id, if one is stored.
    pub fn get(&self, id: MediaId) -> Option<f64> {
        self.inner
            .lock()
            .entries
            .get(&id.0)
            .map(|entry| entry.seconds)
    }

    /// Records a resume position, or clears it when the value is invalid,
    /// effectively at the start, or close enough to the end that playback
    /// should restart from zero.
    pub fn record(&self, id: MediaId, seconds: f64, duration_secs: f64) {
        let mut store = self.inner.lock();
        if !is_resumable_position(seconds, duration_secs) {
            store.entries.remove(&id.0);
            return;
        }
        let recency = store.next_recency;
        store.next_recency = store.next_recency.saturating_add(1);
        store.entries.insert(id.0, ResumeEntry { seconds, recency });
        while store.entries.len() > MAX_RESUME_POSITIONS {
            let Some(victim) = store
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.recency)
                .map(|(id, _)| *id)
            else {
                break;
            };
            store.entries.remove(&victim);
        }
    }

    /// Drops the saved position for a media id.
    pub fn remove(&self, id: MediaId) {
        self.inner.lock().entries.remove(&id.0);
    }

    /// Number of stored positions; bounded by MAX_RESUME_POSITIONS.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.lock().entries.len()
    }

    /// True when no positions are stored.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().entries.is_empty()
    }

    /// Drops positions whose media id no longer exists in the library.
    pub fn prune_missing<F>(&self, mut exists: F)
    where
        F: FnMut(MediaId) -> bool,
    {
        self.inner
            .lock()
            .entries
            .retain(|id, _| exists(MediaId(*id)));
    }
}

impl Serialize for ResumePositions {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let store = self.inner.lock();
        let entries: Vec<_> = store
            .entries
            .iter()
            .filter(|(_, entry)| is_resumable_position(entry.seconds, 0.0))
            .collect();
        let mut map = serializer.serialize_map(Some(entries.len()))?;
        for (id, entry) in entries {
            map.serialize_entry(&id.to_string(), &entry.seconds)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ResumePositions {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Use untyped values so a malformed map or entry (for example null or
        // a string) cannot discard the rest of the user's config. Only finite
        // positive JSON numbers become resume positions.
        let raw = serde_json::Value::deserialize(deserializer)?;
        let Some(map) = raw.as_object() else {
            return Ok(Self::default());
        };
        let mut parsed: Vec<(u64, f64)> = map
            .iter()
            .filter_map(|(raw_id, value)| {
                let id = raw_id.parse::<u64>().ok()?;
                let seconds = value.as_f64()?;
                is_resumable_position(seconds, 0.0).then_some((id, seconds))
            })
            .collect();
        // Deterministic order keeps bound eviction stable across serde map
        // implementations.
        parsed.sort_by_key(|(id, _)| *id);

        let mut store = ResumeStore::default();
        for (id, seconds) in parsed {
            let recency = store.next_recency;
            store.next_recency = store.next_recency.saturating_add(1);
            store.entries.insert(id, ResumeEntry { seconds, recency });
        }
        // Hand-edited or older config files may exceed the bound; keep the
        // highest media ids and discard the rest deterministically.
        while store.entries.len() > MAX_RESUME_POSITIONS {
            let Some(victim) = store.entries.keys().next().copied() else {
                break;
            };
            store.entries.remove(&victim);
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(store)),
        })
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
    /// Most recent committed toolbar searches, newest first. The app trims and
    /// bounds this list when recording; `serde(default)` keeps existing config
    /// files loading and keeps tests off the real config path.
    #[serde(default)]
    pub recent_searches: Vec<String>,
    /// Per-video resume positions in seconds, keyed by media id.
    ///
    /// The value is a shared handle: grid playback state updates it in place,
    /// so the existing config save path persists the latest position without a
    /// second app-level field. The map is validated and bounded on load/save;
    /// older config files without this key start empty.
    #[serde(default)]
    pub resume_positions: ResumePositions,
    /// External editor used by "Edit in External Editor": an absolute macOS
    /// application path or a bundle identifier. `None` (the serde default)
    /// means unset. Values are validated when the action runs, so a hand-edited
    /// or stale config can never launch an unexpected process.
    #[serde(default)]
    pub external_editor: Option<String>,
}

/// Validates an external editor setting at use time.
///
/// Accepts an absolute application path (for example
/// `/Applications/TextEdit.app`) or a bundle identifier (for example
/// `com.apple.TextEdit`). Empty, whitespace-only, control-character,
/// option-like, or otherwise malformed values return `None` so a broken config
/// can never be executed. The caller is still responsible for passing the app
/// and file as separate [`std::process::Command`] arguments.
pub fn validate_external_editor(raw: Option<&str>) -> Option<&str> {
    let editor = raw?.trim();
    if editor.is_empty() || editor.starts_with('-') || editor.chars().any(char::is_control) {
        return None;
    }
    if editor.starts_with('/') {
        // The filesystem root is not an application bundle.
        return (editor.len() > 1).then_some(editor);
    }
    let has_name_character = editor.chars().any(|c| c.is_ascii_alphanumeric());
    let is_bundle_identifier = has_name_character
        && editor
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    is_bundle_identifier.then_some(editor)
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

/// Five of the 2..=8 columns: the thumbnail slider's midpoint (50%).
fn default_grid_columns() -> usize {
    5
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
            recent_searches: Vec::new(),
            resume_positions: ResumePositions::default(),
            external_editor: None,
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

    /// Returns the configured external editor after use-time validation.
    ///
    /// Unset, empty, or malformed values all return `None`; callers that need
    /// to distinguish "unset" from "invalid" should inspect the raw field.
    pub fn validated_external_editor(&self) -> Option<&str> {
        validate_external_editor(self.external_editor.as_deref())
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
    fn test_config_roundtrip_and_restore() {
        let path = temp_config_path("config_roundtrip");

        let cfg = AppConfig {
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
        assert_eq!(loaded.grid_columns, 7);
        assert!(!loaded.card_fit_mode);
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
        assert_eq!(
            cfg.grid_columns, 5,
            "a fresh install starts at the 50% thumbnail size"
        );
        assert!(cfg.card_fit_mode);
    }

    #[test]
    fn test_legacy_config_with_layout_mode_still_loads() {
        let path = temp_config_path("legacy_layout");

        // Configs written before the layout-mode removal still carry the key.
        // AppConfig has no deny_unknown_fields, so the old file must load and
        // the unknown value is simply ignored.
        let mut legacy = serde_json::to_value(AppConfig::default()).unwrap();
        legacy
            .as_object_mut()
            .expect("AppConfig serializes to a JSON object")
            .insert("layout_mode".to_string(), "viewer".into());
        legacy["grid_columns"] = 6.into();
        legacy["card_fit_mode"] = false.into();
        std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.grid_columns, 6,
            "a legacy config containing layout_mode must still deserialize its other fields"
        );
        assert!(!loaded.card_fit_mode);

        // Saving again must not resurrect the removed field.
        loaded.save_to(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(
            !saved.contains("layout_mode"),
            "layout_mode must not be written back after the removal"
        );

        let _ = std::fs::remove_file(&path);
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
    fn test_recent_searches_default_and_roundtrip() {
        assert!(
            AppConfig::default().recent_searches.is_empty(),
            "a new config must start without search history"
        );

        let path = temp_config_path("recent_searches");
        let cfg = AppConfig {
            recent_searches: vec!["camera:sony".to_string(), "is:favorite".to_string()],
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.recent_searches,
            vec!["camera:sony".to_string(), "is:favorite".to_string()],
            "recent searches must round-trip through the config file"
        );

        // Older config files predate the field and must still load.
        std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
        assert!(
            AppConfig::load_from(&path).recent_searches.is_empty(),
            "a missing recent_searches key must default to an empty history"
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
    fn test_resume_positions_config_roundtrip_and_defaults() {
        let path = temp_config_path("resume_roundtrip");

        let cfg = AppConfig::default();
        assert!(
            cfg.resume_positions.is_empty(),
            "a new config must start without resume positions"
        );

        cfg.resume_positions.record(MediaId(7), 41.5, 120.0);
        cfg.resume_positions.record(MediaId(8), 0.5, 120.0);
        cfg.resume_positions.record(MediaId(9), 118.5, 120.0);
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.resume_positions.get(MediaId(7)),
            Some(41.5),
            "a saved resume position must round-trip through the config file"
        );
        assert_eq!(
            loaded.resume_positions.get(MediaId(8)),
            None,
            "positions at the start must not be persisted"
        );
        assert_eq!(
            loaded.resume_positions.get(MediaId(9)),
            None,
            "positions at the end must not be persisted"
        );
        assert_eq!(loaded.resume_positions.len(), 1);

        // Older config files predate the field and must still load.
        std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
        assert!(
            AppConfig::load_from(&path).resume_positions.is_empty(),
            "a missing resume_positions key must default to an empty map"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_resume_positions_validation_and_pruning() {
        let store = ResumePositions::default();
        let id = MediaId(42);

        for invalid in [f64::NAN, f64::INFINITY, 0.0, -3.0, 0.5] {
            store.record(id, invalid, 60.0);
            assert_eq!(
                store.get(id),
                None,
                "invalid position {invalid} must not be stored"
            );
        }

        store.record(id, 9.0, 10.0);
        assert_eq!(
            store.get(id),
            None,
            "positions within the end margin must be dropped"
        );

        store.record(id, 12.0, 60.0);
        assert_eq!(store.get(id), Some(12.0));
        store.record(id, 60.0, 60.0);
        assert_eq!(
            store.get(id),
            None,
            "a completed video must restart at zero"
        );

        store.record(MediaId(1), 12.0, 60.0);
        store.record(MediaId(2), 13.0, 60.0);
        store.record(MediaId(3), 14.0, 60.0);
        store.prune_missing(|media_id| media_id != MediaId(2));
        assert_eq!(store.len(), 2, "pruning must drop only missing media ids");
        assert_eq!(store.get(MediaId(2)), None);
        assert_eq!(store.get(MediaId(1)), Some(12.0));

        // Malformed and out-of-range entries in a hand-edited file are skipped
        // instead of poisoning the whole config.
        let path = temp_config_path("resume_validation");
        std::fs::write(
            &path,
            br#"{"resume_positions":{"1":-2.0,"2":0.0,"3":12.5,"bad":"x","4":0.25}}"#,
        )
        .unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.resume_positions.get(MediaId(3)),
            Some(12.5),
            "valid entries must survive malformed neighbours"
        );
        assert_eq!(loaded.resume_positions.len(), 1);

        // A wrong top-level shape must not discard unrelated settings.
        std::fs::write(&path, br#"{"grid_columns":6,"resume_positions":null}"#).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.grid_columns, 6);
        assert!(loaded.resume_positions.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_resume_positions_are_bounded_by_recency() {
        let store = ResumePositions::default();
        for index in 1..=MAX_RESUME_POSITIONS as u64 {
            store.record(MediaId(index), 5.0, 60.0);
        }
        assert_eq!(store.len(), MAX_RESUME_POSITIONS);

        // Touching the oldest entry makes it the most recent, so the next
        // insert evicts the second entry instead.
        store.record(MediaId(1), 6.0, 60.0);
        store.record(MediaId(MAX_RESUME_POSITIONS as u64 + 1), 7.0, 60.0);

        assert_eq!(store.len(), MAX_RESUME_POSITIONS);
        assert_eq!(
            store.get(MediaId(1)),
            Some(6.0),
            "recently updated positions must survive eviction"
        );
        assert_eq!(
            store.get(MediaId(2)),
            None,
            "the least recently updated position must be evicted"
        );
        assert_eq!(
            store.get(MediaId(MAX_RESUME_POSITIONS as u64 + 1)),
            Some(7.0)
        );

        // Oversized config files are bounded on load as well.
        let path = temp_config_path("resume_bounded_load");
        let entries: Vec<String> = (1..=MAX_RESUME_POSITIONS as u64 + 20)
            .map(|id| format!("\"{id}\":{}.0", id))
            .collect();
        std::fs::write(
            &path,
            format!("{{\"resume_positions\":{{{}}}}}", entries.join(",")),
        )
        .unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.resume_positions.len(), MAX_RESUME_POSITIONS);
        assert_eq!(
            loaded.resume_positions.get(MediaId(1)),
            None,
            "the lowest ids are dropped when a file exceeds the bound"
        );
        assert_eq!(
            loaded
                .resume_positions
                .get(MediaId(MAX_RESUME_POSITIONS as u64 + 20)),
            Some((MAX_RESUME_POSITIONS + 20) as f64)
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_resume_start_time_selection() {
        assert_eq!(resume_start_time(Some(42.0), 120.0), 42.0);
        assert_eq!(
            resume_start_time(Some(118.5), 120.0),
            0.0,
            "a position near the end must restart from zero"
        );
        assert_eq!(
            resume_start_time(Some(42.0), 30.0),
            0.0,
            "a saved position beyond the duration is invalid"
        );
        assert_eq!(resume_start_time(Some(0.0), 120.0), 0.0);
        assert_eq!(resume_start_time(Some(-1.0), 120.0), 0.0);
        assert_eq!(resume_start_time(Some(f64::NAN), 120.0), 0.0);
        assert_eq!(resume_start_time(Some(f64::INFINITY), 120.0), 0.0);
        assert_eq!(resume_start_time(None, 120.0), 0.0);
        assert_eq!(
            resume_start_time(Some(42.0), 0.0),
            0.0,
            "without a known duration the player must not guess a seek target"
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
