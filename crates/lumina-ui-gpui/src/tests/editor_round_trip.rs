//! The external editor round trip: a file saved in
//! place arrives as a watcher batch, and the grid and viewer drop the old
//! picture and decode the new file, keeping the item's id, tags and edits.
//!
//! Decodes run on the engine's pool, which `run_until_parked` does not wait
//! for, so cached textures are placeholders put in before the first draw and
//! the tests assert what is re-requested, not decoded pixels.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::*;
use lumina_core::{CropRect, Edits, EngineEvent, LuminaEngine, MediaId, MediaItem};

use crate::app::state::{THUMB_REQUESTS, THUMB_REQUEST_ITEMS};
use crate::app::types::UiMessage;
use crate::tests::quick_edit::temp_library;
use crate::tests::ui_automation::{focus_app, render_ui};
use crate::views::grid::GridState;
use crate::LuminaApp;

fn photo(path: &Path, size: u32, rgb: [u8; 3]) {
    image::RgbImage::from_pixel(size, size, image::Rgb(rgb))
        .save(path)
        .expect("fixture photo must be written");
}

/// One real photo with a tag and a crop; returns the folder, the engine,
/// a receive-with-timeout on its events and the item.
fn edited_photo(
    prefix: &str,
) -> (
    PathBuf,
    Arc<LuminaEngine>,
    impl Fn(Duration) -> Option<EngineEvent>,
    MediaItem,
) {
    let dir = temp_library(prefix);
    photo(&dir.join("a.png"), 16, [255, 0, 0]);
    let (engine, rx) =
        LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), Some(dir.join("cache")))
            .expect("temp engine must open");
    while let Ok(event) = rx.recv_timeout(Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    let item = engine.get_item_slice(0..1).remove(0);
    engine
        .add_tags_batch(&[item.id], &["beach".into()])
        .expect("seed tag");
    engine.set_edits(&[(item.id, crop())]).expect("seed crop");
    let next = move |timeout| rx.recv_timeout(timeout).ok();
    (dir, engine, next, item)
}

fn crop() -> Edits {
    Edits {
        crop: Some(CropRect {
            x: 0.25,
            y: 0.25,
            w: 0.5,
            h: 0.5,
        }),
        ..Edits::default()
    }
}

/// Opens the app with a placeholder thumbnail already cached for `id`, so
/// the first draw sends no decode; returns the placeholder too.
fn app_with_texture(
    cx: &mut TestAppContext,
    engine: Arc<LuminaEngine>,
    id: MediaId,
) -> (Entity<LuminaApp>, &mut VisualTestContext, Arc<RenderImage>) {
    let placeholder = Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        let mut state = GridState::new(engine);
        // The library's opening scan has finished (ScanComplete).
        state.is_scanning = false;
        state.texture_cache.lock().put(id, placeholder.clone());
        app.grid_state = Some(state);
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx, placeholder)
}

/// True while the grid's texture for `id` is `old`. The re-requested decode
/// may already have landed (the pool is real), so "not old" is either gone
/// or a new texture.
fn shows(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    id: MediaId,
    old: &Arc<RenderImage>,
) -> bool {
    view.update(vcx, |app, _| {
        app.grid_state.as_ref().is_some_and(|state| {
            state
                .texture_cache
                .lock()
                .peek(&id)
                .is_some_and(|texture| Arc::ptr_eq(texture, old))
        })
    })
}

fn reset_requests() {
    THUMB_REQUESTS.with(|n| n.set(0));
    THUMB_REQUEST_ITEMS.with(|items| items.borrow_mut().clear());
}

fn deliver(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, items: Vec<MediaItem>) {
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items }),
            cx,
        )
    });
    render_ui(vcx);
}

/// Saves a different picture over `path` the way editors do (write a temp
/// file, rename it over the original) and returns the watcher's batch.
fn save_over(
    path: &Path,
    next: &impl Fn(Duration) -> Option<EngineEvent>,
    id: MediaId,
) -> Vec<MediaItem> {
    // Written outside the library folder, so the watcher sees only the rename.
    let folder = path.parent().unwrap();
    let temp = folder.with_file_name(format!(
        "{}-saving.png",
        folder.file_name().unwrap().to_string_lossy()
    ));
    photo(&temp, 48, [0, 0, 255]);
    std::fs::rename(&temp, path).expect("the save must replace the file");
    batch_with(next, id)
}

/// The watcher's next batch that carries `id`.
fn batch_with(next: &impl Fn(Duration) -> Option<EngineEvent>, id: MediaId) -> Vec<MediaItem> {
    loop {
        match next(Duration::from_secs(20)) {
            Some(EngineEvent::BatchDiscovered { items }) if items.iter().any(|i| i.id == id) => {
                return items
            }
            Some(_) => {}
            None => panic!("the watcher must report the file within 20 s"),
        }
    }
}

