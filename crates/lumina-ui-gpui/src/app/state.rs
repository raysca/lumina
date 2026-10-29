use gpui::*;
use lumina_core::{
    MatchMode, MediaId, MediaItem, SmartAlbum, SmartAlbumRule, SortSpec, SystemAlbum,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::app::navigation::{NavigationHistory, NavigationSnapshot};
use crate::app::types::{
    BatchSelectionSummary, DeleteConfirmState, LayoutMode, TagSortOrder, UiMessage,
};
use crate::app::windows::{open_floating_inspector_window, open_floating_pip_window};
#[cfg(not(test))]
use crate::config::LastView;
use crate::config::{AppConfig, RestoredSession};
use crate::player;
use crate::views::context_menu::ContextMenuState;
use crate::views::grid::GridState;
use crate::views::sidebar::AlbumSelection;
use crate::views::viewer::ViewerState;

/// How long selection/view/search changes settle before the restored session
/// is written to disk. Keeps arrow-key navigation and typing from saving on
/// every event while still persisting a change the user made a moment ago.
pub(crate) const SESSION_SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

/// Generation guard for the debounced session save.
///
/// Every scheduled save takes the next generation id and only the newest id may
/// write. A change that arrives while a timer is waiting therefore supersedes
/// the older timer, which becomes a no-op instead of persisting stale state.
#[derive(Debug, Default)]
pub(crate) struct SessionSaveState {
    generation: u64,
}

impl SessionSaveState {
    /// Arms the next save and returns the generation its timer must present.
    fn arm(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    /// Arms a save unless the startup restore still owns the on-disk session.
    /// Returns `None` when the caller must leave the config file untouched.
    pub(crate) fn arm_if_allowed(&mut self, allowed: bool) -> Option<u64> {
        if allowed {
            Some(self.arm())
        } else {
            None
        }
    }

    /// True when `generation` is still the newest armed save.
    pub(crate) fn is_current(&self, generation: u64) -> bool {
        generation == self.generation
    }
}

/// Toolbar search suggestion dropdown state.
///
/// Candidate suggestions are derived on demand from the cached recent-search
/// list and cached library tag counts, so nothing here requires SQL. Only the
/// dismissal flag and the keyboard highlight need to be stored.
#[derive(Debug, Default)]
pub struct SearchSuggestionState {
    /// True after Escape hides the dropdown while the field keeps focus.
    pub dismissed: bool,
    /// Highlighted suggestion row; `None` until the user presses an arrow key.
    pub selected: Option<usize>,
}

/// Hardware-accelerated video playback state.
#[derive(Default)]
pub struct VideoPlaybackState {
    pub player: Option<player::NativeVideoPlayer>,
    pub is_playing: bool,
    pub playback_time: f64,
    pub duration: f64,
    pub playback_speed: f32,
    pub is_looping: bool,
    pub is_muted: bool,
    pub volume: f32,
    pub speed_menu_open: bool,
    pub hover_time: Option<f64>,
    pub hover_pct: Option<f32>,
    /// True while the user drags the viewer transport scrubber. The transport
    /// renders the playhead and time readout from `scrub_time` while this is set,
    /// and releasing the pointer commits the final seek.
    pub is_scrubbing: bool,
    /// Destination timestamp for the active scrub drag. `None` whenever
    /// `is_scrubbing` is false.
    pub scrub_time: Option<f64>,
    /// Last time a coalesced scrub seek was sent to the player backend. Drag
    /// moves can arrive at display rate, so seeks are throttled to at most one
    /// per interval while the destination state still follows every move.
    pub last_scrub_seek: Option<std::time::Instant>,
    pub ticker_active: Option<Arc<AtomicBool>>,
    pub ticker_handle: Option<std::thread::JoinHandle<()>>,
    pub frame_pending: Arc<AtomicBool>,
}

/// Tag manager modal state.
pub struct TagManagerState {
    pub open: bool,
    pub search: String,
    pub sort: TagSortOrder,
    pub selected: HashSet<String>,
    pub editing: Option<String>,
    pub editing_buffer: String,
    pub merge_source: Option<String>,
}

impl Default for TagManagerState {
    fn default() -> Self {
        Self {
            open: false,
            search: String::new(),
            sort: TagSortOrder::Alphabetical,
            selected: HashSet::new(),
            editing: None,
            editing_buffer: String::new(),
            merge_source: None,
        }
    }
}

/// Visual smart-album builder draft state.
pub struct AlbumDraftState {
    pub modal_open: bool,
    pub editing_album_id: Option<i64>,
    pub album_title: String,
    pub title_focused: bool,
    pub match_mode: MatchMode,
    pub logic_dropdown_open: bool,
    pub rules: Vec<SmartAlbumRule>,
    pub match_count: usize,
    pub field_dropdown: Option<usize>,
    pub operator_dropdown: Option<usize>,
    pub value_dropdown: Option<usize>,
    pub rule_value_editing: Option<usize>,
    pub rule_value_buffer: String,
    pub show_in_sidebar: bool,
}

impl Default for AlbumDraftState {
    fn default() -> Self {
        Self {
            modal_open: false,
            editing_album_id: None,
            album_title: "My Smart Album".to_string(),
            title_focused: false,
            match_mode: MatchMode::All,
            logic_dropdown_open: false,
            rules: Vec::new(),
            match_count: 0,
            field_dropdown: None,
            operator_dropdown: None,
            value_dropdown: None,
            rule_value_editing: None,
            rule_value_buffer: String::new(),
            show_in_sidebar: true,
        }
    }
}

/// Inline caption editing state for the inspector.
#[derive(Default)]
pub struct CaptionState {
    pub input_active: bool,
    pub input_buffer: String,
    pub title_input_active: bool,
    pub title_input_buffer: String,
    pub date_input_active: bool,
    pub date_input_buffer: String,
    pub location_input_active: bool,
    pub location_input_buffer: String,
    /// Bookkeeping for the batch variant of the same inline editor.
    pub batch: BatchEditState,
}

/// A metadata field the batch inspector can edit for the whole selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchEditField {
    Title,
    Caption,
    Date,
    Location,
}

/// Bookkeeping for the batch inline editor.
///
/// The batch editor reuses the single-item CaptionState buffers, because the
/// inspector render path is shared. `initial_buffer` records the draft the
/// editor opened with so committing an untouched draft can never overwrite N
/// items (selecting a mixed-value batch and pressing Enter must be a no-op).
#[derive(Debug, Default)]
pub struct BatchEditState {
    pub field: Option<BatchEditField>,
    pub initial_buffer: String,
}

/// One metadata field summarized across the batch selection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchFieldSummary {
    /// True when the selected items disagree, so the inspector shows "Mixed".
    pub mixed: bool,
    /// Shared value when every item agrees; `None` also covers "not set".
    pub common: Option<String>,
}

