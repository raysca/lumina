//! The offline map on Places: state, the bundled basemap, the projection
//! and the map's keys. Drawing is in `views/map.rs`.
//!
//! No network: the land outline is `resources/map/land_110m.bin`, embedded
//! in the binary, and the points come from the catalog.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};

use gpui::*;
use lumina_core::{geo_cluster, Cluster, GeoPoint, SystemAlbum};

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;

pub const MIN_ZOOM: u8 = 1;
pub const MAX_ZOOM: u8 = 6;
/// Cluster cell at zoom 1, in degrees. It halves per zoom step while the
/// map doubles, so a cell keeps its size on screen (about 40 px in a 900 px
/// wide map).
const CELL_DEG: f32 = 16.0;
/// Pointer travel, in pixels, after which a press is a drag, not a click.
const DRAG_SLOP: f32 = 3.0;
/// A focused cluster closer than this to the map's edge is brought to the
/// middle.
const EDGE_MARGIN: f32 = 24.0;

const PLACES: AlbumSelection = AlbumSelection::System(SystemAlbum::Places);

#[cfg(test)]
thread_local! {
    /// Makes the points read fail on this test thread.
    pub(crate) static MAP_LOAD_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// One basemap polygon ring, in (lon, lat) degrees.
pub struct Ring {
    /// Water inside land (the Caspian Sea at this scale), drawn in the
    /// water colour over the land.
    pub hole: bool,
    pub points: Vec<(f32, f32)>,
}

/// Top bit of a ring's point count: the ring is a hole.
const HOLE_BIT: u32 = 1 << 31;

/// Reads the basemap format written by `scripts/build_world_basemap.py`,
/// little-endian: a u32 ring count, then per ring a u32 point count (top
/// bit: hole) and that many (f32 lon, f32 lat) pairs. A short or overlong
/// file is an error.
pub fn parse_basemap(bytes: &[u8]) -> anyhow::Result<Vec<Ring>> {
    let mut at = 0usize;
    let mut word = || -> anyhow::Result<[u8; 4]> {
        let chunk = bytes
            .get(at..at + 4)
            .ok_or_else(|| anyhow::anyhow!("basemap truncated at byte {at}"))?;
        at += 4;
        Ok(chunk.try_into().expect("a 4-byte slice"))
    };
    let count = u32::from_le_bytes(word()?);
    let mut rings = Vec::new();
    for _ in 0..count {
        let raw = u32::from_le_bytes(word()?);
        let len = raw & !HOLE_BIT;
        let mut points = Vec::new();
        for _ in 0..len {
            let lon = f32::from_le_bytes(word()?);
            let lat = f32::from_le_bytes(word()?);
            points.push((lon, lat));
        }
        rings.push(Ring {
            hole: raw & HOLE_BIT != 0,
            points,
        });
    }
    anyhow::ensure!(
        at == bytes.len(),
        "basemap has {} trailing bytes",
        bytes.len() - at
    );
    Ok(rings)
}

/// The land and hole fills, tessellated once in degree space (x = lon +
/// 180, y = 90 - lat, one unit per degree). A frame only scales and moves
/// the vertices ([`place`]), so panning and zooming never re-tessellate.
pub(crate) struct Basemap {
    pub land: Path<Pixels>,
    pub holes: Option<Path<Pixels>>,
}

pub(crate) static BASEMAP: LazyLock<Option<Basemap>> = LazyLock::new(|| {
    let rings = match parse_basemap(crate::assets::LAND_110M) {
        Ok(rings) => rings,
        Err(err) => {
            log::error!("the bundled basemap does not parse: {err:?}");
            return None;
        }
    };
    let fill = |hole: bool| {
        let mut path = PathBuilder::fill();
        let mut any = false;
        for ring in rings.iter().filter(|r| r.hole == hole) {
            let mut points = ring
                .points
                .iter()
                .map(|&(lon, lat)| point(px(lon + 180.0), px(90.0 - lat)));
            if let Some(first) = points.next() {
                path.move_to(first);
                points.for_each(|p| path.line_to(p));
                path.close();
                any = true;
            }
        }
        if !any {
            return None;
        }
        path.build()
            .map_err(|err| log::error!("the basemap does not tessellate: {err:?}"))
            .ok()
    };
    Some(Basemap {
        land: fill(false)?,
        holes: fill(true),
    })
});

/// Zoom Show on Map opens Places at, centred on the item (also the
/// inspector location card's zoom).
pub(crate) const FOCUS_ZOOM: u8 = 5;

/// `path` (degree space) scaled by `scale` and moved by `offset`.
pub(crate) fn place(path: &Path<Pixels>, scale: f32, offset: Point<Pixels>) -> Path<Pixels> {
    let at = |p: Point<Pixels>| point(offset.x + p.x * scale, offset.y + p.y * scale);
    let mut placed = path.clone();
    for vertex in &mut placed.vertices {
        vertex.xy_position = at(vertex.xy_position);
    }
    placed.bounds = Bounds::new(
        at(path.bounds.origin),
        size(
            path.bounds.size.width * scale,
            path.bounds.size.height * scale,
        ),
    );
    placed
}

/// Equirectangular projection of the map at one zoom, centre and size.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Projection {
    mid: Point<Pixels>,
    /// Pixels per degree, the same along both axes.
    pub scale: f32,
    center: (f32, f32),
}

