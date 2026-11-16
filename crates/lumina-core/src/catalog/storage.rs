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

/// One scan of the active items; each row looks up its source by primary
/// key, so there is no sort and no temp b-tree.
const STORAGE_SUMMARY_SQL: &str = "SELECT
    COALESCE(SUM(CASE WHEN src.downloaded = 1 OR m.media_type = 'video'
        THEN 0 ELSE m.file_size END), 0),
    COALESCE(SUM(CASE WHEN src.downloaded = 1 THEN 0
        WHEN m.media_type = 'video' THEN m.file_size ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN src.downloaded = 1 THEN m.file_size ELSE 0 END), 0)
 FROM media_items m LEFT JOIN media_sources src ON src.media_id = m.id
 WHERE m.is_trashed = 0 AND m.is_hidden = 0;";

impl Catalog {
    /// Settings › Library's storage card and Library size row. Runs only
    /// when settings show the Library category, off the UI thread.
    pub fn storage_summary(&self) -> Result<StorageSummary> {
        let conn = self.conn.lock();
        let summary = conn.query_row(STORAGE_SUMMARY_SQL, [], |row| {
            Ok(StorageSummary {
                photo_bytes: row.get::<_, i64>(0)?.max(0) as u64,
                video_bytes: row.get::<_, i64>(1)?.max(0) as u64,
                download_bytes: row.get::<_, i64>(2)?.max(0) as u64,
            })
        })?;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_summary_scans_without_a_sort() {
        let catalog = Catalog::open_in_memory().unwrap();
        let plan = catalog.explain_query_plan(STORAGE_SUMMARY_SQL).unwrap();
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
