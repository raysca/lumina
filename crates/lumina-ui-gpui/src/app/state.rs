use gpui::*;
use lumina_core::{MatchMode, MediaId, MediaItem, SmartAlbum, SmartAlbumRule, SystemAlbum};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::app::types::{
    BatchSelectionSummary, DeleteConfirmState, LayoutMode, TagSortOrder, UiMessage,
};
use crate::app::windows::{open_floating_inspector_window, open_floating_pip_window};
use crate::config::AppConfig;
use crate::player;
use crate::views::context_menu::ContextMenuState;
use crate::views::grid::GridState;
use crate::views::sidebar::AlbumSelection;
use crate::views::viewer::ViewerState;

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
    pub active_year: Option<i32>,
    pub is_fullscreen_viewer: bool,
    pub is_immersive_fullscreen: bool,
    pub immersive_hud_visible: bool,
    pub layout_mode: LayoutMode,
    pub card_fit_mode: bool,
    /// User-visible, dismissible error banner (library open/save failures).
    pub error_message: Option<String>,
    /// Most recent action feedback shown in the status bar.
    pub last_action: Option<String>,
    /// Keyboard shortcut cheat sheet visibility.
    pub shortcuts_overlay_open: bool,
    /// Last persisted window bounds: (x, y, width, height).
    pub window_bounds: Option<(f32, f32, f32, f32)>,
    pub(crate) window_bounds_subscription: Option<Subscription>,
    pub(crate) last_bounds_save: Option<std::time::Instant>,

    // Phase 8 Visual Smart Album Builder State
    pub album_draft: AlbumDraftState,

    // Phase 13 Deletion Management State
    pub delete_confirm_state: Option<DeleteConfirmState>,

    // Phase 15 Photo Zoom & Grid Density State
    pub viewer_state: ViewerState,
    pub viewer_shortcuts_open: bool,
    pub grid_columns: usize,

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
        let config = AppConfig::default();
        #[cfg(not(test))]
        let config = AppConfig::load();
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
            active_year: None,
            is_fullscreen_viewer: false,
            is_immersive_fullscreen: false,
            immersive_hud_visible: true,
            layout_mode: restored_layout,
            card_fit_mode: config.card_fit_mode,
            error_message: None,
            last_action: None,
            shortcuts_overlay_open: false,
            window_bounds: config.restored_window_bounds(),
            window_bounds_subscription: None,
            last_bounds_save: None,
            album_draft: AlbumDraftState::default(),
            delete_confirm_state: None,
            viewer_state: ViewerState::default(),
            viewer_shortcuts_open: false,
            grid_columns: config.grid_columns.clamp(2, 8),
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
                app.open_library(path.clone(), cx);
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
