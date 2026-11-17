//! Helpers shared by the core integration test files.

// Each test binary includes this module but uses only some of its helpers.
#![allow(dead_code)]

use lumina_core::{EngineEvent, LuminaEngine, MediaId, MediaItem, MediaType};
use std::path::PathBuf;

/// A catalog row for `path_str`: a Sony camera, 1920x1080, dated `ts`; a
/// `city` adds GPS and a place (France for Paris, Japan otherwise).
pub fn sample_media_item(
    path_str: &str,
    media_type: MediaType,
    ts: i64,
    city: Option<&str>,
    model: Option<&str>,
) -> MediaItem {
    let path = PathBuf::from(path_str);
    let parent_dir = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    MediaItem {
        id: MediaId(0),
        path,
        parent_dir,
        file_name,
        title: None,
        file_size: 2048,
        mtime_epoch_ms: ts,
        media_type,
        width: 1920,
        height: 1080,
        duration_ms: if media_type == MediaType::Video {
            Some(12500)
        } else {
            None
        },
        timestamp_epoch_ms: ts,
        year: 2026,
        month: 10,
        day: 10,
        latitude: if city.is_some() { Some(35.0116) } else { None },
        longitude: if city.is_some() { Some(135.7681) } else { None },
        altitude: if city.is_some() { Some(50.0) } else { None },
        country_code: if city == Some("Paris") {
            Some("FR".to_string())
        } else if city.is_some() {
            Some("JP".to_string())
        } else {
            None
        },
        admin1_region: if city == Some("Paris") {
            Some("Île-de-France".to_string())
        } else if city.is_some() {
            Some("Kansai".to_string())
        } else {
            None
        },
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

/// A fresh, empty directory under the system temp dir, unique per call.
pub fn unique_temp_dir(tag: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "lumina_{tag}_{}_{nanos}_{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Waits for the next `ScanComplete` and returns its total; a scan that does
/// not finish within ten seconds fails the test.
pub fn wait_for_scan(rx: &crossbeam_channel::Receiver<EngineEvent>) -> usize {
    loop {
        match rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(EngineEvent::ScanComplete { total_items }) => return total_items,
            Ok(_) => {}
            Err(err) => panic!("the scan must finish: {err}"),
        }
    }
}

/// A scanned library holding `files` in `<tmp>/library`, with its catalog at
/// `<tmp>/catalog.db`; returns the temp root, the engine and its events.
pub fn library_with(
    prefix: &str,
    files: &[(&str, Vec<u8>)],
) -> (
    PathBuf,
    std::sync::Arc<LuminaEngine>,
    crossbeam_channel::Receiver<EngineEvent>,
) {
    let tmp_dir = unique_temp_dir(prefix);
    let media_dir = tmp_dir.join("library");
    std::fs::create_dir_all(&media_dir).unwrap();
    for (name, bytes) in files {
        std::fs::write(media_dir.join(name), bytes).unwrap();
    }
    let (engine, rx) = LuminaEngine::open_with_paths(
        media_dir,
        tmp_dir.join("catalog.db"),
        Some(tmp_dir.join("cache")),
    )
    .unwrap();
    wait_for_scan(&rx);
    (tmp_dir, engine, rx)
}

/// A scanned one-photo library, with its catalog path for breaking it.
pub fn scanned_library(
    prefix: &str,
) -> (
    PathBuf,
    PathBuf,
    std::sync::Arc<LuminaEngine>,
    crossbeam_channel::Receiver<EngineEvent>,
) {
    let (tmp_dir, engine, rx) = library_with(prefix, &[("one.jpg", b"test-jpeg-content".to_vec())]);
    let db_path = tmp_dir.join("catalog.db");
    (tmp_dir, db_path, engine, rx)
}

/// A second connection to break the catalog; waits out a watcher write.
pub fn raw_catalog(db_path: &std::path::Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    conn
}

/// Inlines `?` parameters as SQL literals so `explain_query_plan` (which binds
/// nothing) can plan a synthesized query. Test-only: values are trusted.
pub fn inline_sql_params(sql: &str, params: &[rusqlite::types::Value]) -> String {
    use rusqlite::types::Value;
    assert_eq!(
        sql.matches('?').count(),
        params.len(),
        "every ? placeholder needs exactly one parameter"
    );
    let mut values = params.iter();
    sql.chars()
        .map(|c| {
            if c != '?' {
                return c.to_string();
            }
            match values.next() {
                Some(Value::Integer(n)) => n.to_string(),
                Some(Value::Real(f)) => f.to_string(),
                Some(Value::Text(t)) => format!("'{}'", t.replace('\'', "''")),
                Some(Value::Null) => "NULL".to_string(),
                other => panic!("inline_sql_params cannot inline {other:?}"),
            }
        })
        .collect()
}
