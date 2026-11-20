//! One-time fix of photo sizes stored before the scanner applied EXIF
//! orientation (portraits stored landscape) or could size an EXIF-less HEIC.

use anyhow::Result;
use rusqlite::params;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use super::files::PRESENT;
use super::Catalog;

/// `app_config` key set once the backfill has finished.
const DIMS_ORIENTED_KEY: &str = "dims_oriented_v1";
/// Rows measured, then written in one transaction, per lock hold.
const BATCH: usize = 256;

impl Catalog {
    /// Records that every stored size is already the displayed one, so
    /// [`Catalog::backfill_oriented_dims`] never runs (a new catalog).
    pub fn mark_dims_oriented(&self) -> Result<()> {
        self.set_config(DIMS_ORIENTED_KEY, "1")
    }

    /// Re-measures every present photo and RAW once with `measure` (the
    /// displayed size, `None` when the file cannot be read) and stores the
    /// sizes that changed, touching no other column. Files are measured with
    /// the lock released; each write only lands if the row still holds the
    /// size read before measuring, so a concurrent rescan wins. An
    /// unreadable file keeps its stored size. Returns how many rows changed.
    ///
    /// The `dims_oriented_v1` marker is set when every batch is done, so a
    /// later call returns at once; a cancelled run leaves it unset and the
    /// next scan starts over.
    /// ponytail: a restart re-measures from the first row; keep a resume
    /// point if a huge RAW library makes the one-time pass too slow.
    pub fn backfill_oriented_dims(
        &self,
        measure: impl Fn(&Path) -> Option<(u32, u32)>,
        cancel: &AtomicBool,
    ) -> Result<usize> {
        if self.get_config(DIMS_ORIENTED_KEY)?.is_some() {
            return Ok(0);
        }
        let rows: Vec<(i64, String, u32, u32)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(&format!(
                "SELECT m.id, m.file_path, m.width, m.height FROM media_items m
                 WHERE m.media_type IN ('image', 'raw') AND {PRESENT} ORDER BY m.id;"
            ))?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut fixed = 0;
        let mut unreadable = 0;
        for batch in rows.chunks(BATCH) {
            let changes: Vec<_> = batch
                .iter()
                .filter_map(|(id, path, w, h)| {
                    // Measuring can take seconds per file, so check per file.
                    if cancel.load(Ordering::Relaxed) {
                        return None;
                    }
                    let measured =
                        measure(&super::paths::decode(path)).filter(|s| s.0 > 0 && s.1 > 0);
                    if measured.is_none() {
                        unreadable += 1;
                    }
                    let size = measured?;
                    (size != (*w, *h)).then_some((*id, (*w, *h), size))
                })
                .collect();
            if changes.is_empty() {
                continue;
            }
            let mut conn = self.conn.lock();
            let tx = conn.transaction()?;
            for (id, (old_w, old_h), (w, h)) in &changes {
                fixed += tx.execute(
                    "UPDATE media_items SET width = ?1, height = ?2
                     WHERE id = ?3 AND width = ?4 AND height = ?5;",
                    params![w, h, id, old_w, old_h],
                )?;
            }
            tx.commit()?;
        }
        // After the loop, so a cancel during the last batch still re-runs.
        if cancel.load(Ordering::Relaxed) {
            return Ok(fixed);
        }
        if unreadable > 0 {
            log::warn!(
                "photo size correction: kept the stored size of {unreadable} unreadable files"
            );
        }
        self.mark_dims_oriented()?;
        Ok(fixed)
    }
}
