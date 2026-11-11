use gpui::*;
use lumina_core::{MatchMode, MediaId, MediaItem, SmartAlbum, SortSpec, SystemAlbum};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::app::history::{NavigationHistory, NavigationSnapshot};
use crate::app::types::{BatchSelectionSummary, DeleteConfirmState, TagSortOrder, UiMessage};
use crate::app::windows::{open_floating_inspector_window, open_floating_pip_window};
#[cfg(not(test))]
use crate::config::LastView;
use crate::config::{AppConfig, RestoredSession};
use crate::player;
use crate::views::context_menu::ContextMenuState;
use crate::views::grid::{grid_metrics, GridMetrics, GridState, DEFAULT_WINDOW_WIDTH};
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

/// Sidebar search suggestion dropdown state.
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

/// The search header's query bar (Design 03).
#[derive(Debug, Default)]
pub struct QueryBarState {
    /// The typed text after the chips: the search input's buffer, rebuilt
    /// from the query each time it is read (`text_buffer`).
    pub typed: String,
    /// The chip ← selected for ⌫ to remove.
    pub chip: Option<usize>,
    /// The user has typed since focusing, so the last token may still be
    /// being typed (see `LuminaApp::search_typing`). Entering the field
    /// clears it: the query's tokens show as chips until the next edit.
    pub open_token: bool,
    /// Scrolls the token row to the caret or the selected chip.
    pub scroll: gpui::ScrollHandle,
    /// The pointer went down outside the bar and blurred search: the
    /// Search screen stays until it comes up, so the click lands on what
    /// was under it rather than on a layout that moved.
    pub held_for_click: bool,
}

/// Hardware-accelerated video playback state.
#[derive(Default)]
pub struct VideoPlaybackState {
    pub player: Option<player::NativeVideoPlayer>,
    /// Media the player session belongs to. Set when a load is attempted, so
    /// it survives a backend that cannot decode the file.
    pub item: Option<lumina_core::MediaItem>,
    /// True while the session is presented in the floating PiP window rather
    /// than the viewer. Frames then go to `pip_frame`, and the viewer and grid
    /// leave the player alone.
    pub in_pip: bool,
    /// Latest decoded frame for the PiP window; kept apart from
    /// `viewer_image` so browsing photos cannot overwrite the video.
    pub pip_frame: Option<Arc<RenderImage>>,
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
    /// Deadline for the bounded post-seek frame refresh.
    ///
    /// A zero-tolerance AVFoundation seek completes asynchronously, so the
    /// destination frame is not available in the same call. After a seek while
    /// paused (including a committed scrub drag) the ticker is started and
    /// polls `next_frame` until a frame arrives or this deadline passes.
    /// `None` whenever no refresh is pending.
    pub seek_refresh_until: Option<std::time::Instant>,
    /// Position a pending frame step started from. A step lands on a time
    /// the app cannot predict, so the paused refresh accepts the first frame
    /// whose position differs from this one.
    pub step_from: Option<f64>,
    pub ticker_active: Option<Arc<AtomicBool>>,
    pub ticker_handle: Option<std::thread::JoinHandle<()>>,
    pub frame_pending: Arc<AtomicBool>,
    /// Decoded scrub-preview frames for the open video, once built.
    pub(crate) preview_strip: Option<crate::app::scrub_preview::ScrubStrip>,
    /// The open video's one strip request (in flight, done or failed).
    pub(crate) strip_request: Option<crate::app::scrub_preview::StripRequest>,
    /// True when a scrub press paused playback, so release resumes it.
    pub resume_after_scrub: bool,
    /// In and out points of the open video, for this session only.
    pub loop_range: crate::app::loop_range::LoopRange,
    /// Audio tracks and chapters of the open video, for this session only.
    pub tracks: crate::app::video_tracks::VideoTracks,
}

