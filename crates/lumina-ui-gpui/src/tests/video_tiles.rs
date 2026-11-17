//! The Videos view's Grid draws 16:9 thumbs with a
//! two-line caption; Mosaic keeps its true-aspect rows.

use super::ui_automation::{app_with_library, leak, render_ui, seed_library};
use crate::app::LuminaApp;
use crate::config::BrowseLayout;
use crate::views::sidebar::AlbumSelection;
use gpui::{Bounds, Entity, Pixels, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId, MediaType, SystemAlbum};
use std::sync::Arc;

/// `n` square 1000x1000 clips beside the seed's photo, as rows only (no
/// file, so no watcher rescan resets them). Clip 0 has a long title.
pub(super) fn library_with_clips(
    prefix: &str,
    n: usize,
) -> (
    super::downloads::TempLibrary,
    Arc<LuminaEngine>,
    Vec<MediaId>,
) {
    let (dir, engine) = seed_library(prefix, 1);
    let tpl = engine.get_item_slice(0..1)[0].clone();
    let clips: Vec<_> = (0..n)
        .map(|i| {
            let mut clip = tpl.clone();
            clip.file_name = format!("clip_{i:02}.mov");
            clip.path = dir.join(&clip.file_name);
            clip.media_type = MediaType::Video;
            clip.duration_ms = Some(78_000);
            (clip.width, clip.height) = (1000, 1000);
            clip.title = (i == 0).then(|| {
                "Ferry crossing at dusk with the whole family on the upper deck".to_string()
            });
            clip
        })
        .collect();
    let stored = engine.catalog().insert_or_update_batch(&clips).unwrap();
    let ids = stored.iter().map(|clip| clip.id).collect();
    (super::downloads::TempLibrary(dir), engine, ids)
}

fn bounds(vcx: &mut VisualTestContext, selector: String) -> Bounds<Pixels> {
    vcx.debug_bounds(leak(selector.clone()))
        .unwrap_or_else(|| panic!("{selector} must be painted"))
}

