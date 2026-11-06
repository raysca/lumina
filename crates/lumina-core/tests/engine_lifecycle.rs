//! Engine lifecycle: dropping the last handle to a `LuminaEngine` must stop
//! every thread it started (decoder pool, watcher, scan, cache cleanup).
//!
//! This file holds a single test so no other test's threads run in the same
//! process while it counts.

use lumina_core::{EngineEvent, LuminaEngine};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Threads in this process, as the OS reports them.
fn thread_count() -> usize {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_dir("/proc/self/task")
            .expect("read /proc/self/task")
            .count()
    }
    #[cfg(not(target_os = "linux"))]
    {
        // `ps -M` prints a header line, then one line per thread.
        let out = std::process::Command::new("ps")
            .args(["-M", "-p", &std::process::id().to_string()])
            .output()
            .expect("run ps -M");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .count()
            .saturating_sub(1)
    }
}

/// Polls until the thread count is at most `limit`, returning the last count.
fn wait_for_threads_at_most(limit: usize) -> usize {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let count = thread_count();
        if count <= limit || Instant::now() >= deadline {
            return count;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Polls until the thread count has not changed for half a second, so
/// threads of a just-dropped engine have finished exiting.
fn settled_thread_count() -> usize {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last = thread_count();
    let mut stable_since = Instant::now();
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        let count = thread_count();
        if count != last {
            last = count;
            stable_since = Instant::now();
        } else if stable_since.elapsed() >= Duration::from_millis(500) {
            break;
        }
    }
    last
}

/// Opens an engine on a fresh temp library, waits for the initial scan, then
/// drops every handle.
fn open_scan_and_drop(cycle: usize) {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root: PathBuf = std::env::temp_dir().join(format!(
        "lumina_engine_lifecycle_{}_{nanos}_{cycle}",
        std::process::id()
    ));
    let media = root.join("media");
    std::fs::create_dir_all(&media).expect("create temp media dir");
    std::fs::write(media.join("note.txt"), b"not media").expect("write temp file");

    let (engine, rx) =
        LuminaEngine::open_with_paths(media, root.join("catalog.db"), Some(root.join("cache")))
            .expect("open engine on a temp library");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(EngineEvent::ScanComplete { .. }) => break,
            Ok(_) => {}
            Err(err) => panic!("no ScanComplete from the initial scan: {err}"),
        }
    }
    drop(engine);
    drop(rx);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn dropping_the_engine_stops_its_threads() {
    // The first cycle can start process-wide threads that outlive any engine
    // (for example the system's file-event dispatch thread); measure after it.
    open_scan_and_drop(0);
    let baseline = settled_thread_count();

    const CYCLES: usize = 2;
    for cycle in 1..=CYCLES {
        open_scan_and_drop(cycle);
    }
    let after = wait_for_threads_at_most(baseline);
    assert!(
        after <= baseline,
        "each dropped engine must stop its threads: {after} threads after \
         {CYCLES} open/drop cycles, {baseline} after the first"
    );
}
