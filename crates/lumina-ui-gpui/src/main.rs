pub mod bridge;
pub mod config;
pub mod views;

use gpui::*;
use lumina_core::{
    DecodedFrame, EngineEvent, LuminaEngine, MatchMode, MediaId, MediaItem, MediaType,
    RuleField, RuleOperator, RuleValue, SmartAlbumRule, SystemAlbum,
};
use std::cmp;
use std::path::PathBuf;
use std::sync::Arc;

use bridge::frame_to_gpui_image;
use config::AppConfig;
use views::grid::{GridState, COLS_COUNT, ROW_HEIGHT_PX};
use views::inspector::InspectorView;
use views::onboarding::OnboardingView;
use views::search_bar::SearchBarView;
use views::sidebar::{AlbumSelection, SidebarView};
use views::viewer::MediaViewer;

pub enum UiMessage {
    Engine(EngineEvent),
    ThumbnailLoaded { id: MediaId, frame: DecodedFrame },
    ViewerLoaded { id: MediaId, frame: DecodedFrame },
}

pub struct LuminaApp {
    pub config: AppConfig,
    pub grid_state: Option<GridState>,
    pub selected_item: Option<MediaItem>,
    pub viewer_image: Option<Arc<RenderImage>>,
    pub has_desktop_pics: bool,
    pub ui_tx: async_channel::Sender<UiMessage>,

    // Phase 5 Presentation Shell State
    pub sidebar_collapsed: bool,
    pub inspector_open: bool,
    pub selection: AlbumSelection,
    pub search_query: String,
    pub active_year: Option<i32>,
    pub is_fullscreen_viewer: bool,
}

