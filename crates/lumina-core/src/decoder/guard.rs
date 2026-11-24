//! Guards for in-process native decodes (ImageIO), which run in Lumina's
//! own process: a file that crashes the decoder takes the app down, and one
//! that hangs holds a decode thread forever.
//!
//! - **Crash guard.** Each decode leaves a marker file while it runs. A
//!   marker still there at the next open was left by a process that died
//!   mid-decode. When that process left one marker, it names the file: a
//!   strike. When it left several (decodes run side by side), any of them
//!   may be the culprit, so each becomes a suspect and is next decoded
//!   alone, where a crash names it. A quit mid-decode leaves markers too,
//!   so it takes two strikes in a row (no clean decode in between) to
//!   refuse a file, until it changes; it then shows "No preview" instead of
//!   crashing the app on every launch.
//! - **Hang guard.** A decode is waited for up to a timeout, then reported
//!   as failed and not tried again this session; the stuck thread cannot
//!   be killed, so it is left to finish and its result is dropped.
// ponytail: one guard per process (the open library's); after a library
// switch, decodes still running for the old one report to the new record.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use parking_lot::{Condvar, Mutex, RwLock};

use crate::MediaId;

/// Strikes before a file is refused.
const STRIKES_TO_REFUSE: u32 = 2;
const STRIKES_FILE: &str = "strikes";
const MARKER_EXTENSION: &str = "decoding";

/// Identifies one version of a file: a changed file gets a fresh start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileKey {
    size: u64,
    mtime_ms: i64,
}

impl FileKey {
    pub(crate) fn of(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok()?;
        let mtime_ms = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis();
        Some(Self {
            size: meta.len(),
            mtime_ms: i64::try_from(mtime_ms).ok()?,
        })
    }
}

/// What the record holds for one item, for the file version it was earned on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Record {
    key: FileKey,
    strikes: u32,
    /// Was in flight when a process died beside other decodes.
    suspect: bool,
}

/// Lets a suspect decode run alone: shared decodes wait while one runs,
/// and it waits for the shared ones to finish. Held by the waiting decode
/// thread, never by the native call, so a hung decode does not keep it.
#[derive(Default)]
struct Turns {
    state: Mutex<(usize, bool)>,
    changed: Condvar,
}

impl Turns {
    fn enter(&self, alone: bool) {
        let mut state = self.state.lock();
        loop {
            let (shared, solo) = *state;
            if !solo && (!alone || shared == 0) {
                if alone {
                    state.1 = true;
                } else {
                    state.0 += 1;
                }
                return;
            }
            self.changed.wait(&mut state);
        }
    }

    fn leave(&self, alone: bool) {
        let mut state = self.state.lock();
        if alone {
            state.1 = false;
        } else {
            state.0 = state.0.saturating_sub(1);
        }
        self.changed.notify_all();
    }
}

/// One library's crash record, kept in a folder of its thumbnail cache.
pub(crate) struct DecodeGuard {
    dir: PathBuf,
    records: Mutex<HashMap<MediaId, Record>>,
    /// Items that hung this session: not decoded again until the next one.
    gave_up: Mutex<HashSet<MediaId>>,
    turns: Turns,
    next_marker: AtomicU64,
}

impl DecodeGuard {
    /// Opens the record at `dir`, turning markers left by a process that
    /// died mid-decode into strikes or suspects.
    pub(crate) fn open(dir: PathBuf) -> Self {
        let mut records = read_records(&dir.join(STRIKES_FILE));
        let mut by_process: HashMap<u32, Vec<(MediaId, FileKey)>> = HashMap::new();
        for marker in leftover_markers(&dir) {
            // This process's own markers belong to decodes still running
            // (a library opened again in the same session).
            if marker.pid == std::process::id() {
                continue;
            }
            by_process
                .entry(marker.pid)
                .or_default()
                .push((marker.id, marker.key));
            if let Err(err) = fs::remove_file(&marker.path) {
                log::warn!(
                    "could not clear decode marker {}: {err}",
                    marker.path.display()
                );
            }
        }
        let changed = !by_process.is_empty();
        for in_flight in by_process.into_values() {
            let alone = in_flight.len() == 1;
            for (id, key) in in_flight {
                let record = records.entry(id).or_insert(Record {
                    key,
                    strikes: 0,
                    suspect: false,
                });
                if record.key != key {
                    *record = Record {
                        key,
                        strikes: 0,
                        suspect: false,
                    };
                }
                if alone {
                    record.strikes += 1;
                    record.suspect = false;
                    log::warn!(
                        "Lumina stopped while decoding item {id:?} ({} times in a row)",
                        record.strikes
                    );
                } else {
                    record.suspect = true;
                }
            }
        }
        let guard = Self {
            dir,
            records: Mutex::new(records),
            gave_up: Mutex::default(),
            turns: Turns::default(),
            next_marker: AtomicU64::new(0),
        };
        if changed {
            guard.save(&guard.records.lock());
        }
        guard
    }