#[core::prelude::v1::test]
fn external_save_refreshes_texture_and_keeps_metadata() {
    let (dir, engine, next, item) = edited_photo("round_trip");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx, old) = app_with_texture(&mut cx, engine.clone(), item.id);
    reset_requests();
    assert!(
        shows(&view, vcx, item.id, &old),
        "the grid shows the old picture"
    );

    let batch = save_over(&item.path, &next, item.id);
    let saved = batch.iter().find(|i| i.id == item.id).unwrap().clone();
    assert_ne!(
        (saved.file_size, saved.mtime_epoch_ms),
        (item.file_size, item.mtime_epoch_ms),
        "the batch carries the new file's signature"
    );
    reset_requests();
    deliver(&view, vcx, batch);
    render_ui(vcx);

    assert!(
        !shows(&view, vcx, item.id, &old),
        "the old thumbnail is dropped"
    );
    // The test harness does not deliver the pool's results to the app, so
    // the card may be asked for on every frame; every ask must be for the
    // saved file.
    let requested = THUMB_REQUEST_ITEMS.with(|items| items.borrow().clone());
    assert!(!requested.is_empty(), "the card is decoded again");
    assert!(
        requested
            .iter()
            .all(|&(id, mtime)| id == item.id && mtime == saved.mtime_epoch_ms),
        "every re-request carries the saved file's signature (the disk cache \
         key), never the row the grid held before the save: {requested:?}"
    );
    let row = engine.get_item_by_id(item.id).expect("same id");
    assert_eq!(row.path, item.path);
    assert_eq!(engine.get_tags_for_item(item.id), vec!["beach".to_string()]);
    assert_eq!(
        engine.get_edits(&[item.id]).unwrap()[&item.id],
        crop(),
        "Lumina's edits stay on top of the saved file"
    );
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn external_save_reloads_the_open_viewer_photo() {
    let (dir, engine, next, item) = edited_photo("round_trip_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx, _) = app_with_texture(&mut cx, engine.clone(), item.id);
    view.update(vcx, |app, cx| {
        app.select_item_at(item.clone(), 0, cx);
        app.open_viewer(item.clone(), cx);
    });
    render_ui(vcx);
    let placeholder = Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    // Only the viewer shows it (the grid's thumbnail was evicted long ago).
    view.update(vcx, |app, _| {
        app.set_viewer_image(Some(placeholder.clone()));
        app.grid_state
            .as_ref()
            .unwrap()
            .texture_cache
            .lock()
            .pop(&item.id);
    });

    let batch = save_over(&item.path, &next, item.id);
    deliver(&view, vcx, batch);

    view.update(vcx, |app, _| {
        assert!(
            !app.viewer_image
                .as_ref()
                .is_some_and(|image| Arc::ptr_eq(image, &placeholder)),
            "the viewer drops the old frame and decodes the saved file"
        );
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(item.id),
            "the viewer stays on the same item"
        );
    });
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn scan_batches_do_not_evict_textures() {
    let (dir, engine, _next, item) = edited_photo("round_trip_scan");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx, old) = app_with_texture(&mut cx, engine.clone(), item.id);
    view.update(vcx, |app, _| {
        app.grid_state.as_mut().unwrap().is_scanning = true;
    });
    reset_requests();

    deliver(&view, vcx, vec![item.clone()]);

    assert!(
        shows(&view, vcx, item.id, &old),
        "a full rescan must not flush the grid"
    );
    assert_eq!(
        THUMB_REQUESTS.with(|n| n.get()),
        0,
        "and decodes nothing again"
    );
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn rename_in_library_keeps_texture_and_never_fails() {
    let (dir, engine, next, item) = edited_photo("round_trip_rename");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx, old) = app_with_texture(&mut cx, engine.clone(), item.id);
    reset_requests();

    let renamed = dir.join("renamed.png");
    std::fs::rename(&item.path, &renamed).expect("rename inside the library");
    let batch = batch_with(&next, item.id);
    let moved = batch.iter().find(|i| i.id == item.id).unwrap();
    assert_eq!(
        moved.path, renamed,
        "the watcher keeps the id at the new path"
    );
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::BatchDiscovered { items: batch }),
            cx,
        );
        // Before the view refetch lands: the next frame decodes from this row.
        assert_eq!(
            app.displayed_rows(0..1).first().map(|row| row.path.clone()),
            Some(renamed.clone()),
            "the grid's cached row has the new path at once"
        );
    });
    render_ui(vcx);

    assert!(
        shows(&view, vcx, item.id, &old),
        "a rename changes no pixels: the thumbnail stays"
    );
    assert_eq!(THUMB_REQUESTS.with(|n| n.get()), 0, "and needs no decode");
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().unwrap();
        assert!(
            !state.is_failed(item.id),
            "the renamed item is never marked failed"
        );
    });
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}
