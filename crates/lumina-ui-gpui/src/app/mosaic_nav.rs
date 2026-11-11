//! Mosaic geometry for the shared grid paths: arrow and Page keys, the
//! visible range (rail, saved anchor), scrolling to an item, and zoom
//! anchoring. Each Grid site branches here when [`LuminaApp::mosaic_shown`];
//! everything reads the cached layout and the list's scroll state, never SQL.

use std::ops::Range;

use gpui::{px, Context, ListOffset, ListState, Pixels, Point};

use crate::app::active_view::ViewKey;
use crate::app::LuminaApp;
use crate::views::grid::GRID_ROW_PADDING_LEFT;
use crate::views::mosaic::{MosaicLayout, RowKind};
use crate::views::search_bar::{size_level, SIZE_COLUMNS};

/// Where a zoomed item should sit once the layout at the new level lands.
#[derive(Debug, Clone)]
pub(crate) struct ZoomAnchor {
    /// The view zoomed: a layout for another view drops the anchor.
    pub key: ViewKey,
    pub item: usize,
    /// The size level the anchor waits for.
    pub level: usize,
    /// How far down its tile the anchor point is, 0-1.
    pub frac: f32,
    /// The anchor point's distance below the list's top edge.
    pub screen_y: f32,
}

/// The row to put at the top for `item`: its day header when the day
/// starts there, else its tile row.
pub(crate) fn row_for_item(layout: &MosaicLayout, item: usize) -> usize {
    let r = layout.row_of(item);
    match r.checked_sub(1).map(|h| layout.rows[h]) {
        Some(h) if matches!(h.kind, RowKind::Header { .. }) && h.first == item => r - 1,
        _ => r,
    }
}

/// The scroll position putting content y `target` at the list's top.
pub(crate) fn offset_for_y(layout: &MosaicLayout, target: f32) -> ListOffset {
    let target = target.max(0.0);
    let r = layout
        .rows_in(target..target + 0.01)
        .start
        .min(layout.rows.len().saturating_sub(1));
    let (y, height) = layout
        .rows
        .get(r)
        .map_or((0.0, 0.0), |row| (row.y, row.height));
    ListOffset {
        item_ix: r,
        offset_in_item: px((target - y).min(height)),
    }
}

/// A relaunch's scroll: `item`'s row on top, unless that leaves the
/// `selected` row off-screen; then the least scroll from there that shows
/// it, as the Grid restore does. A list not yet measured (`viewport` 0)
/// keeps the anchor.
pub(crate) fn restore_offset(
    layout: &MosaicLayout,
    item: usize,
    selected: Option<usize>,
    viewport: f32,
) -> ListOffset {
    let top = row_for_item(layout, item);
    let y = layout.rows[top].y;
    let target = match selected.map(|s| layout.rows[layout.row_of(s)]) {
        Some(sel) if viewport > 0.0 && sel.y < y => sel.y,
        Some(sel) if viewport > 0.0 && sel.y + sel.height > y + viewport => {
            sel.y + sel.height - viewport
        }
        _ => {
            return ListOffset {
                item_ix: top,
                offset_in_item: px(0.0),
            }
        }
    };
    offset_for_y(layout, target)
}

/// Content y of the list's top edge.
fn scroll_y(layout: &MosaicLayout, list: &ListState) -> f32 {
    let top = list.logical_scroll_top();
    layout
        .rows
        .get(top.item_ix)
        .map_or(0.0, |row| row.y + f32::from(top.offset_in_item))
}

impl LuminaApp {
    fn mosaic_list(&self) -> Option<ListState> {
        self.grid_state.as_ref().map(|state| state.mosaic.clone())
    }

    /// Rows on screen, from the list's scroll position and last viewport.
    fn mosaic_visible_rows(&self, layout: &MosaicLayout) -> Range<usize> {
        let Some(list) = self.mosaic_list() else {
            return 0..0;
        };
        let y = scroll_y(layout, &list);
        let height = f32::from(list.viewport_bounds().size.height).max(1.0);
        layout.rows_in(y..y + height)
    }

    /// Items on screen in Mosaic (the rail's lens and bubble).
    pub(crate) fn mosaic_visible_items(&self) -> Range<usize> {
        let Some(layout) = self.mosaic_layout() else {
            return 0..0;
        };
        let rows = self.mosaic_visible_rows(&layout);
        let (Some(first), Some(last)) = (
            layout.rows.get(rows.start),
            rows.end.checked_sub(1).and_then(|r| layout.rows.get(r)),
        ) else {
            return 0..0;
        };
        first.first..(last.first + last.len).max(first.first + 1)
    }

    /// The list's top row (a layout row, headers included).
    pub(crate) fn mosaic_top_row(&self) -> usize {
        self.mosaic_list()
            .map_or(0, |list| list.logical_scroll_top().item_ix)
    }