/// Pixels per degree for a map of `size` at `zoom`: at zoom 1 the whole
/// world fits, and each step doubles.
fn scale_for(size: Size<Pixels>, zoom: u8) -> f32 {
    let fit = (f32::from(size.width) / 360.0).min(f32::from(size.height) / 180.0);
    fit.max(f32::EPSILON) * f32::from(1u16 << (zoom.max(MIN_ZOOM) - 1))
}

/// `lon` in [-180, 180): the same meridian, once around the world.
pub(crate) fn wrap_lon(lon: f32) -> f32 {
    // In range it is returned as is: the round trip through +180 would
    // round it (135° would come back as 135.00002°).
    if (-180.0..180.0).contains(&lon) {
        lon
    } else {
        (lon + 180.0).rem_euclid(360.0) - 180.0
    }
}

/// `center` kept on the map. Along an axis where the world is smaller than
/// the map (always at zoom 1), it is centred. Otherwise the latitude moves
/// as little as needed for the poles to stay at or beyond the map's edge,
/// and the longitude wraps across the date line, the world repeating east
/// and west ([`Projection::basemap_offsets`]).
pub(crate) fn clamp_center(center: (f32, f32), size: Size<Pixels>, zoom: u8) -> (f32, f32) {
    let scale = scale_for(size, zoom);
    let room = |half_world: f32, extent: Pixels| {
        let room = half_world - f32::from(extent) / (2.0 * scale);
        // Under half a pixel of room is none: along the axis that fits the
        // world exactly, rounding leaves a tiny positive room otherwise.
        (room * scale >= 0.5).then_some(room)
    };
    (
        room(180.0, size.width).map_or(0.0, |_| wrap_lon(center.0)),
        room(90.0, size.height).map_or(0.0, |room| center.1.clamp(-room, room)),
    )
}

impl Projection {
    /// At zoom 1 the whole world fits the map. The centre is clamped so the
    /// world always covers the map (or is centred in it).
    pub fn new(bounds: Bounds<Pixels>, zoom: u8, center: (f32, f32)) -> Self {
        Self {
            mid: bounds.center(),
            scale: scale_for(bounds.size, zoom),
            center: clamp_center(center, bounds.size, zoom),
        }
    }

    /// Where (lon, lat) lands: on the copy of the world nearest the
    /// centre, so a place just across the date line is drawn beside it.
    pub fn to_screen(self, lon: f32, lat: f32) -> Point<Pixels> {
        // Both are within ±180, so one turn at most brings them together;
        // exactly 180° apart stays put (180°E on a centred world's east edge).
        let d = lon - self.center.0;
        let d = if d > 180.0 {
            d - 360.0
        } else if d < -180.0 {
            d + 360.0
        } else {
            d
        };
        point(
            self.mid.x + px(d * self.scale),
            self.mid.y - px((lat - self.center.1) * self.scale),
        )
    }

