//! Offline map (plan 5 C4): Places shows a Grid/Map toggle. The map draws
//! the bundled land outline and clusters of geotagged items; Tab and the
//! arrows focus clusters west to east, ↵ searches `within:` the focused
//! cluster, Esc returns to the grid, and ⌘= / ⌘- zoom the map, never the
//! hidden grid.

use std::time::Instant;

use crate::app::map::{clamp_center, parse_basemap, Projection, MAX_ZOOM};
use crate::tests::culling::ids;
use crate::tests::duplicates::run_palette_command;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::{GeoPoint, LuminaEngine, MediaId, SystemAlbum};

const PLACES: AlbumSelection = AlbumSelection::System(SystemAlbum::Places);

fn cluster_selector(index: usize) -> &'static str {
    leak(format!("map-cluster-{index}"))
}

/// Three items near Kyoto and two in Paris; the rest have no location.
fn seed_gps(engine: &LuminaEngine) -> Vec<MediaId> {
    let mut ids: Vec<MediaId> = engine
        .catalog()
        .get_item_slice(0, 100)
        .expect("seeded rows")
        .into_iter()
        .map(|item| item.id)
        .collect();
    ids.sort_by_key(|id| id.0);
    let spots = [
        (35.01, 135.76),
        (35.02, 135.77),
        (35.03, 135.78),
        (48.85, 2.35),
        (48.86, 2.36),
    ];
    for (id, (lat, lon)) in ids.iter().zip(spots) {
        engine
            .set_location(*id, Some(lat), Some(lon))
            .expect("test location write");
    }
    ids
}

fn show_map(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    run_palette_command(vcx, view, "Show Map");
    render_ui(vcx);
    render_ui(vcx);
}

/// The map's centre is clamped, and the world's edge is at or beyond the
/// map's edge (or the world is centred along an axis where it is smaller).
fn assert_world_covers(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, when: &str) {
    view.update(vcx, |app, _| {
        let size = app.map.viewport.get();
        let (zoom, center) = (app.map.zoom, app.map.center);
        assert_eq!(
            clamp_center(center, size, zoom),
            center,
            "{when}: the centre is kept on the world"
        );
        let p = Projection::new(Bounds::new(Point::default(), size), zoom, center);
        let (nw, se) = (p.to_screen(-180.0, 90.0), p.to_screen(180.0, -90.0));
        let axis = |lo: Pixels, hi: Pixels, extent: Pixels, name: &str| {
            let eps = px(0.5);
            if hi - lo >= extent {
                assert!(
                    lo <= eps && hi >= extent - eps,
                    "{when}: the world covers the map {name}: {lo:?}..{hi:?} vs {extent:?}"
                );
            } else {
                assert!(
                    ((lo + hi) / 2.0 - extent / 2.0).abs() < eps,
                    "{when}: a world smaller than the map is centred {name}"
                );
            }
        };
        axis(nw.x, se.x, size.width, "across");
        axis(nw.y, se.y, size.height, "down");
    });
}