    /// The selection step for ↑/↓ (`page`: PageUp/PageDown) in Mosaic: to
    /// the nearest-x tile one row away, or as many rows as are on screen.
    /// 0 without a layout or at either end.
    pub(crate) fn mosaic_vertical_step(&self, down: bool, page: bool) -> isize {
        let Some(layout) = self.mosaic_layout() else {
            return 0;
        };
        let from = self.selected_index.unwrap_or(0);
        let rows = if page {
            let visible = self.mosaic_visible_rows(&layout);
            layout.rows[visible]
                .iter()
                .filter(|r| r.kind == RowKind::Tiles)
                .count()
                .saturating_sub(1)
                .max(1)
        } else {
            1
        };
        let mut to = from;
        for _ in 0..rows {
            match layout.vertical_neighbour(to, down) {
                Some(next) => to = next,
                None => break,
            }
        }
        to as isize - from as isize
    }

    /// Scrolls the Mosaic so `item`'s day header (or row) is at the top.
    pub(crate) fn scroll_mosaic_to_item(&mut self, item: usize, cx: &mut Context<Self>) {
        let Some(list) = self.mosaic_list() else {
            return;
        };
        match self.mosaic_layout() {
            Some(layout) if !layout.rows.is_empty() => list.scroll_to(ListOffset {
                item_ix: row_for_item(&layout, item),
                offset_in_item: px(0.0),
            }),
            // No layout for this view yet: put it on top when one lands.
            _ => {
                self.mosaic.pending_scroll = Some(crate::app::mosaic_layout::PendingScroll {
                    key: self.view_key(),
                    item,
                    reveal: None,
                })
            }
        }
        cx.notify();
    }

    /// Scrolls the least needed to show `item`'s row.
    pub(crate) fn reveal_mosaic_item(&self, item: usize) {
        let (Some(layout), Some(list)) = (self.mosaic_layout(), self.mosaic_list()) else {
            return;
        };
        if !layout.rows.is_empty() {
            list.scroll_to_reveal_item(layout.row_of(item));
        }
    }

    /// The item under window point `pos`, how far down its tile `pos` is,
    /// and `pos`'s distance below the list's top. Over a header, the first
    /// tile below it, at its top.
    fn mosaic_item_at(
        &self,
        layout: &MosaicLayout,
        pos: Point<Pixels>,
    ) -> Option<(usize, f32, f32)> {
        let list = self.mosaic_list()?;
        let viewport = list.viewport_bounds();
        let screen_y = f32::from(pos.y - viewport.origin.y);
        let top = scroll_y(layout, &list);
        let y = top + screen_y;
        let mut r = layout.rows_in(y..y + 0.01).start;
        let mut row = *layout.rows.get(r)?;
        if row.kind != RowKind::Tiles {
            r += 1;
            row = *layout.rows.get(r)?;
            return Some((row.first, 0.0, row.y + row.pad - top));
        }
        let x = f32::from(pos.x - viewport.origin.x) - GRID_ROW_PADDING_LEFT;
        let dist = |i: &usize| {
            let t = layout.tiles[*i];
            (t.x + t.w / 2.0 - x).abs()
        };
        let item = (row.first..row.first + row.len).min_by(|a, b| dist(a).total_cmp(&dist(b)))?;
        let height = (row.height - row.pad).max(1.0);
        let frac = ((y - row.y - row.pad) / height).clamp(0.0, 1.0);
        Some((item, frac, screen_y))
    }

    /// Mosaic zoom: steps the size level and remembers where the anchor
    /// item is on screen, so the layout at the new level lands with it in
    /// the same place. The anchor is the item under `anchor_pos` (wheel,
    /// pinch), else the selected item held inside the viewport (±, ⌘=/⌘-),
    /// else the top item. Returns `true` when the level changed.
    pub(crate) fn zoom_mosaic_at(
        &mut self,
        step: isize,
        anchor_pos: Option<Point<Pixels>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let columns = self.grid_columns.clamp(2, 8);
        let level = (size_level(columns) as isize + step).clamp(0, SIZE_COLUMNS.len() as isize - 1)
            as usize;
        if step == 0 || SIZE_COLUMNS[level] == self.grid_columns {
            return false;
        }
        let anchor = self.mosaic_layout().and_then(|layout| {
            let list = self.mosaic_list()?;
            let (item, frac, screen_y) = match (anchor_pos, self.selected_index) {
                (Some(pos), _) => self.mosaic_item_at(&layout, pos)?,
                (None, selected) => {
                    let item = selected.unwrap_or_else(|| {
                        layout
                            .rows
                            .get(self.mosaic_top_row())
                            .map_or(0, |row| row.first)
                    });
                    let row = layout.rows.get(layout.row_of(item))?;
                    let top = row.y + row.pad - scroll_y(&layout, &list);
                    let room =
                        f32::from(list.viewport_bounds().size.height) - (row.height - row.pad);
                    (item, 0.0, top.min(room.max(0.0)).max(0.0))
                }
            };
            Some(ZoomAnchor {
                key: self.view_key(),
                item,
                level,
                frac,
                screen_y,
            })
        });
        self.mosaic.zoom_anchor = anchor;
        self.grid_columns = SIZE_COLUMNS[level];
        cx.notify();
        true
    }
}
