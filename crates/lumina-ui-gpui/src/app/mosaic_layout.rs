//! The Mosaic body's layout cache and the Grid / Mosaic choice.
//!
//! The layout (one [`LayoutCell`] per item, justified into rows) is built
//! off the UI thread, once per (view, catalog revision, width, size level).
//! A frame only compares those four values and paints the cached rows, so
//! scrolling issues no SQL. A width or level change re-justifies the cached
//! cells without reading the catalog again.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use gpui::{px, Context, ListOffset};
use lumina_core::{DayPlace, LayoutCell, SortField};

use crate::app::active_view::ViewKey;
use crate::app::mosaic_nav::{offset_for_y, restore_offset, row_for_item};
use crate::app::LuminaApp;
use crate::config::BrowseLayout;
use crate::views::grid::{center_pane_width, GRID_ROW_PADDING_LEFT, GRID_ROW_PADDING_RIGHT};
use crate::views::mosaic::{target_for_columns, MosaicLayout, RowKind};
use crate::views::search_bar::size_level;

/// After a layout lands, a refetch caused only by writes from other threads
/// waits this long, so a scan's stream of revisions costs one layout read
/// per interval instead of holding the catalog lock back to back against
/// the grid's row reads. View, width and level changes, and writes made on
/// the UI thread (trash, favourite, rotate), refetch at once. The app's own
/// background writes (quick edit, jobs) count as other threads' and wait
/// like a scan.
pub(crate) const SCAN_REFETCH_COOLDOWN: Duration = Duration::from_millis(750);

