use super::*;
use crate::catalog::files::tests::{set_missing, TempDir};
use crate::{EngineEvent, MediaItem, MediaType};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const T0: i64 = 1_789_430_400_000;

fn item(
    dir: &str,
    name: &str,
    kind: MediaType,
    camera: Option<(&str, &str)>,
    ts: i64,
) -> MediaItem {
    MediaItem {
        id: MediaId(0),
        path: PathBuf::from(format!("{dir}/{name}")),
        parent_dir: dir.to_string(),
        file_name: name.to_string(),
        title: None,
        file_size: 1_000,
        mtime_epoch_ms: ts,
        media_type: kind,
        width: 4032,
        height: 3024,
        duration_ms: None,
        timestamp_epoch_ms: ts,
        year: 2026,
        month: 9,
        day: 1,
        latitude: None,
        longitude: None,
        altitude: None,
        country_code: None,
        admin1_region: None,
        city: None,
        make: camera.map(|c| c.0.to_string()),
        model: camera.map(|c| c.1.to_string()),
        lens: None,
        focal_length: None,
        f_number: None,
        iso: None,
        exposure_time: None,
        is_favorite: false,
        is_hidden: false,
        is_trashed: false,
    }
}

/// Inserts `items` and returns their ids in the same order.
fn insert(catalog: &Catalog, items: &[MediaItem]) -> Vec<MediaId> {
    let stored = catalog.insert_or_update_batch(items).expect("insert items");
    items
        .iter()
        .map(|it| {
            stored
                .iter()
                .find(|s| s.path == it.path)
                .expect("every inserted item comes back")
                .id
        })
        .collect()
}

fn stacks(catalog: &Catalog) -> Vec<Stack> {
    catalog.find_stacks().expect("find_stacks must succeed")
}

const SONY: Option<(&str, &str)> = Some(("SONY", "ILCE-7M4"));
const CANON: Option<(&str, &str)> = Some(("Canon", "EOS R5"));
const NIKON: Option<(&str, &str)> = Some(("NIKON", "Z8"));

