//! Grid thumbnail size: the size ladder, ⌘-wheel and pinch zoom.

use gpui::*;

use crate::app::LuminaApp;
use crate::views::grid::{GridZoomAnchor, GridZoomGeometry};

impl LuminaApp {
    /// Larger thumbnails: one level up the size ladder (`SIZE_COLUMNS`).
    pub fn zoom_in_grid(&mut self, cx: &mut Context<Self>) {
        self.step_grid_size(1, cx);
    }

    /// Smaller thumbnails: one level down the size ladder.
    pub fn zoom_out_grid(&mut self, cx: &mut Context<Self>) {
        self.step_grid_size(-1, cx);
    }

    /// Whether one `step` along the ladder (positive: larger) would change
    /// the size; false at the ladder's ends, where the pill's button disables.
    pub(crate) fn grid_zoom_enabled(&self, step: isize) -> bool {
        use crate::views::search_bar::SIZE_COLUMNS;
        let level = ladder_step(self.grid_cols(), step);
        (0..SIZE_COLUMNS.len() as isize).contains(&level)
    }

    /// ⌘= / ⌘- and the palette: anchored on the selected item, which stays on
    /// screen, or on the item at the viewport's top-left.
    fn step_grid_size(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.zoom_grid_at(step, None, self.grid_scroll_y(), cx) {
            self.save_config();
        }
    }

    fn grid_scroll_y(&self) -> Pixels {
        self.grid_state.as_ref().map_or(px(0.0), |state| {
            state.scroll_handle.0.borrow().base_handle.offset().y
        })
    }

    /// Moves `step` levels along `SIZE_COLUMNS` (positive: fewer, larger
    /// cards) and scrolls so the anchor stays put. The anchor is the item
    /// under `anchor_pos` (wheel, pinch); with no position it is the selected
    /// item, held inside the viewport, or else the top-left item (keyboard).
    /// Returns `true` when the density changed.
    ///
    /// `scroll_y_before` is the offset the anchor is measured against. It is
    /// written back even when the level does not change, which undoes a scroll
    /// the list applied for a ⌘-scroll event. The offset goes to the public
    /// handle with `set_offset` (not a deferred `scroll_to_item`, of which
    /// GPUI keeps only one) and the next prepaint clamps it against the new
    /// content height.
    ///
    /// Limitation: a column change moves row boundaries, so the anchor item
    /// can move horizontally. This preserves its new row and the anchor's
    /// fractional position within that row; it is not a pixel-exact zoom.
    fn zoom_grid_at(
        &mut self,
        step: isize,
        anchor_pos: Option<Point<Pixels>>,
        scroll_y_before: Pixels,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::views::search_bar::SIZE_COLUMNS;
        // Mosaic anchors on its own layout and never touches the uniform
        // list's handle (a queued offset there would jump the Grid later).
        if self.mosaic_shown() {
            return self.zoom_mosaic_at(step, anchor_pos, cx);
        }
        let columns = self.grid_cols();
        let level = ladder_step(columns, step).clamp(0, SIZE_COLUMNS.len() as isize - 1);
        // No step yet (a trackpad burst below the threshold): keep the
        // count, even one that is off the ladder.
        let new_columns = if step == 0 {
            columns
        } else {
            SIZE_COLUMNS[level as usize]
        };
        let item_count = self.get_active_count();
        let handle = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone());
        let set_offset = |y: Pixels| {
            if let Some(handle) = &handle {
                let state = handle.0.borrow();
                let base = &state.base_handle;
                base.set_offset(point(base.offset().x, y));
            }
        };
        if new_columns == columns {
            set_offset(scroll_y_before);
            return false;
        }

        let viewport = handle
            .as_ref()
            .map(|handle| handle.0.borrow().base_handle.bounds())
            .unwrap_or_default();
        let geometry = GridZoomGeometry {
            left: f32::from(viewport.origin.x),
            top: f32::from(viewport.origin.y),
            width: f32::from(viewport.size.width),
            height: f32::from(viewport.size.height),
            scroll_y: f32::from(scroll_y_before),
            row_height: self.row_height(),
            columns,
            column_gap: self.grid_metrics().column_gap,
            item_count,
        };
        let selected = self.selected_index.filter(|&index| index < item_count);
        let anchor = match (anchor_pos, selected) {
            (Some(pos), _) => {
                let (x, y) = (f32::from(pos.x), f32::from(pos.y));
                GridZoomAnchor::at_pointer(x, y, geometry).map(|anchor| (anchor, y))
            }
            (None, Some(index)) => GridZoomAnchor::at_item(index, geometry),
            (None, None) => GridZoomAnchor::at_pointer(geometry.left, geometry.top, geometry)
                .map(|anchor| (anchor, geometry.top)),
        };

