//! GPU texture eviction for retired viewer frames and thumbnails.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn replaced_viewer_frames_are_evicted_but_cached_thumbnails_are_kept() {
    use crate::player::VideoFrame;
    let frame = || VideoFrame {
        width: 2,
        height: 2,
        data: vec![0; 16],
    };
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    let cached = crate::bridge::video_frame_to_gpui_image(frame()).expect("2x2 BGRA frame");
    view.update(vcx, |app, _| {
        app.set_viewer_image(Some(crate::bridge::video_frame_to_gpui_image(frame()).unwrap()));
        app.set_viewer_image(Some(crate::bridge::video_frame_to_gpui_image(frame()).unwrap()));
        assert_eq!(
            app.retired_images.len(),
            1,
            "a replaced video frame must be queued for atlas eviction, or GPU memory grows every frame"
        );
        app.set_viewer_image(Some(cached.clone()));
        app.set_viewer_image(None);
        assert_eq!(
            app.retired_images.len(),
            2,
            "an image still held by the thumbnail cache must keep its texture"
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.retired_images.is_empty(),
            "render must drain the eviction queue"
        );
    });
}

#[core::prelude::v1::test]
fn pip_window_evicts_retired_frames_while_the_main_window_is_not_drawing() {
    use crate::player::VideoFrame;
    let frame = || {
        crate::bridge::video_frame_to_gpui_image(VideoFrame {
            width: 2,
            height: 2,
            data: vec![0; 16],
        })
        .expect("2x2 BGRA frame")
    };
    let mut cx = crate::tests::TestApp::single();
    // The main app has no window here, standing in for an occluded main
    // window whose display link has stopped.
    let app = cx.new(LuminaApp::new);
    let ui_tx = app.read_with(&*cx, |app, _| app.ui_tx.clone());
    let (_pip, vcx) = cx.add_window_view(|window, cx| {
        crate::views::floating_pip::FloatingPipPlayerView::new(app.downgrade(), ui_tx, window, cx)
    });

    app.update(vcx, |app, _| {
        app.set_viewer_image(Some(frame()));
        app.set_viewer_image(Some(frame()));
        assert_eq!(app.retired_images.len(), 1);
    });
    render_ui(vcx);
    app.update(vcx, |app, _| {
        assert!(
            app.retired_images.is_empty(),
            "PiP playback must evict retired frames even when the main window is not drawing"
        );
    });
}

/// Regression: playing with every window minimized retired a frame per tick,
/// but only a render drained the queue, so memory grew ~270 MB/s.
#[core::prelude::v1::test]
fn retired_frames_are_evicted_without_any_window_drawing() {
    let mut cx = crate::tests::TestApp::single();
    let app = cx.new(LuminaApp::new);
    let item = get_sample_items()[0].clone();
    let ui_tx = app.update(&mut *cx, |app, _| {
        app.selected_item = Some(item.clone());
        app.ui_tx.clone()
    });
    for _ in 0..3 {
        ui_tx
            .send_blocking(UiMessage::ViewerLoaded {
                id: item.id,
                frame: tiny_decoded_frame(item.id),
            })
            .unwrap();
    }
    cx.run_until_parked();
    app.read_with(&*cx, |app, _| {
        assert!(app.viewer_image.is_some(), "the latest frame must be shown");
        assert!(
            app.retired_images.is_empty(),
            "replaced frames must be evicted as messages arrive, not only when a window draws"
        );
    });
}

/// Regression: thumbnails evicted from the LRU (or removed with their item)
/// dropped their Arc without freeing the GPU texture, so every scroll pass
/// through the library grew GPU memory by ~240 MB.
#[core::prelude::v1::test]
fn evicted_and_removed_thumbnails_are_queued_for_gpu_eviction() {
    let (_temp_dir, engine) = seed_library("thumb_evict", 2);
    let (other_dir, _other) = seed_library("thumb_evict_other", 1);
    let mut cx = crate::tests::TestApp::single();
    let app = cx.new(LuminaApp::new);
    app.update(&mut *cx, |app, cx| {
        let state = crate::views::grid::GridState::new(engine.clone());
        state
            .texture_cache
            .lock()
            .resize(std::num::NonZeroUsize::new(2).unwrap());
        app.grid_state = Some(state);
        for id in 1..=3 {
            let id = lumina_core::MediaId(id);
            app.handle_ui_message(
                UiMessage::ThumbnailLoaded {
                    id,
                    frame: tiny_decoded_frame(id),
                },
                cx,
            );
        }
        assert_eq!(
            app.retired_images.len(),
            1,
            "the thumbnail pushed out of the LRU must be queued for GPU eviction"
        );
        app.handle_ui_message(
            UiMessage::Engine(lumina_core::EngineEvent::ItemRemoved(lumina_core::MediaId(
                3,
            ))),
            cx,
        );
        assert_eq!(
            app.retired_images.len(),
            2,
            "a removed item's thumbnail must be queued for GPU eviction"
        );

        // Rotating re-decodes the thumbnail, so the stale one must go too.
        let survivor = app
            .grid_state
            .as_ref()
            .and_then(|state| state.texture_cache.lock().peek_lru().map(|(id, _)| *id))
            .expect("one thumbnail is still cached");
        app.selected_item = engine.get_item_by_id(survivor);
        assert!(
            app.selected_item.is_some(),
            "the cached item is in the catalog"
        );
        app.rotate_selection(crate::views::viewer_state::ViewerRotation::Right, cx);
        assert_eq!(
            app.retired_images.len(),
            3,
            "rotation must queue the stale thumbnail for GPU eviction"
        );

        // Switching libraries drops the whole old texture cache.
        let id = lumina_core::MediaId(9);
        app.handle_ui_message(
            UiMessage::ThumbnailLoaded {
                id,
                frame: tiny_decoded_frame(id),
            },
            cx,
        );
        app.open_library_inner(other_dir.clone(), cx);
        assert!(
            other_dir.join("catalog.db").exists(),
            "tests must keep catalogs in the temp library, never the user's data dir"
        );
        assert_eq!(
            app.retired_images.len(),
            4,
            "opening another library must queue the old library's thumbnails for GPU eviction"
        );
    });
}