#[test]
fn raw_jpeg_pair_grouped_by_stem() {
    let catalog = Catalog::open_in_memory().unwrap();
    let ids = insert(
        &catalog,
        &[
            item("/a", "DSC_1.ARW", MediaType::Raw, SONY, T0),
            item("/a", "dsc_1.jpg", MediaType::Image, SONY, T0),
            // Same stem in another folder: not a pair with /a.
            item("/b", "DSC_1.JPG", MediaType::Image, None, T0),
            // Two images, no RAW: never a pair.
            item("/c", "x.jpg", MediaType::Image, None, T0),
            item("/c", "x.png", MediaType::Image, None, T0),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![Stack {
            kind: StackKind::RawJpeg,
            items: vec![ids[1], ids[0]],
        }],
        "same dir + case-insensitive stem pairs one RAW with one image, image first"
    );
}

#[test]
fn ambiguous_pairs_are_skipped() {
    let catalog = Catalog::open_in_memory().unwrap();
    insert(
        &catalog,
        &[
            // One RAW, two JPEGs: which JPEG belongs is unknowable.
            item("/a", "IMG_1.CR3", MediaType::Raw, None, T0),
            item("/a", "IMG_1.JPG", MediaType::Image, None, T0),
            item("/a", "img_1.jpg", MediaType::Image, None, T0),
            // Two RAWs with one image.
            item("/a", "IMG_2.CR3", MediaType::Raw, None, T0),
            item("/a", "IMG_2.DNG", MediaType::Raw, None, T0),
            item("/a", "IMG_2.JPG", MediaType::Image, None, T0),
            // A video with an image (a Live Photo) is not RAW+JPEG.
            item("/a", "IMG_3.MOV", MediaType::Video, None, T0),
            item("/a", "IMG_3.HEIC", MediaType::Image, None, T0),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![],
        "a stem group that is not exactly one RAW plus one image is never paired"
    );
}

#[test]
fn burst_needs_three_within_one_second() {
    let catalog = Catalog::open_in_memory().unwrap();
    let ids = insert(
        &catalog,
        &[
            // Three shots in the same or next second: a burst.
            item("/a", "b1.jpg", MediaType::Image, SONY, T0),
            item("/a", "b2.jpg", MediaType::Image, SONY, T0),
            item("/a", "b3.jpg", MediaType::Image, SONY, T0 + 1000),
            // 1001 ms later: a new run of only two, not a burst.
            item("/a", "c1.jpg", MediaType::Image, SONY, T0 + 2001),
            item("/a", "c2.jpg", MediaType::Image, SONY, T0 + 3001),
            // A video next to them never extends a burst of stills.
            item("/a", "c3.mov", MediaType::Video, SONY, T0 + 3001),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![Stack {
            kind: StackKind::Burst,
            items: ids[0..3].to_vec(),
        }],
        "a burst is >= 3 stills with gaps <= 1000 ms, in time order"
    );
}

#[test]
fn different_cameras_never_stack() {
    let catalog = Catalog::open_in_memory().unwrap();
    insert(
        &catalog,
        &[
            item("/a", "s1.jpg", MediaType::Image, SONY, T0),
            item("/a", "c1.jpg", MediaType::Image, CANON, T0),
            item("/a", "s2.jpg", MediaType::Image, SONY, T0 + 500),
            item("/a", "c2.jpg", MediaType::Image, CANON, T0 + 500),
            // Same make, other model.
            item(
                "/a",
                "s3.jpg",
                MediaType::Image,
                Some(("SONY", "ILCE-1")),
                T0 + 900,
            ),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![],
        "interleaved shots from different make/model never form a burst"
    );
}

#[test]
fn rows_without_camera_never_burst() {
    let catalog = Catalog::open_in_memory().unwrap();
    // Screenshots copied together share one mtime second.
    let mut rows: Vec<MediaItem> = (0..5)
        .map(|i| item("/s", &format!("shot{i}.png"), MediaType::Image, None, T0))
        .collect();
    // Whitespace-only make and/or model count as blank (catches a lost trim).
    for camera in [(" ", " "), ("SONY", " ")] {
        rows.extend((0..3).map(|i| {
            item(
                "/s",
                &format!("blank_{}_{i}.png", camera.0.trim()),
                MediaType::Image,
                Some(camera),
                T0,
            )
        }));
    }
    insert(&catalog, &rows);
    assert_eq!(
        stacks(&catalog),
        vec![],
        "rows with no (or blank) make/model never form a burst"
    );
}

#[test]
fn trashed_items_never_stack() {
    let catalog = Catalog::open_in_memory().unwrap();
    let ids = insert(
        &catalog,
        &[
            item("/a", "P1.NEF", MediaType::Raw, NIKON, T0),
            item("/a", "P1.JPG", MediaType::Image, NIKON, T0),
            item("/a", "b1.jpg", MediaType::Image, CANON, T0 + 10_000),
            item("/a", "b2.jpg", MediaType::Image, CANON, T0 + 10_000),
            item("/a", "b3.jpg", MediaType::Image, CANON, T0 + 10_000),
        ],
    );
    assert_eq!(
        stacks(&catalog).len(),
        2,
        "precondition: a pair and a burst"
    );
    catalog.set_trashed(ids[0], true).unwrap();
    catalog
        .conn
        .lock()
        .execute(
            "UPDATE media_items SET is_hidden = 1 WHERE id = ?1;",
            [ids[4].0 as i64],
        )
        .unwrap();
    assert_eq!(
        stacks(&catalog),
        vec![],
        "a trashed RAW breaks its pair and a hidden shot breaks its burst"
    );
}

#[test]
fn pair_in_burst_counts_once_as_its_image() {
    let catalog = Catalog::open_in_memory().unwrap();
    let ids = insert(
        &catalog,
        &[
            // Shot 1 is RAW+JPEG; shots 2 and 3 are JPEG only.
            item("/a", "D1.ARW", MediaType::Raw, SONY, T0),
            item("/a", "D1.JPG", MediaType::Image, SONY, T0),
            item("/a", "D2.JPG", MediaType::Image, SONY, T0 + 400),
            item("/a", "D3.JPG", MediaType::Image, SONY, T0 + 800),
            // Two RAW+JPEG shots alone are two pairs, not a burst of 4.
            item("/a", "E1.ARW", MediaType::Raw, CANON, T0 + 60_000),
            item("/a", "E1.JPG", MediaType::Image, CANON, T0 + 60_000),
            item("/a", "E2.ARW", MediaType::Raw, CANON, T0 + 60_500),
            item("/a", "E2.JPG", MediaType::Image, CANON, T0 + 60_500),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![
            Stack {
                kind: StackKind::RawJpeg,
                items: vec![ids[1], ids[0]],
            },
            Stack {
                kind: StackKind::Burst,
                items: vec![ids[1], ids[2], ids[3]],
            },
            Stack {
                kind: StackKind::RawJpeg,
                items: vec![ids[5], ids[4]],
            },
            Stack {
                kind: StackKind::RawJpeg,
                items: vec![ids[7], ids[6]],
            },
        ],
        "a pair joins a burst once, as its image; stacks sort by first shot time, pairs first"
    );
}

#[test]
fn missing_items_never_stack() {
    let catalog = Catalog::open_in_memory().unwrap();
    let ids = insert(
        &catalog,
        &[
            item("/a", "P1.NEF", MediaType::Raw, NIKON, T0),
            item("/a", "P1.JPG", MediaType::Image, NIKON, T0),
            item("/a", "b1.jpg", MediaType::Image, CANON, T0 + 10_000),
            item("/a", "b2.jpg", MediaType::Image, CANON, T0 + 10_000),
            item("/a", "b3.jpg", MediaType::Image, CANON, T0 + 10_000),
        ],
    );
    assert_eq!(
        stacks(&catalog).len(),
        2,
        "precondition: a pair and a burst"
    );
    set_missing(&catalog, ids[0]);
    set_missing(&catalog, ids[4]);
    assert_eq!(
        stacks(&catalog),
        vec![],
        "a missing RAW breaks its pair and a missing ghost breaks its burst"
    );
}

#[test]
fn stack_read_walks_timestamp_index() {
    let catalog = Catalog::open_in_memory().unwrap();
    let plan = catalog
        .explain_query_plan(&stack_rows_sql())
        .expect("plan must be readable")
        .join(" | ");
    assert!(
        plan.contains("idx_media_timestamp") && !plan.contains("TEMP B-TREE"),
        "the stack read must walk idx_media_timestamp without a sort, got: {plan}"
    );
}

#[test]
fn camera_mismatch_never_pairs() {
    let catalog = Catalog::open_in_memory().unwrap();
    let ids = insert(
        &catalog,
        &[
            item(
                "/a",
                "DSC_0001.NEF",
                MediaType::Raw,
                Some(("NIKON", "Z8")),
                T0,
            ),
            item("/a", "DSC_0001.JPG", MediaType::Image, SONY, T0),
            // Control: the same camera on both sides pairs.
            item(
                "/a",
                "DSC_0002.NEF",
                MediaType::Raw,
                Some(("NIKON", "Z8")),
                T0 + 5000,
            ),
            item(
                "/a",
                "DSC_0002.JPG",
                MediaType::Image,
                Some(("NIKON", "Z8")),
                T0 + 5000,
            ),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![Stack {
            kind: StackKind::RawJpeg,
            items: vec![ids[3], ids[2]],
        }],
        "a RAW and an image from two different cameras never pair"
    );
}

#[test]
fn unknown_camera_never_pairs() {
    let catalog = Catalog::open_in_memory().unwrap();
    insert(
        &catalog,
        &[
            // A CR3 the EXIF reader cannot parse beside an unrelated iPhone
            // shot: Canon and Apple both name files IMG_NNNN.
            item("/a", "IMG_1234.CR3", MediaType::Raw, None, T0),
            item(
                "/a",
                "IMG_1234.HEIC",
                MediaType::Image,
                Some(("Apple", "iPhone 15 Pro")),
                T0,
            ),
            // Camera on the RAW only.
            item("/a", "DSC_0003.NEF", MediaType::Raw, NIKON, T0 + 5000),
            item("/a", "DSC_0003.JPG", MediaType::Image, None, T0 + 5000),
            // No camera on either side.
            item("/a", "IMG_0001.RAF", MediaType::Raw, None, T0 + 9000),
            item("/a", "IMG_0001.JPG", MediaType::Image, None, T0 + 9000),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![],
        "a pair needs a known, equal make and model on both sides"
    );
}

#[test]
fn bursts_never_span_folders() {
    let catalog = Catalog::open_in_memory().unwrap();
    // A backup copy of an import: same camera and times, two folders.
    insert(
        &catalog,
        &[
            item("/import", "a1.jpg", MediaType::Image, SONY, T0),
            item("/backup", "a1.jpg", MediaType::Image, SONY, T0),
            item("/import", "a2.jpg", MediaType::Image, SONY, T0 + 500),
            item("/backup", "a2.jpg", MediaType::Image, SONY, T0 + 500),
        ],
    );
    assert_eq!(
        stacks(&catalog),
        vec![],
        "two shots per folder are not a burst, even with four across folders"
    );
}

/// An engine over a temp library with one scanned photo given a camera.
fn engine_with_photo(prefix: &str) -> (TempDir, Arc<LuminaEngine>, MediaId) {
    let dir = TempDir::new(prefix);
    dir.write("photo.jpg", b"test-jpeg-content");
    let (engine, rx) = LuminaEngine::open_with_paths(
        dir.0.clone(),
        dir.0.join("catalog.db"),
        Some(dir.0.join("cache")),
    )
    .expect("engine opens");
    while let Ok(event) = rx.recv_timeout(Duration::from_secs(60)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    let id = engine
        .get_item_slice(0..1)
        .first()
        .expect("the scan must catalogue photo.jpg within 60 s")
        .id;
    engine
        .catalog()
        .conn
        .lock()
        .execute(
            "UPDATE media_items SET make = 'SONY', model = 'ILCE-7M4', timestamp_epoch_ms = ?1 WHERE id = ?2;",
            rusqlite::params![T0, id.0 as i64],
        )
        .expect("camera must be writable");
    (dir, engine, id)
}

#[test]
fn edit_copies_never_burst() {
    let (_dir, engine, id) = engine_with_photo("stacks_edit_copies");
    engine.duplicate_for_edit(id).expect("first copy");
    engine.duplicate_for_edit(id).expect("second copy");
    assert_eq!(
        engine.get_item_slice(0..10).len(),
        3,
        "precondition: original plus two copies"
    );
    assert_eq!(
        engine.find_stacks().expect("find_stacks must succeed"),
        vec![],
        "an original and its edit copies share time and camera but are not a burst"
    );
}

#[test]
fn edit_copy_does_not_make_a_burst_of_two_shots() {
    let (dir, engine, id) = engine_with_photo("stacks_edit_neighbour");
    engine.duplicate_for_edit(id).expect("copy");
    let mut neighbour = item(
        &dir.0.to_string_lossy(),
        "next.jpg",
        MediaType::Image,
        SONY,
        T0 + 500,
    );
    neighbour.path = dir.0.join("next.jpg");
    insert(engine.catalog(), &[neighbour]);
    assert_eq!(
        engine.find_stacks().expect("find_stacks must succeed"),
        vec![],
        "an original, its edit copy and one real neighbour are only two real shots"
    );
}