        // An open album zooms its own grid; Library's zoom stays put.
        if self.album_tiles() {
            self.albums.page.columns = Some(new_columns);
        } else {
            self.grid_columns = new_columns;
        }
        match anchor {
            Some((anchor, y)) => {
                let target =
                    anchor.target_scroll_offset(y, geometry, new_columns, self.row_height());
                set_offset(px(-target));
            }
            // No usable geometry (before the first prepaint): change density
            // and let the deferred reveal find the selection, if any.
            None => {
                set_offset(scroll_y_before);
                if anchor_pos.is_none() {
                    self.reveal_grid_selection();
                }
            }
        }
        cx.notify();
        true
    }

    /// Cmd/Ctrl + wheel over the media grid walks the size ladder around the
    /// pointer: one level per line (mouse-wheel notch), one per
    /// `GRID_WHEEL_STEP_PX` of pixel (trackpad) scroll.
    ///
    /// Returns `true` when the event belongs to the zoom gesture, so the caller
    /// can stop propagation; plain wheel scrolling must stay scrolling.
    ///
    /// The uniform list's own scroll listener runs before this bubble handler
    /// (child listeners bubble first), so the event delta is removed from the
    /// live offset to recover the pre-gesture scroll position.
    pub fn zoom_grid_with_wheel(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !event.modifiers.secondary(cx) {
            return false;
        }
        if matches!(event.touch_phase, TouchPhase::Started | TouchPhase::Ended) {
            self.grid_zoom.wheel_accum_px = 0.0;
        }

        let delta_y = f32::from(event.delta.pixel_delta(window.line_height()).y);
        if !delta_y.is_finite() || delta_y.abs() < GRID_ZOOM_WHEEL_EPSILON {
            return false;
        }
        if self.get_active_count() == 0 {
            return false;
        }
        let Some(scroll_handle) = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone())
        else {
            return false;
        };

        let (viewport, offset_after, max_offset) = {
            let state = scroll_handle.0.borrow();
            let base = &state.base_handle;
            (base.bounds(), base.offset(), base.max_offset())
        };
        // When the pointer is over the list, its own scroll listener has already
        // applied this event's scroll delta; undo it so a zoom step never also
        // scrolls. Outside the list (for example the surface's top padding) no
        // scroll was applied. `max_offset` is from the layout the event was
        // dispatched against.
        let scroll_y_before = if viewport.contains(&event.position) {
            (offset_after.y - px(delta_y)).clamp(-max_offset.y, px(0.0))
        } else {
            offset_after.y
        };

        // Wheel up (positive delta) tightens the grid (fewer, larger cards),
        // matching ⌘= / ⌘-.
        let step = match event.delta {
            ScrollDelta::Lines(_) => delta_y.signum() as isize,
            ScrollDelta::Pixels(_) => take_steps(
                &mut self.grid_zoom.wheel_accum_px,
                delta_y,
                GRID_WHEEL_STEP_PX,
            ),
        };
        // Wheel bursts are high frequency; debounce the config write through the
        // existing session-save path instead of writing on every tick.
        if self.zoom_grid_at(step, Some(event.position), scroll_y_before, cx) {
            self.schedule_session_save(cx);
        }
        true
    }

    /// Trackpad pinch over the grid: one ladder level per `GRID_PINCH_STEP`
    /// of magnification, anchored at the pinch centre. Never scrolls.
    pub fn zoom_grid_with_pinch(&mut self, event: &PinchEvent, cx: &mut Context<Self>) {
        // macOS never sends `Cancelled`; a cancelled pinch ends as `Moved`.
        if matches!(event.phase, TouchPhase::Started | TouchPhase::Ended) {
            self.grid_zoom.pinch_accum = 0.0;
        }
        if !event.delta.is_finite() {
            return;
        }
        let step = take_steps(
            &mut self.grid_zoom.pinch_accum,
            event.delta,
            GRID_PINCH_STEP,
        );
        if step != 0 && self.zoom_grid_at(step, Some(event.position), self.grid_scroll_y(), cx) {
            self.schedule_session_save(cx);
        }
    }
}

/// Wheel delta below which a Cmd/Ctrl + wheel event is treated as noise.
const GRID_ZOOM_WHEEL_EPSILON: f32 = 0.01;
/// Pixel (trackpad) ⌘-scroll per grid density level.
const GRID_WHEEL_STEP_PX: f32 = 40.0;
/// Pinch magnification per grid density level (0.12 = 12%).
const GRID_PINCH_STEP: f32 = 0.12;

/// Adds `delta` to `accum` and spends whole `step`s of it: the signed number
/// of ladder levels to move, leaving the remainder for the next event.
fn take_steps(accum: &mut f32, delta: f32, step: f32) -> isize {
    *accum += delta;
    let steps = (*accum / step).trunc();
    *accum -= steps * step;
    steps as isize
}

/// The ladder level `step` levels (positive: larger) from `columns`. An
/// off-ladder count (an album's 5) sits between two levels, so its first
/// step either way lands on the neighbour: 5 zooms in to 4 and out to 6.
fn ladder_step(columns: usize, step: isize) -> isize {
    use crate::views::search_bar::{size_level, SIZE_COLUMNS};
    let level = size_level(columns);
    let off_ladder = SIZE_COLUMNS[level] != columns.clamp(2, 8);
    level as isize + step - isize::from(off_ladder && step > 0)
}
