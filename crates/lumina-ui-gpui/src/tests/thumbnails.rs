//! Failed thumbnails: a card whose decode failed shows "No preview" instead
//! of the skeleton, and is not decoded again until it changes or a rescan.
//!
//! Decodes run on the engine's own thread pool, which `run_until_parked`
//! does not wait for, so these tests never wait on it. They drive the
//! failure through `on_thumbnail_result` (the decode callback's body) and
//! count requests with `THUMB_REQUESTS`.

use super::helpers::{
    focus_app, keystrokes, leak, render_ui, seed_library, seed_library_undecodable,
};
use crate::app::command_palette::CommandId;
use crate::app::state::{on_thumbnail_result, THUMB_DECODE_SIZES, THUMB_REQUESTS};
use crate::app::types::UiMessage;
use crate::tests::helpers::*;
use crate::views::grid::GridState;
use crate::LuminaApp;
use crate::*;
use gpui::{Entity, TestAppContext, VisualTestContext};
use lumina_core::{EngineEvent, LuminaEngine, MediaId};
use std::sync::Arc;

fn requests() -> usize {
    THUMB_REQUESTS.with(|n| n.get())
}

fn reset_requests() {
    THUMB_REQUESTS.with(|n| n.set(0));
}

/// Fails `id` the way the decode callback does when the pool answers `None`.
fn fail(state: &GridState, tx: &async_channel::Sender<UiMessage>, id: MediaId) {
    on_thumbnail_result(
        &state.thumb_failures,
        &state.in_flight,
        tx,
        id,
        crate::views::grid::THUMBNAIL_SIZE_MIN,
        None,
    );
}

/// Opens the app on `engine` with `failed` already failed before the first
/// draw, so no real decode is ever queued for them.
fn app_with_failed<'a>(
    cx: &'a mut TestAppContext,
    engine: Arc<LuminaEngine>,
    failed: &[MediaId],
) -> (Entity<LuminaApp>, &'a mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        let state = GridState::new(engine);
        for id in failed {
            fail(&state, &app.ui_tx, *id);
        }
        app.grid_state = Some(state);
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

fn is_failed(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, id: MediaId) -> bool {
    view.update(vcx, |app, _| {
        app.grid_state.as_ref().is_some_and(|s| s.is_failed(id))
    })
}

#[core::prelude::v1::test]
fn failed_decode_is_marked_once_and_not_rerequested() {
    let (_dir, engine) = seed_library_undecodable("thumb_failed", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    reset_requests();
    let (view, vcx) = app_with_failed(&mut cx, engine, &[id]);

    assert!(
        is_failed(&view, vcx, id),
        "a None decode marks the id failed"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-failed-{}", id.0)))
            .is_some(),
        "a failed thumbnail paints media-card-failed-<id>"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-failed-caption-{}", id.0)))
            .is_some(),
        "the failed card says so in text, not only in a tooltip"
    );
    assert_eq!(crate::i18n::t("grid.no_preview"), "No preview");

    // The failed well has its own tooltip; a click must still reach the card.
    super::helpers::click(vcx, leak(format!("media-card-failed-{}", id.0)));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(id),
            "clicking a failed card selects it like any other card"
        );
    });

    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(
        requests(),
        0,
        "a failed id must not be decoded again on any frame"
    );
}