    /// Whether `id` at `key` stopped the process often enough to be
    /// refused, or hung earlier this session.
    pub(crate) fn refuses(&self, id: MediaId, key: FileKey) -> bool {
        self.gave_up.lock().contains(&id)
            || self
                .records
                .lock()
                .get(&id)
                .is_some_and(|record| record.key == key && record.strikes >= STRIKES_TO_REFUSE)
    }

    /// Whether `id` at `key` must be decoded alone to tell if it crashes.
    fn is_suspect(&self, id: MediaId, key: FileKey) -> bool {
        self.records
            .lock()
            .get(&id)
            .is_some_and(|record| record.key == key && record.suspect)
    }

    /// Marks a decode of `id` as running until the returned marker drops.
    /// Best-effort: without the marker file the decode still runs.
    fn mark(self: &Arc<Self>, id: MediaId, key: FileKey) -> Marker {
        let n = self.next_marker.fetch_add(1, Ordering::Relaxed);
        let path = self.dir.join(format!(
            "{}-{}-{n}.{MARKER_EXTENSION}",
            id.0,
            std::process::id()
        ));
        let written = fs::create_dir_all(&self.dir)
            .and_then(|()| fs::write(&path, format!("{} {}", key.size, key.mtime_ms)));
        if let Err(err) = written {
            log::debug!("could not write decode marker {}: {err}", path.display());
        }
        Marker {
            guard: self.clone(),
            id,
            path,
        }
    }

    /// A decode of `id` ended with the process alive: its record goes.
    fn cleared(&self, id: MediaId) {
        let mut records = self.records.lock();
        if records.remove(&id).is_some() {
            self.save(&records);
        }
    }

    /// Written whole to a temporary file and renamed over the record, so
    /// a crash mid-write cannot lose it.
    fn save(&self, records: &HashMap<MediaId, Record>) {
        let text: String = records
            .iter()
            .map(|(id, record)| {
                format!(
                    "{} {} {} {} {}\n",
                    id.0,
                    record.key.size,
                    record.key.mtime_ms,
                    record.strikes,
                    u8::from(record.suspect)
                )
            })
            .collect();
        let temp = self
            .dir
            .join(format!("{STRIKES_FILE}.{}.tmp", std::process::id()));
        let written = fs::create_dir_all(&self.dir)
            .and_then(|()| fs::write(&temp, text))
            .and_then(|()| fs::rename(&temp, self.dir.join(STRIKES_FILE)));
        if let Err(err) = written {
            log::warn!("could not save the decode crash record: {err}");
        }
    }
}

/// A running decode's marker; dropping it (the decode returned or
/// unwound, so the process lived) removes the file and the item's record.
struct Marker {
    guard: Arc<DecodeGuard>,
    id: MediaId,
    path: PathBuf,
}

impl Drop for Marker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        self.guard.cleared(self.id);
    }
}

fn read_records(path: &Path) -> HashMap<MediaId, Record> {
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let id = MediaId(fields.next()?.parse().ok()?);
            let key = FileKey {
                size: fields.next()?.parse().ok()?,
                mtime_ms: fields.next()?.parse().ok()?,
            };
            let strikes = fields.next()?.parse().ok()?;
            let suspect = fields.next().is_some_and(|flag| flag == "1");
            Some((
                id,
                Record {
                    key,
                    strikes,
                    suspect,
                },
            ))
        })
        .collect()
}

