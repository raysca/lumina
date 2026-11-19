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
    RecentlyAdded,
    Screenshots,
    Places,
    Trash,
    Untagged,
    User(i64),
    /// A hand-picked album, token `album:{id}`.
    Album(i64),
    Tag(String),
    /// The Downloads screen.
    Downloads,
}

impl LastView {
    /// Encodes the view as a config token.
    pub fn token(&self) -> String {
        match self {
            LastView::All => "all".to_string(),
            LastView::Favorites => "favorites".to_string(),
            LastView::Videos => "videos".to_string(),
            LastView::RecentlyAdded => "recently-added".to_string(),
            LastView::Screenshots => "screenshots".to_string(),
            LastView::Places => "places".to_string(),
            LastView::Trash => "trash".to_string(),
            LastView::Untagged => "untagged".to_string(),
            LastView::User(id) => format!("user:{id}"),
            LastView::Album(id) => format!("album:{id}"),
            LastView::Tag(tag) => format!("tag:{tag}"),
            LastView::Downloads => "downloads".to_string(),
        }
    }

    /// Decodes a token. Missing, unknown, or malformed tokens fall back to All.
    pub fn from_token(token: &str) -> Self {
        match token {
            "all" | "" => LastView::All,
            "favorites" => LastView::Favorites,
            "videos" => LastView::Videos,
            "recently-added" => LastView::RecentlyAdded,
            "screenshots" => LastView::Screenshots,
            "places" => LastView::Places,
            "trash" => LastView::Trash,
            "untagged" => LastView::Untagged,
            "downloads" => LastView::Downloads,
            other => {
                if let Some(raw_id) = other.strip_prefix("user:") {
                    return match raw_id.parse::<i64>() {
                        Ok(id) if id > 0 => LastView::User(id),
                        _ => LastView::All,
                    };
                }
                if let Some(raw_id) = other.strip_prefix("album:") {
                    return match raw_id.parse::<i64>() {
                        Ok(id) if id > 0 => LastView::Album(id),
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
            AlbumSelection::System(SystemAlbum::RecentlyAdded) => LastView::RecentlyAdded,
            AlbumSelection::System(SystemAlbum::Screenshots) => LastView::Screenshots,
            AlbumSelection::System(SystemAlbum::Places) => LastView::Places,
            AlbumSelection::System(SystemAlbum::RecentlyDeleted) | AlbumSelection::Trash => {
                LastView::Trash
            }
            // Views not exposed by the sidebar have no persisted token yet.
            AlbumSelection::System(_) => LastView::All,
            AlbumSelection::User(id) => LastView::User(*id),
            AlbumSelection::Album(id) => LastView::Album(*id),
            AlbumSelection::Tag(tag) => LastView::Tag(tag.clone()),
            AlbumSelection::Untagged => LastView::Untagged,
            AlbumSelection::Downloads => LastView::Downloads,
            // Duplicates, the Albums overview and the Tags page have no
            // persisted token; they restore as All.
            AlbumSelection::Duplicates | AlbumSelection::AlbumsOverview | AlbumSelection::Tags => {
                LastView::All
            }
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
            LastView::RecentlyAdded => AlbumSelection::System(SystemAlbum::RecentlyAdded),
            LastView::Screenshots => AlbumSelection::System(SystemAlbum::Screenshots),
            LastView::Places => AlbumSelection::System(SystemAlbum::Places),
            LastView::Trash => AlbumSelection::Trash,
            LastView::Untagged => AlbumSelection::Untagged,
            LastView::User(id) => AlbumSelection::User(*id),
            LastView::Album(id) => AlbumSelection::Album(*id),
            LastView::Tag(tag) => AlbumSelection::Tag(tag.clone()),
            LastView::Downloads => AlbumSelection::Downloads,
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

/// Bounded map of per-video resume positions (seconds), keyed by media id,
/// for one library (`AppConfig::resume_library`).
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
    /// First item of the top visible grid row; see `last_scroll_anchor_id`.
    pub scroll_anchor_id: Option<u64>,
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
    /// Database id of the first item in the top visible grid row when the
    /// config was saved. An item id, not a row, so it survives column and
    /// density changes and new photos shifting indices.
    #[serde(default)]
    pub last_scroll_anchor_id: Option<u64>,
    /// Most recent committed searches from the sidebar field, newest first.
    /// The app trims and bounds this list when recording; `serde(default)` keeps existing config
    /// files loading and keeps tests off the real config path.
    #[serde(default)]
    pub recent_searches: Vec<String>,
    /// Libraries opened successfully, most recent first, canonical and
    /// deduplicated (see `app::recents`). Forgetting one never touches its
    /// folder or catalog.
    #[serde(default)]
    pub recent_libraries: Vec<PathBuf>,
    /// Per-video resume positions in seconds, keyed by media id, for the
    /// library named by `resume_library`.
    ///
    /// The value is a shared handle: grid playback state updates it in place,
    /// so the existing config save path persists the latest position without a
    /// second app-level field. The map is validated and bounded on load/save;
    /// older config files without this key start empty.
    #[serde(default)]
    pub resume_positions: ResumePositions,
    /// The library `resume_positions` belongs to. Media ids restart in every
    /// library, so the points are dropped when another library opens. `None`
    /// in configs written before this field: they belong to
    /// `last_library_path`.
    #[serde(default)]
    pub resume_library: Option<PathBuf>,
    /// External editor used by "Edit in External Editor": an absolute macOS
    /// application path or a bundle identifier. `None` (the serde default)
    /// means unset. Values are validated when the action runs, so a hand-edited
    /// or stale config can never launch an unexpected process.
    #[serde(default)]
    pub external_editor: Option<String>,
    /// Last chosen video playback speed. `None` means 1x; off-menu values
    /// snap to [`VIDEO_SPEEDS`] on load (see [`AppConfig::restored_video_speed`]).
    #[serde(default)]
    pub video_speed: Option<f32>,
    /// Whether video sound was last muted.
    #[serde(default)]
    pub video_muted: bool,
    /// Last video volume (0..=1). `None` means the 0.85 default; see
    /// [`AppConfig::restored_video_volume`].
    #[serde(default)]
    pub video_volume: Option<f32>,
    /// Metadata templates (rights and keywords), in the order the user made
    /// them. They belong to the person, not a library, so they live here.
    #[serde(default)]
    pub templates: Vec<crate::app::templates::MetadataTemplate>,
    /// Seconds per slideshow slide: 3, 5 or 10 (see
    /// [`AppConfig::slideshow_interval`]).
    #[serde(default = "default_slideshow_interval_secs")]
    pub slideshow_interval_secs: u8,
    /// Whether the slideshow wraps to the first item at the end.
    #[serde(default)]
    pub slideshow_loop: bool,
    /// How the browser lays out a view: the uniform Grid (the default) or
    /// justified Mosaic rows. An unknown value reads as Grid.
    #[serde(default, deserialize_with = "lenient_browse_layout")]
    pub browse_layout: BrowseLayout,
    /// The Mosaic's grouping level on the zoom pill: year or month cards,
    /// or the day rows (the default). An unknown value reads as Days.
    #[serde(default, deserialize_with = "lenient_group_level")]
    pub group_level: GroupLevel,
    /// The Albums overview's card order. An unknown value reads as Name.
    #[serde(default, deserialize_with = "lenient")]
    pub album_sort: AlbumSort,
    /// The user hid the album settings panel (⌘K Hide Album Settings).
    #[serde(default)]
    pub album_panel_hidden: bool,
    /// Settings › General › Default view on launch. An unknown value reads
    /// as Last viewed.
    #[serde(default, deserialize_with = "lenient")]
    pub launch_view: LaunchView,
    /// Settings › Downloads › Simultaneous downloads, one of
    /// [`DOWNLOAD_CONCURRENCY_CHOICES`]. Anything else reads as the default.
    #[serde(
        default = "default_download_concurrency",
        deserialize_with = "lenient_download_concurrency"
    )]
    pub download_concurrency: usize,
    /// Settings › About › Check Daily: look for an update at launch, at most
    /// once a day. Off unless the user turns it on (AGENTS.md §5.4).
    #[serde(default)]
    pub auto_update_check: bool,
    /// When the last update check ran (failed attempts count), in Unix seconds.
    #[serde(default)]
    pub last_update_check_secs: Option<u64>,
}

/// The Simultaneous downloads choices.
pub const DOWNLOAD_CONCURRENCY_CHOICES: [usize; 4] = [1, 2, 3, 5];
/// Downloads that run at once until the user picks another number.
pub const DEFAULT_DOWNLOAD_CONCURRENCY: usize = 3;

fn default_download_concurrency() -> usize {
    DEFAULT_DOWNLOAD_CONCURRENCY
}

/// Reads `download_concurrency` so a hand-edited value outside the choices
/// falls back to the default instead of failing the whole config.
fn lenient_download_concurrency<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<usize, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| DOWNLOAD_CONCURRENCY_CHOICES.contains(n))
        .unwrap_or(DEFAULT_DOWNLOAD_CONCURRENCY))
}

