use lumina_core::{
    Catalog, DiskCache, MediaId, MediaItem, MediaType, OfflineGeocoder, PixelFormat, DecodedFrame, Scanner,
};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

fn sample_media_item(
    path_str: &str,
    media_type: MediaType,
    ts: i64,
    city: Option<&str>,
    model: Option<&str>,
) -> MediaItem {
    let path = PathBuf::from(path_str);
    let parent_dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

    MediaItem {
        id: MediaId(0),
        path,
        parent_dir,
        file_name,
        file_size: 2048,
        mtime_epoch_ms: ts,
        media_type,
        width: 1920,
        height: 1080,
        duration_ms: if media_type == MediaType::Video { Some(12500) } else { None },
        duration_secs: if media_type == MediaType::Video { 12.5 } else { 0.0 },
        timestamp_epoch_ms: ts,
        year: 2026,
        month: 10,
        day: 10,
        latitude: Some(35.0116),
        longitude: Some(135.7681),
        altitude: Some(50.0),
        country_code: if city == Some("Paris") { Some("FR".to_string()) } else { Some("JP".to_string()) },
        admin1_region: if city == Some("Paris") { Some("Île-de-France".to_string()) } else { Some("Kansai".to_string()) },
        city: city.map(|s| s.to_string()),
        make: Some("Sony".to_string()),
        model: model.map(|s| s.to_string()),
        lens: Some("FE 35mm F1.4 GM".to_string()),
        focal_length: Some(35.0),
        f_number: Some(1.4),
        iso: Some(100),
        exposure_time: Some("1/500".to_string()),
        is_favorite: false,
        is_hidden: false,
        is_trashed: false,
    }
}

#[test]
fn test_catalog_in_memory() {
    let catalog = Catalog::open_in_memory().expect("open in memory catalog");
    assert_eq!(catalog.total_count().unwrap(), 0);

    let item1 = sample_media_item("/test/photo1.jpg", MediaType::Image, 1000, Some("Kyoto"), Some("ILCE-7M4"));
    let item2 = sample_media_item("/test/video1.mp4", MediaType::Video, 2000, Some("Kyoto"), Some("ILCE-7M4"));

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
fn test_timeline_buckets_aggregation() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let mut item1 = sample_media_item("/test/2026/oct/img1.jpg", MediaType::Image, 1791633600000, None, None);
    item1.year = 2026;
    item1.month = 10;
    item1.day = 10;

    let mut item2 = sample_media_item("/test/2026/oct/img2.jpg", MediaType::Image, 1791633700000, None, None);
    item2.year = 2026;
    item2.month = 10;
    item2.day = 10;

    let mut item3 = sample_media_item("/test/2025/dec/img3.jpg", MediaType::Image, 1765800000000, None, None);
    item3.year = 2025;
    item3.month = 12;
    item3.day = 15;

    catalog.insert_or_update_batch(&[item1, item2, item3]).unwrap();
    catalog.compute_timeline_buckets().unwrap();

    let year_buckets = catalog.get_timeline_buckets("year").unwrap();
    assert_eq!(year_buckets.len(), 2);
    assert_eq!(year_buckets[0].bucket_key, "2026");
    assert_eq!(year_buckets[0].item_count, 2);
    assert_eq!(year_buckets[1].bucket_key, "2025");
    assert_eq!(year_buckets[1].item_count, 1);

    let month_buckets = catalog.get_timeline_buckets("month").unwrap();
    assert_eq!(month_buckets.len(), 2);
    assert_eq!(month_buckets[0].bucket_key, "2026-10");
    assert_eq!(month_buckets[0].item_count, 2);
    assert_eq!(month_buckets[1].bucket_key, "2025-12");
    assert_eq!(month_buckets[1].item_count, 1);

    let day_buckets = catalog.get_timeline_buckets("day").unwrap();
    assert_eq!(day_buckets.len(), 2);
    assert_eq!(day_buckets[0].bucket_key, "2026-10-10");
    assert_eq!(day_buckets[0].item_count, 2);
}