struct LeftMarker {
    path: PathBuf,
    id: MediaId,
    pid: u32,
    key: FileKey,
}

/// Markers in `dir`, named `{id}-{pid}-{n}.decoding`.
fn leftover_markers(dir: &Path) -> Vec<LeftMarker> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some(MARKER_EXTENSION))
        .filter_map(|path| {
            let name = path.file_stem()?.to_str()?;
            let mut parts = name.split('-');
            let id = MediaId(parts.next()?.parse().ok()?);
            let pid = parts.next()?.parse().ok()?;
            let text = fs::read_to_string(&path).ok()?;
            let mut fields = text.split_whitespace();
            let key = FileKey {
                size: fields.next()?.parse().ok()?,
                mtime_ms: fields.next()?.parse().ok()?,
            };
            Some(LeftMarker { path, id, pid, key })
        })
        .collect()
}

/// The open library's guard: set when an engine opens, read by the
/// decoder's static functions.
static CURRENT: OnceLock<RwLock<Option<Arc<DecodeGuard>>>> = OnceLock::new();

fn slot() -> &'static RwLock<Option<Arc<DecodeGuard>>> {
    CURRENT.get_or_init(|| RwLock::new(None))
}

/// Makes `guard` the one in-process decodes report to.
pub(crate) fn install(guard: DecodeGuard) {
    *slot().write() = Some(Arc::new(guard));
}

/// The open library's guard, if an engine is open.
pub(crate) fn current() -> Option<Arc<DecodeGuard>> {
    slot().read().clone()
}

/// Runs the native `decode` of `id` at `path` under `guard`: refused when
/// the file stopped Lumina twice in a row or hung this session, run alone
/// when it is a suspect, marked while it runs, and given up on after
/// `timeout`.
pub(crate) fn guarded_decode<T, F>(
    guard: Option<Arc<DecodeGuard>>,
    id: MediaId,
    path: &Path,
    timeout: Duration,
    decode: F,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let tracked = guard.zip(FileKey::of(path));
    let Some((guard, key)) = tracked else {
        return finish(run_bounded(timeout, decode), path, timeout);
    };
    if guard.refuses(id, key) {
        return Err(anyhow!(
            "{} stopped or stalled Lumina while decoding before; not decoding it again",
            path.display()
        ));
    }
    let alone = guard.is_suspect(id, key);
    guard.turns.enter(alone);
    let marked = guard.clone();
    let outcome = run_bounded(timeout, move || {
        let _marker = marked.mark(id, key);
        decode()
    });
    guard.turns.leave(alone);
    if matches!(outcome, Err(Bounded::TimedOut)) {
        guard.gave_up.lock().insert(id);
    }
    finish(outcome, path, timeout)
}

fn finish<T>(
    outcome: std::result::Result<Result<T>, Bounded>,
    path: &Path,
    timeout: Duration,
) -> Result<T> {
    match outcome {
        Ok(result) => result,
        Err(Bounded::TimedOut) => Err(anyhow!(
            "the native decoder did not finish {} within {timeout:?}",
            path.display()
        )),
        Err(Bounded::Failed) => Err(anyhow!(
            "the native decode of {} could not run or panicked",
            path.display()
        )),
    }
}

/// Why [`run_bounded`] has no result.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Bounded {
    /// Still running at the timeout.
    TimedOut,
    /// The thread could not start, or panicked.
    Failed,
}

/// Runs `work` on a thread of its own and waits up to `timeout`; a late
/// result is dropped.
pub(crate) fn run_bounded<T, F>(timeout: Duration, work: F) -> std::result::Result<T, Bounded>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("lumina-native-decode".into())
        .spawn(move || {
            // The waiter may have given up; a late result is dropped.
            let _ = tx.send(work());
        })
        .map_err(|_| Bounded::Failed)?;
    rx.recv_timeout(timeout).map_err(|err| match err {
        std::sync::mpsc::RecvTimeoutError::Timeout => Bounded::TimedOut,
        std::sync::mpsc::RecvTimeoutError::Disconnected => Bounded::Failed,
    })
}

#[cfg(test)]
mod tests;