/// Mixed-value-aware title/caption summary for the batch inspector.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchMetadataSummary {
    pub title: BatchFieldSummary,
    pub caption: BatchFieldSummary,
    pub date: BatchFieldSummary,
    pub location: BatchFieldSummary,
}

/// Pointer distance that turns a press into a drag rather than a click.
pub const GRID_DRAG_THRESHOLD: f32 = 4.0;

/// Pointer drag-selection state for the library grid.
///
/// Grouped rather than flat fields so a stray mouse-up, Escape, or a view change
/// can reset the whole gesture with one assignment.
#[derive(Debug, Default)]
pub struct GridDragState {
    /// Window-space position where the left button went down.
    pub start: Option<Point<Pixels>>,
    /// Latest pointer position seen while the gesture is live.
    pub current: Option<Point<Pixels>>,
    /// Card the gesture is anchored to; `None` for a background drag until the
    /// pointer first sweeps a card, which then becomes the anchor.
    pub anchor: Option<usize>,
    /// Selection captured at mouse-down that the drag adds to.
    pub base: BTreeSet<usize>,
    /// Card under the pointer at the last update.
    pub hovered: Option<usize>,
    /// True between left-button down and mouse-up.
    pub active: bool,
    /// True once the pointer moved past GRID_DRAG_THRESHOLD.
    pub moved: bool,
    /// Set when a moved gesture ends; the click that follows must not change
    /// the selection the drag just built.
    pub suppress_click: bool,
}