/// Where Lumina opens after a restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchView {
    /// All Media.
    Library,
    /// The view open at quit (the behaviour before this setting).
    #[default]
    LastViewed,
}

/// Reads a field so a hand-edited or future value falls back to the type's
/// default instead of failing the whole config (which would reset it).
fn lenient<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

/// The Albums overview's card order (its "Sort ▾" control).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumSort {
    /// A to Z.
    #[default]
    Name,
    /// Newest album first.
    Created,
    /// Newest cover photo first; albums without one last.
    NewestPhoto,
    /// Most items first.
    ItemCount,
}

impl AlbumSort {
    /// Menu order.
    pub const ALL: [AlbumSort; 4] = [
        AlbumSort::Name,
        AlbumSort::Created,
        AlbumSort::NewestPhoto,
        AlbumSort::ItemCount,
    ];
}

/// The zoom pill's grouping level in Mosaic. Years and Months apply only
/// where the timeline rail is; elsewhere the body is Days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupLevel {
    Years,
    Months,
    #[default]
    Days,
}

/// Reads `group_level` so a hand-edited or future value (such as
/// "moments") falls back to Days instead of failing the whole config.
fn lenient_group_level<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<GroupLevel, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value.as_str() {
        Some("years") => GroupLevel::Years,
        Some("months") => GroupLevel::Months,
        _ => GroupLevel::Days,
    })
}