pub(super) fn open_videos(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        app.select_album(AlbumSelection::System(SystemAlbum::Videos), cx)
    });
    render_ui(vcx);
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn videos_view_uses_wide_captioned_tiles() {
    let (_dir, engine, clips) = library_with_clips("vt_wide", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    open_videos(&view, vcx);

    let first = clips[0].0;
    let photo = bounds(vcx, format!("media-card-photo-{first}"));
    let (w, h) = (f32::from(photo.size.width), f32::from(photo.size.height));
    assert!(
        (w * 9.0 / 16.0 - h).abs() <= 1.0,
        "a Videos thumb is 16:9 (±1px), got {w}x{h}"
    );
    let caption = bounds(vcx, format!("media-card-caption-{first}"));
    assert!(
        caption.top() >= photo.bottom() && caption.left() >= photo.left(),
        "the caption sits under the thumb: {caption:?} vs {photo:?}"
    );
    let title = bounds(vcx, format!("media-card-caption-title-{first}"));
    let meta = bounds(vcx, format!("media-card-caption-meta-{first}"));
    assert!(
        title.size.width <= photo.size.width + gpui::px(0.5)
            && title.right() <= photo.right() + gpui::px(0.5),
        "a long title truncates within the thumb's width: {title:?} vs {photo:?}"
    );
    assert!(
        f32::from(title.size.height) < 20.0,
        "and stays one line: {title:?}"
    );
    assert!(meta.top() >= title.bottom(), "meta sits under the title");

    // Every row has the same pitch, and it is the one navigation, the rail
    // and the zoom anchor read (`row_height`).
    let next_row = bounds(vcx, format!("media-card-photo-{}", clips[4].0));
    // Clips sort newest id first, so clip 4 is a row above clip 0.
    let pitch = f32::from(photo.top() - next_row.top()).abs();
    view.update(vcx, |app, _| {
        assert!(
            (pitch - app.row_height()).abs() < 0.5,
            "the painted row pitch {pitch} must equal row_height {}",
            app.row_height()
        );
    });

    // Selection: the outline traces the thumb itself; the title turns accent.
    view.update(vcx, |app, cx| {
        let index = (0..app.get_active_count())
            .find(|&i| app.get_active_slice(i..i + 1)[0].id.0 == first)
            .expect("clip 0 is in Videos");
        app.select_index(index, cx)
    });
    render_ui(vcx);
    let ring = bounds(vcx, format!("media-card-selected-{first}"));
    assert!(
        (ring.top() - photo.top()).abs() < gpui::px(0.5)
            && (ring.size.height - photo.size.height).abs() < gpui::px(0.5),
        "the selection outline sits on the thumb, not around the caption: {ring:?} vs {photo:?}"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-caption-title-selected-{first}")))
            .is_some(),
        "the selected tile's title is drawn in accent"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-caption-title-{first}")))
            .is_some(),
        "the accent marker does not take over the title's own selector"
    );

    // All Media keeps square wells with no caption.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    let photo = bounds(vcx, format!("media-card-photo-{}", clips[1].0));
    assert!(
        (photo.size.width - photo.size.height).abs() <= gpui::px(1.0),
        "All Media keeps the square well: {photo:?}"
    );
    view.update(vcx, |app, _| {
        assert!(!app.captioned_tiles(), "no captions outside Videos");
    });
}

#[core::prelude::v1::test]
fn videos_mosaic_keeps_true_aspect_rows() {
    let (_dir, engine, clips) = library_with_clips("vt_mosaic", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    view.update(vcx, |app, _| {
        app.config.browse_layout = BrowseLayout::Mosaic
    });
    open_videos(&view, vcx);
    render_ui(vcx);
    let first = clips[0].0;
    let photo = bounds(vcx, format!("media-card-photo-{first}"));
    assert!(
        (photo.size.width - photo.size.height).abs() <= gpui::px(1.0),
        "a square clip stays square in the Videos Mosaic, not 16:9: {photo:?}"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-caption-{first}")))
            .is_none(),
        "Mosaic tiles carry no caption"
    );
}

#[core::prelude::v1::test]
fn short_video_tiles_keep_their_badges() {
    let (_dir, engine, clips) = library_with_clips("vt_badges", 2);
    let clip = clips[0];
    engine
        .set_flags(&[(clip, lumina_core::Flag::Pick)])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // At 1180 wide a 4-column 16:9 thumb is about 120px high, under the
    // square COMPACT_TILE side, yet wide enough for its badge rows.
    vcx.simulate_resize(gpui::size(gpui::px(1180.0), gpui::px(800.0)));
    open_videos(&view, vcx);
    let photo = bounds(vcx, format!("media-card-photo-{}", clip.0));
    assert!(
        f32::from(photo.size.height) < crate::views::grid::COMPACT_TILE
            && f32::from(photo.size.width) >= crate::views::grid::COMPACT_TILE,
        "the case under test: short but wide, got {photo:?}"
    );
    view.update(vcx, |app, cx| {
        app.edits.edited_ids.insert(clip);
        app.missing_items.insert(clip);
        cx.notify();
    });
    render_ui(vcx);
    let id = clip.0;
    for selector in [
        format!("grid-edited-badge-{id}"),
        format!("grid-flag-badge-{id}"),
        format!("grid-missing-badge-{id}"),
        format!("media-card-duration-{id}"),
    ] {
        assert!(
            vcx.debug_bounds(leak(selector.clone())).is_some(),
            "{selector}: a short wide video tile keeps its badges"
        );
    }
}

#[core::prelude::v1::test]
fn arrow_keys_reveal_captioned_rows() {
    let (_dir, engine, _clips) = library_with_clips("vt_keys", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(800.0)));
    open_videos(&view, vcx);
    view.update(vcx, |app, cx| app.select_index(0, cx));
    super::ui_automation::focus_app(vcx, &view);
    for _ in 0..9 {
        super::ui_automation::keystrokes(vcx, "down");
        render_ui(vcx);
    }
    render_ui(vcx);
    let (index, id) = view.update(vcx, |app, _| {
        let index = app.selected_index.expect("a selection");
        (index, app.get_active_slice(index..index + 1)[0].id.0)
    });
    assert_eq!(index, 36, "nine rows down at four columns");
    let grid = vcx.debug_bounds("media-grid").expect("the grid");
    let photo = bounds(vcx, format!("media-card-photo-{id}"));
    let caption = bounds(vcx, format!("media-card-caption-{id}"));
    assert!(
        photo.top() >= grid.top() && caption.bottom() <= grid.bottom() + gpui::px(0.5),
        "the selected tile and its caption are scrolled into view: {photo:?} {caption:?} in {grid:?}"
    );
}

#[core::prelude::v1::test]
fn portrait_clips_cover_fit_and_the_fit_toggle_contains_them() {
    use crate::views::grid::CARD_CONTAIN;
    let (_dir, engine, clips) = library_with_clips("vt_fit", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    open_videos(&view, vcx);
    let tall = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(200, 2000),
    )]));
    // A fresh clip per mode: debug_bounds keeps an element's first-frame
    // entry, so only a thumbnail painted for the first time shows its layout.
    for (contain, clip) in [(false, clips[0]), (true, clips[1])] {
        view.update(vcx, |app, cx| {
            app.card_fit_mode = contain;
            let state = app.grid_state.as_ref().expect("grid state");
            state.texture_cache.lock().put(clip, tall.clone());
            cx.notify();
        });
        render_ui(vcx);
        let photo = bounds(vcx, format!("media-card-photo-{}", clip.0));
        let thumb = bounds(vcx, format!("media-card-thumb-{}", clip.0));
        // The box fills the well in both modes; only the CARD_CONTAIN hook
        // below tells cover from contain.
        assert!(
            (thumb.size.width - photo.size.width).abs() < gpui::px(1.0)
                && (thumb.size.height - photo.size.height).abs() < gpui::px(1.0),
            "contain={contain}: a portrait thumbnail fills the 16:9 well's box: {thumb:?} vs {photo:?}"
        );
        assert_eq!(
            CARD_CONTAIN.with(|m| m.borrow().get(&(clip.0 as usize)).copied()),
            Some(contain),
            "a portrait clip cover-fits; the fit toggle switches it to contain"
        );
    }
}