#[core::prelude::v1::test]
fn failure_callback_notifies() {
    let (_dir, engine) = seed_library_undecodable("thumb_notify", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_failed(&mut cx, engine, &[]);

    let notified = std::rc::Rc::new(std::cell::Cell::new(0));
    let seen = notified.clone();
    vcx.update(|_, cx| {
        cx.observe(&view, move |_, _| seen.set(seen.get() + 1))
            .detach();
    });
    view.update(vcx, |app, cx| {
        // What the decode callback does on the pool, minus the send.
        app.grid_state.as_ref().unwrap().mark_failed(id);
        app.handle_ui_message(UiMessage::ThumbnailFailed, cx)
    });
    vcx.run_until_parked();
    assert!(
        notified.get() > 0,
        "ThumbnailFailed must repaint: nothing else redraws after a decode-pool failure"
    );
}

#[core::prelude::v1::test]
fn thumbnail_callback_marks_and_messages_on_none() {
    let failures = parking_lot::Mutex::new(std::collections::HashSet::new());
    let in_flight = parking_lot::Mutex::new(std::collections::HashSet::new());
    let (tx, rx) = async_channel::unbounded();
    let id = MediaId(7);

    in_flight.lock().insert(id);
    on_thumbnail_result(
        &failures,
        &in_flight,
        &tx,
        id,
        crate::views::grid::THUMBNAIL_SIZE_MIN,
        None,
    );
    assert!(failures.lock().contains(&id), "None marks the id failed");
    assert!(!in_flight.lock().contains(&id), "None ends the request");
    assert!(
        matches!(rx.try_recv(), Ok(UiMessage::ThumbnailFailed)),
        "None sends one ThumbnailFailed"
    );
    assert!(rx.try_recv().is_err(), "exactly one message per failure");

    let other = MediaId(8);
    in_flight.lock().insert(other);
    let frame = super::helpers::tiny_decoded_frame(other);
    on_thumbnail_result(
        &failures,
        &in_flight,
        &tx,
        other,
        crate::views::grid::THUMBNAIL_SIZE_MIN,
        Some(frame),
    );
    assert!(
        !failures.lock().contains(&other),
        "a frame never marks failed"
    );
    assert!(!in_flight.lock().contains(&other));
    assert!(
        matches!(rx.try_recv(), Ok(UiMessage::ThumbnailLoaded { id: got, .. }) if got == other),
        "a frame is sent as ThumbnailLoaded"
    );
}

#[core::prelude::v1::test]
fn conversion_failure_is_marked() {
    let (_dir, engine) = seed_library_undecodable("thumb_convert", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_failed(&mut cx, engine, &[]);

    // 2x2 RGBA needs 16 bytes; 3 cannot become an image.
    let frame = lumina_core::DecodedFrame {
        id,
        width: 2,
        height: 2,
        format: lumina_core::PixelFormat::Rgba8,
        data: vec![0; 3],
    };
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            UiMessage::ThumbnailLoaded {
                id,
                frame,
                requested_size: crate::views::grid::THUMBNAIL_SIZE_MIN,
            },
            cx,
        )
    });
    render_ui(vcx);
    assert!(
        is_failed(&view, vcx, id),
        "a frame that cannot be converted is a failed thumbnail"
    );
    assert!(vcx
        .debug_bounds(leak(format!("media-card-failed-{}", id.0)))
        .is_some());
}

#[core::prelude::v1::test]
fn rescan_clears_failures() {
    let (_dir, engine) = seed_library("thumb_rescan", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_failed(&mut cx, engine, &[id]);

    // A real photo, so the retried decode succeeds rather than failing
    // again before the assertion.
    assert!(is_failed(&view, vcx, id), "starts failed");
    keystrokes(vcx, "secondary-r");
    assert!(!is_failed(&view, vcx, id), "⌘R retries failed thumbnails");

    view.update(vcx, |app, cx| {
        let tx = app.ui_tx.clone();
        fail(app.grid_state.as_ref().unwrap(), &tx, id);
        assert!(app.grid_state.as_ref().unwrap().is_failed(id));
        app.execute_command(CommandId::Rescan, cx);
    });
    assert!(
        !is_failed(&view, vcx, id),
        "the palette's Rescan retries failed thumbnails"
    );
}

#[core::prelude::v1::test]
fn reupserted_item_is_retried() {
    let (_dir, engine) = seed_library_undecodable("thumb_reupsert", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let id = item.id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_failed(&mut cx, engine, &[id]);
    assert!(is_failed(&view, vcx, id));

    reset_requests();
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items: vec![item] }),
            cx,
        )
    });
    // This undecodable fixture fails in process at once (no native tool to
    // wait on), so the retry can mark it failed again before any check of
    // the card; the decode count proves the retry.
    render_ui(vcx);
    assert_eq!(requests(), 1, "the retried card is decoded once");
}