    /// Where the basemap's degree-space origin (180°W, 90°N) lands for the
    /// copy of the world around the centre.
    pub fn basemap_offset(self) -> Point<Pixels> {
        point(
            self.mid.x + px((-180.0 - self.center.0) * self.scale),
            self.mid.y - px((90.0 - self.center.1) * self.scale),
        )
    }

    /// The basemap offsets of every copy of the world that reaches into a
    /// map `width` wide: one, or two when the date line is on the map. A
    /// world no wider than the map is centred and drawn once, since each
    /// place has one pin.
    pub fn basemap_offsets(self, width: Pixels) -> impl Iterator<Item = Point<Pixels>> {
        let origin = self.basemap_offset();
        let world = px(360.0 * self.scale);
        let left = self.mid.x - width / 2.0;
        let right = self.mid.x + width / 2.0;
        let repeats = world > width + px(1.0);
        [-1.0f32, 0.0, 1.0]
            .into_iter()
            .filter(move |&k| repeats || k == 0.0)
            .map(move |k| point(origin.x + world * k, origin.y))
            .filter(move |o| o.x < right && o.x + world > left)
    }

    /// The (lon, lat) under a screen point; the longitude may lie past
    /// ±180 when that point is on a neighbouring copy of the world.
    pub fn to_degrees(self, at: Point<Pixels>) -> (f32, f32) {
        (
            self.center.0 + f32::from(at.x - self.mid.x) / self.scale,
            self.center.1 - f32::from(at.y - self.mid.y) / self.scale,
        )
    }
}

/// Clusters of `points` at `zoom`, west to east (`map-cluster-{index}`
/// order). Runs on the background executor.
fn cluster_sorted(points: &[GeoPoint], zoom: u8) -> Arc<[Cluster]> {
    let mut clusters = geo_cluster::cluster(points, CELL_DEG / f32::from(1u16 << (zoom - 1)));
    clusters.sort_by(|a, b| a.lon.total_cmp(&b.lon).then(a.lat.total_cmp(&b.lat)));
    clusters.into()
}

/// The map's zoom, centre, clusters and focus, and its points.
pub struct MapState {
    /// The Grid/Map view mode chosen for Places. It outlives leaving Places,
    /// so Back and the sidebar return to the map; a new library starts on
    /// the grid.
    pub shown: bool,
    pub zoom: u8,
    /// (lon, lat) at the middle of the map, clamped to the world at `zoom`
    /// once the map's size is known.
    pub center: (f32, f32),
    /// Clusters west to east: `map-cluster-{index}` order. Shared, so a
    /// frame copies a pointer. Between a zoom step and its clustering
    /// landing they are the previous zoom's (still drawn at their places).
    pub clusters: Arc<[Cluster]>,
    pub focus: Option<usize>,
    /// Every geotagged item, once read; `None` until the first read lands.
    pub(crate) points: Option<Arc<Vec<GeoPoint>>>,
    /// The latest read failed. Nothing reads again until Retry or the map
    /// is shown again; the map says so instead of drawing nothing.
    pub(crate) failed: bool,
    /// (library generation, data revision) of the last successful read.
    pub(crate) loaded: Option<(u64, u64)>,
    in_flight: bool,
    /// Bumped per read, so only the latest read lands.
    request: u64,
    /// Bumped per clustering, so only the latest clustering lands.
    cluster_request: u64,
    /// The map's size in the last frame, for dragging and focus panning.
    pub(crate) viewport: Rc<Cell<Size<Pixels>>>,
    /// Last pointer position and travel of a press on the map.
    pub(crate) drag: Option<(Point<Pixels>, f32)>,
    /// The latest press moved the map, so its release is not a click.
    pub(crate) dragged: bool,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            shown: false,
            zoom: MIN_ZOOM,
            center: (0.0, 0.0),
            clusters: Arc::from([]),
            focus: None,
            points: None,
            failed: false,
            loaded: None,
            in_flight: false,
            request: 0,
            cluster_request: 0,
            viewport: Rc::default(),
            drag: None,
            dragged: false,
        }
    }
}

impl MapState {
    fn size(&self) -> Option<Size<Pixels>> {
        let size = self.viewport.get();
        (size.width > px(0.0) && size.height > px(0.0)).then_some(size)
    }

    fn projection(&self) -> Option<Projection> {
        self.size().map(|size| {
            Projection::new(Bounds::new(Point::default(), size), self.zoom, self.center)
        })
    }