impl GridDragState {
    /// Resets the whole gesture.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

pub struct LuminaApp {
    pub config: AppConfig,
    pub grid_state: Option<GridState>,
    pub selected_item: Option<MediaItem>,
    pub selected_index: Option<usize>,
    pub selected_indices: BTreeSet<usize>,
    pub selection_anchor: Option<usize>,
    pub selected_item_tags: Vec<String>,
    pub batch_summary: BatchSelectionSummary,
    /// Mixed-value-aware title/caption summary for the batch inspector.
    pub batch_metadata: BatchMetadataSummary,
    /// Live pointer drag-selection over the grid (marquee or additive sweep).
    pub grid_drag: GridDragState,
    pub viewer_image: Option<Arc<RenderImage>>,
    pub has_desktop_pics: bool,
    pub ui_tx: async_channel::Sender<UiMessage>,
    pub focus_handle: FocusHandle,

    // Phase 5 Presentation Shell State
    pub sidebar_collapsed: bool,
    pub sidebar_width: f32,
    pub is_dragging_sidebar_resizer: bool,
    pub inspector_open: bool,
    pub inspector_width: f32,
    pub is_dragging_inspector_resizer: bool,
    pub inspector_tab: crate::views::inspector::InspectorTab,
    pub floating_inspector_window: Option<AnyWindowHandle>,
    pub floating_pip_window: Option<AnyWindowHandle>,
    pub selection: AlbumSelection,
    pub search_query: String,
    pub search_focused: bool,
    pub search_suggestions: SearchSuggestionState,
    pub active_year: Option<i32>,
    /// Bounded Back/Forward trail of browsing destinations (album, search,
    /// year, selection, viewer). Transient overlays are never recorded.
    pub(crate) navigation_history: NavigationHistory,
    /// Captured from AppConfig at startup and applied once after the initial
    /// library scan. Cleared by user-initiated open_library.
    pub(crate) pending_restore: Option<RestoredSession>,
    pub is_fullscreen_viewer: bool,
    pub is_immersive_fullscreen: bool,
    pub immersive_hud_visible: bool,
    pub layout_mode: LayoutMode,
    pub card_fit_mode: bool,
    /// User-visible, dismissible error banner (library open/save failures).
    pub error_message: Option<String>,
    /// Most recent action feedback shown in the status bar.
    pub last_action: Option<String>,
    /// Transient success/info toast shown bottom-right (errors stay in the banner).
    pub toast: crate::views::toast::ToastState,
    /// Keyboard shortcut cheat sheet visibility.
    pub shortcuts_overlay_open: bool,
    /// Last persisted window bounds: (x, y, width, height).
    pub window_bounds: Option<(f32, f32, f32, f32)>,
    pub(crate) window_bounds_subscription: Option<Subscription>,
    pub(crate) last_bounds_save: Option<std::time::Instant>,
    /// Debounced, generation-guarded writer for the restored-session fields.
    ///
    /// Only the non-test scheduler reads this field; test builds compile that
    /// path out so no test can spawn a timer or touch the user's config.
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) session_save: SessionSaveState,

    // Phase 8 Visual Smart Album Builder State
    pub album_draft: AlbumDraftState,

    // Phase 13 Deletion Management State
    pub delete_confirm_state: Option<DeleteConfirmState>,

    // Phase 15 Photo Zoom & Grid Density State
    pub viewer_state: ViewerState,
    pub viewer_shortcuts_open: bool,
    pub grid_columns: usize,
    /// Current grid ordering (persisted in config).
    pub sort: SortSpec,
    pub sort_menu_open: bool,
    pub preferences_open: bool,
    pub preferences_tab: crate::app::preferences::PreferencesTab,
    pub preferences_cache_bytes: Option<u64>,
    pub command_palette_open: bool,
    pub command_query: String,
    pub command_selected: usize,

    // Phase 16 Hardware-Accelerated Video Player
    pub video: VideoPlaybackState,

