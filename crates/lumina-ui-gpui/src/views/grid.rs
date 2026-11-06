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

/// Horizontal padding inside a grid row, in pixels.
///
/// The renderer and the pointer-anchored zoom math both use this value so the
/// column under the pointer cannot drift from the painted layout.
pub(crate) const GRID_ROW_PADDING_X: f32 = 16.0;

/// Gap between cards in a grid row, in pixels.
///
/// Shared with the renderer for the same reason as `GRID_ROW_PADDING_X`.
pub(crate) const GRID_CARD_GAP: f32 = 10.0;

/// Padding between the card border and its content, in pixels.
///
/// The renderer and [`grid_metrics`] share it so the square photo well cannot
/// drift from the painted card box.
pub(crate) const CARD_PADDING: f32 = 8.0;

/// Empty space below each card inside its uniform-list row, in pixels.
pub(crate) const CARD_ROW_GAP: f32 = 10.0;

/// Width of the pinned year rail when the timeline has buckets.
pub(crate) const YEAR_RAIL_WIDTH: f32 = 44.0;

/// Width of the collapsed sidebar rail (it never fully disappears).
pub(crate) const SIDEBAR_COLLAPSED_WIDTH: f32 = 48.0;

/// Width of the sidebar/inspector drag handles.
pub(crate) const RESIZER_WIDTH: f32 = 4.0;

/// Window width used before the bounds observer reports a real size.
pub(crate) const DEFAULT_WINDOW_WIDTH: f32 = 1280.0;

/// Smallest photo well side, so a very narrow window cannot collapse rows.
pub(crate) const PHOTO_SIDE_MIN: f32 = 48.0;

/// Largest photo well side, so a very wide window cannot produce huge rows.
pub(crate) const PHOTO_SIDE_MAX: f32 = 640.0;

/// Grid layout derived from the window width and the chrome beside the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GridMetrics {
    /// Side of the square photo well, in pixels.
    pub photo_side: f32,
    /// Full uniform-list row height: the card plus [`CARD_ROW_GAP`].
    pub row_height: f32,
}

/// Derives the square photo well and row height from the actual card width.
///
/// The center pane loses the sidebar (or its 48px collapsed rail) plus a 4px
/// resizer, the 44px year rail when it is painted, and the inspector plus its
/// 4px resizer when it is painted. Each row then loses
/// `2 * GRID_ROW_PADDING_X` and `(columns - 1) * GRID_CARD_GAP` before the
/// remaining width is split between the columns. The photo well is the card's
/// content width, and the row is the well plus the card chrome, the fixed info
/// band, and the gap below the card.
pub(crate) fn grid_metrics(
    window_width: f32,
    sidebar_width: f32,
    sidebar_collapsed: bool,
    year_rail_visible: bool,
    inspector_width: Option<f32>,
    columns: usize,
) -> GridMetrics {
    let columns = columns.clamp(2, 8) as f32;
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
    let grid_width = (window_width - sidebar - year_rail - inspector).max(0.0);
    let card_width = (grid_width - 2.0 * GRID_ROW_PADDING_X - GRID_CARD_GAP * (columns - 1.0))
        .max(0.0)
        / columns;
    let photo_side = (card_width - 2.0 * CARD_PADDING).clamp(PHOTO_SIDE_MIN, PHOTO_SIDE_MAX);
    GridMetrics {
        row_height: photo_side + 2.0 * CARD_PADDING + CARD_ROW_GAP,
        photo_side,
    }
}

/// Hover tooltip that keeps the full filename reachable now that the card no
/// longer prints it under the photo.
pub(crate) struct MediaCardTooltip {
    name: SharedString,
}

impl MediaCardTooltip {
    pub(crate) fn new(name: impl Into<SharedString>) -> Self {
        Self { name: name.into() }
    }
}