#[core::prelude::v1::test]
fn failed_card_keeps_card_size() {
    let (_dir, engine) = seed_library_undecodable("thumb_size", 2);
    let items = engine.get_item_slice(0..2);
    let (failed, pending) = (items[0].id, items[1].id);
    let mut cx = crate::tests::TestApp::single();
    // The second card is still decoding (its decode claimed before the first
    // draw); the first has failed. Either way the first ends failed: its own
    // decode of the fake file fails too.
    let (view, vcx) = super::helpers::app_with_pending(&mut cx, engine, &[pending]);
    view.update(vcx, |app, _| {
        let tx = app.ui_tx.clone();
        fail(app.grid_state.as_ref().unwrap(), &tx, failed);
    });
    render_ui(vcx);

    let failed_bounds = vcx
        .debug_bounds(leak(format!("media-card-failed-{}", failed.0)))
        .expect("the failed card paints");
    let skeleton_bounds = vcx
        .debug_bounds(leak(format!("media-card-skeleton-{}", pending.0)))
        .expect("the undecoded card paints its skeleton");
    assert_eq!(
        failed_bounds.size, skeleton_bounds.size,
        "a failed card must fill the same photo well as a skeleton"
    );
}

#[core::prelude::v1::test]
fn library_switch_starts_with_no_failures() {
    let (_dir, engine) = seed_library_undecodable("thumb_switch", 1);
    let (other, _) = seed_library_undecodable("thumb_switch_other", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_failed(&mut cx, engine, &[id]);

    view.update(vcx, |app, cx| {
        // Start a real decode in the old library, then switch before it lands.
        let old = app.grid_state.as_ref().unwrap();
        old.clear_failed(id);
        let old_failures = old.thumb_failures.clone();
        let callback = app
            .begin_thumbnail_request(
                app.grid_state.as_ref().unwrap(),
                id,
                crate::views::grid::THUMBNAIL_SIZE_MIN,
            )
            .expect("the cleared id can be requested again");
        // As the background open lands, in this update.
        super::helpers::install_library(app, other.clone(), cx);
        assert!(
            !app.grid_state.as_ref().unwrap().is_failed(id),
            "a new library starts with no failures"
        );
        // Ids restart per library, so the late answer's id collides.
        callback(None);
        assert!(
            !app.grid_state.as_ref().unwrap().is_failed(id),
            "a closed library's failure must not mark the new one"
        );
        assert!(
            old_failures.lock().contains(&id),
            "the late failure lands in the library that asked"
        );
    });
}

/// Card badges: RAW bottom-right, duration bottom-left,
/// outline heart top-right, and the culling marks beside them, never on top.
#[core::prelude::v1::test]
fn raw_badge_duration_and_heart_sit_where_the_design_puts_them() {
    use lumina_core::{Flag, MediaType};
    let (dir, engine) = super::helpers::seed_library("tile_badges", 1);
    let tpl = engine.get_item_slice(0..1)[0].clone();
    let rows = [
        ("a.arw", MediaType::Raw, None),
        ("clip.mov", MediaType::Video, Some(42_000)),
        ("fav.jpg", MediaType::Image, None),
    ];
    let items: Vec<_> = rows
        .iter()
        .map(|(name, kind, dur)| {
            // No file on disk: a watcher rescan of a fake file would
            // race the test and reset its media type and duration.
            let mut it = tpl.clone();
            it.file_name = name.to_string();
            it.path = dir.join(name);
            it.media_type = *kind;
            it.duration_ms = *dur;
            it
        })
        .collect();
    let stored = engine.catalog().insert_or_update_batch(&items).unwrap();
    let id = |name: &str| stored.iter().find(|s| s.file_name == name).unwrap().id;
    let (raw, clip, fav) = (id("a.arw"), id("clip.mov"), id("fav.jpg"));
    engine.toggle_favorite(fav, true).unwrap();
    engine.set_flags(&[(fav, Flag::Pick)]).unwrap();
    engine.set_ratings(&[(clip, 3)]).unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = super::helpers::app_with_library(&mut cx, engine);
    // The favourite's file is also missing: three top-right marks at once.
    view.update(vcx, |app, cx| {
        app.missing_items.insert(fav);
        cx.notify();
    });
    render_ui(vcx);

    let mut b = |sel: String| {
        vcx.debug_bounds(leak(sel.clone()))
            .unwrap_or_else(|| panic!("{sel} must be painted"))
    };
    let near = |a: gpui::Pixels, want: gpui::Pixels, what: &str| {
        assert!(
            (f32::from(a) - f32::from(want)).abs() <= 0.5,
            "{what}: got {a:?}, want {want:?}"
        );
    };
    let apart = |a: gpui::Bounds<gpui::Pixels>, b: gpui::Bounds<gpui::Pixels>, what: &str| {
        let overlap = a.origin.x < b.right()
            && b.origin.x < a.right()
            && a.origin.y < b.bottom()
            && b.origin.y < a.bottom();
        assert!(!overlap, "{what} must not overlap: {a:?} vs {b:?}");
    };
    let inset = gpui::px(8.0);

    let well = b(format!("media-card-photo-{}", raw.0));
    let badge = b(format!("media-card-raw-{}", raw.0));
    near(badge.right(), well.right() - inset, "RAW badge right inset");
    near(
        badge.bottom(),
        well.bottom() - inset,
        "RAW badge bottom inset",
    );
    near(badge.size.height, gpui::px(18.0), "RAW badge height");

    let well = b(format!("media-card-photo-{}", clip.0));
    let dur = b(format!("media-card-duration-{}", clip.0));
    near(
        dur.origin.x,
        well.origin.x + inset,
        "duration sits bottom-left",
    );
    near(dur.bottom(), well.bottom() - inset, "duration bottom inset");
    let meta = b(format!("media-card-meta-{}", clip.0));
    assert!(
        meta.bottom() <= dur.origin.y,
        "a rated video's meta pill sits above the duration badge"
    );
    apart(meta, dur, "meta pill and duration");

    let well = b(format!("media-card-photo-{}", fav.0));
    let heart = b(format!("media-card-fav-{}", fav.0));
    let flag = b(format!("grid-flag-badge-{}", fav.0));
    let missing = b(format!("grid-missing-badge-{}", fav.0));
    // Centred in the 20 px top row beside the flag and missing badges.
    assert!(
        heart.origin.y >= well.origin.y + inset - gpui::px(0.5)
            && heart.bottom() <= well.origin.y + inset + gpui::px(20.5),
        "the heart sits in the top inset row: {heart:?} in {well:?}"
    );
    assert!(
        heart.right() <= well.right() - inset + gpui::px(0.5),
        "the heart stays inside the top-right inset"
    );
    assert!(
        flag.right() <= heart.origin.x,
        "the flag badge sits left of the heart"
    );
    apart(flag, heart, "flag and heart");
    apart(heart, missing, "heart and missing badge");
    apart(flag, missing, "flag and missing badge");

    drop(view);
}

/// Small tiles drop the heart, edited badge and meta pill so the state
/// badges that remain (check, flag, missing, duration) never overlap.
#[core::prelude::v1::test]
fn small_tiles_keep_state_badges_apart() {
    use lumina_core::{Flag, MediaType};
    let (dir, engine) = super::helpers::seed_library("tile_small", 1);
    let mut clip = engine.get_item_slice(0..1)[0].clone();
    // No file on disk, so no watcher rescan can reset the duration.
    clip.file_name = "clip.mov".into();
    clip.path = dir.join("clip.mov");
    clip.media_type = MediaType::Video;
    clip.duration_ms = Some(42_000);
    let clip = engine.catalog().insert_or_update_batch(&[clip]).unwrap()[0].id;
    engine.toggle_favorite(clip, true).unwrap();
    engine.set_flags(&[(clip, Flag::Pick)]).unwrap();
    engine.set_ratings(&[(clip, 5)]).unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    // Small tiles from the first draw: debug_bounds accumulates across frames.
    view.update(vcx, |app, cx| {
        app.grid_state = Some(GridState::new(engine.clone()));
        app.refresh_album_counts();
        app.grid_columns = 8;
        app.missing_items.insert(clip);
        app.edits.edited_ids.insert(clip);
        app.selected_indices = [0, 1].into_iter().collect();
        cx.notify();
    });
    render_ui(vcx);
    let side = view.update(vcx, |app, _| app.grid_metrics().photo_side);
    assert!(
        (86.0..124.0).contains(&side),
        "setup: 8 columns at 1280 px give a compact tile, got {side}"
    );

    for gone in ["media-card-fav", "media-card-meta", "grid-edited-badge"] {
        assert!(
            vcx.debug_bounds(leak(format!("{gone}-{}", clip.0)))
                .is_none(),
            "{gone} hides on a {side} px tile"
        );
    }
    let kept: Vec<_> = [
        "media-card-check",
        "grid-flag-badge",
        "grid-missing-badge",
        "media-card-duration",
    ]
    .iter()
    .map(|name| {
        let sel = format!("{name}-{}", clip.0);
        vcx.debug_bounds(leak(sel.clone()))
            .unwrap_or_else(|| panic!("{sel} stays on a small tile"))
    })
    .collect();
    let well = vcx
        .debug_bounds(leak(format!("media-card-photo-{}", clip.0)))
        .unwrap();
    for (i, a) in kept.iter().enumerate() {
        assert!(
            a.origin.x >= well.origin.x && a.right() <= well.right(),
            "badge {i} fits inside the {side} px well"
        );
        for b in &kept[i + 1..] {
            let overlap = a.origin.x < b.right()
                && b.origin.x < a.right()
                && a.origin.y < b.bottom()
                && b.origin.y < a.bottom();
            assert!(
                !overlap,
                "small-tile badges must not overlap: {a:?} vs {b:?}"
            );
        }
    }
    drop(view);
}

#[core::prelude::v1::test]
fn ui_automation_card_skeleton_selection_and_open() {
    let (_temp_dir, engine) = seed_library("card_states", 8);
    let first = engine.get_item_slice(0..1)[0].id;
    let first_id = first.0;
    let card = leak(format!("media-card-{}", first_id));
    let photo = leak(format!("media-card-photo-{}", first_id));
    let skeleton = leak(format!("media-card-skeleton-{}", first_id));
    let selected = leak(format!("media-card-selected-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_pending(&mut cx, engine, &[first]);

    assert!(
        vcx.debug_bounds(skeleton).is_some(),
        "a card without a decoded texture must paint media-card-skeleton-<id>"
    );
    let photo_bounds = vcx
        .debug_bounds(photo)
        .expect("every card must paint its photo area");
    let skeleton_bounds = vcx
        .debug_bounds(skeleton)
        .expect("skeleton bounds disappeared between assertions");
    assert!(
        photo_bounds.contains(&skeleton_bounds.center()),
        "the skeleton must sit inside the card photo area"
    );
    assert_eq!(
        photo_bounds.size, skeleton_bounds.size,
        "the skeleton must fill the photo area so the layout cannot jump on reveal"
    );
    assert!(
        vcx.debug_bounds(selected).is_none(),
        "an unselected card must not paint the selected marker"
    );

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(0), "single-click still selects");
        assert!(app.selected_item.is_some());
    });
    assert!(
        vcx.debug_bounds(selected).is_some(),
        "selecting a card must paint media-card-selected-<id>"
    );

    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "double-click must still open the viewer"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_thumbnail_chrome_is_minimal() {
    let (_temp_dir, engine) = seed_library("card_chrome", 8);
    let first_id = engine.get_item_slice(0..1)[0].id.0;
    let card = leak(format!("media-card-{}", first_id));
    let selected = leak(format!("media-card-selected-{}", first_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // Fix the window so the rendered card width and the production row-height
    // formula use the same budget (the resize observer records the bounds).
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    click(vcx, card);
    let card_bounds = vcx
        .debug_bounds(card)
        .expect("the card must still paint its hit target");
    let photo = leak(format!("media-card-photo-{}", first_id));
    let photo_bounds = vcx
        .debug_bounds(photo)
        .expect("the card must paint its photo well");
    let selected_bounds = vcx
        .debug_bounds(selected)
        .expect("selection must still paint its highlight");
    let selected_w = f32::from(selected_bounds.size.width);
    let selected_h = f32::from(selected_bounds.size.height);
    let photo_w = f32::from(photo_bounds.size.width);
    let photo_h = f32::from(photo_bounds.size.height);
    // A 2px ring with a 1px gap: 3px outside the well, half the 6px tile gap,
    // so the rings of two adjacent selected tiles never overlap.
    assert!(
        (selected_w - photo_w - 6.0).abs() <= 0.5 && (selected_h - photo_h - 6.0).abs() <= 0.5,
        "the ring must sit 3px outside the photo well: photo={photo_bounds:?} selected={selected_bounds:?}"
    );
    assert!(
        (f32::from(photo_bounds.left()) - f32::from(selected_bounds.left()) - 3.0).abs() <= 0.5
            && (f32::from(photo_bounds.top()) - f32::from(selected_bounds.top()) - 3.0).abs()
                <= 0.5,
        "the ring must be centred on the well: photo={photo_bounds:?} selected={selected_bounds:?}"
    );
    assert!(
        (selected_w - selected_h).abs() <= 2.0,
        "the thumbnail highlight must be square: selected={selected_bounds:?}"
    );
    assert!(
        photo_w > f32::from(card_bounds.size.width) * 0.8
            && photo_h > f32::from(card_bounds.size.height) * 0.8,
        "the photo well must still fill the card: card={card_bounds:?} photo={photo_bounds:?}"
    );
    // The removed check badge and hover "Open" hint never had debug
    // selectors, so absence assertions on them could not fail; the square,
    // photo-sized highlight above is the observable contract.

    double_click(vcx, card);
    view.update(vcx, |app, _| {
        assert!(
            app.is_fullscreen_viewer,
            "removing the hover hint must not remove the open action"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_card_texture_replaces_skeleton() {
    let (_temp_dir, engine) = seed_library("card_reveal", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card_id = item.id.0;
    let photo = leak(format!("media-card-photo-{}", card_id));
    let skeleton = leak(format!("media-card-skeleton-{}", card_id));
    let thumb = leak(format!("media-card-thumb-{}", card_id));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_pending(&mut cx, engine, &[item.id]);
    assert!(vcx.debug_bounds(skeleton).is_some());
    assert!(vcx.debug_bounds(thumb).is_none());

    let texture = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, cx| {
        let state = app
            .grid_state
            .as_ref()
            .expect("grid state is installed by the test harness");
        state.texture_cache.lock().put(
            item.id,
            crate::views::grid::CachedThumb::new(crate::views::grid::THUMBNAIL_SIZE_MAX, texture),
        );
        cx.notify();
    });
    render_ui(vcx);

    // debug_bounds is cumulative across frames, so the reveal is proven by the
    // thumbnail appearing rather than by the skeleton's stale bounds vanishing.
    assert!(
        vcx.debug_bounds(thumb).is_some(),
        "a cached texture must paint inside the photo area"
    );
    let photo_bounds = vcx
        .debug_bounds(photo)
        .expect("the photo area must stay painted across the reveal");
    let thumb_bounds = vcx
        .debug_bounds(thumb)
        .expect("thumbnail bounds disappeared between assertions");
    assert_eq!(
        photo_bounds.size, thumb_bounds.size,
        "the revealed thumbnail must keep the same layout as the skeleton"
    );
}

#[core::prelude::v1::test]
fn ui_automation_card_photo_well_is_square_at_all_densities() {
    let (_temp_dir, engine) = seed_library("square_well", 8);
    let first = engine.get_item_slice(0..1)[0].clone();
    let first_id = first.id.0;
    engine.catalog().set_rating(first.id, 3).unwrap();
    let photo = leak(format!("media-card-photo-{}", first_id));
    let meta = leak(format!("media-card-meta-{}", first_id));
    let unrated_meta = leak(format!(
        "media-card-meta-{}",
        engine.get_item_slice(1..2)[0].id.0
    ));

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // Fix the window so the rendered card width and the production formula use
    // the same budget; the resize observer records the new bounds before the
    // next draw.
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    for columns in [2usize, 4, 8] {
        view.update(vcx, |app, cx| {
            app.grid_columns = columns;
            cx.notify();
        });
        render_ui(vcx);

        let photo_bounds = vcx
            .debug_bounds(photo)
            .unwrap_or_else(|| panic!("no photo well painted at {columns} columns"));
        let width = f32::from(photo_bounds.size.width);
        let height = f32::from(photo_bounds.size.height);
        assert!(
            (width - height).abs() <= 2.0,
            "the photo well must be square at {columns} columns, got {width}x{height}"
        );

        let expected = view.update(vcx, |app, _| app.grid_metrics().photo_side);
        assert!(
            (height - expected).abs() <= 2.0,
            "the rendered well ({height}) must use the derived side ({expected}) at {columns} columns"
        );

        // No band under the photo. Rating and label ride in a pill
        // on the thumbnail, and only when there is something to show. Compact
        // tiles hide the pill (small_tiles_keep_state_badges_apart).
        if expected < crate::views::grid::COMPACT_TILE {
            continue;
        }
        let meta_bounds = vcx.debug_bounds(meta).unwrap_or_else(|| {
            panic!("a rated card must paint its meta pill at {columns} columns")
        });
        assert!(
            photo_bounds.contains(&meta_bounds.origin)
                && photo_bounds.contains(&meta_bounds.bottom_right()),
            "the meta pill must sit inside the photo well at {columns} columns"
        );
        assert!(
            vcx.debug_bounds(unrated_meta).is_none(),
            "an unmarked card must not paint an empty meta pill"
        );
    }

    // A live resize must relayout from the same formula, not a stale width.
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    let resized = vcx
        .debug_bounds(photo)
        .expect("the photo well must survive a window resize");
    let width = f32::from(resized.size.width);
    let height = f32::from(resized.size.height);
    assert!(
        (width - height).abs() <= 2.0,
        "the photo well must stay square after a resize, got {width}x{height}"
    );

    // The docked inspector narrows the grid: its width and 4px resizer must be
    // part of the same budget.
    view.update(vcx, |app, cx| {
        app.selected_item = app.get_active_slice(0..1).first().cloned();
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);
    let with_inspector = vcx
        .debug_bounds(photo)
        .expect("the photo well must stay painted with the inspector open");
    let width = f32::from(with_inspector.size.width);
    let height = f32::from(with_inspector.size.height);
    let expected = view.update(vcx, |app, _| app.grid_metrics().photo_side);
    assert!(
        (width - height).abs() <= 2.0 && (height - expected).abs() <= 2.0,
        "the inspector chrome must be budgeted: well {width}x{height}, expected {expected}"
    );
}

#[core::prelude::v1::test]
fn ui_automation_card_tooltip_carries_the_filename() {
    let (_temp_dir, engine) = seed_library("card_tooltip", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);

    let center = bounds_center(vcx, card);
    vcx.simulate_mouse_move(center, None, gpui::Modifiers::default());
    // GPUI waits 500ms before building a tooltip.
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("media-card-name-tooltip").is_some(),
        "hovering a card must show the filename tooltip that replaced the label"
    );
}

#[core::prelude::v1::test]
fn settings_pauses_the_library_grid_and_its_thumbnail_loads() {
    let (_dir, engine) = seed_library("settings_pause", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    reset_requests();
    // Settings opens before the library's first draw, so no card has ever
    // asked for a thumbnail.
    view.update(vcx, |app, cx| {
        app.grid_state = Some(GridState::new(engine));
        app.refresh_album_counts();
        app.open_preferences(cx);
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "setup: Settings is open")
    });
    assert!(
        vcx.debug_bounds("media-grid").is_none(),
        "the grid under Settings must not be laid out"
    );
    assert_eq!(
        requests(),
        0,
        "no thumbnail may be requested while Settings covers the library"
    );

    view.update(vcx, |app, cx| app.close_preferences(cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("media-grid").is_some(),
        "the grid lays out again once Settings closes"
    );
    assert!(
        requests() > 0,
        "the library asks for its thumbnails again once Settings closes"
    );
}

/// The grid must ask for a thumbnail the size of the tile it paints, in the
/// display's pixels: a fixed 1x size is what made Retina tiles look blurred.
#[core::prelude::v1::test]
fn thumbnails_are_decoded_for_the_tile_on_this_display() {
    let (_dir, engine) = seed_library("thumb_density", 4);
    let item = engine.get_item_slice(0..1)[0].clone();
    let id = item.id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // The test platform starts at 2x, so setting 1x proves `Render` reads the
    // window rather than the field's default.
    vcx.update(|window, _| window.set_scale_factor(1.0));
    render_ui(vcx);
    assert_eq!(
        view.update(vcx, |app, _| app.window_scale),
        1.0,
        "the app must read the window's backing scale"
    );

    for (columns, scale) in [(4usize, 1.0f32), (4, 2.0), (2, 2.0)] {
        // The real window, so the painted tile and `grid_metrics` share one
        // budget (the resize observer records the bounds).
        vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
        vcx.update(|window, _| window.set_scale_factor(scale));
        view.update(vcx, |app, cx| {
            app.grid_columns = columns;
            cx.notify();
        });
        render_ui(vcx);

        // The painted well, not the derived metric: this is what the user sees.
        let photo = vcx
            .debug_bounds(leak(format!("media-card-photo-{}", id.0)))
            .unwrap_or_else(|| panic!("no photo well painted at {columns} columns"));
        // The painted well is un-floored while `grid_metrics` floors
        // `photo_side`; the 64 px size step absorbs that sub-pixel difference.
        let device = f32::from(photo.size.width) * scale;

        let size = view.update(vcx, |app, _| {
            let state = app.grid_state.as_ref().expect("grid state is installed");
            // Let the same item be requested again for this tile size.
            state.in_flight.lock().clear();
            THUMB_DECODE_SIZES.with(|sizes| sizes.borrow_mut().clear());
            app.thumbnail_request_size()
        });

        // Both request paths: the id/prefetch one and the painted row's.
        view.update(vcx, |app, _| {
            app.queue_thumbnail_request(id);
            app.grid_state.as_ref().unwrap().in_flight.lock().clear();
            app.queue_thumbnail_request_item(&item);
        });

        let requested = THUMB_DECODE_SIZES.with(|sizes| sizes.borrow().clone());
        assert_eq!(
            requested,
            vec![(id, size), (id, size)],
            "{columns} columns at {scale}x must request its own decode size"
        );
        assert!(
            size as f32 >= device,
            "a {device} device px tile at {scale}x needs at least that decode, got {size}"
        );
        if scale > 1.0 {
            assert!(
                size > 320,
                "a Retina tile must not keep the old fixed 320 px decode"
            );
        }
    }
}

/// A texture decoded for a smaller tile (a zoom-in, or a move to a sharper
/// display) is refreshed for the bigger tile while staying on screen: no
/// skeleton flash, and the request converges because the new size is stamped.
#[core::prelude::v1::test]
fn a_smaller_cached_thumbnail_is_refreshed_without_a_skeleton() {
    let (_dir, engine) = seed_library("thumb_refresh", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let id = item.id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // A Retina display, so the tile needs more than the smallest decode.
    vcx.update(|window, _| window.set_scale_factor(2.0));
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    let coarse = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(320, 320),
    )]));
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("grid state is installed");
        state.texture_cache.lock().put(
            id,
            crate::views::grid::CachedThumb::new(crate::views::grid::THUMBNAIL_SIZE_MIN, coarse),
        );
        state.in_flight.lock().clear();
        THUMB_DECODE_SIZES.with(|sizes| sizes.borrow_mut().clear());
    });
    render_ui(vcx);

    let wanted = view.update(vcx, |app, _| app.thumbnail_request_size());
    assert!(
        wanted > crate::views::grid::THUMBNAIL_SIZE_MIN,
        "setup: the tile must want more than the coarse decode, got {wanted}"
    );
    let requested = THUMB_DECODE_SIZES.with(|sizes| sizes.borrow().clone());
    assert!(
        !requested.is_empty(),
        "the smaller decode must be refreshed at the tile's size"
    );
    assert!(
        requested
            .iter()
            .all(|(got_id, size)| *got_id == id && *size == wanted),
        "every refresh must be for this id at the tile's size {wanted}: {requested:?}"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-thumb-{}", id.0)))
            .is_some(),
        "the coarse thumbnail must keep painting while the sharper one decodes"
    );
}

/// The cache stamps the size the decode was *requested* at, not the frame's
/// own size: the test fixtures decode to 2x2, and comparing the frame would
/// re-request a thumbnail on every frame forever.
#[core::prelude::v1::test]
fn a_small_source_is_not_refreshed_forever() {
    let (_dir, engine) = seed_library("thumb_small_source", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.update(|window, _| window.set_scale_factor(2.0));
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(840.0)));
    render_ui(vcx);

    let wanted = view.update(vcx, |app, cx| {
        app.grid_state
            .as_ref()
            .expect("grid state")
            .in_flight
            .lock()
            .clear();
        let size = app.thumbnail_request_size();
        app.handle_ui_message(
            UiMessage::ThumbnailLoaded {
                id,
                frame: super::helpers::tiny_decoded_frame(id),
                requested_size: size,
            },
            cx,
        );
        THUMB_DECODE_SIZES.with(|sizes| sizes.borrow_mut().clear());
        size
    });
    assert!(wanted > 2, "setup: the tile wants more than the 2x2 source");

    render_ui(vcx);
    assert!(
        THUMB_DECODE_SIZES.with(|sizes| sizes.borrow().is_empty()),
        "a texture stamped with its requested size must not be re-requested \
         just because the source is smaller"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-thumb-{}", id.0)))
            .is_some(),
        "the small source's texture still paints"
    );
}
