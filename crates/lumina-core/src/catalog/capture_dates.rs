//! One-time re-dating of items catalogued before the scanner looked past
//! EXIF DateTimeOriginal: those fell back to the file's modification time,
//! which for a downloaded or copied file is the day it arrived, so they sat
//! at the top of the timeline.

use anyhow::Result;
use rusqlite::params;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::files::PRESENT;
use super::Catalog;

/// `app_config` key set once the re-dating has finished.
const CAPTURE_DATES_KEY: &str = "capture_dates_v1";
/// A day in ms: a fallback date this close to the import time is the
/// import day, whatever the time zone.
const DAY_MS: i64 = 86_400_000;

impl Catalog {
    /// Records that every stored date comes from the current scanner, so
    /// [`Catalog::backfill_capture_dates`] never runs (a new catalog).
    pub fn mark_capture_dates_current(&self) -> Result<()> {
        self.set_config(CAPTURE_DATES_KEY, "1")
    }

    /// Re-dates, once, the present items whose date is the old fallback on
    /// their import day: date not edited, equal to the file's modification
    /// time (so no embedded date was found) and within a day of when the
    /// item was added. `measure` gives the date the scanner would store now
    /// (`None` for an unreadable file, which keeps its date). Files are read
    /// with the lock released; a write lands only if the row still holds the
    /// date read before, so a rescan or an edit in between wins. Returns how
    /// many rows changed.
    ///
    /// Finding candidates is one pass over the table; only candidates are
    /// read from disk, a header each. The marker is set when done, so later
    /// calls return at once; a cancelled run leaves it unset and the next
    /// scan starts over.
    pub fn backfill_capture_dates(
        &self,
        measure: impl Fn(&Path) -> Option<i64>,
        cancel: &AtomicBool,
    ) -> Result<usize> {
        if self.get_config(CAPTURE_DATES_KEY)?.is_some() {
            return Ok(0);
        }
        let rows: Vec<(i64, String, i64)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(&format!(
                "SELECT m.id, m.file_path, m.timestamp_epoch_ms FROM media_items m
                 WHERE m.date_edited = 0
                   AND m.timestamp_epoch_ms = m.mtime_epoch_ms
                   AND ABS(m.timestamp_epoch_ms - m.added_epoch_ms) < {DAY_MS}
                   AND {PRESENT}
                 ORDER BY m.id;"
            ))?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut changes = Vec::new();
        for (id, path, old) in rows {
            if cancel.load(Ordering::Relaxed) {
                return Ok(0);
            }
            match measure(&super::paths::decode(&path)) {
                Some(new) if new != old => changes.push((id, old, new)),
                _ => {}
            }
        }
        let mut fixed = 0;
        if !changes.is_empty() {
            let mut conn = self.conn.lock();
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "UPDATE media_items SET timestamp_epoch_ms = ?1, year = ?2, month = ?3, day = ?4
                     WHERE id = ?5 AND timestamp_epoch_ms = ?6 AND date_edited = 0;",
                )?;
                for (id, old, new) in &changes {
                    let (year, month, day) = crate::scanner::epoch_to_date_components(*new);
                    fixed += stmt.execute(params![new, year, month, day, id, old])?;
                }
            }
            Self::rebuild_timeline_buckets(&tx)?;
            tx.commit()?;
        }
        self.mark_capture_dates_current()?;
        Ok(fixed)
    }
}
