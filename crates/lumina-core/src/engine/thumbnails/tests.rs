//! Tests for the parent module, kept beside it.

use super::*;
use crate::tests::{request_thumbnail_frame, temp_library, TEST_WAIT};
use std::fs;
use std::sync::mpsc;

#[test]
fn engine_rotation_persists_and_rotates_decodes_without_touching_originals() {
    let dir = temp_library("engine_rotation");
    let image_path = dir.join("landscape.png");
    let source = image::RgbaImage::from_fn(8, 4, |x, y| {
        image::Rgba([(x * 20) as u8, (y * 30) as u8, 40, 255])
    });
    source
        .save(&image_path)
        .expect("test image must be writable");
    let original_bytes = fs::read(&image_path).expect("test image must be readable");

    let (engine, events) =
        LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), Some(dir.join("cache")))
            .expect("engine must open");
    while let Ok(event) = events.recv_timeout(TEST_WAIT) {
        if matches!(event, EngineEvent::ScanComplete { .. }) {
            break;
        }
    }

    let item = engine
        .get_item_slice(0..1)
        .into_iter()
        .next()
        .expect("scanned image must be cataloged");
    assert_eq!(
        engine
            .catalog
            .get_rotation(item.id)
            .expect("rotation read must work"),
        0,
        "a scanned item must start unrotated"
    );

    let unrotated = request_thumbnail_frame(&engine, item.id, 320);
    assert_eq!((unrotated.width, unrotated.height), (8, 4));
    let cache_file = engine.cache.cache_file_path(&DiskCache::compute_key(
        &item.path,
        item.file_size,
        item.mtime_epoch_ms,
        320,
    ));
    assert!(
        cache_file.exists(),
        "the first decode must fill the disk cache"
    );

    engine
        .set_rotations(&[(item.id, 1)])
        .expect("rotation write must succeed");
    assert_eq!(
        engine.catalog.get_rotation(item.id).expect("rotation read"),
        1
    );
    assert!(
        cache_file.exists(),
        "rotating must keep the cached decode: it holds the unrotated frame"
    );

    // Served from the disk cache: rotation is applied after the lookup,
    // so the cached unrotated bytes stay valid.
    let rotated = request_thumbnail_frame(&engine, item.id, 320);
    assert_eq!(
        (rotated.width, rotated.height),
        (4, 8),
        "a cached frame decoded after rotation must come back rotated"
    );

    assert_eq!(
        fs::read(&image_path).expect("original must stay readable"),
        original_bytes,
        "rotation must never rewrite the original file"
    );

    engine
        .set_rotations(&[])
        .expect("an empty rotation batch must be a no-op");
    engine
        .set_rotations(&[(item.id, 3), (MediaId(9_999), 3)])
        .expect("unknown ids must not fail the batch");
    assert_eq!(
        engine.catalog.get_rotation(item.id).expect("rotation read"),
        3
    );

    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}

/// An engine over `dir` holding one generated image, scanned.
fn engine_with_image(
    dir: &Path,
    name: &str,
    width: u32,
    height: u32,
) -> (Arc<LuminaEngine>, MediaItem) {
    let source = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x * 255 / width) as u8, (y * 255 / height) as u8, 90])
    });
    source
        .save(dir.join(name))
        .expect("test image must be writable");
    let (engine, events) = LuminaEngine::open_with_paths(
        dir.to_path_buf(),
        dir.join("catalog.db"),
        Some(dir.join("cache")),
    )
    .expect("engine must open");
    while let Ok(event) = events.recv_timeout(TEST_WAIT) {
        if matches!(event, EngineEvent::ScanComplete { .. }) {
            break;
        }
    }
    let item = engine.get_item_slice(0..1).remove(0);
    (engine, item)
}

fn crop_edits(x: f32, y: f32, w: f32, h: f32) -> Edits {
    Edits {
        crop: Some(CropRect { x, y, w, h }),
        ..Default::default()
    }
}

