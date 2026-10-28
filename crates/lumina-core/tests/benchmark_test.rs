use lumina_core::{LuminaEngine, Scanner};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn benchmark_cold_scan_synthetic() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_bench_synthetic_{}_{}",
        std::process::id(),
        nanos
    ));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let count = 2000;
    for i in 0..count {
        let p = temp_dir.join(format!("synthetic_photo_{:04}.jpg", i));
        let mut f = File::create(p).unwrap();
        f.write_all(b"fake jpeg content").unwrap();
    }

    let (tx, rx) = crossbeam_channel::unbounded();
    let scanner = Scanner::new(temp_dir.clone(), tx);

    let start = Instant::now();
    let scanned = scanner.scan().expect("scanner benchmark");
    let elapsed = start.elapsed();

    let mut drained = 0;
    while let Ok(evt) = rx.try_recv() {
        if let lumina_core::EngineEvent::BatchDiscovered { items } = evt {
            drained += items.len();
        }
    }

    let items_per_sec = (scanned as f64) / elapsed.as_secs_f64();
    println!(
        "Cold Scan Benchmark: scanned {} synthetic files in {:?} ({:.0} items/sec)",
        scanned, elapsed, items_per_sec
    );

    assert_eq!(scanned, count);
    assert_eq!(drained, count);
    assert!(
        items_per_sec > 1000.0,
        "Scan throughput should exceed 1,000 items/sec"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn benchmark_desktop_pics_cold_scan_and_decodes() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_bench_pics_{}_{}",
        std::process::id(),
        nanos
    ));
    let db_path = temp_dir.join("bench_catalog.db");
    let cache_dir = temp_dir.join("bench_cache");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let desktop_pics = PathBuf::from("/Users/raymondottun/Desktop/pics");
    let (target_folder, _cleanup_media) = if desktop_pics.exists() && desktop_pics.is_dir() {
        (desktop_pics, false)
    } else {
        // Fallback: create synthetic real JPEG files so the benchmark runs everywhere
        let media_dir = temp_dir.join("media");
        std::fs::create_dir_all(&media_dir).unwrap();
        for i in 0..10 {
            let img = image::RgbImage::from_fn(64, 64, |x, y| {
                image::Rgb([(x * 4) as u8, (y * 4) as u8, (i * 25) as u8])
            });
            img.save(media_dir.join(format!("img_{:02}.jpg", i)))
                .unwrap();
        }
        (media_dir, true)
    };

    let start = Instant::now();
    // Open with ISOLATED db and cache to prevent touching production database!
    let (engine, rx) = LuminaEngine::open_with_paths(target_folder, db_path, Some(cache_dir))
        .expect("open engine with isolated paths");

    let mut total_discovered = 0;
    while let Ok(evt) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
        if let lumina_core::EngineEvent::ScanComplete { total_items } = evt {
            total_discovered = total_items;
            break;
        }
    }
    let scan_elapsed = start.elapsed();
    let scan_rate = (total_discovered as f64) / scan_elapsed.as_secs_f64();

    println!(
        "Desktop/pics Scan Benchmark: indexed {} items in {:?} ({:.0} items/sec)",
        total_discovered, scan_elapsed, scan_rate
    );

    // Benchmark thumbnail decode throughput
    let slice = engine.get_item_slice(0..10);
    let decode_start = Instant::now();
    let (tx, rx) = crossbeam_channel::unbounded();

    for item in &slice {
        let tx_c = tx.clone();
        engine.request_thumbnail(item.id, 256, move |frame| {
            let _ = tx_c.send(frame.is_some());
        });
    }

    let mut decoded_count = 0;
    for _ in 0..slice.len() {
        if let Ok(success) = rx.recv_timeout(std::time::Duration::from_secs(5)) {
            if success {
                decoded_count += 1;
            }
        }
    }
    let decode_elapsed = decode_start.elapsed();
    let decode_rate = (decoded_count as f64) / decode_elapsed.as_secs_f64();

    println!(
        "Desktop/pics Thumbnail Benchmark: decoded {}/{} items in {:?} ({:.1} frames/sec)",
        decoded_count,
        slice.len(),
        decode_elapsed,
        decode_rate
    );

    assert_eq!(decoded_count, slice.len());
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn benchmark_scale_and_gates() {
    use lumina_core::{Catalog, MediaId, MediaItem, MediaType, StructuredQuery, SystemAlbum};

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_db = std::env::temp_dir().join(format!(
        "lumina_gate_scale_{}_{}.db",
        std::process::id(),
        nanos
    ));
    let _ = std::fs::remove_file(&temp_db);

    let catalog = Catalog::open(&temp_db).expect("open catalog on disk");

    // Insert 25,000 synthetic items
    let count = 25_000;
    let batch_size = 5_000;
    let mut items = Vec::with_capacity(batch_size);

    let insert_start = Instant::now();
    for i in 0..count {
        let is_video = i % 10 == 0;
        let is_raw = i % 15 == 0;
        let m_type = if is_video {
            MediaType::Video
        } else if is_raw {
            MediaType::Raw
        } else {
            MediaType::Image
        };

        items.push(MediaItem {
            id: MediaId(0),
            path: PathBuf::from(format!("/library/folder_{}/photo_{:06}.jpg", i / 1000, i)),
            parent_dir: format!("/library/folder_{}", i / 1000),
            file_name: format!("photo_{:06}.jpg", i),
            file_size: 4_000_000 + (i as u64 % 100_000),
            mtime_epoch_ms: 1789430400000 + (i as i64 * 60_000),
            media_type: m_type,
            width: 3840,
            height: 2160,
            duration_ms: if is_video { Some(15_000) } else { None },
            timestamp_epoch_ms: 1789430400000 + (i as i64 * 60_000),
            year: 2026,
            month: ((i % 12) + 1) as u32,
            day: ((i % 28) + 1) as u32,
            latitude: if i % 3 == 0 { Some(35.0116) } else { None },
            longitude: if i % 3 == 0 { Some(135.7681) } else { None },
            altitude: Some(50.0),
            country_code: if i % 3 == 0 {
                Some("JP".to_string())
            } else {
                None
            },
            admin1_region: if i % 3 == 0 {
                Some("Kansai".to_string())
            } else {
                None
            },
            city: if i % 3 == 0 {
                Some("Kyoto".to_string())
            } else {
                None
            },
            make: if i % 2 == 0 {
                Some("Sony".to_string())
            } else {
                Some("Canon".to_string())
            },
            model: if i % 2 == 0 {
                Some("A7IV".to_string())
            } else {
                Some("R5".to_string())
            },
            lens: Some("35mm F1.4".to_string()),
            focal_length: Some(35.0),
            f_number: Some(1.4),
            iso: Some(400 + (i as u32 % 3200)),
            exposure_time: Some("1/250".to_string()),
            is_favorite: i % 7 == 0,
            is_hidden: false,
            is_trashed: i % 100 == 0,
        });

        if items.len() >= batch_size {
            catalog
                .insert_or_update_batch(&items)
                .expect("insert batch");
            items.clear();
        }
    }
    let insert_elapsed = insert_start.elapsed();
    println!(
        "Scale Test: Inserted {} items in {:.2}s ({:.0} items/sec)",
        count,
        insert_elapsed.as_secs_f64(),
        count as f64 / insert_elapsed.as_secs_f64()
    );

    // Recompute timeline buckets
    catalog
        .compute_timeline_buckets()
        .expect("compute timeline buckets");

    // Gate 1: Check Database Size on Disk
    let db_metadata = std::fs::metadata(&temp_db).expect("metadata db");
    let db_size_bytes = db_metadata.len();
    let bytes_per_record = db_size_bytes as f64 / count as f64;
    let projected_250k_mb = (bytes_per_record * 250_000.0) / (1024.0 * 1024.0);
    println!(
        "Gate 1 - DB Size: {} items = {:.2} MB ({:.1} bytes/item). Projected 250,000 items = {:.1} MB (< 180 MB target)",
        count,
        db_size_bytes as f64 / (1024.0 * 1024.0),
        bytes_per_record,
        projected_250k_mb
    );
    assert!(
        projected_250k_mb < 180.0,
        "Projected DB size for 250k items must be < 180 MB (got {:.1} MB)",
        projected_250k_mb
    );

    // Gate 2: Timeline Seek Latency (< 1ms)
    let seek_start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        let _ = catalog.get_item_slice(10_000, 50);
    }
    let seek_elapsed = seek_start.elapsed();
    let seek_per_query_ms = (seek_elapsed.as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 2 - Timeline Seek Latency: {:.3} ms/slice (< 1.0 ms target)",
        seek_per_query_ms
    );
    assert!(
        seek_per_query_ms < 1.0,
        "Timeline seek must resolve in < 1ms"
    );

    // Gate 2: Structured Query Latency (< 5ms)
    let q = StructuredQuery::parse("camera:Sony is:favorite iso:>800");
    let search_start = Instant::now();
    for _ in 0..iterations {
        let _ = catalog.search_structured(&q, 50, 0);
    }
    let search_elapsed = search_start.elapsed();
    let search_per_query_ms = (search_elapsed.as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 2 - Structured Query Latency: {:.3} ms/query (< 5.0 ms target)",
        search_per_query_ms
    );
    assert!(
        search_per_query_ms < 5.0,
        "Structured query must resolve in < 5ms"
    );

    // Gate 3: Smart Album Evaluation Latency (< 8ms)
    let album_start = Instant::now();
    for _ in 0..iterations {
        let _ = catalog.query_system_album(SystemAlbum::Favorites, 50, 0);
    }
    let album_elapsed = album_start.elapsed();
    let album_per_query_ms = (album_elapsed.as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 3 - Smart Album Latency: {:.3} ms/album (< 8.0 ms target)",
        album_per_query_ms
    );
    assert!(
        album_per_query_ms < 8.0,
        "Smart album query must resolve in < 8ms"
    );

    let _ = std::fs::remove_file(&temp_db);
}
