use crate::config::ResumePositions;
use crate::theme;
use gpui::*;
use lru::LruCache;
use lumina_core::{LuminaEngine, MediaId};
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;

/// Left padding inside a grid row, in pixels (Library frame: 28).
///
/// The renderer, [`grid_metrics`] and the pointer-anchored zoom math all use
/// the left/right pair so the column under the pointer cannot drift from the
/// painted layout.
pub(crate) const GRID_ROW_PADDING_LEFT: f32 = 28.0;

/// Right padding inside a grid row, in pixels (Library frame: 20).
pub(crate) const GRID_ROW_PADDING_RIGHT: f32 = 20.0;

/// Gap between cards in a grid row, in pixels.
///
/// Shared with the renderer for the same reason as the row padding.
pub(crate) const GRID_CARD_GAP: f32 = 6.0;

/// Padding between the card box and its photo well, in pixels. Zero: the
/// well is the tile.
///
/// The renderer and [`grid_metrics`] share it so the square photo well cannot
/// drift from the painted card box.
pub(crate) const CARD_PADDING: f32 = 0.0;

/// Vertical gap between grid rows, in pixels (the frame's tile gap, 6, both
/// ways). The renderer puts it above the cards inside each uniform-list row,
/// so every row has the same height and the first row's selection ring has
/// room above it.
pub(crate) const CARD_ROW_GAP: f32 = 6.0;

/// Space between the month band's (or the results head's) text and the
/// first row's tiles (frame: 10).
pub(crate) const BAND_TO_ROW: f32 = 10.0;

/// Opacity of an unselected card while two or more cards are selected
/// (selection mode).
pub(crate) const UNSELECTED_CARD_OPACITY: f32 = 0.62;

/// Below this photo side the top rows (check + edited, flag + heart +
/// missing: 46 + 62 + 2 insets) would collide, so the heart, edited badge and
/// meta pill hide. ponytail: under ~86 px (8 columns, narrow window, inspector
/// open) check + flag + missing can still touch; shrink badges if that matters.
pub(crate) const COMPACT_TILE: f32 = 124.0;

