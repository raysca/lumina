//! An open album's Grid fills square tiles,
//! five columns by default, 10 apart, inside 28 left / 22 top padding.
//! Library keeps its own zoom, gaps and Fit toggle.

use super::album_page::open_album;
use super::helpers::{app_with_library, focus_app, keystrokes, leak, render_ui, seed_library};
use crate::app::command_palette::CommandId;
use crate::app::LuminaApp;
use crate::config::BrowseLayout;
use crate::views::grid::CARD_CONTAIN;
use crate::views::sidebar::AlbumSelection;
use gpui::{Bounds, Entity, Pixels, VisualTestContext};
use lumina_core::MediaId;
use std::sync::Arc;

/// `n` photos, all in one album.
fn library_with_album(
    prefix: &str,
    n: usize,
) -> (
    super::helpers::TempLibrary,
    Arc<lumina_core::LuminaEngine>,
    i64,
) {
    let (dir, engine) = seed_library(prefix, n);
    let ids: Vec<MediaId> = engine.get_item_slice(0..n).iter().map(|i| i.id).collect();
    let album = engine.create_album_with("Trip", &ids).unwrap();
    (dir, engine, album.id)
}

fn bounds(vcx: &mut VisualTestContext, selector: String) -> Bounds<Pixels> {
    vcx.debug_bounds(leak(selector.clone()))
        .unwrap_or_else(|| panic!("{selector} must be painted"))
}

/// Library at 4 columns in Grid with the Fit toggle on, then the album.
fn open(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, album: i64) {
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        app.card_fit_mode = true;
        app.config.browse_layout = BrowseLayout::Grid;
        app.set_sort_field(lumina_core::SortField::DateTaken, cx);
    });
    open_album(view, vcx, album);
}

/// Item ids in display order.
fn shown_ids(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, n: usize) -> Vec<usize> {
    view.update(vcx, |app, _| {
        app.displayed_rows(0..n)
            .iter()
            .map(|i| i.id.0 as usize)
            .collect()
    })
}

fn near(a: Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() <= 0.5
}

#[core::prelude::v1::test]
fn album_grid_has_five_filled_columns_ten_apart_inside_the_frame_padding() {
    let (_dir, engine, album) = library_with_album("album_grid_frame", 12);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx, album);
    let ids = shown_ids(&view, vcx, 12);
    assert_eq!(ids.len(), 12, "the album's photos are shown");
    // A texture for every tile, so the fit hook records each one.
    let image = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(300, 200),
    )]));
    view.update(vcx, |app, cx| {
        let state = app.grid_state.as_ref().expect("grid state");
        for &id in &ids {
            state
                .texture_cache
                .lock()
                .put(MediaId(id as _), image.clone());
        }
        cx.notify();
    });
    render_ui(vcx);

    let tiles: Vec<_> = ids
        .iter()
        .map(|id| bounds(vcx, format!("media-card-photo-{id}")))
        .collect();
    for (i, tile) in tiles.iter().enumerate().take(5).skip(1) {
        assert_eq!(
            tile.top(),
            tiles[0].top(),
            "tile {i} is on the first row: 5 columns"
        );
        assert!(
            near(tile.left() - tiles[i - 1].right(), 10.0),
            "tiles are 10 apart across: {:?} after {:?}",
            tile,
            tiles[i - 1]
        );
    }
    assert!(
        tiles[5].top() > tiles[0].top(),
        "the sixth tile wraps: 5 columns"
    );
    assert_eq!(
        tiles[5].left(),
        tiles[0].left(),
        "the sixth tile starts row two"
    );
    assert!(
        near(tiles[5].top() - tiles[0].bottom(), 10.0),
        "rows are 10 apart: {:?} under {:?}",
        tiles[5],
        tiles[0]
    );
    assert!(
        near(tiles[0].size.width - tiles[0].size.height, 0.0),
        "tiles are square: {:?}",
        tiles[0]
    );

    // Date sorted (the default), yet no month band: the frame has none.
    assert!(
        vcx.debug_bounds("grid-month-header").is_none(),
        "an album's Grid paints no month band"
    );
    let surface = bounds(vcx, "media-grid-surface".into());
    assert!(
        near(tiles[0].left() - surface.left(), 28.0),
        "the grid's left padding is 28: {:?} in {surface:?}",
        tiles[0]
    );
    assert!(
        near(tiles[0].top() - surface.top(), 22.0),
        "the grid's top padding is 22: {:?} in {surface:?}",
        tiles[0]
    );

    // Fill, not the Library's Fit: the image covers the whole tile.
    let id = ids[0];
    let thumb = bounds(vcx, format!("media-card-thumb-{id}"));
    assert_eq!(thumb, tiles[0], "the image's box is the tile's");
    assert_eq!(
        CARD_CONTAIN.with(|m| m.borrow().get(&id).copied()),
        Some(false),
        "an album tile cover-fits even with the Fit toggle on"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.command_applies(CommandId::ToggleFit),
            "the Fit toggle is not offered on an album page"
        );
    });
}

#[core::prelude::v1::test]
fn album_zoom_and_arrows_use_album_columns_and_leave_library_zoom_alone() {
    let (_dir, engine, album) = library_with_album("album_grid_zoom", 12);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx, album);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| app.select_index(0, cx));
    render_ui(vcx);
    keystrokes(vcx, "down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(5),
            "↓ moves one row: 5 on an album page"
        );
    });

    view.update(vcx, |app, cx| app.zoom_out_grid(cx));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_cols(),
            6,
            "zoom out from the album's 5 is the ladder's 6"
        );
        app.albums.page.columns = None;
    });
    view.update(vcx, |app, cx| app.zoom_in_grid(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_cols(),
            4,
            "zoom in from the album's 5 is the ladder's 4"
        );
        assert_eq!(app.grid_columns, 4, "Library's zoom is untouched");
    });

    // Back in Library: its 4 columns, its 6 gap and the Fit toggle.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_cols(), 4, "Library lays out its own 4 columns");
        assert!(
            app.command_applies(CommandId::ToggleFit),
            "the Fit toggle is offered in Library"
        );
    });
    let ids = shown_ids(&view, vcx, 2);
    let image = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(300, 200),
    )]));
    view.update(vcx, |app, cx| {
        let state = app.grid_state.as_ref().expect("grid state");
        for &id in &ids {
            state
                .texture_cache
                .lock()
                .put(MediaId(id as _), image.clone());
        }
        cx.notify();
    });
    render_ui(vcx);
    let (a, b) = (
        bounds(vcx, format!("media-card-photo-{}", ids[0])),
        bounds(vcx, format!("media-card-photo-{}", ids[1])),
    );
    assert!(
        near(b.left() - a.right(), 6.0),
        "Library tiles stay 6 apart"
    );
    assert_eq!(
        CARD_CONTAIN.with(|m| m.borrow().get(&ids[0]).copied()),
        Some(true),
        "Library still letterboxes with Fit on"
    );

    // The album reopens at its own zoom.
    open_album(&view, vcx, album);
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_cols(), 4, "the album keeps its zoom this session");
    });
}
