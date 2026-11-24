//! Tests for the parent module, kept beside it.

use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lumina_decode_guard_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A file to decode, with its version key.
fn photo(dir: &Path, name: &str) -> (PathBuf, FileKey) {
    let path = dir.join(name);
    fs::write(&path, name).expect("fixture file");
    let key = FileKey::of(&path).expect("key");
    (path, key)
}

/// A process (`pid`) that died while decoding `in_flight`: its markers stay.
fn died_during(dir: &Path, pid: u32, in_flight: &[(MediaId, FileKey)]) {
    for (n, (id, key)) in in_flight.iter().enumerate() {
        fs::write(
            dir.join(format!("{}-{pid}-{n}.{MARKER_EXTENSION}", id.0)),
            format!("{} {}", key.size, key.mtime_ms),
        )
        .expect("marker");
    }
}

const DEAD: u32 = u32::MAX - 1;
const LONG: Duration = Duration::from_secs(10);

fn decodes(guard: &Arc<DecodeGuard>, id: MediaId, path: &Path) -> Result<u8> {
    guarded_decode(Some(guard.clone()), id, path, LONG, || Ok(1))
}

#[test]
fn a_file_that_stopped_lumina_twice_alone_is_refused_until_it_changes() {
    let dir = temp_dir("refused");
    let (path, key) = photo(&dir, "a.heic");
    let id = MediaId(7);
    died_during(&dir, DEAD, &[(id, key)]);
    assert!(
        !DecodeGuard::open(dir.clone()).refuses(id, key),
        "one stop may be a quit mid-decode: the file is tried again"
    );
    died_during(&dir, DEAD - 1, &[(id, key)]);
    let guard = Arc::new(DecodeGuard::open(dir.clone()));
    let refused = decodes(&guard, id, &path);
    assert!(refused.is_err(), "a second stop in a row refuses the file");
    assert!(
        decodes(&guard, MediaId(8), &path).is_ok(),
        "other items are not affected"
    );
    fs::write(&path, "saved again, longer").expect("change the file");
    assert!(
        decodes(&guard, id, &path).is_ok(),
        "a changed file is tried again"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn files_in_flight_together_become_suspects_not_strikes() {
    let dir = temp_dir("suspects");
    let (path, key) = photo(&dir, "a.heic");
    let (_, other) = photo(&dir, "b.heic");
    let (a, b) = (MediaId(1), MediaId(2));
    for pid in [DEAD, DEAD - 1] {
        died_during(&dir, pid, &[(a, key), (b, other)]);
    }
    let guard = Arc::new(DecodeGuard::open(dir.clone()));
    assert!(
        !guard.refuses(a, key) && !guard.refuses(b, other),
        "any of the decodes running side by side may be the culprit"
    );
    assert!(guard.is_suspect(a, key), "so each is decoded alone next");
    assert!(decodes(&guard, a, &path).is_ok());
    assert!(
        !guard.is_suspect(a, key),
        "a clean decode clears the suspicion"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_clean_decode_clears_the_strikes() {
    let dir = temp_dir("cleared");
    let (path, key) = photo(&dir, "a.heic");
    let id = MediaId(3);
    died_during(&dir, DEAD, &[(id, key)]);
    let guard = Arc::new(DecodeGuard::open(dir.clone()));
    assert!(decodes(&guard, id, &path).is_ok());
    drop(guard);
    died_during(&dir, DEAD - 1, &[(id, key)]);
    assert!(
        !DecodeGuard::open(dir.clone()).refuses(id, key),
        "the stops were not in a row, so the file is not refused"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn markers_of_this_process_are_running_decodes_not_crashes() {
    let dir = temp_dir("own_markers");
    let (_, key) = photo(&dir, "a.heic");
    let id = MediaId(4);
    for _ in 0..2 {
        died_during(&dir, std::process::id(), &[(id, key)]);
        assert!(!DecodeGuard::open(dir.clone()).refuses(id, key));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_decode_leaves_no_marker_and_the_record_survives_reopening() {
    let dir = temp_dir("record");
    let (path, key) = photo(&dir, "a.heic");
    let guard = Arc::new(DecodeGuard::open(dir.clone()));
    assert!(decodes(&guard, MediaId(1), &path).is_ok());
    assert!(leftover_markers(&dir).is_empty());
    died_during(&dir, DEAD, &[(MediaId(9), key)]);
    drop(DecodeGuard::open(dir.clone()));
    died_during(&dir, DEAD - 1, &[(MediaId(9), key)]);
    assert!(DecodeGuard::open(dir.clone()).refuses(MediaId(9), key));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_hung_decode_is_reported_and_not_tried_again_this_session() {
    let dir = temp_dir("hung");
    let (path, _) = photo(&dir, "a.heic");
    let guard = Arc::new(DecodeGuard::open(dir.clone()));
    // Nothing is ever sent: the decode blocks until the test ends.
    let (_never, stuck) = std::sync::mpsc::channel::<()>();
    let started = std::time::Instant::now();
    let result: Result<u8> = guarded_decode(
        Some(guard.clone()),
        MediaId(5),
        &path,
        Duration::from_millis(50),
        move || {
            let _ = stuck.recv();
            Ok(1)
        },
    );
    assert!(result.is_err(), "the wait gives up");
    assert!(started.elapsed() < Duration::from_secs(5));
    let again = decodes(&guard, MediaId(5), &path);
    assert!(again.is_err(), "and does not start another stuck decode");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_panicking_decode_is_a_failure_not_a_timeout() {
    let result = run_bounded(LONG, || -> u8 { panic!("decoder panicked") });
    assert_eq!(result, Err(Bounded::Failed));
}