#[cfg(test)]
thread_local! {
    /// Test hook: the opacity each drawn card was given, by item id. Colour
    /// and opacity are not readable from a headless frame.
    pub(crate) static CARD_OPACITY: std::cell::RefCell<HashMap<usize, f32>> =
        std::cell::RefCell::new(HashMap::new());
    /// Test hook: whether each drawn thumbnail was contain-fit (else cover),
    /// by item id; like opacity, it is not readable from a headless frame.
    pub(crate) static CARD_CONTAIN: std::cell::RefCell<HashMap<usize, bool>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Gap between an open album's tiles, both ways.
const ALBUM_TILE_GAP: f32 = 10.0;

/// Right padding of an open album's grid (frame: 24; the left stays 28).
const ALBUM_PADDING_RIGHT: f32 = 24.0;

/// From the grid's top edge to an open album's first row of tiles (frame: 22).
pub(crate) const ALBUM_PADDING_TOP: f32 = 22.0;

/// Corner radius of an open album's tiles (frame: 5).
pub(crate) const ALBUM_TILE_RADIUS: f32 = 5.0;

/// Columns an open album's grid starts at (frame: 5). Off the zoom
/// ladder; ⌘+ / ⌘− step from it to the ladder's 3 and 6.
pub(crate) const ALBUM_COLUMNS: usize = 5;

/// Width of the pinned year rail when the timeline has buckets.
pub(crate) const YEAR_RAIL_WIDTH: f32 = 83.5;

/// Width of the collapsed sidebar rail (it never fully disappears).
pub(crate) const SIDEBAR_COLLAPSED_WIDTH: f32 = 48.0;

/// Width of the sidebar/inspector drag handles.
pub(crate) const RESIZER_WIDTH: f32 = 4.0;

/// Window width used before the bounds observer reports a real size.
pub(crate) const DEFAULT_WINDOW_WIDTH: f32 = 1280.0;

/// Smallest photo well side, so a very narrow window cannot collapse rows.
const PHOTO_SIDE_MIN: f32 = 48.0;

/// Largest photo well side, so a very wide window cannot produce huge rows.
const PHOTO_SIDE_MAX: f32 = 640.0;

/// Smallest thumbnail decode, in device pixels on its long edge: the size
/// the grid used to ask for unconditionally, so a 1x display or a tiny tile
/// never decodes less than before.
pub(crate) const THUMBNAIL_SIZE_MIN: u32 = 320;

/// Largest thumbnail decode. Covers the widest tile (`PHOTO_SIDE_MAX`) at the
/// 2x Retina scale, so a tile is drawn from at least its own device pixels.
pub(crate) const THUMBNAIL_SIZE_MAX: u32 = 2 * PHOTO_SIDE_MAX as u32;

/// Decode sizes round up to this step so a live resize, which changes the
/// tile width every frame, does not mint a cache entry per pixel.
const THUMBNAIL_SIZE_STEP: u32 = 64;

/// The long edge, in device pixels, a thumbnail needs when its tile is
/// `side` points wide on a display scaled by `scale`.
///
/// The grid used to request a fixed 320 px whatever the tile or display, so a
/// Retina tile (2x) or a zoomed-in 2-column tile drew from a source smaller
/// than its own pixels and the GPU bilinear-stretched it. This is the tile's
/// own size in device pixels instead, rounded up to [`THUMBNAIL_SIZE_STEP`]
/// (so adjacent window sizes share a decode) and clamped to the thumbnail
/// range.
pub(crate) fn thumbnail_decode_size(side: f32, scale: f32) -> u32 {
    if !side.is_finite() || !scale.is_finite() {
        return THUMBNAIL_SIZE_MIN;
    }
    let device = (side.max(0.0) * scale.max(1.0)).ceil() as u32;
    device
        .div_ceil(THUMBNAIL_SIZE_STEP)
        .saturating_mul(THUMBNAIL_SIZE_STEP)
        .clamp(THUMBNAIL_SIZE_MIN, THUMBNAIL_SIZE_MAX)
}

/// Bytes of decoded grid textures kept in memory: the old fixed footprint
/// (800 tiles at 320² BGRA8), so a larger decode size trades cache depth for
/// detail instead of growing memory without bound. A heuristic, not a hard
/// cap: stamps from a previous, larger size can briefly put the total above
/// it until the LRU drains them.
const TEXTURE_CACHE_BUDGET_BYTES: usize = 800 * 320 * 320 * 4;

/// Texture-cache entries that fit [`TEXTURE_CACHE_BUDGET_BYTES`] at `size`²
/// BGRA8, within 48..=800. The floor is a viewport plus its prefetch at the
/// largest decode.
fn texture_cache_capacity(size: u32) -> NonZeroUsize {
    let bytes = (size as usize)
        .saturating_mul(size as usize)
        .saturating_mul(4)
        .max(1);
    NonZeroUsize::new((TEXTURE_CACHE_BUDGET_BYTES / bytes).clamp(48, 800))
        .expect("the clamp keeps the capacity non-zero")
}

/// A decoded grid thumbnail and the decode size it was requested at (long
/// edge, device px).
///
/// The *requested* size, not the frame's own, is what a tile compares against:
/// a small source file or a crop decodes below the request, and comparing the
/// frame would re-request it on every frame.
#[derive(Clone)]
pub struct CachedThumb {
    pub requested: u32,
    pub image: Arc<RenderImage>,
}

impl CachedThumb {
    /// Pairs a decoded image with the size its decode was requested at.
    pub fn new(requested: u32, image: Arc<RenderImage>) -> Self {
        Self { requested, image }
    }
}

/// Column gap between Videos tiles.
const VIDEO_COLUMN_GAP: f32 = 14.0;

/// Row gap between Videos tiles, caption to next thumb (frame: 22).
pub(crate) const VIDEO_ROW_GAP: f32 = 22.0;

/// Fixed height of a Videos tile's caption, so every `uniform_list` row is
/// the same: gap 9, title line 16 (Inter 12.5), gap 3, meta line 13
/// (JetBrains Mono 10). Each line truncates, so it never wraps taller.
pub(crate) const CAPTION_HEIGHT: f32 = 9.0 + 16.0 + 3.0 + 13.0;

/// Shape of a Grid tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileShape {
    /// A square well (every view but Videos).
    Square,
    /// A 16:9 thumb over a two-line caption (Videos).
    Captioned,
    /// A square, cover-filled tile on an open album's page:
    /// [`ALBUM_TILE_GAP`] both ways, [`ALBUM_PADDING_RIGHT`].
    Album,
}

/// Grid layout derived from the window width and the chrome beside the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridMetrics {
    /// Width of the photo well, in pixels (also its height when square).
    pub photo_side: f32,
    /// Height of the photo well: `photo_side`, or 9/16 of it when captioned.
    pub photo_height: f32,
    /// Gap between the cards of a row.
    pub column_gap: f32,
    /// Space above the card inside each row: the gap between rows.
    pub row_gap: f32,
    /// Right padding of a row (the left is always [`GRID_ROW_PADDING_LEFT`]).
    pub padding_right: f32,
    /// Full uniform-list row height: [`CARD_ROW_GAP`] above the card, the
    /// well, the caption when captioned, and the rest of the row gap below.
    pub row_height: f32,
}