    // Zero-SQLite UI Rendering Caches
    pub total_library_count: usize,
    pub system_album_counts: HashMap<SystemAlbum, usize>,
    pub user_album_counts: HashMap<i64, usize>,
    pub user_smart_albums: Vec<SmartAlbum>,
    pub library_tag_counts: Vec<(String, usize)>,
    pub tag_input_buffer: String,
    pub tag_input_active: bool,
    pub editing_tag: Option<String>,
    pub editing_tag_buffer: String,
    pub context_menu_state: Option<ContextMenuState>,
    pub tag_manager: TagManagerState,
    pub timeline_year_buckets: Vec<(i32, usize)>,
    pub item_ratings: HashMap<MediaId, u8>,
    pub item_color_labels: HashMap<MediaId, u8>,
    pub item_captions: HashMap<MediaId, String>,
    pub caption: CaptionState,
    /// Reversible catalog edits (ratings, tags, captions, favorites, trash).
    pub undo: crate::app::undo::UndoState,
    pub(crate) event_receiver_thread: Option<std::thread::JoinHandle<()>>,
}

impl LuminaApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        #[cfg(test)]
        let mut config = AppConfig::default();
        #[cfg(not(test))]
        let mut config = AppConfig::load();
        // Keep stale or hand-edited config files from unbounded history growth.
        config
            .recent_searches
            .truncate(crate::app::library::MAX_RECENT_SEARCHES);
        let default_pics =
            dirs::picture_dir().or_else(|| dirs::home_dir().map(|h| h.join("Pictures")));
        let has_desktop_pics = default_pics
            .as_ref()
            .map(|p| p.exists() && p.is_dir())
            .unwrap_or(false);

        let (ui_tx, ui_rx) = async_channel::unbounded::<UiMessage>();
        let focus_handle = cx.focus_handle();
        let spawn_tx = ui_tx.clone();

        // Async task on GPUI event loop to receive messages and trigger 120 FPS UI updates
        cx.spawn(async move |weak_self, async_cx| {
            while let Ok(msg) = ui_rx.recv().await {
                match msg {
                    UiMessage::OpenFloatingInspector => {
                        let weak_app = weak_self.clone();
                        let tx = spawn_tx.clone();
                        let _ = async_cx.update(|cx: &mut App| {
                            open_floating_inspector_window(weak_app, tx, cx);
                        });
                    }
                    UiMessage::OpenFloatingPip => {
                        let weak_app = weak_self.clone();
                        let tx = spawn_tx.clone();
                        let _ = async_cx.update(|cx: &mut App| {
                            open_floating_pip_window(weak_app, tx, cx);
                        });
                    }
                    other => {
                        let _ = weak_self.update(async_cx, |this, cx| {
                            this.handle_ui_message(other, cx);
                        });
                    }
                }
            }
        })
        .detach();

        let restored_layout = config.restored_layout_mode();

        #[cfg_attr(test, allow(unused_mut))]
        let mut app = Self {
            config: config.clone(),
            grid_state: None,
            selected_item: None,
            selected_index: None,
            selected_indices: BTreeSet::new(),
            selection_anchor: None,
            selected_item_tags: Vec::new(),
            batch_summary: BatchSelectionSummary::default(),
            batch_metadata: BatchMetadataSummary::default(),
            grid_drag: GridDragState::default(),
            viewer_image: None,
            has_desktop_pics,
            ui_tx,
            focus_handle,
            sidebar_collapsed: !config.sidebar_visible,
            sidebar_width: config.sidebar_width,
            is_dragging_sidebar_resizer: false,
            inspector_open: config.inspector_visible,
            inspector_width: config.inspector_width,
            is_dragging_inspector_resizer: false,
            inspector_tab: config.restored_inspector_tab(),
            floating_inspector_window: None,
            floating_pip_window: None,
            selection: AlbumSelection::AllLibrary,
            search_query: String::new(),
            search_focused: false,
            search_suggestions: SearchSuggestionState::default(),
            active_year: None,
            navigation_history: NavigationHistory::new(NavigationSnapshot::default()),
            pending_restore: Some(config.restored_session()),
            is_fullscreen_viewer: false,
            is_immersive_fullscreen: false,
            immersive_hud_visible: true,
            layout_mode: restored_layout,
            card_fit_mode: config.card_fit_mode,
            error_message: None,
            last_action: None,
            toast: crate::views::toast::ToastState::default(),
            shortcuts_overlay_open: false,
            window_bounds: config.restored_window_bounds(),
            window_bounds_subscription: None,
            last_bounds_save: None,
            session_save: SessionSaveState::default(),
            album_draft: AlbumDraftState::default(),
            delete_confirm_state: None,
            viewer_state: ViewerState::default(),
            viewer_shortcuts_open: false,
            grid_columns: config.grid_columns.clamp(2, 8),
            sort: config.restored_sort(),
            sort_menu_open: false,
            preferences_open: false,
            preferences_tab: crate::app::preferences::PreferencesTab::General,
            preferences_cache_bytes: None,
            command_palette_open: false,
            command_query: String::new(),
            command_selected: 0,
            video: VideoPlaybackState {
                playback_speed: 1.0,
                is_looping: true,
                volume: 0.85,
                frame_pending: Arc::new(AtomicBool::new(false)),
                ..Default::default()
            },
            total_library_count: 0,
            system_album_counts: HashMap::new(),
            user_album_counts: HashMap::new(),
            user_smart_albums: Vec::new(),
            library_tag_counts: Vec::new(),
            tag_input_buffer: String::new(),
            tag_input_active: false,
            editing_tag: None,
            editing_tag_buffer: String::new(),
            context_menu_state: None,
            tag_manager: TagManagerState::default(),
            timeline_year_buckets: Vec::new(),
            item_ratings: HashMap::new(),
            item_color_labels: HashMap::new(),
            item_captions: HashMap::new(),
            caption: CaptionState::default(),
            undo: crate::app::undo::UndoState::default(),
            event_receiver_thread: None,
        };

        // If previously selected library exists, open it immediately.
        // Tests are isolated: they must set up their own temporary library.
        #[cfg(not(test))]
        if let Some(ref path) = config.last_library_path {
            if path.exists() && path.is_dir() {
                // Startup keeps the pending restore; a user-chosen folder clears it.
                app.open_library_inner(path.clone(), cx);
            }
        }

        app
    }

    pub fn save_config(&mut self) {
        #[cfg(test)]
        return;
        #[cfg(not(test))]
        {
            let mut cfg = self.config.clone();
            cfg.sidebar_width = self.sidebar_width;
            cfg.inspector_width = self.inspector_width;
            cfg.sidebar_visible = !self.sidebar_collapsed;
            cfg.inspector_visible = self.inspector_open;
            cfg.layout_mode = match self.layout_mode {
                LayoutMode::Browser => "browser".to_string(),
                LayoutMode::Split => "split".to_string(),
                LayoutMode::Viewer => "viewer".to_string(),
            };
            cfg.grid_columns = self.grid_columns.clamp(2, 8);
            cfg.card_fit_mode = self.card_fit_mode;
            cfg.inspector_tab = Some(self.inspector_tab.config_key().to_string());
            cfg.sort_field = Some(self.sort.field_key().to_string());
            cfg.sort_direction = Some(self.sort.direction_key().to_string());
            // Keep the on-disk session intact until the startup restore has been
            // applied; otherwise a window-bounds save during the initial scan could
            // overwrite it before it is used.
            if self.pending_restore.is_none() {
                cfg.last_view = Some(LastView::from_selection(&self.selection).token());
                cfg.last_search_query = if self.search_query.is_empty() {
                    None
                } else {
                    Some(self.search_query.clone())
                };
                cfg.last_active_year = self.active_year;
                cfg.last_selected_media_id = self.selected_item.as_ref().map(|item| item.id.0);
            }
            // The grid scroll row is deliberately not persisted: GPUI 0.2.2 only
            // exposes UniformListScrollHandle::logical_scroll_top_index under the
            // test-support feature, so production cannot read the top row without
            // inventing one. The restored selection's row is revealed instead.
            if let Some((x, y, width, height)) = self.window_bounds {
                cfg.window_x = Some(x);
                cfg.window_y = Some(y);
                cfg.window_width = Some(width);
                cfg.window_height = Some(height);
            }
            if let Err(err) = cfg.save() {
                self.error_message = Some(format!("Could not save settings: {err}"));
            }
        }
    }

    /// Schedules a debounced write of the restored-session fields.
    ///
    /// Selection, view, search, and year changes are high-frequency (arrow keys,
    /// typing), so saving synchronously on every change would hammer the config
    /// file with half-typed queries. The timer coalesces them and the generation
    /// guard lets only the newest scheduled save write.
    ///
    /// While [`LuminaApp::pending_restore`] is set the on-disk session belongs to
    /// the startup restore and must not be overwritten; scheduling resumes once
    /// the restore has been applied.
    pub(crate) fn schedule_session_save(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        {
            // Tests must never spawn timers or write the user's config.
            let _ = (self, cx);
        }
        #[cfg(not(test))]
        {
            let Some(generation) = self
                .session_save
                .arm_if_allowed(self.pending_restore.is_none())
            else {
                return;
            };
            cx.spawn(async move |this, async_cx| {
                async_cx
                    .background_executor()
                    .timer(SESSION_SAVE_DEBOUNCE)
                    .await;
                // A newer change supersedes this timer, so a stale task can never
                // persist an older view or selection.
                let _ = this.update(async_cx, |this, _cx| {
                    if this.session_save.is_current(generation) {
                        this.save_config();
                    }
                });
            })
            .detach();
        }
    }

    pub fn copy_to_clipboard(&self, text: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
    }

    pub fn row_height(&self) -> f32 {
        match self.grid_columns {
            2 => 380.0,
            3 => 260.0,
            4 => 200.0,
            5 => 170.0,
            6 => 140.0,
            7 => 120.0,
            8 => 100.0,
            _ => 200.0,
        }
    }

    pub(crate) fn queue_thumbnail_request(&self, item_id: MediaId) {
        if let Some(ref state) = self.grid_state {
            let should_request = {
                let mut in_flight = state.in_flight.lock();
                in_flight.insert(item_id)
            };
            if should_request {
                let tx = self.ui_tx.clone();
                let in_flight_clone = state.in_flight.clone();

                state
                    .engine
                    .request_thumbnail(item_id, 320, move |frame_opt| {
                        in_flight_clone.lock().remove(&item_id);
                        if let Some(frame) = frame_opt {
                            let _ =
                                tx.send_blocking(UiMessage::ThumbnailLoaded { id: item_id, frame });
                        }
                    });
            }
        }
    }

    pub(crate) fn prefetch_rows(&self, range: std::ops::Range<usize>) {
        if range.start < range.end {
            let cols = self.grid_columns.clamp(2, 8);
            let start = range.start * cols;
            let end = range.end * cols;
            let items = self.get_active_slice(start..end);
            if let Some(ref state) = self.grid_state {
                for item in items {
                    let is_cached = state.texture_cache.lock().contains(&item.id);
                    if !is_cached {
                        self.queue_thumbnail_request(item.id);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod session_save_tests {
    use super::{SessionSaveState, SESSION_SAVE_DEBOUNCE};
    use std::time::Duration;

    #[core::prelude::v1::test]
    fn session_save_config_generation_guard_accepts_only_the_newest() {
        let mut state = SessionSaveState::default();
        let first = state.arm();
        let second = state.arm();
        assert!(
            !state.is_current(first),
            "a superseded timer must not write the session"
        );
        assert!(
            state.is_current(second),
            "the newest timer must write the session"
        );
    }

    #[core::prelude::v1::test]
    fn session_save_config_waits_for_the_pending_restore() {
        let mut state = SessionSaveState::default();
        assert_eq!(
            state.arm_if_allowed(false),
            None,
            "no timer may be armed while the startup restore owns the config"
        );
        assert_eq!(
            state.arm_if_allowed(true),
            Some(1),
            "scheduling resumes once the restore is applied"
        );
        assert!(state.is_current(1));
    }

    #[core::prelude::v1::test]
    fn session_save_config_generation_guard_handles_wraparound() {
        let mut state = SessionSaveState {
            generation: u64::MAX,
        };
        assert_eq!(state.arm(), 0, "generation ids wrap without panicking");
        assert!(state.is_current(0));
        assert!(!state.is_current(u64::MAX));
    }

    #[core::prelude::v1::test]
    fn session_save_config_debounce_delay_stays_in_the_agreed_range() {
        assert!(
            SESSION_SAVE_DEBOUNCE >= Duration::from_millis(300),
            "session saves should wait at least 300ms before writing"
        );
        assert!(
            SESSION_SAVE_DEBOUNCE <= Duration::from_millis(500),
            "session saves should settle within 500ms"
        );
    }
}