/// Tag manager modal state.
pub struct TagManagerState {
    pub open: bool,
    pub search: String,
    /// Whether the filter field is the active keyboard target. Typing already
    /// filters while the modal is open; this drives the focus ring and the
    /// explicit click target.
    pub search_focused: bool,
    pub sort: TagSortOrder,
    pub selected: HashSet<String>,
    pub editing: Option<String>,
    pub editing_buffer: String,
    /// The focused tag's first items, for the detail grid. Loaded on focus,
    /// never per frame.
    pub tagged_items: Vec<lumina_core::MediaId>,
    /// Tag shown in the detail panel. Separate from `selected`, which holds
    /// the checkbox set for bulk actions.
    pub focused: Option<String>,
    /// Tracks the list so ↑/↓ can keep the focused row in view.
    pub list_scroll: gpui::ScrollHandle,
    /// Distinct items under the focused keyword (itself and its children),
    /// loaded on focus: the detail count and what a rename or move changes.
    pub focused_count: usize,
    /// The rename in `editing` is a Move to…: `editing_buffer` holds the new
    /// parent path and the keyword keeps its last segment.
    pub moving: bool,
    /// The alias field in the detail panel takes typing.
    pub alias_editing: bool,
    pub alias_buffer: String,
}

impl Default for TagManagerState {
    fn default() -> Self {
        Self {
            open: false,
            search: String::new(),
            search_focused: false,
            sort: TagSortOrder::Alphabetical,
            selected: HashSet::new(),
            editing: None,
            editing_buffer: String::new(),
            tagged_items: Vec::new(),
            focused: None,
            list_scroll: gpui::ScrollHandle::new(),
            focused_count: 0,
            moving: false,
            alias_editing: false,
            alias_buffer: String::new(),
        }
    }
}

/// Keyword vocabulary for autocomplete and the tag manager, loaded with the
/// tag counts and never queried while rendering.
#[derive(Debug, Default)]
pub struct KeywordState {
    /// `(alias, keyword)` rows, A–Z by alias.
    pub aliases: Vec<(String, String)>,
    /// The tag field's highlighted suggestion (↑/↓), `None` while typing.
    pub suggestion_focus: Option<usize>,
}

/// Smart album builder sheet state. See `smart_draft` for the row model.
pub struct AlbumDraftState {
    pub modal_open: bool,
    pub editing_album_id: Option<i64>,
    /// Empty means "Untitled Smart Album".
    pub album_title: String,
    pub match_mode: MatchMode,
    pub rows: Vec<crate::app::smart_draft::DraftRow>,
    pub focus: crate::app::smart_draft::DraftFocus,
    pub popup: Option<crate::app::smart_draft::DraftPopup>,
    /// Live match count; `None` while any row is invalid.
    pub match_count: Option<usize>,
    /// The first few matches, shown as thumbnails.
    pub preview: Vec<MediaItem>,
    /// Why valid-looking rules cannot run (a saved rule the engine rejects).
    pub problem: Option<String>,
    /// The smart album whose delete confirmation is open.
    pub confirm_delete: Option<i64>,
}