/// The center pane's width: the window less the sidebar (or its collapsed
/// rail) and its resizer, the year rail when painted, and the inspector
/// (`Some` when painted) and its resizer.
pub(crate) fn center_pane_width(
    window_width: f32,
    sidebar_width: f32,
    sidebar_collapsed: bool,
    year_rail_visible: bool,
    inspector_width: Option<f32>,
) -> f32 {
    let sidebar = if sidebar_collapsed {
        SIDEBAR_COLLAPSED_WIDTH
    } else {
        sidebar_width.max(0.0) + RESIZER_WIDTH
    };
    let year_rail = if year_rail_visible {
        YEAR_RAIL_WIDTH
    } else {
        0.0
    };
    let inspector = inspector_width.map_or(0.0, |width| width.max(0.0) + RESIZER_WIDTH);
    (window_width - sidebar - year_rail - inspector).max(0.0)
}

/// Derives the square photo well and row height from the actual card width.
///
/// The center pane loses the sidebar (or its 48px collapsed rail) plus a 4px
/// resizer, the year rail when it is painted, and the inspector plus its
/// 4px resizer when it is painted. Each row then loses the left and right
/// row padding and `(columns - 1) * GRID_CARD_GAP` before the
/// remaining width is split between the columns. The photo well is the card's
/// content width, and the row is the well plus the gap above the card.
///
/// A captioned tile uses the Videos column gap, a 16:9 well (floored, like
/// the width, to whole pixels) and a fixed caption, and its row ends with
/// the rest of the 22px row gap. An album tile is square with the album
/// gap both ways and the album right padding.
pub(crate) fn grid_metrics(
    window_width: f32,
    sidebar_width: f32,
    sidebar_collapsed: bool,
    year_rail_visible: bool,
    inspector_width: Option<f32>,
    columns: usize,
    shape: TileShape,
) -> GridMetrics {
    let (column_gap, row_gap, padding_right) = match shape {
        TileShape::Square => (GRID_CARD_GAP, CARD_ROW_GAP, GRID_ROW_PADDING_RIGHT),
        TileShape::Captioned => (VIDEO_COLUMN_GAP, CARD_ROW_GAP, GRID_ROW_PADDING_RIGHT),
        TileShape::Album => (ALBUM_TILE_GAP, ALBUM_TILE_GAP, ALBUM_PADDING_RIGHT),
    };
    let columns = columns.clamp(2, 8) as f32;
    let grid_width = center_pane_width(
        window_width,
        sidebar_width,
        sidebar_collapsed,
        year_rail_visible,
        inspector_width,
    );
    let card_width =
        (grid_width - GRID_ROW_PADDING_LEFT - padding_right - column_gap * (columns - 1.0))
            .max(0.0)
            / columns;
    // Whole pixels: GPUI snaps the list's row height to the pixel grid, and
    // the scroll math (`grid_top_row`, zoom anchor) divides by this value, so
    // a fractional row (e.g. 234.125) drifts a row behind after deep scrolls.
    let photo_side = (card_width - 2.0 * CARD_PADDING)
        .floor()
        .clamp(PHOTO_SIDE_MIN, PHOTO_SIDE_MAX);
    let (photo_height, below) = match shape {
        TileShape::Square | TileShape::Album => (photo_side, 0.0),
        TileShape::Captioned => (
            (photo_side * 9.0 / 16.0).floor(),
            CAPTION_HEIGHT + VIDEO_ROW_GAP - CARD_ROW_GAP,
        ),
    };
    GridMetrics {
        row_height: row_gap + photo_height + 2.0 * CARD_PADDING + below,
        photo_side,
        photo_height,
        column_gap,
        row_gap,
        padding_right,
    }
}

/// Hover tooltip that keeps the full filename reachable now that the card no
/// longer prints it under the photo.
struct MediaCardTooltip {
    name: SharedString,
}

impl MediaCardTooltip {
    fn new(name: impl Into<SharedString>) -> Self {
        Self { name: name.into() }
    }
}

impl Render for MediaCardTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "media-card-name-tooltip".to_string())
            .px_2()
            .py_1()
            .bg(theme::bg_float())
            .border_1()
            .border_color(theme::line())
            .rounded(theme::RADIUS_MD)
            .text_size(theme::TEXT_SMALL)
            .text_color(theme::t1())
            .child(self.name.clone())
    }
}

/// Builds the card tooltip closure carrying the full filename.
pub(crate) fn media_card_tooltip(
    name: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let name = name.into();
    move |_window, cx| cx.new(|_cx| MediaCardTooltip::new(name.clone())).into()
}

