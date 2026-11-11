//! The Mosaic body's layout cache and the Grid / Mosaic choice.
//!
//! The layout (one [`LayoutCell`] per item, justified into rows) is built
//! off the UI thread, once per (view, catalog revision, width, size level).
//! A frame only compares those four values and paints the cached rows, so
//! scrolling issues no SQL. A width or level change re-justifies the cached
//! cells without reading the catalog again.

use std::sync::Arc;
use std::time::Duration;

use gpui::{px, Context, ListOffset};
use lumina_core::{LayoutCell, SortField};

use crate::app::active_view::ViewKey;
use crate::app::LuminaApp;
use crate::config::BrowseLayout;
use crate::views::grid::{center_pane_width, GRID_ROW_PADDING_LEFT, GRID_ROW_PADDING_RIGHT};
use crate::views::mosaic::{target_for_columns, MosaicLayout};
use crate::views::search_bar::size_level;

/// After a layout lands, a refetch caused only by other threads' writes (a
/// scan) waits this long, so a scan's stream of revisions costs one layout
/// read per interval instead of holding the catalog lock back to back
/// against the grid's row reads. View, width and level changes, and the
/// app's own writes, refetch at once.
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
}

impl LuminaApp {
    /// Mosaic is on and the view has a layout scope. Elsewhere (Duplicates,
    /// a smart album that is gone) the Grid body is drawn.
    pub(crate) fn mosaic_shown(&self) -> bool {
        self.config.browse_layout == BrowseLayout::Mosaic
            && self.view_query().layout_scope().is_some()
    }

    /// Which view-mode segment is on: (Mosaic, Grid, Map).
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
            .map(|b| b.cells.clone());
        #[cfg(test)]
        if cells.is_none() {
            LAYOUT_FETCHES.with(|n| n.set(n.get() + 1));
        }
        let sort = self.sort;
        let sections = sort.field == SortField::DateTaken;
        let target = target_for_columns(self.grid_columns);
        self.mosaic.in_flight = true;
        let task = cx.background_executor().spawn(async move {
            let (revision, cells) = match cells {
                Some(cells) => (revision, cells),
                // Read before querying: a write that lands meanwhile
                // leaves this revision behind, and a later frame refetches.
                None => (
                    engine.data_revision(),
                    Arc::new(engine.view_layout(&scope, sort)),
                ),
            };
            let layout = Arc::new(MosaicLayout::justify(&cells, width, target, sections));
            Built {
                key,
                revision,
                width: width as u32,
                level,
                cells,
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
                Some((row.first, top.offset_in_item))
            });
        list.reset(built.layout.rows.len());
        if let Some((item, offset)) = restore.filter(|_| !built.layout.rows.is_empty()) {
            let item_ix = built.layout.row_of(item);
            let height = px(built.layout.rows[item_ix].height);
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
