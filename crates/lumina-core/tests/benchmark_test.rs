use lumina_core::{LuminaEngine, Scanner};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

#[test]
#[ignore = "performance gate: run via make bench"]
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
#[ignore = "performance gate: run via make bench"]
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

    // Opt-in external media directory via LUMINA_BENCH_MEDIA_DIR; defaults to synthetic
    // real JPEG files so the test runs isolated and deterministically per AGENTS.md §2.
    let custom_media = std::env::var_os("LUMINA_BENCH_MEDIA_DIR").map(PathBuf::from);
    let (target_folder, _cleanup_media) =
        if let Some(media_dir) = custom_media.filter(|p| p.is_dir()) {
            (media_dir, false)
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
        "Scan Benchmark: indexed {} items in {:?} ({:.0} items/sec)",
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
        "Thumbnail Benchmark: decoded {}/{} items in {:?} ({:.1} frames/sec)",
        decoded_count,
        slice.len(),
        decode_elapsed,
        decode_rate
    );

    // QuickLook/AVFoundation poster extraction may be unavailable in headless,
    // sandboxed, or CI environments. Report partial decode results but only treat
    // them as fatal in strict benchmark mode.
    if decoded_count < slice.len() {
        eprintln!(
            "warning: decoded {}/{} thumbnails; QuickLook/AVFoundation may be unavailable",
            decoded_count,
            slice.len()
        );
    }
    assert!(
        decoded_count > 0,
        "expected at least one decodable thumbnail"
    );
    if std::env::var_os("LUMINA_STRICT_BENCH").is_some() {
        assert_eq!(decoded_count, slice.len());
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
#[ignore = "performance gate: run via make bench"]
fn benchmark_scale_and_gates() {
    use lumina_core::{Catalog, Flag, MediaId, MediaItem, MediaType, StructuredQuery, SystemAlbum};

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

        // Every 50th file is a "beach" shot so `-beach` has rows to exclude.
        // Every 20th from 8 is a Sony (even i) "sunset" shot for the
        // saved-search album gate; the rating spread below rates about 30%
        // of them. Every 1000th from 7 (25 rows, none of the others) is an
        // "aurora" shot: the rare-term saved search.
        let file_name = if i % 50 == 0 {
            format!("beach_{:06}.jpg", i)
        } else if i % 20 == 8 {
            format!("sunset_{:06}.jpg", i)
        } else if i % 1000 == 7 {
            format!("aurora_{:06}.jpg", i)
        } else {
            format!("photo_{:06}.jpg", i)
        };
        items.push(MediaItem {
            id: MediaId(0),
            path: PathBuf::from(format!("/library/folder_{}/{}", i / 1000, file_name)),
            parent_dir: format!("/library/folder_{}", i / 1000),
            file_name,
            title: None,
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

    // Realistic rating/label spreads so the rating and color gates measure
    // real matches: ratings 1-5 on 30% of rows, red on 2%, other labels on 8%.
    let ids: Vec<MediaId> = catalog
        .get_item_slice(0, count)
        .expect("list bench ids")
        .into_iter()
        .map(|m| m.id)
        .collect();
    let ratings: Vec<(MediaId, u8)> = ids
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 10 < 3)
        .map(|(i, &id)| (id, (i / 10 % 5) as u8 + 1))
        .collect();
    let labels: Vec<(MediaId, u8)> = ids
        .iter()
        .enumerate()
        .filter_map(|(i, &id)| match i % 50 {
            0 => Some((id, 1)),
            1..=4 => Some((id, (i / 50 % 5) as u8 + 2)),
            _ => None,
        })
        .collect();
    catalog.set_ratings(&ratings).expect("seed bench ratings");
    catalog
        .set_color_labels(&labels)
        .expect("seed bench labels");
    // Culling: rejects on 2% of rows (rare, like `color:red`), picks on 6%.
    let flags: Vec<(MediaId, Flag)> = ids
        .iter()
        .enumerate()
        .filter_map(|(i, &id)| match i % 50 {
            25 => Some((id, Flag::Reject)),
            26..=28 => Some((id, Flag::Pick)),
            _ => None,
        })
        .collect();
    catalog.set_flags(&flags).expect("seed bench flags");

    // Recompute timeline buckets
    catalog
        .compute_timeline_buckets()
        .expect("compute timeline buckets");

    // Gate 1: Check Database Size on Disk
    // In WAL mode recent pages live in catalog.db-wal until a checkpoint, so
    // the main file alone underreports; fold the WAL in before measuring.
    let busy: i64 = rusqlite::Connection::open(&temp_db)
        .and_then(|c| c.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0)))
        .expect("checkpoint the WAL before measuring the catalog size");
    assert_eq!(
        busy, 0,
        "WAL checkpoint was blocked; the size gate would underreport"
    );
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

    // Debug builds are slower and shared CI runners are noisy; release keeps
    // the strict 1ms/5ms/8ms gates. Run the bench targets in release for a
    // production-equivalent signal.
    let (seek_budget, query_budget, album_budget) = if cfg!(debug_assertions) {
        (5.0, 25.0, 40.0)
    } else {
        (1.0, 5.0, 8.0)
    };

    // Gate 2: Timeline Seek Latency
    let seek_start = Instant::now();
    let iterations = 100;
    for _ in 0..iterations {
        let _ = catalog.get_item_slice(10_000, 50);
    }
    let seek_elapsed = seek_start.elapsed();
    let seek_per_query_ms = (seek_elapsed.as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 2 - Timeline Seek Latency: {:.3} ms/slice (< {:.1} ms target)",
        seek_per_query_ms, seek_budget
    );
    assert!(
        seek_per_query_ms < seek_budget,
        "Timeline seek must resolve in < {seek_budget}ms (got {seek_per_query_ms:.3}ms)"
    );

    // Gate 2: Structured Query Latency (< 5ms)
    // (query, offset). Offset 450 on `color:red` / `is:reject` is nearly the
    // last page of ~495 rows: the deep-page case for a rare filter. Both have
    // their own index (`idx_media_color`, `idx_media_flag`); at this 25k
    // scale a date-index walk would still pass, so the plan tests in
    // core_tests.rs are what guard those indexes.
    let gate = |text: &str, offset: usize, note: &str| {
        let q = StructuredQuery::parse(text);
        let first = catalog
            .search_structured(&q, 50, offset)
            .unwrap_or_else(|e| panic!("bench query `{text}` must run: {e:#}"));
        assert!(
            !first.is_empty(),
            "bench query `{text}` must match rows so the gate measures real work"
        );
        let search_start = Instant::now();
        for _ in 0..iterations {
            catalog
                .search_structured(&q, 50, offset)
                .expect("bench query must keep running");
        }
        let search_elapsed = search_start.elapsed();
        let search_per_query_ms = (search_elapsed.as_secs_f64() * 1000.0) / iterations as f64;
        println!(
            "Gate 2 - Structured Query Latency `{text}`{note} @{offset}: {:.3} ms/query (< {:.1} ms target)",
            search_per_query_ms, query_budget
        );
        assert!(
            search_per_query_ms < query_budget,
            "Structured query `{text}`{note} must resolve in < {query_budget}ms (got {search_per_query_ms:.3}ms)"
        );
    };
    for (text, offset) in [
        ("camera:Sony is:favorite iso:>800", 0),
        ("-beach is:favorite", 0),
        ("rating:>=4", 0),
        ("color:red", 0),
        ("color:red", 450),
        ("is:unrated", 0),
        ("is:unlabeled", 0),
        ("is:pick", 0),
        ("is:reject", 0),
        ("is:reject", 450),
    ] {
        gate(text, offset, "");
    }
    // Rare rejects: thin them to 0.1% of rows, so page 0 is also the last
    // page and a date-index walk would cover the whole timeline.
    let unreject: Vec<(MediaId, Flag)> = ids
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 50 == 25 && i % 1000 != 25)
        .map(|(_, &id)| (id, Flag::Unflagged))
        .collect();
    catalog.set_flags(&unreject).expect("thin bench rejects");
    gate("is:reject", 0, " (rare 0.1%, last page)");

    // Gate 3: Smart Album Evaluation Latency (< 8ms)
    let album_start = Instant::now();
    for _ in 0..iterations {
        let _ = catalog.query_system_album(SystemAlbum::Favorites, 50, 0);
    }
    let album_elapsed = album_start.elapsed();
    let album_per_query_ms = (album_elapsed.as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 3 - Smart Album Latency: {:.3} ms/album (< {:.1} ms target)",
        album_per_query_ms, album_budget
    );
    assert!(
        album_per_query_ms < album_budget,
        "Smart album query must resolve in < {album_budget}ms (got {album_per_query_ms:.3}ms)"
    );

    // Gate 3: saved searches (smart album `Query` rules) under the default
    // date sort: a common term with a LIKE and a rating filter, and a rare
    // term (25 rows), which walks the date index. Then the sidebar count for
    // each, which drives from the FTS rowids instead.
    for (label, text) in [
        ("common", "sunset camera:Sony rating:>=3"),
        ("rare", "aurora"),
    ] {
        let saved_search = lumina_core::SmartAlbum::new(
            text.to_string(),
            "folder".to_string(),
            lumina_core::MatchMode::All,
            vec![lumina_core::SmartAlbumRule {
                field: lumina_core::RuleField::Query,
                operator: lumina_core::RuleOperator::Equals,
                value: lumina_core::RuleValue::Text(text.to_string()),
            }],
        );
        assert!(
            !catalog
                .query_smart_album(&saved_search, 50, 0)
                .expect("saved-search album must run")
                .is_empty(),
            "the {label} saved-search album must match rows so the gate measures real work"
        );
        let saved_start = Instant::now();
        for _ in 0..iterations {
            catalog
                .query_smart_album(&saved_search, 50, 0)
                .expect("saved-search album must keep running");
        }
        let saved_per_query_ms = (saved_start.elapsed().as_secs_f64() * 1000.0) / iterations as f64;
        println!(
            "Gate 3 - Saved Search Album Latency ({label} `{text}`): {:.3} ms/album (< {:.1} ms target)",
            saved_per_query_ms, album_budget
        );
        assert!(
            saved_per_query_ms < album_budget,
            "{label} saved-search album must resolve in < {album_budget}ms (got {saved_per_query_ms:.3}ms)"
        );

        assert!(
            catalog
                .query_smart_album_count(&saved_search)
                .expect("saved-search count must run")
                > 0,
            "the {label} saved-search count must be non-zero so the gate measures real work"
        );
        let count_start = Instant::now();
        for _ in 0..iterations {
            catalog
                .query_smart_album_count(&saved_search)
                .expect("saved-search count must keep running");
        }
        let count_per_query_ms = (count_start.elapsed().as_secs_f64() * 1000.0) / iterations as f64;
        println!(
            "Gate 3 - Saved Search Count Latency ({label} `{text}`): {:.3} ms/count (< {:.1} ms target)",
            count_per_query_ms, album_budget
        );
        assert!(
            count_per_query_ms < album_budget,
            "{label} saved-search count must resolve in < {album_budget}ms (got {count_per_query_ms:.3}ms)"
        );
    }

    // Gate 3: the Downloaded count the sidebar refreshes after each
    // download (1% of the library downloaded).
    for (n, mut item) in catalog
        .get_item_slice(0, count / 100)
        .expect("slice")
        .into_iter()
        .enumerate()
    {
        item.path = format!("/bench/Imports/dl_{n}.mp4").into();
        catalog
            .insert_with_source(&item, &format!("https://example.com/{n}"), None)
            .expect("insert download");
    }
    let count_start = Instant::now();
    for _ in 0..iterations {
        let _ = catalog.query_system_album_count(SystemAlbum::Downloaded);
    }
    let count_per_query_ms = (count_start.elapsed().as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 3 - Downloaded Count Latency: {:.3} ms/count (< {:.1} ms target)",
        count_per_query_ms, album_budget
    );
    assert!(
        count_per_query_ms < album_budget,
        "Downloaded count must resolve in < {album_budget}ms (got {count_per_query_ms:.3}ms)"
    );

    // Gate 3: hand-picked albums, a 500-member and a 20k-member one. The
    // join cannot use idx_media_timestamp for the ORDER BY, so each page of
    // the big album sorts its members; the deep offset is near the last page.
    let small_album = catalog.create_album("Small").expect("create album");
    let small_members: Vec<MediaId> = ids.iter().copied().step_by(50).collect();
    catalog
        .add_to_album(small_album.id, &small_members)
        .expect("fill small album");
    let big_album = catalog.create_album("Big").expect("create album");
    catalog
        .add_to_album(big_album.id, &ids[..20_000])
        .expect("fill big album");
    let sort = lumina_core::SortSpec::default();
    for (label, album, offset) in [
        ("500-member", small_album.id, 0),
        ("20k-member", big_album.id, 0),
        ("20k-member", big_album.id, 19_000),
    ] {
        assert_eq!(
            catalog
                .query_album_sorted(album, 50, offset, sort)
                .expect("album slice must run")
                .len(),
            50,
            "the {label} album page must be full so the gate measures real work"
        );
        let slice_start = Instant::now();
        for _ in 0..iterations {
            catalog
                .query_album_sorted(album, 50, offset, sort)
                .expect("album slice must keep running");
        }
        let slice_ms = (slice_start.elapsed().as_secs_f64() * 1000.0) / iterations as f64;
        println!(
            "Gate 3 - Album Slice Latency ({label} @{offset}): {slice_ms:.3} ms/slice (< {album_budget:.1} ms target)"
        );
        assert!(
            slice_ms < album_budget,
            "{label} album slice @{offset} must resolve in < {album_budget}ms (got {slice_ms:.3}ms)"
        );
    }
    for (label, album) in [("500-member", small_album.id), ("20k-member", big_album.id)] {
        let count_start = Instant::now();
        for _ in 0..iterations {
            catalog
                .query_album_count(album)
                .expect("album count must keep running");
        }
        let count_ms = (count_start.elapsed().as_secs_f64() * 1000.0) / iterations as f64;
        println!(
            "Gate 3 - Album Count Latency ({label}): {count_ms:.3} ms/count (< {album_budget:.1} ms target)"
        );
        assert!(
            count_ms < album_budget,
            "{label} album count must resolve in < {album_budget}ms (got {count_ms:.3}ms)"
        );
    }
    let list_start = Instant::now();
    for _ in 0..iterations {
        catalog
            .list_albums_with_counts()
            .expect("album list must keep running");
    }
    let list_ms = (list_start.elapsed().as_secs_f64() * 1000.0) / iterations as f64;
    println!(
        "Gate 3 - Album List With Counts Latency: {list_ms:.3} ms/list (< {album_budget:.1} ms target)"
    );
    assert!(
        list_ms < album_budget,
        "album list with counts must resolve in < {album_budget}ms (got {list_ms:.3}ms)"
    );

    let _ = std::fs::remove_file(&temp_db);
}