impl Render for MediaCardTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "media-card-name-tooltip".to_string())
            .px_2()
            .py_1()
            .bg(theme::bg_hud())
            .border_1()
            .border_color(theme::border())
            .rounded(theme::RADIUS_MD)
            .text_xs()
            .text_color(theme::text())
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
        let column = column_at_pointer(pointer_x, geometry.left, geometry.width, columns);
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
fn column_at_pointer(pointer_x: f32, left: f32, width: f32, columns: usize) -> usize {
    let inner_width = width - 2.0 * GRID_ROW_PADDING_X;
    let card_space = inner_width - columns.saturating_sub(1) as f32 * GRID_CARD_GAP;
    if !card_space.is_finite() || card_space <= 0.0 {
        return 0;
    }
    let card_width = card_space / columns as f32;
    if card_width <= 0.0 {
        return 0;
    }
    let relative = pointer_x - left - GRID_ROW_PADDING_X;
    ((relative / (card_width + GRID_CARD_GAP)).floor() as isize)
        .clamp(0, columns.saturating_sub(1) as isize) as usize
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
    pub texture_cache: Arc<Mutex<LruCache<MediaId, Arc<RenderImage>>>>,
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

    pub fn new(engine: Arc<LuminaEngine>) -> Self {
        let total_count = engine.total_count();
        let cache_capacity = NonZeroUsize::new(800).unwrap();

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
        // 1280px window, expanded sidebar (230 + 4px), year rail (44px), no inspector.
        let four = grid_metrics(1280.0, 230.0, false, true, None, 4);
        assert!(
            (four.photo_side - 219.0).abs() < 0.01,
            "got {}",
            four.photo_side
        );
        assert!(
            (four.row_height - 245.0).abs() < 0.01,
            "got {}",
            four.row_height
        );

        let two = grid_metrics(1280.0, 230.0, false, true, None, 2);
        assert!(
            (two.photo_side - 464.0).abs() < 0.01,
            "got {}",
            two.photo_side
        );

        let eight = grid_metrics(1280.0, 230.0, false, true, None, 8);
        assert!(
            (eight.photo_side - 96.5).abs() < 0.01,
            "got {}",
            eight.photo_side
        );
    }

    #[core::prelude::v1::test]
    fn metrics_account_for_each_layout_chrome() {
        // Hidden year rail: the grid gains its 44px.
        let no_rail = grid_metrics(1280.0, 230.0, false, false, None, 4);
        assert!(
            (no_rail.photo_side - 230.0).abs() < 0.01,
            "got {}",
            no_rail.photo_side
        );

        // Collapsed sidebar is a 48px rail with no resizer.
        let collapsed = grid_metrics(1280.0, 230.0, true, true, None, 4);
        assert!(
            (collapsed.photo_side - 265.5).abs() < 0.01,
            "got {}",
            collapsed.photo_side
        );

        // Open inspector: its width plus the 4px resizer are gone from the grid.
        let with_inspector = grid_metrics(1280.0, 230.0, false, true, Some(310.0), 4);
        assert!(
            (with_inspector.photo_side - 140.5).abs() < 0.01,
            "got {}",
            with_inspector.photo_side
        );
    }

    #[core::prelude::v1::test]
    fn metrics_clamp_extreme_windows_and_keep_the_row_identity() {
        let narrow = grid_metrics(700.0, 230.0, false, false, None, 8);
        assert_eq!(narrow.photo_side, PHOTO_SIDE_MIN);
        assert!((narrow.row_height - narrow.photo_side - CARD_CHROME).abs() < 0.01);

        let wide = grid_metrics(6000.0, 230.0, false, true, None, 2);
        assert_eq!(wide.photo_side, PHOTO_SIDE_MAX);
        assert!((wide.row_height - wide.photo_side - CARD_CHROME).abs() < 0.01);

        for width in [640.0, 960.0, 1280.0, 1920.0, 3008.0] {
            for columns in 2..=8 {
                for rail in [false, true] {
                    let metrics = grid_metrics(width, 230.0, false, rail, None, columns);
                    assert!(metrics.photo_side >= PHOTO_SIDE_MIN);
                    assert!(metrics.photo_side <= PHOTO_SIDE_MAX);
                    assert!((metrics.row_height - metrics.photo_side - CARD_CHROME).abs() < 0.01);
                }
            }
        }
    }

    #[core::prelude::v1::test]
    fn card_tooltip_payload_keeps_the_complete_filename() {
        let tooltip = MediaCardTooltip::new("IMG_2036.HEIC");
        assert_eq!(tooltip.name, "IMG_2036.HEIC");
    }
}