/// Measured geometry of the scrollable media grid in window pixels.
///
/// The uniform list scroll handle exposes the viewport bounds, offset, and
/// maximum offset, but not a usable per-row bounds list (see
/// `GridZoomAnchor::at_pointer`). Callers therefore supply the row height that
/// the current density renders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridZoomGeometry {
    /// Left edge of the scrollable list in window space.
    pub left: f32,
    /// Top edge of the scrollable list in window space.
    pub top: f32,
    /// Visible width of the scrollable list.
    pub width: f32,
    /// Visible height of the scrollable list.
    pub height: f32,
    /// GPUI vertical scroll offset: zero at the top, negative as the content
    /// scrolls up.
    pub scroll_y: f32,
    /// Rendered height of one row at `columns`.
    pub row_height: f32,
    /// Columns rendered before the density change.
    pub columns: usize,
    /// Gap between the cards of a row ([`GridMetrics::column_gap`]).
    pub column_gap: f32,
    /// Flat item count in the active view.
    pub item_count: usize,
}

impl GridZoomGeometry {
    /// False before the first prepaint of this layout or with no items.
    fn is_usable(&self) -> bool {
        self.item_count > 0
            && self.row_height > 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.scroll_y.is_finite()
            && self.row_height.is_finite()
    }
}

/// The item under the pointer when a density change starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridZoomAnchor {
    /// Flat index of the item under the pointer.
    pub item: usize,
    /// Row that item occupied before the density change.
    pub row: usize,
    /// Pointer position inside the row, as a 0..=1 fraction of the row height.
    ///
    /// Zoom preserves this fraction rather than an absolute pixel distance so
    /// the pointer stays at the same relative place inside the row.
    pub fraction_within_row: f32,
}

impl GridZoomAnchor {
    /// Captures the item under the pointer for a later density change.
    ///
    /// Returns `None` when the grid has no items or the measured geometry is
    /// not usable yet (before the first prepaint of this layout).
    ///
    /// GPUI's `UniformList` never fills `ScrollHandle::child_bounds`
    /// (it calls `Interactivity::prepaint` directly), so `top_item()` and
    /// `bounds_for_item()` stay empty for this list. The visible row is
    /// derived from the public `offset()` instead of inventing a top item.
    pub fn at_pointer(pointer_x: f32, pointer_y: f32, geometry: GridZoomGeometry) -> Option<Self> {
        let columns = geometry.columns.clamp(2, 8);
        if !geometry.is_usable() {
            return None;
        }

        let content_y = pointer_y - geometry.top - geometry.scroll_y;
        if !content_y.is_finite() || content_y < 0.0 {
            return None;
        }

        let total_rows = geometry.item_count.div_ceil(columns);
        let row = ((content_y / geometry.row_height).floor() as usize).min(total_rows - 1);
        let row_top = row as f32 * geometry.row_height;
        let fraction_within_row = ((content_y - row_top) / geometry.row_height).clamp(0.0, 1.0);
        let column = column_at_pointer(
            pointer_x,
            geometry.left,
            geometry.width,
            columns,
            geometry.column_gap,
        );
        let item = (row * columns + column).min(geometry.item_count - 1);

        Some(Self {
            item,
            row,
            fraction_within_row,
        })
    }

    /// Anchors on `item` (the keyboard selection) rather than the pointer.
    ///
    /// Returns the anchor and the window y to hold it at: the centre of the
    /// item's row, clamped into the viewport, so a selection scrolled out of
    /// view is brought back in by the same density change.
    pub fn at_item(item: usize, geometry: GridZoomGeometry) -> Option<(Self, f32)> {
        if !geometry.is_usable() || item >= geometry.item_count {
            return None;
        }
        let row = item / geometry.columns.clamp(2, 8);
        let centre = geometry.top + geometry.scroll_y + (row as f32 + 0.5) * geometry.row_height;
        let half = (geometry.row_height * 0.5).min(geometry.height * 0.5);
        let y = centre.clamp(geometry.top + half, geometry.top + geometry.height - half);
        let anchor = Self {
            item,
            row,
            fraction_within_row: 0.5,
        };
        Some((anchor, y))
    }

