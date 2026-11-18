//! What the library's items take on disk, by kind, for Settings › Library.

use anyhow::Result;

use super::Catalog;

/// Bytes of the library's active items (not trashed, not hidden: the items
/// the library counts), by kind. A download counts once, as a download.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorageSummary {
    /// Images and RAW files.
    pub photo_bytes: u64,
    /// Videos Lumina did not download.
    pub video_bytes: u64,
    /// Items Lumina downloaded from a link (the Downloaded album's rule).
    pub download_bytes: u64,
}

/// One id range of the active items: the range is a rowid seek and each row
/// looks up its source by primary key, so there is no sort and no temp b-tree.
const STORAGE_SUMMARY_SQL: &str = "SELECT
    COALESCE(SUM(CASE WHEN src.downloaded = 1 OR m.media_type = 'video'
        THEN 0 ELSE m.file_size END), 0),
    COALESCE(SUM(CASE WHEN src.downloaded = 1 THEN 0
        WHEN m.media_type = 'video' THEN m.file_size ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN src.downloaded = 1 THEN m.file_size ELSE 0 END), 0)
 FROM media_items m LEFT JOIN media_sources src ON src.media_id = m.id
 WHERE m.id > ?1 AND m.id <= ?2 AND m.is_trashed = 0 AND m.is_hidden = 0;";

/// Ids per locked read. One scan of 105k rows held the catalog lock for
/// about 22 ms; chunks keep each hold to a few ms, so a UI-thread read waits
/// for at most one chunk.
const STORAGE_CHUNK: i64 = 20_000;

impl Catalog {
    /// Settings › Library's storage card and Library size row. Runs only
    /// when settings show the Library category, off the UI thread.
    pub fn storage_summary(&self) -> Result<StorageSummary> {
        self.storage_summary_in_chunks(STORAGE_CHUNK)
    }

    /// Sums `chunk` ids at a time and takes the lock once per chunk. An item
    /// added or removed between chunks can leave the total a moment stale,
    /// which a size readout tolerates.
    fn storage_summary_in_chunks(&self, chunk: i64) -> Result<StorageSummary> {
        let max_id: i64 = self.conn.lock().query_row(
            "SELECT COALESCE(MAX(id), 0) FROM media_items",
            [],
            |row| row.get(0),
        )?;
        let mut summary = StorageSummary::default();
        let mut start = 0;
        while start < max_id {
            let end = start.saturating_add(chunk);
            let part = self
                .conn
                .lock()
                .query_row(STORAGE_SUMMARY_SQL, [start, end], |row| {
                    Ok(StorageSummary {
                        photo_bytes: row.get::<_, i64>(0)?.max(0) as u64,
                        video_bytes: row.get::<_, i64>(1)?.max(0) as u64,
                        download_bytes: row.get::<_, i64>(2)?.max(0) as u64,
                    })
                })?;
            summary.photo_bytes += part.photo_bytes;
            summary.video_bytes += part.video_bytes;
            summary.download_bytes += part.download_bytes;
            start = end;
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_sum_matches_one_pass_across_chunk_boundaries() {
        let catalog = Catalog::open_in_memory().unwrap();
        let items: Vec<_> = (1..=7)
            .map(|i| crate::MediaItem {
                file_size: 10u64.pow(i as u32),
                ..super::super::places::tests::item(i, "Lisbon", "PT")
            })
            .collect();
        let ids = catalog.insert_or_update_batch(&items).unwrap();
        catalog.set_trashed(ids[3].id, true).unwrap();
        let one_pass = catalog.storage_summary_in_chunks(i64::MAX).unwrap();
        assert_eq!(
            one_pass.photo_bytes, 11_101_110,
            "every active photo is summed once; the trashed one is left out"
        );
        for chunk in [1, 2, 3, 6, 7] {
            assert_eq!(
                catalog.storage_summary_in_chunks(chunk).unwrap(),
                one_pass,
                "chunks of {chunk} ids must count every item exactly once"
            );
        }
    }

    #[test]
    fn storage_summary_scans_without_a_sort() {
        let catalog = Catalog::open_in_memory().unwrap();
        let sql = STORAGE_SUMMARY_SQL
            .replace("?1", "0")
            .replace("?2", "20000");
        let plan = catalog.explain_query_plan(&sql).unwrap();
        println!("storage_summary plan: {plan:?}");
        assert!(
            plan.iter().all(|step| !step.contains("TEMP B-TREE")),
            "the summary must not sort: {plan:?}"
        );
        assert!(
            plan.iter()
                .any(|step| step.contains("src") && step.contains("PRIMARY KEY")),
            "each item finds its source by primary key: {plan:?}"
        );
    }
}