#[cfg(test)]
thread_local! {
    /// Layout reads (SQL) started on this test thread.
    pub(crate) static LAYOUT_FETCHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A landed layout and what it was built from.
struct Built {
    key: ViewKey,
    revision: u64,
    width: u32,
    level: usize,
    cells: Arc<Vec<LayoutCell>>,
    /// Each dated day's most common place, read with `cells` (date sort
    /// only: other sorts have no day headers).
    places: Arc<HashMap<u32, DayPlace>>,
    layout: Arc<MosaicLayout>,
}

/// The cached Mosaic layout. At most one build runs at a time; changes
/// that arrive meanwhile coalesce into one build after it lands.
#[derive(Default)]
pub(crate) struct MosaicCache {
    built: Option<Built>,
    in_flight: bool,
    /// Set when a layout lands, cleared [`SCAN_REFETCH_COOLDOWN`] later.
    cooling: bool,
    /// Where a zoom's anchor item goes once its level's layout lands.
    pub(crate) zoom_anchor: Option<crate::app::mosaic_nav::ZoomAnchor>,
    /// A scroll asked for before its view's first layout landed (the
    /// session restore, a rail click or a Years/Months card).
    pub(crate) pending_scroll: Option<PendingScroll>,
}

/// An item to put on top once `key`'s first layout lands.
pub(crate) struct PendingScroll {
    pub key: ViewKey,
    pub item: usize,
    /// The relaunch's selection, revealed if the anchor leaves it off
    /// screen; `None` for a rail or card jump.
    pub reveal: Option<usize>,
}

impl LuminaApp {
    /// Mosaic is on and the view has a layout scope. Elsewhere (Duplicates,
    /// a smart album that is gone) the Grid body is drawn.
    pub(crate) fn mosaic_shown(&self) -> bool {
        self.config.browse_layout == BrowseLayout::Mosaic
            && self.view_query().layout_scope().is_some()
    }

    /// Which view-mode segment is on: (Mosaic, Grid, Map). Mosaic reads as
    /// on wherever it is the saved layout, even in a view that draws the
    /// Grid body as its fallback (see [`Self::mosaic_shown`]).
    pub(crate) fn view_mode_on(&self) -> (bool, bool, bool) {
        let map = self.map_shown();
        let mosaic = self.config.browse_layout == BrowseLayout::Mosaic;
        (!map && mosaic, !map && !mosaic, map)
    }

    /// The header's Mosaic and Grid segments, ⌘K Show Mosaic and Show
    /// Grid: leaves the map and sets the layout, saved in the config. A
    /// failed save puts the layout back and is in the error banner.
    pub(crate) fn set_browse_layout(&mut self, layout: BrowseLayout, cx: &mut Context<Self>) {
        self.show_grid(cx);
        let previous = self.config.browse_layout;
        if previous == layout {
            return;
        }
        self.config.browse_layout = layout;
        if !self.save_config() {
            self.config.browse_layout = previous;
        }
        cx.notify();
    }

    /// The landed layout for the current view (perhaps a revision, width
    /// or level behind while the next one builds). Never queries.
    pub(crate) fn mosaic_layout(&self) -> Option<Arc<MosaicLayout>> {
        let key = self.view_key();
        self.mosaic
            .built
            .as_ref()
            .filter(|built| built.key == key)
            .map(|built| built.layout.clone())
    }

    /// The most common place of `day` in the current view, from the same
    /// build as [`Self::mosaic_layout`]. Never queries.
    pub(crate) fn mosaic_day_place(&self, day: u32) -> Option<&DayPlace> {
        let key = self.view_key();
        self.mosaic
            .built
            .as_ref()
            .filter(|built| built.key == key)?
            .places
            .get(&day)
    }

    /// Item range of the day holding item `index`: its header's run.
    /// `None` without a layout or under a non-date sort (no headers).
    fn mosaic_day_range(&self, index: usize) -> Option<Range<usize>> {
        let layout = self.mosaic_layout()?;
        let rows = layout.rows.get(..=layout.row_of(index))?;
        rows.iter().rev().find_map(|row| match row.kind {
            RowKind::Header { count, .. } => Some(row.first..row.first + count),
            RowKind::Tiles => None,
        })
    }

    /// First item of the top visible row, from the list's scroll position.
    fn mosaic_top_item(&self) -> Option<usize> {
        let layout = self.mosaic_layout()?;
        let top = self.grid_state.as_ref()?.mosaic.logical_scroll_top();
        layout.rows.get(top.item_ix).map(|row| row.first)
    }

    /// ⌘K Select Day: every item of the selected item's day, or of the top
    /// visible day when nothing is selected.
    pub(crate) fn select_mosaic_day(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.selected_index.or_else(|| self.mosaic_top_item()) else {
            return;
        };
        if let Some(range) = self.mosaic_day_range(index) {
            self.select_mosaic_range(range, cx);
        }
    }

    /// A header's Select and ⌘K Select Day. Does nothing in the full-screen
    /// viewer (the photo shown must stay the selected one) or while the
    /// layout is behind the catalog (a scan, the cool-down): its day ranges
    /// may then name another day's items. The next layout makes it work.
    pub(crate) fn select_mosaic_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        if self.is_fullscreen_viewer || !self.mosaic_layout_current() {
            return;
        }
        self.select_range(range, cx);
    }

    /// The built layout is this view's at the catalog's current revision.
    pub(crate) fn mosaic_layout_current(&self) -> bool {
        let (Some(engine), Some(built)) = (self.grid_state.as_ref(), self.mosaic.built.as_ref())
        else {
            return false;
        };
        built.key == self.view_key() && built.revision == engine.engine.data_revision()
    }

    /// Width of the Mosaic's content: the center pane less the row padding.
    fn mosaic_width(&self) -> f32 {
        let window_width = self.window_bounds.map_or(
            crate::views::grid::DEFAULT_WINDOW_WIDTH,
            |(_, _, width, _)| width,
        );
        let inspector =
            (self.inspector_open && !self.is_fullscreen_viewer).then_some(self.inspector_width);
        let pane = center_pane_width(
            window_width,
            self.sidebar_width,
            self.sidebar_collapsed,
            self.timeline_rail_shown(),
            inspector,
        );
        (pane - GRID_ROW_PADDING_LEFT - GRID_ROW_PADDING_RIGHT).max(1.0)
    }

    /// Starts a background build when the cached layout is missing or for
    /// another view, revision, width or level. Called per frame while the
    /// Mosaic shows; it only compares a few values then.
    pub(crate) fn refresh_mosaic_layout(&mut self, cx: &mut Context<Self>) {
        if self.mosaic.in_flight {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let query = self.view_query();
        let Some(scope) = query.layout_scope() else {
            return;
        };
        let key = query.key().clone();
        let revision = engine.data_revision();
        let width = self.mosaic_width();
        let level = size_level(self.grid_columns);
        let same_view = self.mosaic.built.as_ref().filter(|b| b.key == key);
        if same_view
            .is_some_and(|b| b.revision == revision && b.width == width as u32 && b.level == level)
        {
            return;
        }
        // Only the revision moved, and not by this app's own write: a scan.
        let scan_only = same_view.is_some_and(|b| {
            b.revision != revision
                && b.revision >= engine.revision_written_here()
                && b.width == width as u32
                && b.level == level
        });
        if scan_only && self.mosaic.cooling {
            return;
        }
        let cells = same_view
            .filter(|b| b.revision == revision)
            .map(|b| (b.cells.clone(), b.places.clone()));
        #[cfg(test)]
        if cells.is_none() {
            LAYOUT_FETCHES.with(|n| n.set(n.get() + 1));
        }
        let sort = self.sort;
        let sections = sort.field == SortField::DateTaken;
        let target = target_for_columns(self.grid_columns);
        // ponytail: a panicking build would leave this set and freeze the
        // layout; view_layout logs and returns no cells instead of failing,
        // and justify takes any cells.
        self.mosaic.in_flight = true;
        let task = cx.background_executor().spawn(async move {
            let (revision, (cells, places)) = match cells {
                Some(read) => (revision, read),
                // Read before querying: a write that lands meanwhile
                // leaves this revision behind, and a later frame refetches.
                None => {
                    let revision = engine.data_revision();
                    let cells = Arc::new(engine.view_layout(&scope, sort));
                    let places = if sections {
                        engine.day_places(&scope)
                    } else {
                        HashMap::new()
                    };
                    (revision, (cells, Arc::new(places)))
                }
            };
            let layout = Arc::new(MosaicLayout::justify(&cells, width, target, sections));
            Built {
                key,
                revision,
                width: width as u32,
                level,
                cells,
                places,
                layout,
            }
        });
        cx.spawn(async move |this, cx| {
            let built = task.await;
            let _ = this.update(cx, |app, cx| {
                app.mosaic.in_flight = false;
                if built.key == app.view_key() {
                    app.apply_mosaic_layout(built);
                    app.start_mosaic_cooldown(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Swaps in a landed layout. In the same view the top item stays at the
    /// top; a new view starts at the top.
    fn apply_mosaic_layout(&mut self, built: Built) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let list = &state.mosaic;
        let restore = self
            .mosaic
            .built
            .as_ref()
            .filter(|old| old.key == built.key)
            .and_then(|old| {
                let top = list.logical_scroll_top();
                let row = old.layout.rows.get(top.item_ix)?;
                let header = matches!(row.kind, RowKind::Header { .. });
                Some((row.first, header, top.offset_in_item))
            });
        list.reset(built.layout.rows.len());
        let layout = &built.layout;
        // An anchor for another view is dropped; one for another level
        // waits for that level's layout.
        let zoom = match self.mosaic.zoom_anchor.take() {
            Some(a) if a.key == built.key && a.level != built.level => {
                self.mosaic.zoom_anchor = Some(a);
                None
            }
            Some(a) if a.key == built.key && restore.is_some() => Some(a),
            _ => None,
        };
        // Like the zoom anchor, a relaunch scroll for another view is dropped.
        let pending = self
            .mosaic
            .pending_scroll
            .take()
            .filter(|p| p.key == built.key);
        if layout.rows.is_empty() {
            // Nothing to scroll.
        } else if let Some(p) = pending {
            let viewport = f32::from(list.viewport_bounds().size.height);
            list.scroll_to(restore_offset(layout, p.item, p.reveal, viewport));
        } else if let Some(anchor) = zoom {
            // A zoom: the anchor item's point stays at the same screen y.
            let row = layout.rows[layout.row_of(anchor.item)];
            let y = row.y + row.pad + anchor.frac * (row.height - row.pad) - anchor.screen_y;
            list.scroll_to(offset_for_y(layout, y));
        } else if let Some((item, header, offset)) = restore {
            // `row_of` is the tile row; a header on top stays on top.
            let item_ix = if header {
                row_for_item(layout, item)
            } else {
                layout.row_of(item)
            };
            let height = px(layout.rows[item_ix].height);
            list.scroll_to(ListOffset {
                item_ix,
                offset_in_item: offset.min(height),
            });
        }
        self.mosaic.built = Some(built);
    }

    /// Holds scan-driven refetches off for [`SCAN_REFETCH_COOLDOWN`], then
    /// redraws so a frame can pick up what changed meanwhile.
    fn start_mosaic_cooldown(&mut self, cx: &mut Context<Self>) {
        if self.mosaic.cooling {
            return;
        }
        self.mosaic.cooling = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SCAN_REFETCH_COOLDOWN).await;
            let _ = this.update(cx, |app, cx| {
                app.mosaic.cooling = false;
                cx.notify();
            });
        })
        .detach();
    }
}