    /// Content scroll offset (a positive distance from the content top) that
    /// keeps this anchor at the same window position after the density change.
    ///
    /// The result is clamped to the scrollable range of the new layout. Row
    /// boundaries move when the column count changes, so this preserves the
    /// anchor item's row and the pointer's fractional position inside it; it
    /// is not a pixel-exact transform.
    pub fn target_scroll_offset(
        self,
        pointer_y: f32,
        geometry: GridZoomGeometry,
        new_columns: usize,
        new_row_height: f32,
    ) -> f32 {
        let current_offset = (-geometry.scroll_y).max(0.0);
        if geometry.item_count == 0 || new_row_height <= 0.0 || geometry.height <= 0.0 {
            return current_offset;
        }

        let new_columns = new_columns.clamp(2, 8);
        let total_rows = geometry.item_count.div_ceil(new_columns);
        let new_row = (self.item / new_columns).min(total_rows.saturating_sub(1));
        let target_content_y =
            new_row as f32 * new_row_height + self.fraction_within_row * new_row_height;
        // Distance from the content top to the pointer, and the scroll offset
        // that puts the target row at that same distance.
        let pointer_distance = pointer_y - geometry.top;
        let target_scroll = target_content_y - pointer_distance;
        let max_scroll = (total_rows as f32 * new_row_height - geometry.height).max(0.0);
        target_scroll.clamp(0.0, max_scroll)
    }
}

/// Column under `pointer_x` for a row laid out with the grid's padding and gap.
/// ponytail: assumes the Library right padding (20); an album row has 24, so
/// its card width reads under a pixel wide. Pass the padding if zoom anchors
/// ever need to be pixel-exact there.
fn column_at_pointer(pointer_x: f32, left: f32, width: f32, columns: usize, gap: f32) -> usize {
    let inner_width = width - GRID_ROW_PADDING_LEFT - GRID_ROW_PADDING_RIGHT;
    let card_space = inner_width - columns.saturating_sub(1) as f32 * gap;
    if !card_space.is_finite() || card_space <= 0.0 {
        return 0;
    }
    let card_width = card_space / columns as f32;
    if card_width <= 0.0 {
        return 0;
    }
    let relative = pointer_x - left - GRID_ROW_PADDING_LEFT;
    ((relative / (card_width + gap)).floor() as isize).clamp(0, columns.saturating_sub(1) as isize)
        as usize
}

/// Grid-owned playback resume state.
///
/// The position map is shared with AppConfig so the existing config save path
/// persists it. The current media id stays with the grid instead of becoming a
/// flat app field, and lets a stop record the video that actually played even
/// when the selection has already moved to a different item.
#[derive(Clone, Default)]
pub struct VideoResumeState {
    pub positions: ResumePositions,
    current: Arc<Mutex<Option<MediaId>>>,
}

impl VideoResumeState {
    /// Marks the media whose player is now loading or loaded.
    pub fn begin(&self, id: MediaId) {
        *self.current.lock() = Some(id);
    }

    /// Media id of the active player, if any.
    pub fn current(&self) -> Option<MediaId> {
        *self.current.lock()
    }

    /// Clears the active player id after its final position is recorded.
    pub fn finish(&self) {
        *self.current.lock() = None;
    }

    /// Saved resume position for a media id.
    pub fn saved(&self, id: MediaId) -> Option<f64> {
        self.positions.get(id)
    }

    /// Records a position for the active player. Returns false when no player
    /// media is tracked, so callers can fall back without inventing an id.
    pub fn record_current(&self, seconds: f64, duration_secs: f64) -> bool {
        let Some(id) = self.current() else {
            return false;
        };
        self.positions.record(id, seconds, duration_secs);
        true
    }

    /// Drops positions whose media no longer exists in the library.
    pub fn prune_missing(&self, exists: impl FnMut(MediaId) -> bool) {
        self.positions.prune_missing(exists);
    }
}

#[derive(Clone)]
pub struct GridState {
    pub engine: Arc<LuminaEngine>,
    pub total_count: usize,
    pub is_scanning: bool,
    pub scan_progress: usize,
    pub texture_cache: Arc<Mutex<LruCache<MediaId, CachedThumb>>>,
    pub in_flight: Arc<Mutex<HashSet<MediaId>>>,
    /// When each thumbnail finished decoding, so only fresh ones fade in;
    /// a card scrolled back into view shows its cached image at once.
    pub fresh_thumbs: Arc<Mutex<HashMap<MediaId, Instant>>>,
    /// Ids whose thumbnail decode failed for good. Decode callbacks capture
    /// this library's `Arc`, so a late failure from a closed library cannot
    /// mark an id in the next one. Cleared when the file is re-upserted or
    /// removed, and wholesale by Rescan (the retry).
    pub thumb_failures: Arc<Mutex<HashSet<MediaId>>>,
    pub scroll_handle: UniformListScrollHandle,
    /// The Mosaic body's variable-height list: one item per layout row
    /// (headers and tile rows), never per media item.
    pub mosaic: ListState,
    /// Per-video resume positions for the open library.
    pub video_resume: VideoResumeState,
}