impl Default for AlbumDraftState {
    fn default() -> Self {
        Self {
            modal_open: false,
            editing_album_id: None,
            album_title: String::new(),
            match_mode: MatchMode::All,
            rows: Vec::new(),
            focus: crate::app::smart_draft::DraftFocus::Title,
            popup: None,
            match_count: None,
            preview: Vec::new(),
            problem: None,
            confirm_delete: None,
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

/// Sub-step progress of a grid density gesture. Trackpad ⌘-scroll and pinch
/// arrive as many small events; each accumulates here until it passes its
/// step threshold, so one gesture walks the ladder one level at a time.
#[derive(Debug, Default)]
pub struct GridZoomGesture {
    /// Pinch magnification not yet spent on a level.
    pub pinch_accum: f32,
    /// Pixel ⌘-scroll not yet spent on a level.
    pub wheel_accum_px: f32,
}

pub struct LuminaApp {
    pub config: AppConfig,
    pub grid_state: Option<GridState>,
    pub selected_item: Option<MediaItem>,
    pub selected_index: Option<usize>,
    pub selected_indices: BTreeSet<usize>,
    pub selection_anchor: Option<usize>,
    pub selected_item_tags: Vec<String>,
    /// Get Info extras for the shown item (see `app::info_details`).
    pub info_details: Option<crate::app::info_details::InfoDetails>,
    /// For a multi-selection, how many selected items carry each tag, so the
    /// batch inspector can mark tags that are only on some of them. Empty for
    /// a single item.
    pub selected_tag_counts: Vec<(String, usize)>,
    pub batch_summary: BatchSelectionSummary,
    /// Mixed-value-aware title/caption summary for the batch inspector.
    pub batch_metadata: BatchMetadataSummary,
    /// The background summary of a large selection.
    pub(crate) batch_load: crate::app::batch_summary::BatchLoad,
    /// Live pointer drag-selection over the grid (marquee or additive sweep).
    pub grid_drag: GridDragState,
    pub grid_zoom: GridZoomGesture,
    pub viewer_image: Option<Arc<RenderImage>>,
    /// The selected item whose full-size viewer decode failed, so the stage
    /// says so instead of waiting. Kept apart from the grid's failure set.
    pub viewer_failed: Option<MediaId>,
    /// Images no longer shown (replaced viewer frames, evicted thumbnails)
    /// awaiting GPU atlas eviction. GPUI never evicts a `RenderImage` on its
    /// own, so without this every video frame, opened photo and scrolled-past
    /// thumbnail would stay resident on the GPU.
    pub retired_images: Vec<Arc<RenderImage>>,
    pub has_desktop_pics: bool,
    /// "Building your library" while an empty catalog is first scanned.
    pub first_scan: Option<crate::app::first_scan::FirstScanState>,
    /// Welcome screen: recent-library focus, availability, missing notice.
    pub(crate) welcome: crate::app::recents::WelcomeState,
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
    pub query_bar: QueryBarState,
    pub(crate) top_matches: crate::app::search::TopMatchesState,
    pub active_year: Option<i32>,
    /// Bounded Back trail of browsing destinations (album, search, year,
    /// selection, viewer). Transient overlays are never recorded.
    pub(crate) navigation_history: NavigationHistory,
    /// Captured from AppConfig at startup and applied once after the initial
    /// library scan. Cleared by user-initiated open_library.
    pub(crate) pending_restore: Option<RestoredSession>,
    pub is_fullscreen_viewer: bool,
    pub is_immersive_fullscreen: bool,
    pub immersive_hud_visible: bool,
    pub(crate) hud_idle: crate::app::viewer_actions::HudIdle,
    pub(crate) mark_pop: crate::motion::MarkPop,
    pub card_fit_mode: bool,
    /// User-visible, dismissible error banner (library open/save failures).
    pub error_message: Option<String>,
    /// What the banner's Retry runs; None hides Retry.
    pub error_retry: Option<crate::app::library::ErrorRetry>,
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
    /// The viewer's star popover (1–5 rate, 0 clears), while open.
    pub rate_popover_open: bool,
    /// The Add filter menu, while open.
    pub filter_menu: Option<crate::views::filter_bar::FilterMenu>,
    pub preferences_open: bool,
    pub preferences_tab: crate::app::preferences::PreferencesTab,
    pub preferences_cache: Option<crate::app::preferences::CacheUsage>,
    /// Inline "Cleared" after Clear Cache; reset when Preferences reopens.
    pub preferences_cache_cleared: bool,
    /// When the last scan finished in this session (not persisted).
    pub last_scan_at: Option<std::time::Instant>,
    pub command_palette_open: bool,
    pub command_query: String,
    /// The platform text input behind the active field; see app/text_input.rs.
    pub(crate) field_input: Option<crate::app::text_input::FieldInput>,
    pub command_selected: usize,
    /// Keeps the keyboard-selected palette row scrolled into view.
    pub command_scroll: ScrollHandle,
    /// Lets the shortcut sheet scroll from the keyboard.
    pub shortcuts_scroll: ScrollHandle,

    // Phase 16 Hardware-Accelerated Video Player
    pub video: VideoPlaybackState,

    // Zero-SQLite UI Rendering Caches
    pub total_library_count: usize,
    pub system_album_counts: HashMap<SystemAlbum, usize>,
    pub user_album_counts: HashMap<i64, usize>,
    pub user_smart_albums: Vec<SmartAlbum>,
    /// Hand-picked albums in the sidebar.
    pub albums: crate::app::albums::AlbumsState,
    pub library_tag_counts: Vec<(String, usize)>,
    /// Cached count of active media with no tags, shown by the Untagged row.
    pub untagged_count: usize,
    /// Cached count of duplicate items across all duplicate groups.
    pub duplicate_items_count: usize,
    /// Cached duplicate groups.
    pub duplicate_groups: Vec<lumina_core::DuplicateGroup>,
    pub tag_input_buffer: String,
    pub tag_input_active: bool,
    pub keywords: KeywordState,
    pub editing_tag: Option<String>,
    pub editing_tag_buffer: String,
    pub context_menu_state: Option<ContextMenuState>,
    pub tag_manager: TagManagerState,
    /// Metadata template editor and pickers.
    pub templates: crate::app::templates::TemplateState,
    pub timeline_year_buckets: Vec<(i32, usize)>,
    pub month_header: MonthHeader,
    /// Timeline rail months and the grid range it tracks.
    pub rail: crate::app::render::rail::TimelineRail,
    pub item_ratings: HashMap<MediaId, u8>,
    pub item_color_labels: HashMap<MediaId, u8>,
    /// Picked and rejected items (unflagged ones are left out).
    pub item_flags: HashMap<MediaId, lumina_core::Flag>,
    pub item_captions: HashMap<MediaId, String>,
    /// Items whose file is gone from disk (badged in the grid).
    pub missing_items: HashSet<MediaId>,
    /// Import dialog and a running copy.
    pub(crate) import: crate::app::import::ImportState,
    /// Network video import dialog and running download.
    pub(crate) network_import: crate::app::network_import::NetworkImportState,
    /// Export and XMP import running in the background.
    pub(crate) jobs: crate::app::jobs::ForegroundJobs,
    pub(crate) downloads: crate::app::downloads::DownloadQueue,
    pub caption: CaptionState,
    /// Reversible catalog edits (ratings, tags, captions, favorites, trash).
    pub undo: crate::app::undo::UndoState,
    /// Edit mode and the edited-ids cache behind the grid badge.
    pub edits: crate::app::quick_edit::QuickEdits,
    /// Compare / survey mode; `None` when it is closed.
    pub(crate) compare: Option<crate::app::present::CompareState>,
    /// The Duplicates destination's tab and its Similar shots list.
    pub(crate) stacks: crate::app::stacks::StacksState,
    /// The offline map on Places (toggle, zoom, clusters, focus).
    pub(crate) map: crate::app::map::MapState,
    /// The running slideshow; `None` when there is none.
    pub(crate) slideshow: Option<crate::app::slideshow::SlideshowState>,
    pub(crate) event_receiver_thread: Option<std::thread::JoinHandle<()>>,
    /// Bumped per opened library; see `UiMessage::LibraryEvent`.
    pub(crate) library_generation: u64,
    /// The grid's count and rows, fetched off the UI thread.
    pub(crate) active_view: crate::app::active_view::ActiveView,
    /// The Mosaic body's layout, built off the UI thread.
    pub(crate) mosaic: crate::app::mosaic_layout::MosaicCache,
    /// The Mosaic's Years and Months cards: covers and keyboard focus.
    pub(crate) levels: crate::app::render::bucket_cards::GroupLevels,
    /// Latest selected-item tag load; older results are dropped.
    pub(crate) selected_tags_request: u64,
}

impl LuminaApp {
    /// The app as a window's root view, with keyboard focus on it so key
    /// events and the text input handler (app/text_input.rs) reach it.
    pub fn new_in_window(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app = Self::new(cx);
        window.focus(&app.focus_handle, cx);
        app
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        #[cfg(test)]
        let mut config = TEST_CONFIG.with(|c| c.take()).unwrap_or_default();
        #[cfg(not(test))]
        let mut config = AppConfig::load();
        // Keep stale or hand-edited config files from unbounded history growth.
        config
            .recent_searches
            .truncate(crate::app::search::MAX_RECENT_SEARCHES);
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
                        async_cx.update(|cx: &mut App| {
                            open_floating_inspector_window(weak_app, tx, cx);
                        });
                    }
                    UiMessage::OpenFloatingPip => {
                        let weak_app = weak_self.clone();
                        let tx = spawn_tx.clone();
                        async_cx.update(|cx: &mut App| {
                            open_floating_pip_window(weak_app, tx, cx);
                        });
                    }
                    other => {
                        let _ = weak_self.update(async_cx, |this, cx| {
                            this.handle_ui_message(other, cx);
                            // No window is mid-update here, so every window's
                            // atlas can drop them now. Waiting for a render
                            // would pile up frames while minimized.
                            for image in std::mem::take(&mut this.retired_images) {
                                cx.drop_image(image, None);
                            }
                        });
                    }
                }
            }
        })
        .detach();

        #[cfg_attr(test, allow(unused_mut))]
        let mut app = Self {
            config: config.clone(),
            grid_state: None,
            selected_item: None,
            selected_index: None,
            selected_indices: BTreeSet::new(),
            selection_anchor: None,
            selected_item_tags: Vec::new(),
            info_details: None,
            selected_tag_counts: Vec::new(),
            batch_summary: BatchSelectionSummary::default(),
            batch_metadata: BatchMetadataSummary::default(),
            batch_load: crate::app::batch_summary::BatchLoad::new(cx.background_executor().clone()),
            grid_drag: GridDragState::default(),
            grid_zoom: GridZoomGesture::default(),
            viewer_image: None,
            viewer_failed: None,
            retired_images: Vec::new(),
            has_desktop_pics,
            first_scan: None,
            welcome: Default::default(),
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
            query_bar: QueryBarState::default(),
            top_matches: Default::default(),
            active_year: None,
            navigation_history: NavigationHistory::new(NavigationSnapshot::default()),
            pending_restore: Some(config.restored_session()),
            is_fullscreen_viewer: false,
            is_immersive_fullscreen: false,
            immersive_hud_visible: true,
            hud_idle: Default::default(),
            mark_pop: Default::default(),
            card_fit_mode: config.card_fit_mode,
            error_message: None,
            error_retry: None,
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
            grid_columns: crate::views::search_bar::ladder_columns(config.grid_columns),
            sort: config.restored_sort(),
            sort_menu_open: false,
            rate_popover_open: false,
            filter_menu: None,
            preferences_open: false,
            preferences_tab: crate::app::preferences::PreferencesTab::General,
            preferences_cache: None,
            preferences_cache_cleared: false,
            last_scan_at: None,
            command_palette_open: false,
            command_query: String::new(),
            field_input: None,
            command_selected: 0,
            command_scroll: ScrollHandle::new(),
            shortcuts_scroll: ScrollHandle::new(),
            video: VideoPlaybackState {
                playback_speed: config.restored_video_speed(),
                is_muted: config.video_muted,
                is_looping: true,
                volume: 0.85,
                frame_pending: Arc::new(AtomicBool::new(false)),
                ..Default::default()
            },
            total_library_count: 0,
            system_album_counts: HashMap::new(),
            user_album_counts: HashMap::new(),
            user_smart_albums: Vec::new(),
            albums: Default::default(),
            library_tag_counts: Vec::new(),
            untagged_count: 0,
            duplicate_items_count: 0,
            duplicate_groups: Vec::new(),
            tag_input_buffer: String::new(),
            tag_input_active: false,
            keywords: KeywordState::default(),
            editing_tag: None,
            editing_tag_buffer: String::new(),
            context_menu_state: None,
            tag_manager: TagManagerState::default(),
            templates: Default::default(),
            timeline_year_buckets: Vec::new(),
            month_header: MonthHeader::default(),
            rail: Default::default(),
            item_ratings: HashMap::new(),
            item_color_labels: HashMap::new(),
            item_flags: HashMap::new(),
            item_captions: HashMap::new(),
            missing_items: HashSet::new(),
            import: Default::default(),
            network_import: Default::default(),
            jobs: Default::default(),
            downloads: Default::default(),
            caption: CaptionState::default(),
            undo: crate::app::undo::UndoState::default(),
            edits: Default::default(),
            compare: None,
            stacks: Default::default(),
            slideshow: None,
            event_receiver_thread: None,
            library_generation: 0,
            map: Default::default(),
            active_view: Default::default(),
            mosaic: Default::default(),
            levels: Default::default(),
            selected_tags_request: 0,
        };

        // If previously selected library exists, open it immediately.
        // Tests are isolated: they must set up their own temporary library.
        #[cfg(not(test))]
        app.apply_startup_library(crate::app::recents::startup_library(&config), cx);

        app
    }

    /// Writes the config, putting a failure in the error banner. Returns
    /// false on failure so callers can revert an optimistic change.
    pub fn save_config(&mut self) -> bool {
        match self.try_save_config() {
            Ok(()) => true,
            Err(err) => {
                self.error_message = Some(format!("Could not save settings: {err}"));
                false
            }
        }
    }

    /// Writes the config with the live window, layout and session state.
    /// A no-op under test (AGENTS §2), unless the test-only `SAVE_FAILS` asks it to fail.
    pub(crate) fn try_save_config(&mut self) -> anyhow::Result<()> {
        #[cfg(test)]
        {
            if SAVE_FAILS.with(std::cell::Cell::get) {
                anyhow::bail!("the disk is full");
            }
            Ok(())
        }
        #[cfg(not(test))]
        {
            let mut cfg = self.config.clone();
            cfg.sidebar_width = self.sidebar_width;
            cfg.inspector_width = self.inspector_width;
            cfg.sidebar_visible = !self.sidebar_collapsed;
            cfg.inspector_visible = self.inspector_open;
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
                cfg.last_scroll_anchor_id = self.grid_top_item_id().map(|id| id.0);
            }
            if let Some((x, y, width, height)) = self.window_bounds {
                cfg.window_x = Some(x);
                cfg.window_y = Some(y);
                cfg.window_width = Some(width);
                cfg.window_height = Some(height);
            }
            cfg.save()
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

    /// Grid geometry for the current window and layout chrome.
    ///
    /// The card's photo well is a square derived from the actual card width, so
    /// row height depends on the window width, the painted sidebar/rail/inspector,
    /// and the column count. `window_bounds` is the center-pane budget; the
    /// 1280px fallback matches the app's default window before the bounds
    /// observer reports a real size.
    pub(crate) fn grid_metrics(&self) -> GridMetrics {
        let window_width = self
            .window_bounds
            .map_or(DEFAULT_WINDOW_WIDTH, |(_, _, width, _)| width);
        let inspector_width =
            (self.inspector_open && !self.is_fullscreen_viewer).then_some(self.inspector_width);
        grid_metrics(
            window_width,
            self.sidebar_width,
            self.sidebar_collapsed,
            self.timeline_rail_shown(),
            inspector_width,
            self.grid_columns,
        )
    }

    pub fn row_height(&self) -> f32 {
        self.grid_metrics().row_height
    }

    pub(crate) fn queue_thumbnail_request(&self, item_id: MediaId) {
        if let Some(state) = self.grid_state.as_ref() {
            if let Some(callback) = self.begin_thumbnail_request(state, item_id) {
                state.engine.request_thumbnail(item_id, 320, callback);
            }
        }
    }

    pub(crate) fn queue_thumbnail_request_item(&self, item: &lumina_core::MediaItem) {
        if let Some(state) = self.grid_state.as_ref() {
            if let Some(callback) = self.begin_thumbnail_request(state, item.id) {
                #[cfg(test)]
                THUMB_REQUEST_ITEMS
                    .with(|items| items.borrow_mut().push((item.id, item.mtime_epoch_ms)));
                state
                    .engine
                    .request_thumbnail_item(item.clone(), 320, callback);
            }
        }
    }

    /// Claims a decode of `id` and returns its callback, or `None` when one
    /// is already in flight or the thumbnail failed before (Rescan retries).
    pub(crate) fn begin_thumbnail_request(
        &self,
        state: &GridState,
        id: MediaId,
    ) -> Option<impl FnOnce(Option<lumina_core::DecodedFrame>) + Send + 'static> {
        if state.is_failed(id) || !state.in_flight.lock().insert(id) {
            return None;
        }
        #[cfg(test)]
        THUMB_REQUESTS.with(|n| n.set(n.get() + 1));
        let tx = self.ui_tx.clone();
        let failures = state.thumb_failures.clone();
        let in_flight = state.in_flight.clone();
        Some(move |frame| on_thumbnail_result(&failures, &in_flight, &tx, id, frame))
    }

    /// The viewer's full-size decode of `id`. Its failure stays out of the
    /// grid's failure set: a full-size decode can time out on a file whose
    /// small thumbnail decodes fine. The grid's `in_flight` claim is not
    /// touched either (the sizes differ).
    pub(crate) fn request_viewer_frame(&self, state: &GridState, id: MediaId) {
        let tx = self.ui_tx.clone();
        state.engine.request_thumbnail(
            id,
            crate::app::viewer_actions::VIEWER_IMAGE_SIZE,
            move |frame| on_viewer_frame(&tx, id, frame),
        );
    }

    pub(crate) fn prefetch_rows(&self, range: std::ops::Range<usize>) {
        if range.start < range.end {
            let cols = self.grid_columns.clamp(2, 8);
            let start = range.start * cols;
            let end = range.end * cols;
            let items = self.displayed_rows(start..end);
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

/// A thumbnail decode's answer, on the decoder pool. `failures` and
/// `in_flight` belong to the library that asked, so a late answer from a
/// closed library cannot mark an id in the open one. A `None` (unreadable,
/// offline or undecodable file) is recorded so the card shows "No preview"
/// instead of being decoded again every frame.
pub(crate) fn on_thumbnail_result(
    failures: &parking_lot::Mutex<HashSet<MediaId>>,
    in_flight: &parking_lot::Mutex<HashSet<MediaId>>,
    tx: &async_channel::Sender<UiMessage>,
    id: MediaId,
    frame: Option<lumina_core::DecodedFrame>,
) {
    // Mark the failure before releasing the in-flight claim, so a render in
    // between cannot see the id as neither and queue a duplicate decode.
    if frame.is_none() {
        failures.lock().insert(id);
    }
    in_flight.lock().remove(&id);
    // A closed receiver means the app is gone; nothing is left to repaint.
    let _ = match frame {
        Some(frame) => tx.send_blocking(UiMessage::ThumbnailLoaded { id, frame }),
        None => tx.send_blocking(UiMessage::ThumbnailFailed),
    };
}

/// The viewer decode's answer, on the decoder pool.
pub(crate) fn on_viewer_frame(
    tx: &async_channel::Sender<UiMessage>,
    id: MediaId,
    frame: Option<lumina_core::DecodedFrame>,
) {
    // A closed receiver means the app is gone; nothing is left to repaint.
    let _ = match frame {
        Some(frame) => tx.send_blocking(UiMessage::ViewerLoaded { id, frame }),
        None => tx.send_blocking(UiMessage::ViewerFailed { id }),
    };
}

#[cfg(test)]
thread_local! {
    /// Thumbnail decodes actually sent to the engine from this test thread.
    pub(crate) static THUMB_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Id and file mtime of each decode sent with a row the UI already held;
    /// a stale row would decode (or hit the disk cache for) the old file.
    pub(crate) static THUMB_REQUEST_ITEMS: std::cell::RefCell<Vec<(MediaId, i64)>> =
        const { std::cell::RefCell::new(Vec::new()) };
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

/// Sticky month band over the date-sorted grid: the month of the top visible
/// row, plus library-wide month counts for its "N items".
#[derive(Default)]
pub struct MonthHeader {
    /// (year, month) of the first visible item, recorded while rows render.
    pub top: Option<(i32, u32)>,
    pub counts: HashMap<(i32, u32), usize>,
}

#[cfg(test)]
thread_local! {
    /// Makes [`LuminaApp::try_save_config`] fail on this test thread, so the
    /// revert-on-failure paths can be tested without a real config file.
    pub(crate) static SAVE_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A config for the next [`LuminaApp::new`] on this test thread, which
    /// otherwise starts from `AppConfig::default()`. Taken once, never saved.
    pub(crate) static TEST_CONFIG: std::cell::RefCell<Option<AppConfig>> =
        const { std::cell::RefCell::new(None) };
}