impl LuminaApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let config = AppConfig::load();
        let desktop_pics = PathBuf::from("/Users/raymondottun/Desktop/pics");
        let has_desktop_pics = desktop_pics.exists() && desktop_pics.is_dir();

        let (ui_tx, ui_rx) = async_channel::unbounded::<UiMessage>();

        // Async task on GPUI event loop to receive messages and trigger 120 FPS UI updates
        cx.spawn(async move |weak_self, async_cx| {
            while let Ok(msg) = ui_rx.recv().await {
                let _ = weak_self.update(async_cx, |this, cx| {
                    this.handle_ui_message(msg, cx);
                });
            }
        })
        .detach();

        let mut app = Self {
            config: config.clone(),
            grid_state: None,
            selected_item: None,
            viewer_image: None,
            has_desktop_pics,
            ui_tx,
            sidebar_collapsed: false,
            inspector_open: false,
            selection: AlbumSelection::AllLibrary,
            search_query: String::new(),
            active_year: None,
            is_fullscreen_viewer: false,
        };

        // If previously selected library exists, open it immediately
        if let Some(ref path) = config.last_library_path {
            if path.exists() && path.is_dir() {
                app.open_library(path.clone(), cx);
            }
        }

        app
    }

    pub fn handle_ui_message(&mut self, msg: UiMessage, cx: &mut Context<Self>) {
        match msg {
            UiMessage::Engine(evt) => {
                if let Some(ref mut state) = self.grid_state {
                    match evt {
                        EngineEvent::BatchDiscovered { .. } => {
                            state.total_count = state.engine.total_count();
                            cx.notify();
                        }
                        EngineEvent::ScanProgress {
                            items_found,
                            is_complete,
                        } => {
                            state.scan_progress = items_found;
                            state.total_count = items_found;
                            if is_complete {
                                state.is_scanning = false;
                            }
                            cx.notify();
                        }
                        EngineEvent::ScanComplete { total_items } => {
                            state.total_count = total_items;
                            state.is_scanning = false;
                            cx.notify();
                        }
                        EngineEvent::ItemAdded(_) | EngineEvent::ItemRemoved(_) => {
                            state.total_count = state.engine.total_count();
                            cx.notify();
                        }
                    }
                }
            }
            UiMessage::ThumbnailLoaded { id, frame } => {
                if let Some(ref state) = self.grid_state {
                    state.in_flight.lock().remove(&id);
                    if let Ok(gpui_img) = frame_to_gpui_image(frame) {
                        state.texture_cache.lock().put(id, gpui_img);
                        cx.notify();
                    }
                }
            }
            UiMessage::ViewerLoaded { id, frame } => {
                if let Some(ref current) = self.selected_item {
                    if current.id == id {
                        if let Ok(gpui_img) = frame_to_gpui_image(frame) {
                            self.viewer_image = Some(gpui_img);
                            cx.notify();
                        }
                    }
                }
            }
        }
    }

    pub fn open_library(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.config.last_library_path = Some(path.clone());
        let _ = self.config.save();

        let (engine, rx) = match LuminaEngine::open(path.clone()) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Failed to open library at {:?}: {:?}", path, e);
                return;
            }
        };

        let grid_state = GridState::new(engine, path);
        self.grid_state = Some(grid_state);
        self.selected_item = None;
        self.viewer_image = None;
        self.selection = AlbumSelection::AllLibrary;
        self.search_query.clear();
        self.active_year = None;
        self.is_fullscreen_viewer = false;

        let tx = self.ui_tx.clone();
        std::thread::Builder::new()
            .name("lumina-event-receiver".to_string())
            .spawn(move || {
                for event in rx {
                    let _ = tx.send_blocking(UiMessage::Engine(event));
                }
            })
            .ok();

        cx.notify();
    }

    pub fn rescan(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut state) = self.grid_state {
            state.is_scanning = true;
            state.engine.rescan();
            cx.notify();
        }
    }

    pub fn pick_folder_dialog(&mut self, cx: &mut Context<Self>) {
        let current_dir = self
            .grid_state
            .as_ref()
            .map(|s| s.library_path.clone())
            .or_else(|| self.config.last_library_path.clone());

        let mut dialog = rfd::FileDialog::new().set_title("Select Photos & Videos Folder");
        if let Some(ref dir) = current_dir {
            if let Some(parent) = dir.parent() {
                dialog = dialog.set_directory(parent);
            }
        }

        if let Some(folder) = dialog.pick_folder() {
            self.open_library(folder, cx);
        }
    }

    pub fn get_active_count(&self) -> usize {
        let state = match self.grid_state.as_ref() {
            Some(s) => s,
            None => return 0,
        };

        if !self.search_query.is_empty() {
            return state.engine.search_query_count(&self.search_query);
        }

        if let Some(year) = self.active_year {
            return state.engine.search_query_count(&format!("year:{}", year));
        }

        match self.selection {
            AlbumSelection::AllLibrary => state.engine.total_count(),
            AlbumSelection::System(album) => state.engine.query_system_album_count(album),
            AlbumSelection::User(id) => {
                if let Some(album) = state.engine.list_smart_albums().into_iter().find(|a| a.id == id) {
                    state.engine.query_smart_album_count(&album)
                } else {
                    0
                }
            }
            AlbumSelection::Trash => state.engine.query_system_album_count(SystemAlbum::RecentlyDeleted),
        }
    }

    pub fn get_active_slice(&self, range: std::ops::Range<usize>) -> Vec<MediaItem> {
        let state = match self.grid_state.as_ref() {
            Some(s) => s,
            None => return Vec::new(),
        };

        let offset = range.start;
        let limit = range.end.saturating_sub(range.start);
        if limit == 0 {
            return Vec::new();
        }

        if !self.search_query.is_empty() {
            return state.engine.search_query(&self.search_query, limit, offset);
        }

        if let Some(year) = self.active_year {
            return state.engine.search_query(&format!("year:{}", year), limit, offset);
        }

        match self.selection {
            AlbumSelection::AllLibrary => state.engine.get_item_slice(range),
            AlbumSelection::System(album) => state.engine.query_system_album(album, limit, offset),
            AlbumSelection::User(id) => {
                if let Some(album) = state.engine.list_smart_albums().into_iter().find(|a| a.id == id) {
                    state.engine.query_smart_album(&album, limit, offset)
                } else {
                    Vec::new()
                }
            }
            AlbumSelection::Trash => {
                state.engine.query_system_album(SystemAlbum::RecentlyDeleted, limit, offset)
            }
        }
    }

    pub fn select_album(&mut self, selection: AlbumSelection, cx: &mut Context<Self>) {
        self.selection = selection;
        self.search_query.clear();
        self.active_year = None;
        cx.notify();
    }

    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = !self.inspector_open;
        cx.notify();
    }

    pub fn append_search_pill(&mut self, pill: &str, cx: &mut Context<Self>) {
        if self.search_query.is_empty() {
            self.search_query = pill.to_string();
        } else if !self.search_query.contains(pill) {
            self.search_query = format!("{} {}", self.search_query, pill);
        }
        self.active_year = None;
        cx.notify();
    }

    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search_query.clear();
        self.active_year = None;
        cx.notify();
    }

    pub fn set_active_year(&mut self, year: Option<i32>, cx: &mut Context<Self>) {
        self.active_year = year;
        self.search_query.clear();
        cx.notify();
    }

    pub fn toggle_favorite_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut item) = self.selected_item {
            let new_fav = !item.is_favorite;
            item.is_favorite = new_fav;
            if let Some(ref state) = self.grid_state {
                let _ = state.engine.toggle_favorite(item.id, new_fav);
            }
            cx.notify();
        }
    }

    pub fn create_new_smart_album(&mut self, cx: &mut Context<Self>) {
        if let Some(ref state) = self.grid_state {
            let rules = vec![
                SmartAlbumRule {
                    field: RuleField::IsFavorite,
                    operator: RuleOperator::Equals,
                    value: RuleValue::Bool(true),
                },
                SmartAlbumRule {
                    field: RuleField::CameraMake,
                    operator: RuleOperator::Contains,
                    value: RuleValue::Text("Sony".to_string()),
                },
            ];

            if let Ok(album) = state.engine.create_smart_album(
                "Sony Favorites",
                "heart",
                MatchMode::All,
                &rules,
            ) {
                self.selection = AlbumSelection::User(album.id);
                cx.notify();
            }
        }
    }

    pub fn select_item(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        self.selected_item = Some(item.clone());
        self.viewer_image = None;
        self.inspector_open = true;

        if let Some(ref state) = self.grid_state {
            if let Some(cached) = state.texture_cache.lock().get(&item.id).cloned() {
                self.viewer_image = Some(cached);
            }

            let tx = self.ui_tx.clone();
            state.engine.request_thumbnail(item.id, 1024, move |frame_opt| {
                if let Some(frame) = frame_opt {
                    let _ = tx.send_blocking(UiMessage::ViewerLoaded {
                        id: item.id,
                        frame,
                    });
                }
            });
        }

        cx.notify();
    }

    pub fn open_viewer(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        self.select_item(item, cx);
        self.is_fullscreen_viewer = true;
        cx.notify();
    }

    pub fn close_viewer(&mut self, cx: &mut Context<Self>) {
        self.is_fullscreen_viewer = false;
        cx.notify();
    }

    fn queue_thumbnail_request(&self, item_id: MediaId) {
        if let Some(ref state) = self.grid_state {
            let mut in_flight = state.in_flight.lock();
            if in_flight.insert(item_id) {
                let tx = self.ui_tx.clone();
                let in_flight_clone = state.in_flight.clone();

                state.engine.request_thumbnail(item_id, 320, move |frame_opt| {
                    in_flight_clone.lock().remove(&item_id);
                    if let Some(frame) = frame_opt {
                        let _ = tx.send_blocking(UiMessage::ThumbnailLoaded {
                            id: item_id,
                            frame,
                        });
                    }
                });
            }
        }
    }

    fn prefetch_rows(&self, range: std::ops::Range<usize>) {
        if range.start < range.end {
            let start = range.start * COLS_COUNT;
            let end = range.end * COLS_COUNT;
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

impl Render for LuminaApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.grid_state.is_none() {
            return div()
                .size_full()
                .child(OnboardingView::render(
                    cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.pick_folder_dialog(cx);
                    }),
                    cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        let desktop_pics = PathBuf::from("/Users/raymondottun/Desktop/pics");
                        this.open_library(desktop_pics, cx);
                    }),
                    self.has_desktop_pics,
                ))
                .into_any_element();
        }

        let state = self.grid_state.as_ref().unwrap();
        let total_count = self.get_active_count();
        let total_rows = total_count.div_ceil(COLS_COUNT);
        let is_scanning = state.is_scanning;
        let library_name = state
            .library_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Media Folder")
            .to_string();

        let selected_item = self.selected_item.clone();
        let viewer_image = self.viewer_image.clone();
        let inspector_open = self.inspector_open && selected_item.is_some();
        let is_fullscreen_viewer = self.is_fullscreen_viewer;

        div()
            .size_full()
            .bg(rgb(0x09090b))
            .flex()
            .flex_col()
            .relative()
            // Top Primary Header Bar
            .child(
                div()
                    .w_full()
                    .h(px(52.0))
                    .px_4()
                    .bg(rgb(0x121215))
                    .border_b_1()
                    .border_color(rgb(0x27272a))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .text_color(rgb(0xffffff))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            // Sidebar Toggle Button
                            .child(
                                div()
                                    .id("sidebar-toggle-btn")
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(0x18181b))
                                    .hover(|s| s.bg(rgb(0x27272a)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(rgb(0xa1a1aa))
                                    .child(if self.sidebar_collapsed { "⊞ Sidebar" } else { "⊟ Sidebar" })
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.toggle_sidebar(cx);
                                    })),
                            )
                            // Brand
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(24.0))
                                            .h(px(24.0))
                                            .rounded_md()
                                            .bg(rgb(0x3b82f6))
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .child(div().text_xs().font_weight(FontWeight::BOLD).child("✦")),
                                    )
                                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("Lumina")),
                            )
                            .child(
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .bg(rgb(0x18181b))
                                    .rounded_full()
                                    .text_xs()
                                    .text_color(rgb(0xa1a1aa))
                                    .child(format!("📁 {}", library_name)),
                            )
                            // Scan / Indexing Badge
                            .child(if is_scanning {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .px_2p5()
                                    .py_0p5()
                                    .bg(rgb(0x1e293b))
                                    .rounded_full()
                                    .text_xs()
                                    .text_color(rgb(0x38bdf8))
                                    .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(rgb(0x38bdf8)))
                                    .child(format!("Scanning... {} items", total_count))
                                    .into_any_element()
                            } else {
                                div()
                                    .px_2p5()
                                    .py_0p5()
                                    .bg(rgb(0x18181b))
                                    .rounded_full()
                                    .text_xs()
                                    .text_color(rgb(0x10b981))
                                    .child(format!("✓ {} items", total_count))
                                    .into_any_element()
                            }),
                    )
                    // Action Buttons on Top Right
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            // Inspector Toggle Button
                            .child(
                                div()
                                    .id("inspector-toggle-btn")
                                    .px_3()
                                    .py_1()
                                    .bg(if self.inspector_open { rgb(0x2563eb) } else { rgb(0x27272a) })
                                    .hover(|s| s.bg(rgb(0x3f3f46)))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("ℹ Info")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.toggle_inspector(cx);
                                    })),
                            )
                            // Rescan Button
                            .child(
                                div()
                                    .id("rescan-btn")
                                    .px_3()
                                    .py_1()
                                    .bg(rgb(0x27272a))
                                    .hover(|s| s.bg(rgb(0x3f3f46)))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("↻ Rescan")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.rescan(cx);
                                    })),
                            )
                            // Change Folder Button
                            .child(
                                div()
                                    .id("change-folder-btn")
                                    .px_3()
                                    .py_1()
                                    .bg(rgb(0x3b82f6))
                                    .hover(|s| s.bg(rgb(0x2563eb)))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Folder...")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.pick_folder_dialog(cx);
                                    })),
                            ),
                    ),
            )
            // Unified Top Search Bar & Scrubber
            .child(SearchBarView::render(self, cx))
            // Workspace Horizontal Split: Sidebar + Virtualized Grid + Metadata Inspector
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    // 1. Left Navigation Sidebar
                    .child(SidebarView::render(self, cx))
                    // 2. Center 120 FPS Virtualized Grid Canvas
                    .child(
                        div()
                            .flex_1()
                            .size_full()
                            .px_4()
                            .pt_3()
                            .child(
                                uniform_list(
                                    "media-grid",
                                    total_rows,
                                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                                        let mut rows = Vec::new();
                                        if let Some(ref state) = this.grid_state {
                                            // Prefetch 2 rows ahead
                                            let prefetch_start = range.end;
                                            let prefetch_end = cmp::min(prefetch_start + 2, total_rows);
                                            this.prefetch_rows(prefetch_start..prefetch_end);

                                            for row_ix in range {
                                                let start = row_ix * COLS_COUNT;
                                                let end = cmp::min(start + COLS_COUNT, total_count);
                                                let items = this.get_active_slice(start..end);

                                                let mut row_cards = Vec::with_capacity(COLS_COUNT);
                                                for item in &items {
                                                    let texture = state.texture_cache.lock().get(&item.id).cloned();
                                                    if texture.is_none() {
                                                        this.queue_thumbnail_request(item.id);
                                                    }

                                                    let item_id_num = item.id.0 as usize;
                                                    let is_video = item.media_type == MediaType::Video;
                                                    let is_raw = item.media_type == MediaType::Raw;
                                                    let is_fav = item.is_favorite;
                                                    let is_selected = this
                                                        .selected_item
                                                        .as_ref()
                                                        .map(|s| s.id == item.id)
                                                        .unwrap_or(false);

                                                    let item_for_click = item.clone();
                                                    let item_for_viewer = item.clone();

                                                    row_cards.push(
                                                        div()
                                                            .id(item_id_num)
                                                            .flex_1()
                                                            .h(px(ROW_HEIGHT_PX - 8.0))
                                                            .bg(rgb(0x18181b))
                                                            .rounded_lg()
                                                            .overflow_hidden()
                                                            .relative()
                                                            .cursor_pointer()
                                                            .border_2()
                                                            .border_color(if is_selected { rgb(0x3b82f6) } else { rgb(0x18181b) })
                                                            .hover(|s| s.border_color(rgb(0x60a5fa)))
                                                            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                                                this.select_item(item_for_click.clone(), cx);
                                                            }))
                                                            .child(if let Some(render_img) = texture {
                                                                div()
                                                                    .size_full()
                                                                    .child(img(render_img).size_full().object_fit(ObjectFit::Cover))
                                                                    .into_any_element()
                                                            } else {
                                                                div()
                                                                    .size_full()
                                                                    .bg(rgb(0x27272a))
                                                                    .flex()
                                                                    .justify_center()
                                                                    .items_center()
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(rgb(0x71717a))
                                                                            .child("..."),
                                                                    )
                                                                    .into_any_element()
                                                            })
                                                            // Badges Overlay
                                                            // Favorite Star Badge (Top Right)
                                                            .child(if is_fav {
                                                                div()
                                                                    .absolute()
                                                                    .top_2()
                                                                    .right_2()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .bg(rgba(0x000000cc))
                                                                    .rounded_sm()
                                                                    .text_xs()
                                                                    .text_color(rgb(0xfbbf24))
                                                                    .child("★")
                                                                    .into_any_element()
                                                            } else {
                                                                Empty.into_any_element()
                                                            })
                                                            // Media Type Badge (Bottom Left)
                                                            .child(if is_video {
                                                                div()
                                                                    .absolute()
                                                                    .bottom_2()
                                                                    .left_2()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .bg(rgba(0x000000cc))
                                                                    .rounded_sm()
                                                                    .text_xs()
                                                                    .text_color(rgb(0xffffff))
                                                                    .child("▶ Video")
                                                                    .into_any_element()
                                                            } else if is_raw {
                                                                div()
                                                                    .absolute()
                                                                    .bottom_2()
                                                                    .left_2()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .bg(rgba(0x000000cc))
                                                                    .rounded_sm()
                                                                    .text_xs()
                                                                    .text_color(rgb(0xa78bfa))
                                                                    .child("RAW")
                                                                    .into_any_element()
                                                            } else {
                                                                Empty.into_any_element()
                                                            })
                                                            // Fullscreen Expand Quick Button (Bottom Right)
                                                            .child(
                                                                div()
                                                                    .id(ElementId::NamedInteger("expand-card".into(), item.id.0))
                                                                    .absolute()
                                                                    .bottom_2()
                                                                    .right_2()
                                                                    .px_1p5()
                                                                    .py_0p5()
                                                                    .bg(rgba(0x000000cc))
                                                                    .hover(|s| s.bg(rgb(0x3b82f6)))
                                                                    .rounded_sm()
                                                                    .text_xs()
                                                                    .text_color(rgb(0xffffff))
                                                                    .child("⤢")
                                                                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                                                        this.open_viewer(item_for_viewer.clone(), cx);
                                                                    })),
                                                            )
                                                            .into_any_element(),
                                                    );
                                                }

                                                while row_cards.len() < COLS_COUNT {
                                                    row_cards.push(
                                                        div()
                                                            .flex_1()
                                                            .h(px(ROW_HEIGHT_PX - 8.0))
                                                            .into_any_element(),
                                                    );
                                                }

                                                rows.push(
                                                    div()
                                                        .id(row_ix)
                                                        .w_full()
                                                        .h(px(ROW_HEIGHT_PX))
                                                        .flex()
                                                        .flex_row()
                                                        .gap_2()
                                                        .children(row_cards),
                                                );
                                            }
                                        }
                                        rows
                                    }),
                                )
                                .h_full(),
                            ),
                    )
                    // 3. Right Slide-out Metadata & Location Inspector
                    .child(if inspector_open {
                        div()
                            .child(InspectorView::render(self, cx))
                            .into_any_element()
                    } else {
                        Empty.into_any_element()
                    }),
            )
            // Fullscreen Detail Viewer Modal
            .child(if is_fullscreen_viewer && selected_item.is_some() {
                div()
                    .size_full()
                    .child(MediaViewer::render_overlay(
                        selected_item.as_ref().unwrap(),
                        viewer_image,
                        cx.listener(|this, _event: &ClickEvent, _window, cx| {
                            this.close_viewer(cx);
                        }),
                    ))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            .into_any_element()
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(840.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Lumina - 120 FPS Media Manager".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(LuminaApp::new),
        )
        .unwrap();
        cx.activate(true);
    });
}