    /// Sets the centre, clamped to the world once the size is known.
    fn set_center(&mut self, center: (f32, f32)) {
        self.center = match self.size() {
            Some(size) => clamp_center(center, size, self.zoom),
            None => center,
        };
    }

    fn focused(&self) -> Option<Cluster> {
        self.focus.and_then(|i| self.clusters.get(i).copied())
    }

    /// Takes new clusters. Focus moves to the cluster nearest the one
    /// focused before; with `center`, the map centres on it.
    fn apply_clusters(&mut self, clusters: Arc<[Cluster]>, center: bool) {
        let anchor = self.focused();
        self.focus = anchor.and_then(|a| {
            let distance = |c: &Cluster| (c.lon - a.lon).powi(2) + (c.lat - a.lat).powi(2);
            (0..clusters.len())
                .min_by(|&i, &j| distance(&clusters[i]).total_cmp(&distance(&clusters[j])))
        });
        self.clusters = clusters;
        if let (true, Some(c)) = (center, self.focused()) {
            self.set_center((c.lon, c.lat));
        }
    }
}

impl LuminaApp {
    /// The map is what the window shows: Places with the view mode on Map.
    pub(crate) fn map_shown(&self) -> bool {
        self.map.shown && self.selection == PLACES && self.grid_state.is_some()
    }

    /// Compare, a slideshow or edit mode is up: the map cannot be shown
    /// under it (the palette and View > Show Map both check this).
    pub(crate) fn map_blocked(&self) -> bool {
        self.grid_state.is_none()
            || self.compare_holds_view()
            || self.slideshow.is_some()
            || self.edits.session.is_some()
    }

    /// ⌘K Show Map, View > Show Map, the header's Map segment: Places on
    /// the map. The map holds no selection, so nothing acts on hidden grid
    /// items. Showing it from the grid reads the points again (after a
    /// failed read, too); asking again while it shows does nothing.
    pub(crate) fn show_map(&mut self, cx: &mut Context<Self>) {
        // Not part of `map_blocked`: the palette asks that while it is open.
        if self.map_blocked() || self.import_blocked_by_modal() {
            return;
        }
        // Already on the map (the active Map segment): nothing to reload.
        if self.map_shown() && !self.is_fullscreen_viewer {
            return;
        }
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        if self.selection != PLACES {
            self.select_album(PLACES, cx);
        }
        self.map.shown = true;
        self.map.failed = false;
        self.map.loaded = None;
        self.clear_selection(cx);
        cx.notify();
    }

    /// The selected item's (lat, lon), when it has both.
    pub(crate) fn selected_item_location(&self) -> Option<(f64, f64)> {
        let item = self.selected_item.as_ref()?;
        item.latitude.zip(item.longitude)
    }

    /// ⌘K Show on Map, the location card's Open map: Places on the map,
    /// zoomed in and centred on the selected item. The map takes the
    /// selection away (`show_map`), so the location is read first.
    pub(crate) fn show_item_on_map(&mut self, cx: &mut Context<Self>) {
        let Some((lat, lon)) = self.selected_item_location() else {
            return;
        };
        self.show_map(cx);
        if !self.map_shown() {
            return;
        }
        self.map.zoom = FOCUS_ZOOM;
        self.map.focus = None;
        self.map.set_center((lon as f32, lat as f32));
        // A read in flight clustered at the old zoom; this drops it.
        self.recluster_map(false, cx);
        cx.notify();
    }

    /// Leaves the map for the browse layout (Grid or Mosaic): Esc on the
    /// map, and the first step of the Mosaic / Grid segments and commands.
    pub(crate) fn show_grid(&mut self, cx: &mut Context<Self>) {
        self.map.shown = false;
        self.map.drag = None;
        cx.notify();
    }

    /// Retry on the error banner: read the points again.
    fn retry_map_points(&mut self, cx: &mut Context<Self>) {
        self.map.failed = false;
        self.map.loaded = None;
        self.refresh_map_points(cx);
        cx.notify();
    }