impl GridState {
    /// Notes a thumbnail that just arrived and forgets ones long settled.
    pub fn mark_fresh_thumb(&self, id: MediaId) {
        let mut fresh = self.fresh_thumbs.lock();
        fresh.retain(|_, at| at.elapsed() < crate::motion::THUMB_IN * 4);
        fresh.insert(id, Instant::now());
    }

    /// True while `id`'s thumbnail is still within its fade-in.
    pub fn thumb_is_fresh(&self, id: MediaId) -> bool {
        self.fresh_thumbs
            .lock()
            .get(&id)
            .is_some_and(|at| at.elapsed() < crate::motion::THUMB_IN * 2)
    }

    /// Marks `id`'s thumbnail as failed: it shows "No preview" and is not
    /// decoded again until cleared.
    pub fn mark_failed(&self, id: MediaId) {
        self.thumb_failures.lock().insert(id);
    }

    /// True when `id`'s thumbnail failed to decode.
    pub fn is_failed(&self, id: MediaId) -> bool {
        self.thumb_failures.lock().contains(&id)
    }

    /// Lets `id` be decoded again (the file changed or went away).
    pub fn clear_failed(&self, id: MediaId) {
        self.thumb_failures.lock().remove(&id);
    }

    /// Lets every failed thumbnail be decoded again (Rescan).
    pub fn clear_all_failed(&self) {
        self.thumb_failures.lock().clear();
    }

    /// Keeps the texture cache near [`TEXTURE_CACHE_BUDGET_BYTES`] when the
    /// thumbnail decode size changes (a display scale change or a zoom):
    /// bigger images mean fewer of them. Called from `Render`, which holds no
    /// other lock.
    ///
    /// Returns the images it evicted, which the caller must retire: dropping
    /// their `Arc` alone would leave the texture in the GPU atlas.
    pub fn resize_texture_cache(&self, thumbnail_size: u32) -> Vec<Arc<RenderImage>> {
        let capacity = texture_cache_capacity(thumbnail_size);
        let mut cache = self.texture_cache.lock();
        if cache.cap() == capacity {
            return Vec::new();
        }
        let mut evicted = Vec::new();
        while cache.len() > capacity.get() {
            match cache.pop_lru() {
                Some((_, thumb)) => evicted.push(thumb.image),
                None => break,
            }
        }
        // Only changes the cap here: the loop above already trimmed to it, and
        // `LruCache::resize` evicts silently, which would drop GPU textures
        // without anyone able to retire them.
        debug_assert!(cache.len() <= capacity.get());
        cache.resize(capacity);
        evicted
    }

    pub fn new(engine: Arc<LuminaEngine>) -> Self {
        let total_count = engine.total_count();
        let cache_capacity = texture_cache_capacity(THUMBNAIL_SIZE_MIN);

        Self {
            engine,
            total_count,
            is_scanning: true,
            scan_progress: 0,
            texture_cache: Arc::new(Mutex::new(LruCache::new(cache_capacity))),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            fresh_thumbs: Arc::new(Mutex::new(HashMap::new())),
            thumb_failures: Arc::new(Mutex::new(HashSet::new())),
            scroll_handle: UniformListScrollHandle::new(),
            // ponytail: a fixed overdraw of about one viewport; the window
            // height is not known here.
            mosaic: ListState::new(0, ListAlignment::Top, px(900.0)),
            video_resume: VideoResumeState::default(),
        }
    }

