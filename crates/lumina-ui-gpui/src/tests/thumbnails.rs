//! Failed thumbnails: a card whose decode failed shows "No preview" instead
//! of the skeleton, and is not decoded again until it changes or a rescan.
//!
//! Decodes run on the engine's own thread pool, which `run_until_parked`
//! does not wait for, so these tests never wait on it. They drive the
//! failure through `on_thumbnail_result` (the decode callback's body) and
//! count requests with `THUMB_REQUESTS`.

use super::ui_automation::{
    focus_app, keystrokes, leak, render_ui, seed_library, seed_library_undecodable,
};
use crate::app::command_palette::CommandId;
use crate::app::state::{on_thumbnail_result, THUMB_REQUESTS};
use crate::app::types::UiMessage;
use crate::views::grid::GridState;
use crate::LuminaApp;
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
    on_thumbnail_result(&state.thumb_failures, &state.in_flight, tx, id, None);
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
    let (dir, engine) = seed_library_undecodable("thumb_failed", 1);
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
    super::ui_automation::click(vcx, leak(format!("media-card-failed-{}", id.0)));
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

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn failure_callback_notifies() {
    let (dir, engine) = seed_library_undecodable("thumb_notify", 1);
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

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn thumbnail_callback_marks_and_messages_on_none() {
    let failures = parking_lot::Mutex::new(std::collections::HashSet::new());
    let in_flight = parking_lot::Mutex::new(std::collections::HashSet::new());
    let (tx, rx) = async_channel::unbounded();
    let id = MediaId(7);

    in_flight.lock().insert(id);
    on_thumbnail_result(&failures, &in_flight, &tx, id, None);
    assert!(failures.lock().contains(&id), "None marks the id failed");
    assert!(!in_flight.lock().contains(&id), "None ends the request");
    assert!(
        matches!(rx.try_recv(), Ok(UiMessage::ThumbnailFailed)),
        "None sends one ThumbnailFailed"
    );
    assert!(rx.try_recv().is_err(), "exactly one message per failure");

    let other = MediaId(8);
    in_flight.lock().insert(other);
    let frame = super::viewer_and_video::tiny_decoded_frame(other);
    on_thumbnail_result(&failures, &in_flight, &tx, other, Some(frame));
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
    let (dir, engine) = seed_library_undecodable("thumb_convert", 1);
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
        app.handle_ui_message(UiMessage::ThumbnailLoaded { id, frame }, cx)
    });
    render_ui(vcx);
    assert!(
        is_failed(&view, vcx, id),
        "a frame that cannot be converted is a failed thumbnail"
    );
    assert!(vcx
        .debug_bounds(leak(format!("media-card-failed-{}", id.0)))
        .is_some());

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rescan_clears_failures() {
    let (dir, engine) = seed_library("thumb_rescan", 1);
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

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn reupserted_item_is_retried() {
    let (dir, engine) = seed_library_undecodable("thumb_reupsert", 1);
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
    assert!(!is_failed(&view, vcx, id), "a re-upserted file is retried");
    render_ui(vcx);
    assert_eq!(requests(), 1, "the retried card is decoded once");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn failed_card_keeps_card_size() {
    let (dir, engine) = seed_library_undecodable("thumb_size", 2);
    let items = engine.get_item_slice(0..2);
    let (failed, pending) = (items[0].id, items[1].id);
    let mut cx = crate::tests::TestApp::single();
    // The second card is still decoding (its decode claimed before the first
    // draw); the first has failed. Either way the first ends failed: its own
    // decode of the fake file fails too.
    let (view, vcx) = super::ui_automation::app_with_pending(&mut cx, engine, &[pending]);
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

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn library_switch_starts_with_no_failures() {
    let (dir, engine) = seed_library_undecodable("thumb_switch", 1);
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
            .begin_thumbnail_request(app.grid_state.as_ref().unwrap(), id)
            .expect("the cleared id can be requested again");
        app.open_library(other.clone(), cx);
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

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}

/// Card badges (design 01 Task 5): RAW bottom-right, duration bottom-left,
/// outline heart top-right, and the culling marks beside them, never on top.
#[core::prelude::v1::test]
fn raw_badge_duration_and_heart_sit_where_the_design_puts_them() {
    use lumina_core::{Flag, MediaType};
    let (dir, engine) = super::ui_automation::seed_library("tile_badges", 1);
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
    engine.set_rating(clip, 3).unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = super::ui_automation::app_with_library(&mut cx, engine);
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
    let _ = std::fs::remove_dir_all(&dir);
}

/// Small tiles drop the heart, edited badge and meta pill so the state
/// badges that remain (check, flag, missing, duration) never overlap.
#[core::prelude::v1::test]
fn small_tiles_keep_state_badges_apart() {
    use lumina_core::{Flag, MediaType};
    let (dir, engine) = super::ui_automation::seed_library("tile_small", 1);
    let mut clip = engine.get_item_slice(0..1)[0].clone();
    // No file on disk, so no watcher rescan can reset the duration.
    clip.file_name = "clip.mov".into();
    clip.path = dir.join("clip.mov");
    clip.media_type = MediaType::Video;
    clip.duration_ms = Some(42_000);
    let clip = engine.catalog().insert_or_update_batch(&[clip]).unwrap()[0].id;
    engine.toggle_favorite(clip, true).unwrap();
    engine.set_flags(&[(clip, Flag::Pick)]).unwrap();
    engine.set_rating(clip, 5).unwrap();

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
    let _ = std::fs::remove_dir_all(&dir);
}
