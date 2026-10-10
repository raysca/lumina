use lumina_core::{LuminaEngine, Scanner};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

#[test]
fn benchmark_cold_scan_synthetic() {
    let temp_dir = std::env::temp_dir().join("lumina_bench_synthetic");
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
    assert!(items_per_sec > 1000.0, "Scan throughput should exceed 1,000 items/sec");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn benchmark_desktop_pics_cold_scan_and_decodes() {
    let desktop_pics = PathBuf::from("/Users/raymondottun/Desktop/pics");
    if !desktop_pics.exists() {
        return;
    }

    let start = Instant::now();
    let (engine, rx) = LuminaEngine::open(desktop_pics).expect("open engine");

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
}