    /// Builds grid state that shares the config's resume map, so positions
    /// recorded during playback are part of the next config save.
    pub fn with_resume_positions(
        engine: Arc<LuminaEngine>,
        resume_positions: ResumePositions,
    ) -> Self {
        let mut state = Self::new(engine);
        state.video_resume.positions = resume_positions;
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD_CHROME: f32 = CARD_ROW_GAP + 2.0 * CARD_PADDING;

    #[core::prelude::v1::test]
    fn metrics_derive_expected_squares_at_default_window() {
        // 1280px window, expanded sidebar (230 + 4px), year rail (83.5px), no
        // inspector: 962.5px pane, less 28 + 20 row padding and 6px gaps,
        // floored to whole pixels (224.125 -> 224).
        let four = grid_metrics(1280.0, 230.0, false, true, None, 4, TileShape::Square);
        assert!(
            (four.photo_side - 224.0).abs() < 0.01,
            "got {}",
            four.photo_side
        );
        assert!(
            (four.row_height - 230.0).abs() < 0.01,
            "got {}",
            four.row_height
        );
        assert!(
            (four.row_height - four.photo_side - 6.0).abs() < 0.01,
            "the row pitch is the tile plus the frame's 6px gap"
        );

        let two = grid_metrics(1280.0, 230.0, false, true, None, 2, TileShape::Square);
        assert!(
            (two.photo_side - 454.0).abs() < 0.01,
            "got {}",
            two.photo_side
        );

        let eight = grid_metrics(1280.0, 230.0, false, true, None, 8, TileShape::Square);
        assert!(
            (eight.photo_side - 109.0).abs() < 0.01,
            "got {}",
            eight.photo_side
        );

        // Videos (Grid): 14px column gaps, a 16:9 thumb and a fixed caption.
        // (962.5 - 48 - 3 * 14) / 4 = 218.125 -> 218 wide, 218 * 9/16 =
        // 122.6 -> 122 high; the row adds the 6px top gap, the caption and
        // the 16px that makes the frame's 22px row gap.
        let wide = grid_metrics(1280.0, 230.0, false, true, None, 4, TileShape::Captioned);
        assert_eq!(
            (wide.photo_side, wide.photo_height, wide.column_gap),
            (218.0, 122.0, VIDEO_COLUMN_GAP),
            "a Videos thumb is 16:9 at the design's column gap"
        );
        assert_eq!(
            wide.row_height,
            CARD_ROW_GAP + 122.0 + CAPTION_HEIGHT + (VIDEO_ROW_GAP - CARD_ROW_GAP),
            "the row is the thumb, the fixed caption and the 22px row gap"
        );
        assert_eq!(
            (four.photo_height, four.column_gap),
            (four.photo_side, GRID_CARD_GAP),
            "the square shape keeps a square well and the 6px gap"
        );
    }

    #[core::prelude::v1::test]
    fn captioned_rows_stay_whole_pixels_and_uniform() {
        for width in [640.0, 960.0, 1280.0, 1600.0, 3008.0] {
            for columns in 2..=8 {
                let m = grid_metrics(
                    width,
                    230.0,
                    false,
                    true,
                    None,
                    columns,
                    TileShape::Captioned,
                );
                assert_eq!(m.photo_height, m.photo_height.floor(), "whole-pixel thumb");
                assert_eq!(
                    m.row_height,
                    m.row_height.floor(),
                    "whole-pixel rows for the scroll math"
                );
                assert!(
                    (m.photo_height - (m.photo_side * 9.0 / 16.0).floor()).abs() < 0.01,
                    "16:9 at {width}px, {columns} columns: {m:?}"
                );
            }
        }
    }

    #[core::prelude::v1::test]
    fn metrics_account_for_each_layout_chrome() {
        // Hidden year rail: the grid gains its 83.5px.
        let no_rail = grid_metrics(1280.0, 230.0, false, false, None, 4, TileShape::Square);
        assert!(
            (no_rail.photo_side - 245.0).abs() < 0.01,
            "got {}",
            no_rail.photo_side
        );

        // Collapsed sidebar is a 48px rail with no resizer.
        let collapsed = grid_metrics(1280.0, 230.0, true, true, None, 4, TileShape::Square);
        assert!(
            (collapsed.photo_side - 270.0).abs() < 0.01,
            "got {}",
            collapsed.photo_side
        );

        // Open inspector: its width plus the 4px resizer are gone from the grid.
        let with_inspector = grid_metrics(
            1280.0,
            230.0,
            false,
            true,
            Some(310.0),
            4,
            TileShape::Square,
        );
        assert!(
            (with_inspector.photo_side - 145.0).abs() < 0.01,
            "got {}",
            with_inspector.photo_side
        );
    }

    #[core::prelude::v1::test]
    fn metrics_clamp_extreme_windows_and_keep_the_row_identity() {
        let narrow = grid_metrics(700.0, 230.0, false, false, None, 8, TileShape::Square);
        assert_eq!(narrow.photo_side, PHOTO_SIDE_MIN);
        assert!((narrow.row_height - narrow.photo_side - CARD_CHROME).abs() < 0.01);

        let wide = grid_metrics(6000.0, 230.0, false, true, None, 2, TileShape::Square);
        assert_eq!(wide.photo_side, PHOTO_SIDE_MAX);
        assert!((wide.row_height - wide.photo_side - CARD_CHROME).abs() < 0.01);

        for width in [640.0, 960.0, 1280.0, 1920.0, 3008.0] {
            for columns in 2..=8 {
                for rail in [false, true] {
                    let metrics =
                        grid_metrics(width, 230.0, false, rail, None, columns, TileShape::Square);
                    assert!(metrics.photo_side >= PHOTO_SIDE_MIN);
                    assert!(metrics.photo_side <= PHOTO_SIDE_MAX);
                    assert!((metrics.row_height - metrics.photo_side - CARD_CHROME).abs() < 0.01);
                }
            }
        }
    }

    #[core::prelude::v1::test]
    fn column_at_pointer_follows_the_asymmetric_row_padding() {
        // 1000px row, 4 columns: 28 left + 20 right padding, three 6px gaps,
        // so cards are 233.5px wide and column 1 starts at 28 + 239.5 = 267.5.
        assert_eq!(
            column_at_pointer(100.0 + 265.0, 100.0, 1000.0, 4, GRID_CARD_GAP),
            0
        );
        assert_eq!(
            column_at_pointer(100.0 + 268.0, 100.0, 1000.0, 4, GRID_CARD_GAP),
            1,
            "column 1 starts 28px plus one card and one gap from the left edge"
        );
        // The last card ends 20px before the right edge.
        assert_eq!(
            column_at_pointer(100.0 + 979.0, 100.0, 1000.0, 4, GRID_CARD_GAP),
            3
        );
        // Captioned (14px gaps): cards are 227.5px, so column 1 starts at
        // 28 + 241.5 = 269.5; at the 6px gap 268 would already be column 1.
        assert_eq!(
            column_at_pointer(100.0 + 268.0, 100.0, 1000.0, 4, VIDEO_COLUMN_GAP),
            0,
            "the wider Videos gap moves the column boundaries"
        );
        assert_eq!(
            column_at_pointer(100.0 + 270.0, 100.0, 1000.0, 4, VIDEO_COLUMN_GAP),
            1
        );
        assert_eq!(
            column_at_pointer(100.0 + 979.0, 100.0, 1000.0, 4, VIDEO_COLUMN_GAP),
            3
        );
    }

    #[core::prelude::v1::test]
    fn card_tooltip_payload_keeps_the_complete_filename() {
        let tooltip = MediaCardTooltip::new("IMG_2036.HEIC");
        assert_eq!(tooltip.name, "IMG_2036.HEIC");
    }

    #[core::prelude::v1::test]
    fn thumbnails_decode_at_the_tile_device_size() {
        // 4 columns at 1280 px: a 239 px well, so a Retina tile is ~478
        // device px. The old fixed 320 left it 1.5x upscaled.
        let four = grid_metrics(1280.0, 252.0, false, false, None, 4, TileShape::Square);
        let side = four.photo_side;
        let retina = thumbnail_decode_size(side, 2.0);
        assert!(
            retina as f32 >= side * 2.0,
            "a 2x tile must decode at least its {side} px at 2x, got {retina}"
        );
        assert_eq!(
            retina % THUMBNAIL_SIZE_STEP,
            0,
            "decode sizes are quantized so a resize shares a cache entry"
        );

        // 1x never decodes below the old size (a large tile is still > 320).
        assert_eq!(thumbnail_decode_size(side, 1.0), THUMBNAIL_SIZE_MIN);

        // Zooming in (2 columns, 485 px) asks for more than 4 columns.
        let two = grid_metrics(1280.0, 252.0, false, false, None, 2, TileShape::Square);
        assert!(
            thumbnail_decode_size(two.photo_side, 2.0) > retina,
            "a wider tile must ask for a bigger decode"
        );

        // The widest tile at 2x is covered: no clamp gap at the top.
        assert_eq!(
            thumbnail_decode_size(PHOTO_SIDE_MAX, 2.0),
            2 * PHOTO_SIDE_MAX as u32,
            "the largest tile must decode at its own 2x size"
        );
        assert_eq!(thumbnail_decode_size(f32::NAN, 2.0), THUMBNAIL_SIZE_MIN);
        assert_eq!(
            thumbnail_decode_size(side, f32::INFINITY),
            THUMBNAIL_SIZE_MIN
        );
    }

    #[core::prelude::v1::test]
    fn texture_cache_depth_follows_the_decode_size() {
        // The old footprint: 800 tiles at 320 px.
        assert_eq!(texture_cache_capacity(THUMBNAIL_SIZE_MIN).get(), 800);
        // A 512 px decode keeps roughly the same bytes with fewer entries.
        assert_eq!(texture_cache_capacity(512).get(), 312);
        // The largest decode still keeps a viewport and its prefetch, and its
        // entries stay inside the byte budget.
        let max_entries = texture_cache_capacity(THUMBNAIL_SIZE_MAX).get();
        assert!(max_entries >= 48, "got {max_entries}");
        assert!(max_entries < 312);
        assert!(
            max_entries * (THUMBNAIL_SIZE_MAX as usize).pow(2) * 4 <= TEXTURE_CACHE_BUDGET_BYTES,
            "the largest size must not exceed the cache budget"
        );
    }
}
