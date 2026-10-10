use lumina_core::{
    Catalog, DiskCache, MediaId, MediaItem, MediaType, PixelFormat, DecodedFrame, Scanner,
};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[test]
fn test_catalog_in_memory() {
    let catalog = Catalog::open_in_memory().expect("open in memory catalog");
    assert_eq!(catalog.total_count().unwrap(), 0);

    let item1 = MediaItem {
        id: MediaId(0),
        path: PathBuf::from("/test/photo1.jpg"),
        media_type: MediaType::Image,
        width: 1920,
        height: 1080,
        duration_secs: 0.0,
        file_size: 2048,
        mtime_epoch_ms: 1000,
        timestamp_epoch_ms: 1000,
    };

    let item2 = MediaItem {
        id: MediaId(0),
        path: PathBuf::from("/test/video1.mp4"),
        media_type: MediaType::Video,
        width: 1280,
        height: 720,
        duration_secs: 12.5,
        file_size: 40960,
        mtime_epoch_ms: 2000,
        timestamp_epoch_ms: 2000,
    };

    let inserted = catalog
        .insert_or_update_batch(&[item1.clone(), item2.clone()])
        .expect("insert batch");
    assert_eq!(inserted.len(), 2);
    assert_eq!(catalog.total_count().unwrap(), 2);

    let slice = catalog.get_item_slice(0, 10).expect("get item slice");
    assert_eq!(slice.len(), 2);
    // Ordered by timestamp desc: video1 (2000) should be first
    assert_eq!(slice[0].path, item2.path);
    assert_eq!(slice[1].path, item1.path);

    // Test config key-value store
    catalog.set_config("last_library", "/path/to/library").unwrap();
    assert_eq!(
        catalog.get_config("last_library").unwrap(),
        Some("/path/to/library".to_string())
    );

    // Test removing items
    let mut valid = HashSet::new();
    valid.insert(item2.path.clone());
    let removed = catalog.remove_items_not_in(&valid).unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(catalog.total_count().unwrap(), 1);
}

#[test]
fn test_disk_cache() {
    let temp_dir = std::env::temp_dir().join("lumina_test_cache");
    let cache = DiskCache::with_directory(&temp_dir).expect("create test cache");
    cache.clear().expect("clear test cache");

    let test_path = PathBuf::from("/test/photo.jpg");
    let frame = DecodedFrame {
        id: MediaId(42),
        width: 4,
        height: 4,
        format: PixelFormat::Rgba8,
        data: vec![255; 4 * 4 * 4],
    };

    cache
        .put_cached(&test_path, 1024, 12345, 256, &frame)
        .expect("put cache");

    let retrieved = cache
        .get_cached(MediaId(42), &test_path, 1024, 12345, 256)
        .expect("get cached");

    assert_eq!(retrieved.width, 4);
    assert_eq!(retrieved.height, 4);
    assert_eq!(retrieved.format, PixelFormat::Rgba8);
    assert_eq!(retrieved.data.len(), 4 * 4 * 4);

    // Missing key returns None
    assert!(cache.get_cached(MediaId(42), &test_path, 9999, 12345, 256).is_none());

    let _ = cache.clear();
}

#[test]
fn test_scanner_on_temp_dir() {
    let temp_dir = std::env::temp_dir().join("lumina_test_scanner");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create dummy files
    let mut f1 = File::create(temp_dir.join("sample.jpg")).unwrap();
    f1.write_all(b"fake image data").unwrap();

    let mut f2 = File::create(temp_dir.join("clip.mp4")).unwrap();
    f2.write_all(b"fake video data").unwrap();

    let mut f3 = File::create(temp_dir.join("ignore.txt")).unwrap();
    f3.write_all(b"text file").unwrap();

    let mut f4 = File::create(temp_dir.join(".hidden.jpg")).unwrap();
    f4.write_all(b"hidden").unwrap();

    let (tx, rx) = crossbeam_channel::unbounded();
    let scanner = Scanner::new(temp_dir.clone(), tx);
    let count = scanner.scan().expect("scan");
    assert_eq!(count, 2);

    let mut batch_count = 0;
    while let Ok(evt) = rx.try_recv() {
        if let lumina_core::EngineEvent::BatchDiscovered { items } = evt {
            batch_count += items.len();
        }
    }
    assert_eq!(batch_count, 2);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_desktop_pics_integration() {
    let desktop_pics = PathBuf::from("/Users/raymondottun/Desktop/pics");
    if !desktop_pics.exists() {
        return;
    }

    let (engine, rx) = lumina_core::LuminaEngine::open(desktop_pics).expect("open engine on desktop pics");

    // Wait for ScanComplete
    let mut total_discovered = 0;
    while let Ok(evt) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
        match evt {
            lumina_core::EngineEvent::ScanComplete { total_items } => {
                total_discovered = total_items;
                break;
            }
            _ => {}
        }
    }

    println!("Scanned desktop pics: found {} items", total_discovered);
    assert!(total_discovered > 0, "Should discover items in Desktop/pics");

    let slice = engine.get_item_slice(0..10);
    assert!(!slice.is_empty());

    // Test thumbnail decoding for first item
    let first = &slice[0];
    let (tx_thumb, rx_thumb) = crossbeam_channel::bounded(1);
    engine.request_thumbnail(first.id, 256, move |frame| {
        let _ = tx_thumb.send(frame);
    });

    let frame_opt = rx_thumb.recv_timeout(std::time::Duration::from_secs(5)).expect("receive thumbnail");
    assert!(frame_opt.is_some(), "Thumbnail decode succeeded");
    let frame = frame_opt.unwrap();
    assert!(frame.width > 0 && frame.height > 0);
    assert_eq!(frame.data.len(), (frame.width * frame.height * 4) as usize);

    // Also verify a video file in Desktop/pics decodes
    let video_item = slice.iter().find(|i| i.media_type == MediaType::Video);
    if let Some(video) = video_item {
        let (tx_v, rx_v) = crossbeam_channel::bounded(1);
        engine.request_thumbnail(video.id, 256, move |frame| {
            let _ = tx_v.send(frame);
        });
        let v_opt = rx_v.recv_timeout(std::time::Duration::from_secs(5)).expect("receive video thumbnail");
        assert!(v_opt.is_some(), "Video thumbnail decode succeeded");
        let v_frame = v_opt.unwrap();
        assert!(v_frame.width > 0 && v_frame.height > 0);
        println!("Video thumbnail decoded: {}x{}", v_frame.width, v_frame.height);
    }
}