/// The browser body: square tiles in columns, or justified rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowseLayout {
    #[default]
    Grid,
    Mosaic,
}

/// Reads `browse_layout` so a hand-edited or future value falls back to
/// Grid instead of failing the whole config (which would reset it).
fn lenient_browse_layout<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BrowseLayout, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value.as_str() {
        Some("mosaic") => BrowseLayout::Mosaic,
        _ => BrowseLayout::Grid,
    })
}

/// Video volume before the user sets one.
pub const DEFAULT_VIDEO_VOLUME: f32 = 0.85;

/// Playback speeds the video speed menu offers.
pub const VIDEO_SPEEDS: [f32; 6] = [0.25, 0.5, 1.0, 1.5, 2.0, 4.0];

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
    252.0
}

fn default_inspector_width() -> f32 {
    300.0
}

fn default_sidebar_visible() -> bool {
    true
}

fn default_inspector_visible() -> bool {
    false
}

/// Four of the 2..=8 columns: the middle level in SIZE_COLUMNS [8, 6, 4, 3, 2].
fn default_grid_columns() -> usize {
    4
}

fn default_card_fit_mode() -> bool {
    true
}

fn default_slideshow_interval_secs() -> u8 {
    5
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
            last_scroll_anchor_id: None,
            recent_searches: Vec::new(),
            recent_libraries: Vec::new(),
            resume_positions: ResumePositions::default(),
            resume_library: None,
            external_editor: None,
            video_speed: None,
            video_muted: false,
            video_volume: None,
            templates: Vec::new(),
            slideshow_interval_secs: default_slideshow_interval_secs(),
            slideshow_loop: false,
            browse_layout: BrowseLayout::Grid,
            group_level: GroupLevel::Days,
            album_sort: AlbumSort::Name,
            album_panel_hidden: false,
            launch_view: LaunchView::LastViewed,
            download_concurrency: DEFAULT_DOWNLOAD_CONCURRENCY,
            auto_update_check: false,
            last_update_check_secs: None,
        }
    }
}

