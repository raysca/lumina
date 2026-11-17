//! Pointer drag-selection across grid cards.

use gpui::*;
use std::cmp;
use std::collections::BTreeSet;

use crate::app::state::GRID_DRAG_THRESHOLD;
use crate::app::LuminaApp;

impl LuminaApp {
    /// Starts a pointer drag-selection gesture at `position`.
    ///
    /// `anchor` is the card the press landed on; a background press passes
    /// `None` and the first card the pointer sweeps becomes the anchor.
    /// `additive` keeps the current selection as the base, so a drag beginning
    /// on an already-selected card cannot collapse a multi-selection.
    pub fn begin_grid_drag(
        &mut self,
        position: Point<Pixels>,
        anchor: Option<usize>,
        additive: bool,
    ) {
        self.grid_drag = crate::app::state::GridDragState {
            start: Some(position),
            current: Some(position),
            anchor,
            base: if additive {
                self.selected_indices.clone()
            } else {
                BTreeSet::new()
            },
            hovered: anchor,
            active: true,
            moved: false,
            suppress_click: false,
        };
    }

    /// Updates the live drag from a raw pointer move.
    ///
    /// Returns true once the gesture has moved past the click threshold. A move
    /// with no button held abandons a stale gesture: a mouse-up delivered
    /// outside the window never reaches the grid.
    pub fn grid_pointer_moved(
        &mut self,
        position: Point<Pixels>,
        pressed: Option<MouseButton>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.grid_drag.active {
            return false;
        }
        if pressed != Some(MouseButton::Left) {
            self.grid_drag.clear();
            cx.notify();
            return false;
        }
        self.update_grid_drag_position(position, cx)
    }

    fn update_grid_drag_position(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.grid_drag.active {
            return false;
        }
        self.grid_drag.current = Some(position);
        if let Some(start) = self.grid_drag.start {
            if (start.x - position.x).abs() > px(GRID_DRAG_THRESHOLD)
                || (start.y - position.y).abs() > px(GRID_DRAG_THRESHOLD)
            {
                self.grid_drag.moved = true;
            }
        }
        if self.grid_drag.moved {
            cx.notify();
        }
        self.grid_drag.moved
    }

    /// Extends the drag selection to the card under the pointer.
    pub fn drag_over_card(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.grid_drag.active {
            return;
        }
        if !self.update_grid_drag_position(position, cx) {
            return;
        }
        self.grid_drag.hovered = Some(index);
        if self.grid_drag.anchor.is_none() {
            self.grid_drag.anchor = Some(index);
        }
        self.apply_grid_drag_range(index, cx);
    }

    /// Replaces the selection with the drag base plus the grid rectangle
    /// spanned by the anchor and target cards, matching the marquee (in
    /// Mosaic, the reading-order range between them). The base preserves a
    /// multi-selection the drag started inside.
    fn apply_grid_drag_range(&mut self, target: usize, cx: &mut Context<Self>) {
        let total = self.get_active_count();
        if total == 0 {
            return;
        }
        let Some(anchor) = self.grid_drag.anchor.map(|anchor| anchor.min(total - 1)) else {
            return;
        };
        let target = target.min(total - 1);
        let mut next = self.grid_drag.base.clone();
        if self.mosaic_shown() {
            // Uneven rows have no columns: the reading-order range.
            next.extend(cmp::min(anchor, target)..=cmp::max(anchor, target));
        } else {
            // Must match the column count `render_browser_grid` lays out.
            let cols = self.grid_cols();
            let (rows, columns) = (
                cmp::min(anchor / cols, target / cols)..=cmp::max(anchor / cols, target / cols),
                cmp::min(anchor % cols, target % cols)..=cmp::max(anchor % cols, target % cols),
            );
            next.extend(
                rows.flat_map(|row| columns.clone().map(move |col| row * cols + col))
                    .filter(|&index| index < total),
            );
        }
        if next == self.selected_indices && self.selected_index == Some(target) {
            return;
        }
        self.selected_indices = next;
        self.selected_index = Some(target);
        self.update_batch_summary();
        self.schedule_session_save(cx);
        cx.notify();
    }

    /// Finishes the gesture on mouse-up.
    ///
    /// A still press on empty space clears the selection; a moved gesture
    /// finalizes the focused item and suppresses the click that follows.
    pub fn finish_grid_drag(&mut self, cx: &mut Context<Self>) {
        if !self.grid_drag.active {
            return;
        }
        let moved = self.grid_drag.moved;
        let anchor = self.grid_drag.anchor;
        if !moved {
            if anchor.is_none() {
                // Plain click on the grid background keeps its old meaning.
                self.clear_selection(cx);
            } else {
                self.grid_drag.clear();
                cx.notify();
            }
            return;
        }
        // Re-apply over the release target so a drag that ended over a gap still
        // includes the card it started on.
        if let Some(target) = self.grid_drag.hovered.or(anchor) {
            self.apply_grid_drag_range(target, cx);
        } else if self.grid_drag.base.is_empty() {
            self.clear_selection(cx);
            return;
        }
        if let Some(index) = self.selected_index {
            if let Some(item) = self.get_active_slice(index..index + 1).into_iter().next() {
                self.select_item_internal(item, cx);
            }
        }
        self.refresh_selected_tags();
        self.update_batch_summary();
        self.schedule_session_save(cx);
        self.grid_drag.clear();
        self.grid_drag.suppress_click = true;
        cx.notify();
    }

    /// True when the click that follows a drag must be ignored. Consumes the
    /// suppression so a later, genuine click is not swallowed.
    pub fn consume_grid_drag_click(&mut self) -> bool {
        let suppress = self.grid_drag.moved || self.grid_drag.suppress_click;
        if suppress {
            self.grid_drag.suppress_click = false;
        }
        suppress
    }
}