#[test]
fn test_fts5_search() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item("/photos/vacation/sunset_kyoto.jpg", MediaType::Image, 1000, Some("Kyoto"), Some("A7IV"));
    let item2 = sample_media_item("/photos/paris/eiffel_tower.jpg", MediaType::Image, 2000, Some("Paris"), Some("EOS R5"));

    catalog.insert_or_update_batch(&[item1, item2]).unwrap();

    let matches_kyoto = catalog.search_fts("Kyoto", 10).unwrap();
    assert_eq!(matches_kyoto.len(), 1);
    assert_eq!(matches_kyoto[0].city.as_deref(), Some("Kyoto"));

    let matches_paris = catalog.search_fts("Paris", 10).unwrap();
    assert_eq!(matches_paris.len(), 1);
    assert_eq!(matches_paris[0].city.as_deref(), Some("Paris"));

    let matches_camera = catalog.search_fts("A7IV", 10).unwrap();
    assert_eq!(matches_camera.len(), 1);
    assert_eq!(matches_camera[0].model.as_deref(), Some("A7IV"));
}

#[test]
fn test_offline_geocoder_nearest_lookup() {
    let geocoder = OfflineGeocoder::new();

    // Tokyo: 35.6895, 139.6917
    let tokyo = geocoder.reverse_geocode(35.6895, 139.6917).expect("geocode tokyo");
    assert_eq!(tokyo.city, "Tokyo");
    assert_eq!(tokyo.country_code, "JP");

    // Paris: 48.8566, 2.3522
    let paris = geocoder.reverse_geocode(48.8566, 2.3522).expect("geocode paris");
    assert_eq!(paris.city, "Paris");
    assert_eq!(paris.country_code, "FR");

    // San Francisco: 37.7749, -122.4194
    let sf = geocoder.reverse_geocode(37.7749, -122.4194).expect("geocode sf");
    assert_eq!(sf.city, "San Francisco");
    assert_eq!(sf.country_code, "US");

    // Microsecond latency benchmark test: 1,000 lookups
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = geocoder.reverse_geocode(35.0116, 135.7681); // Kyoto
    }
    let elapsed = start.elapsed();
    let per_lookup_micros = elapsed.as_micros() as f64 / 1000.0;
    println!("Reverse geocode latency: {:.2} µs/lookup", per_lookup_micros);
    assert!(per_lookup_micros < 4.0, "Reverse geocoding must resolve in < 4µs");
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

    let _ = cache.clear();
}

#[test]
fn test_scanner_on_temp_dir() {
    let temp_dir = std::env::temp_dir().join("lumina_test_scanner");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut f1 = File::create(temp_dir.join("sample.jpg")).unwrap();
    f1.write_all(b"fake image data").unwrap();

    let mut f2 = File::create(temp_dir.join("clip.mp4")).unwrap();
    f2.write_all(b"fake video data").unwrap();

    let mut f3 = File::create(temp_dir.join("ignore.txt")).unwrap();
    f3.write_all(b"text file").unwrap();

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

    assert!(total_discovered > 0, "Should discover items in Desktop/pics");

    let slice = engine.get_item_slice(0..10);
    assert!(!slice.is_empty());

    // Verify rich metadata parsed
    for item in &slice {
        assert!(item.year >= 1970);
        assert!(item.month >= 1 && item.month <= 12);
        assert!(item.day >= 1 && item.day <= 31);
        assert!(!item.file_name.is_empty());
    }

    // Verify timeline buckets computed
    let year_buckets = engine.get_timeline_buckets("year");
    assert!(!year_buckets.is_empty(), "Should compute year buckets");

    // Test thumbnail decoding
    let first = &slice[0];
    let (tx_thumb, rx_thumb) = crossbeam_channel::bounded(1);
    engine.request_thumbnail(first.id, 256, move |frame| {
        let _ = tx_thumb.send(frame);
    });

    let frame_opt = rx_thumb.recv_timeout(std::time::Duration::from_secs(5)).expect("receive thumbnail");
    assert!(frame_opt.is_some(), "Thumbnail decode succeeded");
    let frame = frame_opt.unwrap();
    assert!(frame.width > 0 && frame.height > 0);
}