    /// Reads the points, and clusters them at the current zoom, on the
    /// background executor when the map shows and the catalog moved since
    /// the last read (a scan, a location edit, a trash). Called per frame;
    /// it only compares two numbers then.
    pub(crate) fn refresh_map_points(&mut self, cx: &mut Context<Self>) {
        if !self.map_shown() || self.map.in_flight || self.map.failed {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let generation = self.library_generation;
        if self.map.loaded == Some((generation, engine.data_revision())) {
            return;
        }
        self.map.in_flight = true;
        self.map.request += 1;
        self.map.cluster_request += 1;
        let token = (generation, self.map.request);
        let cluster_token = self.map.cluster_request;
        let zoom = self.map.zoom;
        #[cfg(test)]
        let fail = MAP_LOAD_FAILS.with(Cell::get);
        #[cfg(not(test))]
        let fail = false;
        let task = cx.background_executor().spawn(async move {
            // Read before querying: a write that lands meanwhile leaves this
            // revision behind, and the next frame reads again.
            let revision = engine.data_revision();
            let points = if fail {
                Err(anyhow::anyhow!("database is locked"))
            } else {
                engine.catalog().geo_points().map(Arc::new)
            };
            let clusters = points.as_ref().ok().map(|p| cluster_sorted(p, zoom));
            (revision, points, clusters)
        });
        cx.spawn(async move |this, cx| {
            let (revision, points, clusters) = task.await;
            let _ = this.update(cx, |app, cx| {
                if (app.library_generation, app.map.request) != token {
                    return;
                }
                app.map.in_flight = false;
                match points {
                    Ok(points) => {
                        app.map.loaded = Some((generation, revision));
                        app.map.points = Some(points);
                        // A zoom step meanwhile wants these points at its
                        // zoom: cluster them again rather than show stale ones.
                        match clusters {
                            Some(clusters) if app.map.cluster_request == cluster_token => {
                                app.map.apply_clusters(clusters, false)
                            }
                            _ => app.recluster_map(false, cx),
                        }
                    }
                    Err(err) => {
                        // Not marked loaded; `failed` stops a read per frame
                        // until Retry or the map is shown again.
                        log::warn!("could not read photo locations: {err:?}");
                        app.map.failed = true;
                        app.set_error_with_retry(
                            t("map.load_failed"),
                            |app, cx| app.retry_map_points(cx),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Clusters the points at the current zoom on the background executor.
    /// Only the latest clustering of this library lands.
    fn recluster_map(&mut self, center: bool, cx: &mut Context<Self>) {
        // Bumped even with no points yet: a first read in flight clustered
        // at the old zoom, and its result must not land as current.
        self.map.cluster_request += 1;
        let Some(points) = self.map.points.clone() else {
            return;
        };
        let token = (self.library_generation, self.map.cluster_request);
        let zoom = self.map.zoom;
        let task = cx
            .background_executor()
            .spawn(async move { cluster_sorted(&points, zoom) });
        cx.spawn(async move |this, cx| {
            let clusters = task.await;
            let _ = this.update(cx, |app, cx| {
                if (app.library_generation, app.map.cluster_request) != token {
                    return;
                }
                app.map.apply_clusters(clusters, center);
                cx.notify();
            });
        })
        .detach();
    }

    /// ⌘= / ⌘- on the map, the zoom buttons. A focused cluster ends up in
    /// the middle (as far as the world's edge allows), so zooming in on it
    /// needs no panning.
    pub(crate) fn zoom_map(&mut self, step: i8, cx: &mut Context<Self>) {
        let zoom = (self.map.zoom as i8 + step).clamp(MIN_ZOOM as i8, MAX_ZOOM as i8) as u8;
        if zoom == self.map.zoom {
            return;
        }
        self.map.zoom = zoom;
        let center = self
            .map
            .focused()
            .map_or(self.map.center, |c| (c.lon, c.lat));
        self.map.set_center(center);
        self.recluster_map(true, cx);
        cx.notify();
    }

    /// Tab / arrows: the next cluster east (`step` 1) or west (-1),
    /// wrapping. A cluster near or past the edge is brought to the middle.
    fn move_map_focus(&mut self, step: isize, cx: &mut Context<Self>) {
        let n = self.map.clusters.len();
        if n == 0 {
            return;
        }
        let next = match self.map.focus {
            None if step > 0 => 0,
            None => n - 1,
            Some(i) => (i as isize + step).rem_euclid(n as isize) as usize,
        };
        self.map.focus = Some(next);
        let c = self.map.clusters[next];
        if let (Some(projection), Some(size)) = (self.map.projection(), self.map.size()) {
            let p = projection.to_screen(c.lon, c.lat);
            let inside =
                |v: Pixels, max: Pixels| v >= px(EDGE_MARGIN) && v <= max - px(EDGE_MARGIN);
            if !inside(p.x, size.width) || !inside(p.y, size.height) {
                self.map.set_center((c.lon, c.lat));
            }
        }
        cx.notify();
    }

    /// ↵ or a click on a cluster: the grid, searched `within:` the
    /// cluster. The search moves the sidebar to Library (D11); Back returns
    /// to Places on the map.
    pub(crate) fn apply_map_cluster(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(c) = self.map.clusters.get(index).copied() else {
            return;
        };
        self.map.focus = Some(index);
        self.set_search_query(&c.within_query(), cx);
        cx.notify();
    }

    /// Keys while the map shows: Tab/⇧Tab and the arrows focus a cluster,
    /// ↵ searches it, Esc shows the grid, ⌘= / ⌘- zoom. Every other key
    /// that gets here is absorbed, so a grid shortcut never acts on the
    /// hidden grid. Global shortcuts (⌘K, ⌘Z…) ran before this.
    pub(crate) fn handle_map_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        if !self.map_shown() || self.is_fullscreen_viewer {
            return false;
        }
        let m = &keystroke.modifiers;
        let command = m.secondary(cx) || m.control;
        let plain = !(m.platform || m.control || m.alt || m.function);
        match keystroke.key.as_str() {
            "=" | "+" if command => self.zoom_map(1, cx),
            "-" | "_" if command => self.zoom_map(-1, cx),
            "tab" if plain => self.move_map_focus(if m.shift { -1 } else { 1 }, cx),
            "left" | "up" if plain && !m.shift => self.move_map_focus(-1, cx),
            "right" | "down" if plain && !m.shift => self.move_map_focus(1, cx),
            "enter" | "return" if plain && !m.shift => {
                if let Some(index) = self.map.focus {
                    self.apply_map_cluster(index, cx);
                }
            }
            "escape" => self.show_grid(cx),
            _ => {}
        }
        true
    }

    /// A press on the map. The move and release are followed at window
    /// level while it lasts (`views/map.rs`), so a release outside the map
    /// ends it.
    pub(crate) fn begin_map_drag(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.map.drag = Some((at, 0.0));
        self.map.dragged = false;
        cx.notify();
    }

    /// Pans by the pointer's move since the last event while the button is
    /// held; the poles never come inside the map, and east-west it wraps
    /// across the date line.
    pub(crate) fn drag_map(&mut self, at: Point<Pixels>, held: bool, cx: &mut Context<Self>) {
        let Some((last, travel)) = self.map.drag else {
            return;
        };
        if !held {
            self.end_map_drag(cx);
            return;
        }
        let (dx, dy) = (f32::from(at.x - last.x), f32::from(at.y - last.y));
        let travel = travel + dx.abs() + dy.abs();
        self.map.drag = Some((at, travel));
        if travel <= DRAG_SLOP {
            return;
        }
        self.map.dragged = true;
        if let Some(projection) = self.map.projection() {
            // From the clamped centre, so dragging back after hitting the
            // edge moves the map at once.
            let (lon, lat) = projection.to_degrees(projection.mid);
            self.map
                .set_center((lon - dx / projection.scale, lat + dy / projection.scale));
            cx.notify();
        }
    }

    pub(crate) fn end_map_drag(&mut self, cx: &mut Context<Self>) {
        if self.map.drag.take().is_some() {
            cx.notify();
        }
    }

    /// Replaces the points without a catalog read and clusters them (in
    /// the background), for the frame benchmark.
    #[cfg(test)]
    pub(crate) fn set_map_points_for_test(
        &mut self,
        points: Vec<GeoPoint>,
        cx: &mut Context<Self>,
    ) {
        self.map.points = Some(Arc::new(points));
        // Hold off the next catalog read, which would replace them.
        if let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) {
            self.map.loaded = Some((self.library_generation, engine.data_revision()));
        }
        self.recluster_map(false, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn projection_centres_and_fits_the_world() {
        let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(720.0), px(500.0)));
        let p = Projection::new(bounds, 1, (0.0, 0.0));
        assert_eq!(p.scale, 2.0, "zoom 1 fits 360° into the width");
        assert_eq!(p.to_screen(0.0, 0.0), bounds.center());
        assert_eq!(p.to_screen(-180.0, 90.0), point(px(10.0), px(90.0)));
        assert_eq!(p.to_screen(180.0, -90.0), point(px(730.0), px(450.0)));
        let z3 = Projection::new(bounds, 3, (135.0, 35.0));
        assert_eq!(z3.scale, 8.0, "each step doubles");
        assert_eq!(
            z3.to_screen(135.0, 35.0),
            bounds.center(),
            "the centre is the middle"
        );
        assert_eq!(
            z3.basemap_offset(),
            bounds.center() - point(px(315.0 * 8.0), px(55.0 * 8.0)),
            "the world's 180°W, 90°N corner, 315° west and 55° north of the centre"
        );
        assert_eq!(z3.to_degrees(bounds.center()), (135.0, 35.0));
    }

    #[core::prelude::v1::test]
    fn zoom_one_centres_exactly_at_any_width() {
        for width in 400..1600 {
            let map = size(px(width as f32), px(width as f32));
            assert_eq!(
                clamp_center((5.0, 3.0), map, 1),
                (0.0, 0.0),
                "at zoom 1 the world fits, centred, at width {width}"
            );
            // A wide map: the height axis is the one that fits exactly.
            let wide = size(px(width as f32 * 3.0), px(width as f32));
            assert_eq!(
                clamp_center((5.0, 3.0), wide, 1).1,
                0.0,
                "at zoom 1 the latitude is centred on a wide map, height {width}"
            );
        }
    }

    #[core::prelude::v1::test]
    fn centre_is_clamped_so_the_world_covers_the_map() {
        let map = size(px(720.0), px(500.0));
        assert_eq!(
            clamp_center((120.0, 60.0), map, 1),
            (0.0, 0.0),
            "at zoom 1 the world fits and the centre is fixed"
        );
        // Zoom 2: 4 px/°, the world is 1440 × 720 px.
        assert_eq!(
            clamp_center((170.0, 80.0), map, 2),
            (170.0, 27.5),
            "the north edge stops at the map's edge; east-west is free"
        );
        assert_eq!(clamp_center((-30.0, -10.0), map, 2), (-30.0, -10.0));
        assert_eq!(
            clamp_center((190.0, 0.0), map, 2),
            (-170.0, 0.0),
            "past 180°E the centre wraps to the west"
        );
    }

    /// Regression: the map and the location card stopped at the date line,
    /// so a place at 179°E next to one at 179°W was drawn a world apart.
    #[core::prelude::v1::test]
    fn projection_wraps_across_the_date_line() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(720.0), px(500.0)));
        // Zoom 3: 8 px/°, the world is 2880 px wide.
        let p = Projection::new(bounds, 3, (179.0, 0.0));
        let mid = bounds.center();
        assert_eq!(
            p.to_screen(-179.0, 0.0),
            point(mid.x + px(16.0), mid.y),
            "179°W is drawn 2° east of 179°E, not a world away"
        );
        assert_eq!(p.to_screen(179.0, 0.0), mid);
        let copies: Vec<_> = p.basemap_offsets(bounds.size.width).collect();
        assert_eq!(
            copies,
            vec![
                p.basemap_offset(),
                point(p.basemap_offset().x + px(2880.0), p.basemap_offset().y)
            ],
            "both sides of the date line have land drawn"
        );

        let inland = Projection::new(bounds, 3, (0.0, 0.0));
        assert_eq!(
            inland.basemap_offsets(bounds.size.width).count(),
            1,
            "away from the date line the world is drawn once"
        );
        let world = Projection::new(bounds, 1, (0.0, 0.0));
        assert_eq!(
            world.basemap_offsets(bounds.size.width).count(),
            1,
            "a world that fits the map is drawn once"
        );
    }
}