#[test]
fn edits_never_touch_original_or_cache() {
    let dir = temp_library("edits_untouched");
    let (engine, item) = engine_with_image(&dir, "photo.jpg", 64, 32);
    let original_hash = xxhash_rust::xxh3::xxh3_64(&fs::read(&item.path).expect("read"));
    engine
        .catalog
        .set_edits(&[(item.id, crop_edits(0.0, 0.0, 0.5, 0.5))])
        .expect("edits write");

    let frame = request_thumbnail_frame(&engine, item.id, 32);
    assert_eq!(
        (frame.width, frame.height),
        (32, 16),
        "the thumbnail shows the top-left quarter of the 64x32 decode"
    );
    assert_eq!(
        xxhash_rust::xxh3::xxh3_64(&fs::read(&item.path).expect("read")),
        original_hash,
        "an edit must never rewrite the original file"
    );
    let cached = engine
        .cache
        .get_cached(item.id, &item.path, item.file_size, item.mtime_epoch_ms, 64)
        .expect("a 2x crop decode is cached at its decode size");
    assert_eq!(
        (cached.width, cached.height),
        (64, 32),
        "the cache holds the uncropped decode"
    );

    let unedited = {
        let (tx, rx) = mpsc::channel();
        engine.request_unedited_frame(item.id, 64, move |frame| {
            let _ = tx.send(frame);
        });
        rx.recv_timeout(TEST_WAIT)
            .expect("callback arrives")
            .expect("frame decodes")
    };
    assert_eq!(
        (unedited.width, unedited.height),
        (64, 32),
        "the unedited frame ignores the crop"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unedited_frame_ignores_rotation_and_exposure() {
    let dir = temp_library("unedited_frame");
    let (engine, item) = engine_with_image(&dir, "photo.png", 8, 4);
    let plain = request_thumbnail_frame(&engine, item.id, 16);
    engine
        .catalog
        .set_edits(&[(
            item.id,
            Edits {
                rotation: 1,
                exposure_ev: 1.0,
                ..Default::default()
            },
        )])
        .expect("edits write");
    assert_eq!(
        (request_thumbnail_frame(&engine, item.id, 16).width),
        4,
        "the thumbnail is rotated"
    );
    let (tx, rx) = mpsc::channel();
    engine.request_unedited_frame(item.id, 16, move |frame| {
        let _ = tx.send(frame);
    });
    let unedited = rx
        .recv_timeout(TEST_WAIT)
        .expect("callback arrives")
        .expect("frame decodes");
    assert_eq!(
        (unedited.width, unedited.height),
        (plain.width, plain.height),
        "the unedited frame has the size of the decode before any edit"
    );
    // The first decode was fresh; this one is read from the cache, which
    // stores entries as JPEG, so pixels match within compression error.
    let worst = unedited
        .data
        .iter()
        .zip(&plain.data)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(
        worst <= 12,
        "the unedited frame matches the decode before any edit (worst channel off by {worst})"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}

fn cache_file_count(dir: &Path) -> usize {
    fn walk(path: &Path) -> usize {
        let Ok(entries) = fs::read_dir(path) else {
            return 0;
        };
        entries
            .flatten()
            .map(|entry| match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&entry.path()),
                Ok(kind) if kind.is_file() => 1,
                _ => 0,
            })
            .sum()
    }
    walk(&dir.join("cache"))
}

#[test]
fn oversize_edit_decode_is_not_cached() {
    let dir = temp_library("oversize_edit");
    let (engine, item) = engine_with_image(&dir, "photo.png", 64, 64);
    request_thumbnail_frame(&engine, item.id, 16);
    assert_eq!(
        cache_file_count(&dir),
        1,
        "control: an unedited decode is written to the cache"
    );
    engine
        .catalog
        .set_edits(&[(item.id, crop_edits(0.5, 0.5, 0.25, 0.25))])
        .expect("edits write");
    // Decode size is 16 / 0.25 = 64 > 2 x 16: decoded per request.
    let frame = request_thumbnail_frame(&engine, item.id, 16);
    assert_eq!((frame.width, frame.height), (16, 16), "the crop is applied");
    assert_eq!(
        cache_file_count(&dir),
        1,
        "a decode above twice the target must not be written to the disk cache"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn uncacheable_edit_decode_still_reads_the_cache() {
    let dir = temp_library("uncacheable_read");
    let (engine, item) = engine_with_image(&dir, "photo.png", 64, 64);
    // The unedited frame at 64 is cached, the size a 0.25 crop at 16 needs.
    let (tx, rx) = mpsc::channel();
    engine.request_unedited_frame(item.id, 64, move |frame| {
        let _ = tx.send(frame);
    });
    rx.recv_timeout(TEST_WAIT)
        .expect("callback arrives")
        .expect("frame decodes");
    fs::remove_file(&item.path).expect("original removable");
    engine
        .catalog
        .set_edits(&[(item.id, crop_edits(0.5, 0.5, 0.25, 0.25))])
        .expect("edits write");
    let frame = request_thumbnail_frame(&engine, item.id, 16);
    assert_eq!(
        (frame.width, frame.height),
        (16, 16),
        "a decode too large to write is still read from the cache when present"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cropped_viewer_decode_is_not_cached() {
    let dir = temp_library("viewer_crop");
    let (engine, item) = engine_with_image(&dir, "photo.png", 2100, 1050);
    // The viewer's request size (VIEWER_IMAGE_SIZE in the UI).
    let viewer = 2048;
    let plain = request_thumbnail_frame(&engine, item.id, viewer);
    assert_eq!((plain.width, plain.height), (2048, 1024));
    assert_eq!(
        cache_file_count(&dir),
        0,
        "a viewer-size decode must not be written to the disk cache"
    );
    engine
        .catalog
        .set_edits(&[(item.id, crop_edits(0.0, 0.0, 0.5, 0.5))])
        .expect("edits write");
    // Decoded at the original's 2100 px: over the 2048 cache bound.
    let frame = request_thumbnail_frame(&engine, item.id, viewer);
    assert_eq!(
        (frame.width, frame.height),
        (1050, 525),
        "the crop comes from a full-size decode"
    );
    assert_eq!(
        cache_file_count(&dir),
        0,
        "a cropped viewer decode must not be written to the disk cache"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn grid_size_decodes_are_cached_and_viewer_size_decodes_are_not() {
    let dir = temp_library("viewer_size_cache");
    let (engine, item) = engine_with_image(&dir, "photo.png", 2100, 1050);
    request_thumbnail_frame(&engine, item.id, 1280);
    assert_eq!(
        cache_file_count(&dir),
        1,
        "the largest grid thumbnail is written to the disk cache"
    );
    request_thumbnail_frame(&engine, item.id, 2048);
    assert_eq!(
        cache_file_count(&dir),
        1,
        "a 2048 px viewer decode must not be written to the disk cache"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&dir);
}