// One startup parse shared by `main` (menus, window bounds) and
// `LuminaApp::new`, so the corrupt-file message survives until the app can
// show it instead of being lost to an earlier `load` that moved the file.
thread_local! {
    static STARTUP_CONFIG: std::cell::RefCell<Option<(AppConfig, Option<String>)>> =
        const { std::cell::RefCell::new(None) };
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        base.join("lumina").join("config.json")
    }

    /// Settings › Reset to defaults: the UI preferences (panel widths and
    /// visibility, grid density and fit, sort, browse layout and grouping,
    /// album order and panel, Default view on launch) go back to their
    /// defaults. The library, recents, resume points, restored session,
    /// window bounds, inspector tab, editor, playback, slideshow and
    /// templates are kept: they are the user's data or where they left off.
    pub fn reset_ui_preferences(&mut self) {
        let d = Self::default();
        self.sidebar_width = d.sidebar_width;
        self.inspector_width = d.inspector_width;
        self.sidebar_visible = d.sidebar_visible;
        self.inspector_visible = d.inspector_visible;
        self.grid_columns = d.grid_columns;
        self.card_fit_mode = d.card_fit_mode;
        self.sort_field = d.sort_field;
        self.sort_direction = d.sort_direction;
        self.browse_layout = d.browse_layout;
        self.group_level = d.group_level;
        self.album_sort = d.album_sort;
        self.album_panel_hidden = d.album_panel_hidden;
        self.launch_view = d.launch_view;
    }

    /// Loads the config once for startup and caches the result, including a
    /// corrupt-file message. `main` reads it before the window exists (menus,
    /// window bounds) and `LuminaApp::new` takes the same cached result, so the
    /// file is parsed once and the message is never lost to an earlier `load`
    /// that already moved the corrupt file aside.
    #[cfg_attr(test, allow(dead_code))]
    pub fn load_startup_once() -> (Self, Option<String>) {
        STARTUP_CONFIG.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(Self::load_impl(&Self::config_path()));
            }
            slot.clone().expect("startup config was just loaded")
        })
    }

    #[cfg(test)]
    pub fn load_from(path: &Path) -> Self {
        Self::load_impl(path).0
    }

    fn load_impl(path: &Path) -> (Self, Option<String>) {
        if !path.exists() {
            return (Self::default(), None);
        }
        let loaded = fs::read(path)
            .map_err(|err| format!("could not be read ({err})"))
            .and_then(|bytes| {
                serde_json::from_slice::<Self>(&bytes)
                    .map_err(|err| format!("could not be read ({err})"))
            });
        match loaded {
            Ok(cfg) => (cfg, None),
            Err(why) => {
                // Move the file aside before any default can be written over
                // it: an unreadable or unparseable config must not become a
                // silent reset. rename() works on the directory entry, so it
                // also preserves a file we could not read.
                let corrupt = path.with_extension("json.corrupt");
                let message = match fs::rename(path, &corrupt) {
                    Ok(()) => {
                        log::error!("settings {why}; kept at {}", corrupt.display());
                        format!(
                            "Your settings {why}, so Lumina started with defaults. \
                             The old file was kept as {}.",
                            corrupt.display()
                        )
                    }
                    Err(rename_err) => {
                        log::error!("settings {why}; could not preserve the file: {rename_err}");
                        "Your settings could not be read, so Lumina started with defaults. \
                         The old file could not be moved aside and may be overwritten when \
                         settings are next saved."
                            .to_string()
                    }
                };
                (Self::default(), Some(message))
            }
        }
    }

    /// Writes the user's real config. Compiled out of tests so no test path
    /// can overwrite it; tests use `save_to` with a temp path.
    #[cfg(not(test))]
    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        // Write beside the target and rename: a crash mid-write can leave a
        // temp file behind, but never a truncated config.json that reads as
        // defaults and is then overwritten. The nanosecond suffix keeps two
        // saves in one process from colliding on the temp name.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = path.with_extension(format!("json.tmp.{}.{}", std::process::id(), nanos));
        if let Err(err) = fs::write(&tmp, &data) {
            if let Err(remove_err) = fs::remove_file(&tmp) {
                log::warn!("could not remove the temporary settings file {tmp:?}: {remove_err}");
            }
            return Err(err.into());
        }
        if let Err(err) = fs::rename(&tmp, path) {
            if let Err(remove_err) = fs::remove_file(&tmp) {
                log::warn!("could not remove the temporary settings file {tmp:?}: {remove_err}");
            }
            return Err(err.into());
        }
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

    /// The saved video speed snapped to the nearest menu value; 1x when unset
    /// or not a usable positive number (a hand-edited config).
    pub fn restored_video_speed(&self) -> f32 {
        match self.video_speed {
            Some(saved) if saved.is_finite() && saved > 0.0 => VIDEO_SPEEDS
                .into_iter()
                .min_by(|a, b| (a - saved).abs().total_cmp(&(b - saved).abs()))
                .unwrap_or(1.0),
            _ => 1.0,
        }
    }

    /// The saved video volume clamped to 0..=1; 0.85 when unset or not a
    /// number (a hand-edited config).
    pub fn restored_video_volume(&self) -> f32 {
        match self.video_volume {
            Some(saved) if !saved.is_nan() => saved.clamp(0.0, 1.0),
            _ => DEFAULT_VIDEO_VOLUME,
        }
    }

    /// The slideshow interval; a hand-edited value off the 3/5/10 menu is
    /// read as the 5 second default.
    pub fn slideshow_interval(&self) -> std::time::Duration {
        let secs = match self.slideshow_interval_secs {
            secs @ (3 | 5 | 10) => secs,
            _ => default_slideshow_interval_secs(),
        };
        std::time::Duration::from_secs(secs.into())
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
        // Default view on launch = Library: a clean All Media, with no
        // search, year, selection or scroll position from the last session.
        if self.launch_view == LaunchView::Library {
            return RestoredSession {
                view: LastView::All,
                search_query: String::new(),
                active_year: None,
                selected_media_id: None,
                scroll_anchor_id: None,
            };
        }
        let view = match self.last_view.as_deref() {
            Some(token) => LastView::from_token(token),
            None => LastView::All,
        };
        RestoredSession {
            view,
            search_query: self.last_search_query.clone().unwrap_or_default(),
            active_year: self.last_active_year,
            selected_media_id: self.last_selected_media_id,
            scroll_anchor_id: self.last_scroll_anchor_id,
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
        self.last_scroll_anchor_id = None;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn recently_added_last_view_round_trips() {
        use super::LastView;
        let selection =
            crate::views::sidebar::AlbumSelection::System(lumina_core::SystemAlbum::RecentlyAdded);
        let view = LastView::from_selection(&selection);
        assert_eq!(
            LastView::from_token(&view.token()).to_selection(),
            selection,
            "Recently Added is a sidebar view, so it reopens after a restart"
        );
    }

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

    /// Regression: an unreadable config silently loaded as defaults and was
    /// then overwritten on the next save, permanently losing the settings.
    #[test]
    fn a_corrupt_config_is_preserved_and_reports_an_error() {
        let path = temp_config_path("corrupt_preserve");
        std::fs::write(&path, b"{ this is not json").unwrap();

        let (cfg, err) = AppConfig::load_impl(&path);

        assert_eq!(
            cfg.sort_field,
            AppConfig::default().sort_field,
            "an unreadable config falls back to defaults"
        );
        assert!(
            err.is_some(),
            "a corrupt config must report an error so the user knows"
        );
        let corrupt = path.with_extension("json.corrupt");
        assert!(
            corrupt.exists(),
            "the unreadable file must be kept for recovery, not deleted"
        );
        assert_eq!(
            std::fs::read(&corrupt).unwrap(),
            b"{ this is not json",
            "the preserved file must be the user's original bytes"
        );
        assert!(
            !path.exists(),
            "the unreadable file is moved aside so a later save cannot overwrite it"
        );
    }

    /// Regression: a read error (permissions, transient I/O) must not later
    /// become a silent overwrite. The file is moved aside just like an
    /// unparseable one, because `rename` does not need read permission.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_config_is_moved_aside_not_overwritten() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_config_path("unreadable_moved");
        std::fs::write(&path, b"{\"sidebar_width\": 400.0}").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&path, perms).unwrap();
        if std::fs::read(&path).is_ok() {
            // Running as root or on a mode-ignoring filesystem: the scenario
            // cannot be staged, so this is not a pass or a failure.
            return;
        }

        let (cfg, err) = AppConfig::load_impl(&path);

        assert_eq!(cfg.sidebar_width, AppConfig::default().sidebar_width);
        assert!(err.is_some(), "a read error must be surfaced");
        let corrupt = path.with_extension("json.corrupt");
        assert!(
            corrupt.exists(),
            "an unreadable file must be moved aside so the next save cannot overwrite it"
        );
        assert!(!path.exists());

        // Restore permission for cleanup and confirm the bytes survived.
        let mut perms = std::fs::metadata(&corrupt).unwrap().permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&corrupt, perms).unwrap();
        assert_eq!(
            std::fs::read(&corrupt).unwrap(),
            b"{\"sidebar_width\": 400.0}"
        );
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_file() {
        let path = temp_config_path("atomic_save");
        let cfg = AppConfig {
            sidebar_width: 321.0,
            ..Default::default()
        };
        cfg.save_to(&path).unwrap();

        assert_eq!(AppConfig::load_from(&path).sidebar_width, 321.0);
        let leftovers: Vec<String> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".tmp."))
            .collect();
        assert!(
            leftovers.is_empty(),
            "an atomic save must not leave a temp file behind: {leftovers:?}"
        );
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
            cfg.grid_columns, 4,
            "a fresh install starts at 4 columns matching SIZE_COLUMNS"
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
                scroll_anchor_id: None,
            },
            "a saved session must round-trip through the config file"
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn slideshow_settings_round_trip_and_old_config_defaults() {
        let path = temp_config_path("slideshow");
        // Older config files predate both keys and must still load.
        std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
        let old = AppConfig::load_from(&path);
        assert_eq!(old.slideshow_interval_secs, 5, "5 s by default");
        assert!(!old.slideshow_loop, "no loop by default");

        let cfg = AppConfig {
            slideshow_interval_secs: 10,
            slideshow_loop: true,
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.slideshow_interval(),
            std::time::Duration::from_secs(10)
        );
        assert!(loaded.slideshow_loop, "Loop must round-trip");

        let odd = AppConfig {
            slideshow_interval_secs: 0,
            ..AppConfig::default()
        };
        assert_eq!(
            odd.slideshow_interval(),
            std::time::Duration::from_secs(5),
            "an off-menu interval reads as the default, never a zero timer"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn scroll_anchor_round_trips_and_old_config_defaults_none() {
        let path = temp_config_path("scroll_anchor");
        let cfg = AppConfig {
            last_scroll_anchor_id: Some(24),
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();
        assert_eq!(
            AppConfig::load_from(&path)
                .restored_session()
                .scroll_anchor_id,
            Some(24),
            "the scroll anchor must round-trip through the config file"
        );

        // Older config files predate the key and must still load.
        std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
        assert_eq!(
            AppConfig::load_from(&path).last_scroll_anchor_id,
            None,
            "a missing last_scroll_anchor_id key must default to None"
        );

        let mut cleared = cfg.clone();
        cleared.clear_session_state();
        assert_eq!(
            cleared.last_scroll_anchor_id, None,
            "a new library starts at the top"
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
        let mut cfg = cfg;
        cfg.resume_library = Some(PathBuf::from("/Libraries/Family"));
        cfg.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.resume_library.as_deref(),
            Some(Path::new("/Libraries/Family")),
            "the library the points belong to must round-trip with them"
        );
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
        assert_eq!(
            AppConfig::load_from(&path).resume_library,
            None,
            "an older config has no library tag; it belongs to last_library_path"
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
    #[test]
    fn config_video_volume_roundtrip_and_default() {
        let path = temp_config_path("video_volume");
        let cfg = AppConfig {
            video_volume: Some(0.4),
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.video_volume, Some(0.4), "volume must round-trip");
        assert_eq!(loaded.restored_video_volume(), 0.4);

        std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
        let old = AppConfig::load_from(&path);
        assert_eq!(old.video_volume, None, "a missing volume key is unset");
        assert_eq!(
            old.restored_video_volume(),
            0.85,
            "an unset volume restores the 0.85 default"
        );

        for (saved, want) in [(f32::NAN, 0.85), (1.7, 1.0), (-0.2, 0.0)] {
            let cfg = AppConfig {
                video_volume: Some(saved),
                ..AppConfig::default()
            };
            assert_eq!(
                cfg.restored_video_volume(),
                want,
                "saved volume {saved} restores as {want}"
            );
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn config_video_speed_and_mute_roundtrip_and_default() {
        let path = temp_config_path("video_speed_mute");
        let cfg = AppConfig {
            video_speed: Some(2.0),
            video_muted: true,
            ..AppConfig::default()
        };
        cfg.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(loaded.video_speed, Some(2.0), "speed must round-trip");
        assert!(loaded.video_muted, "mute must round-trip");
        assert_eq!(loaded.restored_video_speed(), 2.0);

        // Older config files predate the fields and must load with defaults.
        std::fs::write(&path, br#"{"last_view":"all"}"#).unwrap();
        let old = AppConfig::load_from(&path);
        assert_eq!(old.video_speed, None, "a missing speed key must be unset");
        assert!(!old.video_muted, "a missing mute key must mean unmuted");
        assert_eq!(
            old.restored_video_speed(),
            1.0,
            "unset speed restores as 1x"
        );

        // Hand-edited values snap to a speed the menu offers.
        for (saved, want) in [
            (1.3, 1.5),
            (0.1, 0.25),
            (9.0, 4.0),
            (f32::NAN, 1.0),
            (f32::INFINITY, 1.0),
            (-2.0, 1.0),
        ] {
            let cfg = AppConfig {
                video_speed: Some(saved),
                ..AppConfig::default()
            };
            assert_eq!(
                cfg.restored_video_speed(),
                want,
                "saved speed {saved} must restore as the menu value {want}"
            );
        }

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn browse_layout_defaults_to_grid_and_round_trips() {
        let path = temp_config_path("browse_layout");
        std::fs::write(&path, br#"{"grid_columns":6}"#).unwrap();
        assert_eq!(
            AppConfig::load_from(&path).browse_layout,
            BrowseLayout::Grid,
            "a config without the key starts on Grid"
        );
        let cfg = AppConfig {
            browse_layout: BrowseLayout::Mosaic,
            ..Default::default()
        };
        cfg.save_to(&path).unwrap();
        assert_eq!(
            AppConfig::load_from(&path).browse_layout,
            BrowseLayout::Mosaic
        );
        for bad in [br#""tiles""#.as_slice(), b"42", b"null"] {
            let mut json = br#"{"grid_columns":6,"browse_layout":"#.to_vec();
            json.extend_from_slice(bad);
            json.push(b'}');
            std::fs::write(&path, json).unwrap();
            let loaded = AppConfig::load_from(&path);
            assert_eq!(
                loaded.browse_layout,
                BrowseLayout::Grid,
                "unknown value reads as Grid"
            );
            assert_eq!(loaded.grid_columns, 6, "and keeps the rest of the config");
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn group_level_defaults_to_days_and_round_trips() {
        let path = temp_config_path("group_level");
        std::fs::write(&path, br#"{"grid_columns":6}"#).unwrap();
        assert_eq!(
            AppConfig::load_from(&path).group_level,
            GroupLevel::Days,
            "a config without the key starts on Days"
        );
        for level in [GroupLevel::Years, GroupLevel::Months, GroupLevel::Days] {
            let cfg = AppConfig {
                group_level: level,
                ..Default::default()
            };
            cfg.save_to(&path).unwrap();
            assert_eq!(AppConfig::load_from(&path).group_level, level);
        }
        for bad in [br#""moments""#.as_slice(), b"3", b"null"] {
            let mut json = br#"{"grid_columns":6,"group_level":"#.to_vec();
            json.extend_from_slice(bad);
            json.push(b'}');
            std::fs::write(&path, json).unwrap();
            let loaded = AppConfig::load_from(&path);
            assert_eq!(
                loaded.group_level,
                GroupLevel::Days,
                "unknown value reads as Days"
            );
            assert_eq!(loaded.grid_columns, 6, "and keeps the rest of the config");
        }
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn album_sort_defaults_to_name_and_round_trips() {
        let path = temp_config_path("album_sort");
        std::fs::write(&path, br#"{"grid_columns":6}"#).unwrap();
        assert_eq!(
            AppConfig::load_from(&path).album_sort,
            AlbumSort::Name,
            "a config without the key sorts albums by name"
        );
        for sort in AlbumSort::ALL {
            let cfg = AppConfig {
                album_sort: sort,
                ..Default::default()
            };
            cfg.save_to(&path).unwrap();
            assert_eq!(AppConfig::load_from(&path).album_sort, sort);
        }
        std::fs::write(&path, br#"{"grid_columns":6,"album_sort":"updated"}"#).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.album_sort,
            AlbumSort::Name,
            "unknown value reads as Name"
        );
        assert_eq!(loaded.grid_columns, 6, "and keeps the rest of the config");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn launch_view_defaults_to_last_viewed_and_round_trips() {
        let path = temp_config_path("launch_view");
        std::fs::write(&path, br#"{"last_view":"favorites","grid_columns":6}"#).unwrap();
        let mut loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.launch_view,
            LaunchView::LastViewed,
            "a config written before the setting keeps today's behaviour"
        );
        assert_eq!(loaded.restored_session().view, LastView::Favorites);

        loaded.launch_view = LaunchView::Library;
        loaded.save_to(&path).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            loaded.launch_view,
            LaunchView::Library,
            "the choice round-trips"
        );
        assert_eq!(
            loaded.restored_session().view,
            LastView::All,
            "Library reopens All Media"
        );
        assert_eq!(loaded.grid_columns, 6, "and keeps the rest of the config");

        std::fs::write(&path, br#"{"launch_view":"albums","grid_columns":6}"#).unwrap();
        let loaded = AppConfig::load_from(&path);
        assert_eq!(
            (loaded.launch_view, loaded.grid_columns),
            (LaunchView::LastViewed, 6),
            "an unknown value reads as Last viewed without resetting the config"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn download_concurrency_reads_leniently() {
        let path = temp_config_path("download_concurrency");
        for (json, expected, why) in [
            (
                r#"{"grid_columns":6}"#,
                3,
                "a config written before the setting runs 3",
            ),
            (
                r#"{"download_concurrency":5,"grid_columns":6}"#,
                5,
                "a choice round-trips",
            ),
            (
                r#"{"download_concurrency":4,"grid_columns":6}"#,
                3,
                "4 is not a choice",
            ),
            (
                r#"{"download_concurrency":0,"grid_columns":6}"#,
                3,
                "0 would stall the queue",
            ),
            (
                r#"{"download_concurrency":"two","grid_columns":6}"#,
                3,
                "a string reads as 3",
            ),
        ] {
            std::fs::write(&path, json).unwrap();
            let loaded = AppConfig::load_from(&path);
            assert_eq!(
                (loaded.download_concurrency, loaded.grid_columns),
                (expected, 6),
                "{why}, without resetting the rest of the config"
            );
        }
        let mut cfg = AppConfig {
            download_concurrency: 1,
            ..AppConfig::default()
        };
        cfg.reset_ui_preferences();
        assert_eq!(
            cfg.download_concurrency, 1,
            "Reset to defaults keeps the download setting"
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn launch_view_library_restores_a_clean_all_media() {
        let saved = AppConfig {
            last_view: Some(LastView::Favorites.token()),
            last_search_query: Some("camera:sony".to_string()),
            last_active_year: Some(2024),
            last_selected_media_id: Some(42),
            last_scroll_anchor_id: Some(7),
            launch_view: LaunchView::Library,
            ..Default::default()
        };
        assert_eq!(
            saved.restored_session(),
            RestoredSession {
                view: LastView::All,
                search_query: String::new(),
                active_year: None,
                selected_media_id: None,
                scroll_anchor_id: None,
            },
            "Library opens unfiltered All Media with nothing selected"
        );
        let last = AppConfig {
            launch_view: LaunchView::LastViewed,
            ..saved
        };
        assert_eq!(
            last.restored_session(),
            RestoredSession {
                view: LastView::Favorites,
                search_query: "camera:sony".to_string(),
                active_year: Some(2024),
                selected_media_id: Some(42),
                scroll_anchor_id: Some(7),
            },
            "Last viewed still restores the whole session"
        );
    }
}