fn counts(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<usize> {
    view.update(vcx, |app, _| {
        app.map.clusters.iter().map(|c| c.count).collect()
    })
}

#[core::prelude::v1::test]
fn basemap_asset_parses() {
    let bytes = crate::assets::LAND_110M;
    assert!(
        bytes.len() <= 200 * 1024,
        "the bundled basemap stays within 200 KB, got {} bytes",
        bytes.len()
    );
    let rings = parse_basemap(bytes).expect("the bundled basemap parses");
    assert!(!rings.is_empty(), "the basemap has land rings");
    let points: usize = rings.iter().map(|r| r.points.len()).sum();
    assert_eq!(
        4 + rings.len() * 4 + points * 8,
        bytes.len(),
        "every byte is a header, a ring length or a point: the hole bit is \
         masked off the length"
    );
    for ring in &rings {
        assert!(ring.points.len() >= 3, "a ring is a polygon");
        for &(lon, lat) in &ring.points {
            assert!(
                (-180.0..=180.0).contains(&lon) && (-90.0..=90.0).contains(&lat),
                "coordinates are degrees in range, got ({lon}, {lat})"
            );
        }
    }
    let holes: Vec<_> = rings.iter().filter(|r| r.hole).collect();
    assert_eq!(
        holes.len(),
        1,
        "the one hole at this scale is the Caspian Sea"
    );
    assert!(
        holes[0]
            .points
            .iter()
            .all(|&(lon, lat)| (45.0..56.0).contains(&lon) && (36.0..48.0).contains(&lat)),
        "the hole ring lies on the Caspian Sea"
    );
    let basemap = crate::app::map::BASEMAP
        .as_ref()
        .expect("the land tessellates into a fill");
    assert!(!basemap.land.vertices.is_empty(), "land has triangles");
    assert!(
        basemap
            .holes
            .as_ref()
            .is_some_and(|h| !h.vertices.is_empty()),
        "the hole is its own fill, drawn in the water colour"
    );
    eprintln!(
        "basemap: {} land and {} hole vertices",
        basemap.land.vertices.len(),
        basemap.holes.as_ref().map_or(0, |h| h.vertices.len())
    );
    assert!(
        parse_basemap(&bytes[..bytes.len() - 3]).is_err(),
        "a truncated file is an error, not a short map"
    );
}

#[core::prelude::v1::test]
fn map_empty_state() {
    let (dir, engine) = seed_library("map_empty", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, PLACES, "Show Map opens Places");
        assert!(app.map_shown(), "and shows the map");
    });
    assert!(vcx.debug_bounds("map-view").is_some(), "the map is painted");
    assert!(
        vcx.debug_bounds("map-empty").is_some(),
        "with no geotagged item the map says so"
    );
    assert!(vcx.debug_bounds(cluster_selector(0)).is_none());
    assert!(counts(&view, vcx).is_empty());

    // Keys on an empty map do nothing and never reach the grid.
    keystrokes(vcx, "tab right enter");
    view.update(vcx, |app, _| {
        assert!(app.map_shown(), "↵ with no cluster stays on the map");
        assert_eq!(app.map.focus, None);
        assert!(app.search_query.is_empty());
        assert!(app.selected_indices.is_empty() && app.selected_item.is_none());
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_shows_cluster_counts_for_seeded_gps() {
    let (dir, engine) = seed_library("map_counts", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);

    assert_eq!(
        counts(&view, vcx),
        vec![2, 3],
        "Paris (west) then Kyoto; the item without a location is left out"
    );
    assert!(vcx.debug_bounds("map-empty").is_none());
    // The map holds no selection: ⌘A selects nothing a menu could act on.
    keystrokes(vcx, "cmd-a");
    view.update(vcx, |app, _| {
        assert!(
            app.selected_indices.is_empty() && app.selected_item.is_none(),
            "⌘A on the map must not select the hidden grid"
        )
    });
    let paris = vcx.debug_bounds(cluster_selector(0)).expect("Paris drawn");
    let kyoto = vcx.debug_bounds(cluster_selector(1)).expect("Kyoto drawn");
    assert!(
        paris.center().x < kyoto.center().x,
        "clusters are numbered west to east"
    );
    assert!(
        paris.center().y < kyoto.center().y,
        "Paris (48.9°N) is drawn north of Kyoto (35°N)"
    );
    assert!(
        vcx.debug_bounds("map-cluster-label-0-2").is_some()
            && vcx.debug_bounds("map-cluster-label-1-3").is_some(),
        "each circle shows its count"
    );

    // The window shrinks: the clusters follow the new map size.
    let map_before = vcx.debug_bounds("map-view").unwrap();
    vcx.simulate_resize(size(px(900.0), px(600.0)));
    render_ui(vcx);
    render_ui(vcx);
    let map = vcx
        .debug_bounds("map-view")
        .expect("the map survives a resize");
    assert!(
        map.size.width < map_before.size.width,
        "the map got narrower"
    );
    for i in 0..2 {
        let c = vcx.debug_bounds(cluster_selector(i)).unwrap().center();
        assert!(
            map.contains(&c),
            "cluster {i} stays inside the resized map: {c:?} not in {map:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn enter_on_focused_cluster_filters_grid() {
    let (dir, engine) = seed_library("map_enter", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);

    keystrokes(vcx, "tab");
    view.update(vcx, |app, _| {
        assert_eq!(app.map.focus, Some(0), "Tab focuses the westmost")
    });
    keystrokes(vcx, "tab");
    view.update(vcx, |app, _| {
        assert_eq!(app.map.focus, Some(1), "then the next east")
    });
    keystrokes(vcx, "shift-tab right left");
    view.update(vcx, |app, _| {
        assert_eq!(app.map.focus, Some(0), "⇧Tab and arrows move back")
    });
    keystrokes(vcx, "right");
    assert!(
        vcx.debug_bounds("map-cluster-focused").is_some(),
        "the focused cluster is marked"
    );

    keystrokes(vcx, "enter");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.starts_with("within:"),
            "↵ searches within the cluster, got {:?}",
            app.search_query
        );
        assert!(!app.map_shown(), "and returns to the grid");
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "a search covers the library"
        );
        assert_eq!(
            app.get_active_count(),
            3,
            "the grid holds the Kyoto cluster"
        );
    });
    assert!(vcx.debug_bounds("map-view").is_none());

    // Back returns to Places on the map.
    keystrokes(vcx, "secondary-[");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, PLACES);
        assert!(app.map_shown(), "Back returns to the map");
    });

    // A mouse click on a cluster does the same as ↵.
    click(vcx, cluster_selector(0));
    view.update(vcx, |app, _| {
        assert!(app.search_query.starts_with("within:"));
        assert_eq!(app.get_active_count(), 2, "the Paris pair");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_plus_zooms_map_not_grid() {
    let (dir, engine) = seed_library("map_zoom", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let columns = view.update(vcx, |app, _| app.grid_columns);
    show_map(&view, vcx);
    keystrokes(vcx, "tab tab");

    // Kyoto, the easternmost cluster: far from the middle at zoom 1.
    keystrokes(vcx, "secondary-=");
    view.update(vcx, |app, _| {
        assert_eq!(app.map.zoom, 2, "⌘= zooms the map");
        assert_eq!(
            app.grid_columns, columns,
            "and leaves the hidden grid alone"
        );
        let focused = app.map.focus.expect("focus survives the zoom");
        assert_eq!(app.map.clusters[focused].count, 3, "still on Kyoto");
    });
    assert_world_covers(&view, vcx, "zoom 2 on a far-east cluster");
    for _ in 0..10 {
        keystrokes(vcx, "secondary-=");
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.map.zoom, MAX_ZOOM, "zoom stops at {MAX_ZOOM}");
        assert_eq!(app.grid_columns, columns);
        let c = app.map.clusters[app.map.focus.expect("focus kept")];
        assert_eq!(c.count, 3);
        assert_eq!(
            app.map.center,
            (c.lon, c.lat),
            "zoomed in, the focused cluster is centred"
        );
    });
    let map = vcx.debug_bounds("map-view").unwrap().center();
    let focused = vcx
        .debug_bounds("map-cluster-focused")
        .expect("the focused cluster is drawn")
        .center();
    assert!(
        (map.x - focused.x).abs() < px(1.5) && (map.y - focused.y).abs() < px(1.5),
        "the focused cluster sits in the middle of the map: {focused:?} vs {map:?}"
    );
    assert_world_covers(&view, vcx, "zoom 6");

    for zoom in (1..MAX_ZOOM).rev() {
        keystrokes(vcx, "secondary--");
        assert_world_covers(&view, vcx, leak(format!("zoomed out to {zoom}")));
    }
    keystrokes(vcx, "secondary--");
    view.update(vcx, |app, _| {
        assert_eq!(app.map.zoom, 1, "⌘- stops at 1");
        assert_eq!(
            app.map.center,
            (0.0, 0.0),
            "at zoom 1 the world fits, centred"
        );
        assert_eq!(app.grid_columns, columns);
    });

    // Leaving the map: ⌘= is the grid's again.
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.map_shown(), "Esc returns to the grid")
    });
    keystrokes(vcx, "secondary-=");
    view.update(vcx, |app, _| {
        assert_ne!(
            app.grid_columns, columns,
            "on the grid ⌘= changes the density"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_toggle_reachable_from_palette() {
    let (dir, engine) = seed_library("map_toggle", 4);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let shown = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.map_shown())
    };

    show_map(&view, vcx);
    assert!(shown(&view, vcx), "⌘K Show Map from All Photos");
    view.update(vcx, |app, _| assert_eq!(app.selection, PLACES));
    run_palette_command(vcx, &view, "Show Grid");
    assert!(!shown(&view, vcx), "⌘K Show Grid");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, PLACES, "still Places")
    });

    // View > Show Map.
    vcx.dispatch_action(crate::actions::ShowMap);
    render_ui(vcx);
    assert!(shown(&view, vcx), "View > Show Map");

    // The sidebar toggle, by mouse.
    click(vcx, "map-toggle");
    assert!(!shown(&view, vcx), "the toggle shows the grid");
    click(vcx, "map-toggle");
    render_ui(vcx);
    assert!(shown(&view, vcx), "and the map again");
    view.update(vcx, |app, _| assert_eq!(app.selection, PLACES));

    // Leaving Places and coming back keeps the choice.
    click(vcx, "sidebar-nav-all");
    assert!(
        vcx.debug_bounds("map-view").is_none(),
        "other views show the grid"
    );
    assert!(
        vcx.debug_bounds("map-toggle").is_none(),
        "the toggle lives on Places"
    );
    click(vcx, "sidebar-nav-places");
    render_ui(vcx);
    assert!(shown(&view, vcx), "Places comes back on the map");
    assert!(vcx.debug_bounds("map-view").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_keys_stay_in_palette_and_search_field() {
    let (dir, engine) = seed_library("map_modal_keys", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);
    keystrokes(vcx, "tab");
    let state = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| (app.map.zoom, app.map.focus, app.map_shown()))
    };
    let before = state(&view, vcx);

    // The palette is open: its keys never move or zoom the map.
    keystrokes(vcx, "secondary-k");
    keystrokes(vcx, "secondary-= tab right down");
    assert_eq!(
        state(&view, vcx),
        before,
        "palette keys stay in the palette"
    );
    keystrokes(vcx, "escape");
    assert_eq!(
        state(&view, vcx),
        before,
        "Esc closes the palette, not the map"
    );

    // The search field: typed characters are text, not map keys.
    keystrokes(vcx, "secondary-f");
    keystrokes(vcx, "= - right");
    view.update(vcx, |app, _| {
        assert!(app.search_focused);
        assert_eq!(app.map.zoom, before.0, "typed = and - are text");
        assert_eq!(app.map.focus, before.1);
    });
    keystrokes(vcx, "secondary-=");
    view.update(vcx, |app, _| {
        assert_eq!(app.map.zoom, before.0, "⌘= in the field is not a map zoom")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_follows_locations_and_library_switch() {
    let (dir, engine) = seed_library("map_live", 3);
    let (other, other_engine) = seed_library("map_live_other", 2);
    other_engine
        .set_location(
            other_engine.catalog().get_item_slice(0, 1).unwrap()[0].id,
            Some(-33.87),
            Some(151.21),
        )
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let all = ids(&view, vcx);
    show_map(&view, vcx);
    assert!(vcx.debug_bounds("map-empty").is_some());

    // An item gains a location (an edit, a rescan): the map catches up.
    engine
        .set_location(all[0], Some(51.5), Some(-0.12))
        .unwrap();
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(counts(&view, vcx), vec![1], "the new location shows");
    assert!(vcx.debug_bounds("map-empty").is_none());
    // Near the antimeridian and a pole: drawn inside the map.
    engine
        .set_location(all[1], Some(-16.5), Some(179.9))
        .unwrap();
    engine
        .set_location(all[2], Some(89.9), Some(-179.9))
        .unwrap();
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(counts(&view, vcx).len(), 3);
    let map = vcx.debug_bounds("map-view").unwrap();
    for i in 0..3 {
        let c = vcx.debug_bounds(cluster_selector(i)).unwrap().center();
        assert!(map.contains(&c), "cluster {i} at the edge is on the map");
    }
    // ...and loses them again.
    for id in &all {
        engine.set_location(*id, None, None).unwrap();
    }
    render_ui(vcx);
    render_ui(vcx);
    assert!(
        counts(&view, vcx).is_empty(),
        "removed locations leave the map"
    );
    assert!(vcx.debug_bounds("map-empty").is_some());

    // Another library, opened while the map shows a cluster: the map
    // closes and shows that library's points next.
    engine
        .set_location(all[0], Some(51.5), Some(-0.12))
        .unwrap();
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(counts(&view, vcx), vec![1]);
    view.update(vcx, |app, cx| app.open_library_inner(other.clone(), cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.map.shown, "a new library opens on the grid");
        assert!(
            app.map.clusters.is_empty(),
            "no cluster of the old library is kept"
        );
    });
    focus_app(vcx, &view);
    show_map(&view, vcx);
    assert_eq!(counts(&view, vcx), vec![1], "the new library's point");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[core::prelude::v1::test]
fn map_frame_with_many_points() {
    let (dir, engine) = seed_library("map_perf", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let frame = |vcx: &mut VisualTestContext| {
        (0..3)
            .map(|_| {
                let start = Instant::now();
                render_ui(vcx);
                start.elapsed().as_secs_f64() * 1000.0
            })
            .fold(f64::MAX, f64::min)
    };
    let grid_ms = frame(vcx);
    show_map(&view, vcx);
    let empty_map_ms = frame(vcx);
    // 100k points spread over the world, injected past the catalog.
    let points: Vec<GeoPoint> = (0..100_000u64)
        .map(|i| GeoPoint {
            id: MediaId(i + 1),
            lat: ((i * 7919) % 17_000) as f32 / 100.0 - 85.0,
            lon: ((i * 104_729) % 35_800) as f32 / 100.0 - 179.0,
        })
        .collect();
    let start = Instant::now();
    view.update(vcx, |app, cx| {
        app.set_map_points_for_test(points.clone(), cx)
    });
    vcx.run_until_parked();
    let cluster_ms = start.elapsed().as_secs_f64() * 1000.0;
    let clusters = view.update(vcx, |app, _| app.map.clusters.len());
    render_ui(vcx);
    let frame_ms = frame(vcx);
    eprintln!(
        "map: 100k points -> {clusters} clusters in {cluster_ms:.1} ms; frame {frame_ms:.1} ms \
         (grid {grid_ms:.1} ms, map without clusters {empty_map_ms:.1} ms)"
    );
    assert!(clusters > 100, "the points spread over many clusters");
    // Debug budgets only: the release numbers go in the commit body.
    // Clustering runs on the background executor, so the UI never waits on
    // it, and under a full parallel test run its wall time swings well past
    // 1 s (~0.4 s alone). The budget catches a complexity blow-up, not load.
    assert!(
        cluster_ms < 10_000.0,
        "clustering 100k points took {cluster_ms:.1} ms"
    );
    assert!(frame_ms < 1000.0, "a map frame took {frame_ms:.1} ms");

    // Zoom 6: one cluster per occupied 0.5° cell, about the point count;
    // the frame visits only the visible band.
    let start = Instant::now();
    view.update(vcx, |app, cx| {
        app.map.zoom = MAX_ZOOM;
        app.set_map_points_for_test(points, cx)
    });
    vcx.run_until_parked();
    let cluster6_ms = start.elapsed().as_secs_f64() * 1000.0;
    let clusters6 = view.update(vcx, |app, _| app.map.clusters.len());
    render_ui(vcx);
    let frame6_ms = frame(vcx);
    eprintln!(
        "map zoom 6: {clusters6} clusters in {cluster6_ms:.1} ms (background); frame {frame6_ms:.1} ms"
    );
    assert!(clusters6 > 10_000, "zoom 6 splits the points finely");
    assert!(frame6_ms < 1000.0, "a zoom 6 frame took {frame6_ms:.1} ms");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn slideshow_from_map_plays_places() {
    let (dir, engine) = seed_library("map_slideshow", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);
    keystrokes(vcx, "alt-secondary-s");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_some(), "⌥⌘S on the map plays Places");
        assert_eq!(app.selected_index, Some(0), "from its first item");
        assert!(!app.map_shown(), "from the grid, which holds a selection");
        assert_eq!(app.selection, PLACES);
    });
    keystrokes(vcx, "escape");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_read_failure_offers_retry_and_reads_again() {
    use crate::app::map::MAP_LOAD_FAILS;
    let (dir, engine) = seed_library("map_read_fail", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    MAP_LOAD_FAILS.with(|f| f.set(true));
    show_map(&view, vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(crate::i18n::t("map.load_failed").as_str()),
            "a failed read says so in the banner"
        );
        assert!(app.error_retry.is_some(), "and offers Retry");
        assert!(app.map.points.is_none());
    });
    assert!(
        vcx.debug_bounds("map-error").is_some(),
        "the map shows the error, not a blank map"
    );
    // Not retried per frame: with the fault gone, frames alone read nothing.
    MAP_LOAD_FAILS.with(|f| f.set(false));
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.map.points.is_none(), "no read loop")
    });

    click(vcx, "error-banner-retry");
    render_ui(vcx);
    assert_eq!(counts(&view, vcx), vec![2, 3], "Retry reads again");
    assert!(vcx.debug_bounds("map-error").is_none());
    view.update(vcx, |app, _| assert!(app.error_message.is_none()));

    // Showing the map again also reads again after a failure.
    MAP_LOAD_FAILS.with(|f| f.set(true));
    click(vcx, "map-toggle");
    click(vcx, "map-toggle");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.map.failed && app.error_retry.is_some())
    });
    MAP_LOAD_FAILS.with(|f| f.set(false));
    keystrokes(vcx, "escape"); // the banner
    click(vcx, "map-toggle");
    click(vcx, "map-toggle");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.map.failed, "toggling reads again")
    });
    assert_eq!(counts(&view, vcx), vec![2, 3]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_drag_pans_and_clicks_need_no_drag() {
    use gpui::{Modifiers, MouseButton};
    let (dir, engine) = seed_library("map_drag", 6);
    seed_gps(&engine);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);
    keystrokes(vcx, "secondary-= secondary-=");
    let center = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.map.center)
    };
    let query = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.search_query.clone())
    };
    let none = Modifiers::default();
    let map = vcx.debug_bounds("map-view").unwrap();

    // A drag on the map pans it and searches nothing.
    let start = map.center();
    let before = center(&view, vcx);
    vcx.simulate_mouse_down(start, MouseButton::Left, none);
    render_ui(vcx);
    vcx.simulate_mouse_move(start + point(px(-40.0), px(30.0)), MouseButton::Left, none);
    render_ui(vcx);
    let moved = center(&view, vcx);
    assert!(
        moved.0 > before.0 && moved.1 > before.1,
        "dragging left and down shows what is east and north: {before:?} -> {moved:?}"
    );
    // The pointer leaves the map (over the sidebar) with the button held:
    // the drag goes on, and the release there ends it.
    let outside = point(map.origin.x - px(40.0), start.y + px(30.0));
    vcx.simulate_mouse_move(outside, MouseButton::Left, none);
    render_ui(vcx);
    let outside_center = center(&view, vcx);
    assert!(
        outside_center.0 > moved.0,
        "the drag continues outside the map"
    );
    vcx.simulate_mouse_up(outside, MouseButton::Left, none);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.map.drag.is_none(), "a release outside ends it")
    });
    vcx.simulate_mouse_move(start, None, none);
    render_ui(vcx);
    assert_eq!(center(&view, vcx), outside_center, "no jump on re-entry");
    assert!(query(&view, vcx).is_empty(), "a drag searches nothing");
    assert_world_covers(&view, vcx, "after a drag");

    // A drag that starts and ends on a cluster is a pan, not a click.
    // Focusing Kyoto brings it onto the map.
    keystrokes(vcx, "tab tab");
    let pin = vcx.debug_bounds(cluster_selector(1)).unwrap().center();
    let before = center(&view, vcx);
    vcx.simulate_mouse_down(pin, MouseButton::Left, none);
    render_ui(vcx);
    let end = pin + point(px(20.0), px(0.0));
    vcx.simulate_mouse_move(end, MouseButton::Left, none);
    render_ui(vcx);
    assert_ne!(center(&view, vcx), before, "the map moved");
    assert!(
        vcx.debug_bounds(cluster_selector(1))
            .unwrap()
            .contains(&end),
        "the cluster moved with the map and is under the pointer"
    );
    vcx.simulate_mouse_up(end, MouseButton::Left, none);
    render_ui(vcx);
    assert!(
        query(&view, vcx).is_empty(),
        "a drag ending on a cluster searches nothing"
    );
    view.update(vcx, |app, _| assert!(app.map_shown()));

    // A press that moves less than the slop is a click.
    let pin = vcx.debug_bounds(cluster_selector(1)).unwrap().center();
    vcx.simulate_mouse_down(pin, MouseButton::Left, none);
    render_ui(vcx);
    let nudge = pin + point(px(1.0), px(1.0));
    vcx.simulate_mouse_move(nudge, MouseButton::Left, none);
    render_ui(vcx);
    vcx.simulate_mouse_up(nudge, MouseButton::Left, none);
    render_ui(vcx);
    assert!(
        query(&view, vcx).starts_with("within:"),
        "a press under the slop on a cluster searches it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn map_zoom_buttons_zoom() {
    let (dir, engine) = seed_library("map_zoom_buttons", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    show_map(&view, vcx);
    let zoom = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.map.zoom)
    };
    click(vcx, "map-zoom-in");
    assert_eq!(zoom(&view, vcx), 2, "+ zooms in");
    click(vcx, "map-zoom-in");
    assert_eq!(zoom(&view, vcx), 3);
    click(vcx, "map-zoom-out");
    assert_eq!(zoom(&view, vcx), 2, "− zooms out");
    view.update(vcx, |app, _| {
        assert!(
            app.map_shown() && app.search_query.is_empty(),
            "a button click is not a drag or a search"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn show_map_waits_for_compare() {
    let (dir, engine) = seed_library("map_compare", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "cmd-a c");
    view.update(vcx, |app, _| {
        assert!(app.compare.is_some(), "compare is open")
    });
    vcx.dispatch_action(crate::actions::ShowMap);
    view.update(vcx, |app, cx| {
        app.show_map(cx);
        assert!(!app.map.shown, "View > Show Map does nothing under compare");
        assert_ne!(app.selection, PLACES, "nor moves the view behind it");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn late_points_from_previous_library_are_dropped() {
    use crate::app::map::MAP_LOAD_FAILS;
    let (dir, engine) = seed_library("map_late", 6);
    seed_gps(&engine);
    let (other, _other_engine) = seed_library("map_late_other", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.show_map(cx);
        // The read starts; the library changes before it lands.
        app.refresh_map_points(cx);
        app.open_library_inner(other.clone(), cx);
        // The new library's map starts its first read, which reuses the old
        // read's request number, so only the library generation tells them
        // apart. It fails, so the old read is the only one that could land
        // points.
        app.show_map(cx);
        MAP_LOAD_FAILS.with(|f| f.set(true));
        app.refresh_map_points(cx);
        MAP_LOAD_FAILS.with(|f| f.set(false));
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            app.map.points.is_none() && app.map.clusters.is_empty(),
            "the old library's points never land in the new one"
        );
        assert!(app.map.failed, "the new library's own read reported");
    });
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[core::prelude::v1::test]
fn zoom_during_first_read_clusters_at_the_new_zoom() {
    let (dir, engine) = seed_library("map_zoom_race", 2);
    let mut ids: Vec<MediaId> = engine
        .catalog()
        .get_item_slice(0, 10)
        .unwrap()
        .into_iter()
        .map(|item| item.id)
        .collect();
    ids.sort_by_key(|id| id.0);
    // 8° apart in longitude: one 16° cell at zoom 1, two 8° cells at zoom 2.
    engine.set_location(ids[0], Some(1.0), Some(1.0)).unwrap();
    engine.set_location(ids[1], Some(1.0), Some(9.0)).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.show_map(cx);
        app.refresh_map_points(cx);
        // The first read is still running when the zoom changes.
        app.zoom_map(1, cx);
    });
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| assert_eq!(app.map.zoom, 2));
    assert_eq!(
        counts(&view, vcx),
        vec![1, 1],
        "the clusters are zoom 2's, not the zoom 1 cell the read started at"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
