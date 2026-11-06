//! Source attribution: which items Lumina downloaded from a URL, as opposed
//! to a web address that only came from an imported XMP sidecar.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

use super::Catalog;
use crate::MediaId;

/// A library item (not in the trash) that Lumina downloaded from a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorDownload {
    pub media_id: MediaId,
    pub path: PathBuf,
    pub imported_epoch_ms: i64,
}

impl Catalog {
    /// The item Lumina downloaded from `source_url`, if it is still in the
    /// library. Sources that only came from a sidecar never match.
    pub fn find_prior_download(&self, source_url: &str) -> Result<Option<PriorDownload>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT m.id, m.file_path, s.imported_epoch_ms
             FROM media_sources s JOIN media_items m ON m.id = s.media_id
             WHERE s.source_url = ?1 AND s.downloaded = 1 AND m.is_trashed = 0
             ORDER BY s.imported_epoch_ms DESC LIMIT 1;",
        )?;
        let prior = stmt
            .query_row(params![source_url], |row| {
                Ok(PriorDownload {
                    media_id: MediaId(row.get::<_, i64>(0)? as u64),
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    imported_epoch_ms: row.get(2)?,
                })
            })
            .optional()?;
        Ok(prior)
    }

    /// Decides v13 rows (`downloaded IS NULL`): downloads only ever land in
    /// `<library_root>/Imports/`, so a source on a file there is a download
    /// and any other came from a sidecar. Idempotent; returns rows decided.
    pub fn backfill_downloaded_sources(&self, library_root: &Path) -> Result<usize> {
        let mut prefix = String::new();
        for ch in library_root
            .join(crate::import::IMPORTS_DIR)
            .to_string_lossy()
            .chars()
        {
            if matches!(ch, '%' | '_' | '\\') {
                prefix.push('\\');
            }
            prefix.push(ch);
        }
        prefix.push_str("/%");
        let conn = self.conn.lock();
        let decided = conn.execute(
            r#"UPDATE media_sources SET downloaded = EXISTS (
                   SELECT 1 FROM media_items m
                   WHERE m.id = media_sources.media_id AND m.file_path LIKE ?1 ESCAPE '\'
               )
               WHERE downloaded IS NULL;"#,
            params![prefix],
        )?;
        Ok(decided)
    }
}

#[cfg(test)]
mod tests {
    use crate::catalog::Catalog;
    use crate::{MediaId, SystemAlbum, SCHEMA_VERSION};
    use std::path::{Path, PathBuf};

    use crate::catalog::tests_support::video_item as item;

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[item(path)]).unwrap()[0].id
    }

    const URL: &str = "https://example.com/watch?v=1";

    #[test]
    fn download_marks_source_downloaded() {
        let catalog = Catalog::open_in_memory().unwrap();
        let inserted = catalog
            .insert_with_source(&item("/lib/Imports/2026/a.mp4"), URL, None)
            .unwrap();
        let prior = catalog
            .find_prior_download(URL)
            .unwrap()
            .expect("a download is a prior download");
        assert_eq!(prior.media_id, inserted.id);
        assert_eq!(prior.path, PathBuf::from("/lib/Imports/2026/a.mp4"));
        assert!(prior.imported_epoch_ms > 0, "the download date is kept");
    }

    #[test]
    fn sidecar_source_is_not_a_download() {
        let catalog = Catalog::open_in_memory().unwrap();
        let id = plain(&catalog, "/lib/a.mp4");
        catalog.record_media_source(id, URL, None).unwrap();
        assert!(
            catalog.find_prior_download(URL).unwrap().is_none(),
            "a web address from a sidecar is not a download"
        );
    }

    #[test]
    fn sidecar_import_keeps_download_flag() {
        let catalog = Catalog::open_in_memory().unwrap();
        let inserted = catalog
            .insert_with_source(&item("/lib/Imports/2026/a.mp4"), URL, None)
            .unwrap();
        let downloaded_at = catalog.find_prior_download(URL).unwrap().unwrap();
        catalog
            .conn
            .lock()
            .execute("UPDATE media_sources SET imported_epoch_ms = 5", [])
            .unwrap();
        catalog.record_media_source(inserted.id, URL, None).unwrap();
        let prior = catalog
            .find_prior_download(URL)
            .unwrap()
            .expect("importing a sidecar onto a download must keep it a download");
        assert_eq!(
            prior.imported_epoch_ms, 5,
            "a sidecar import must not change the download date (was {})",
            downloaded_at.imported_epoch_ms
        );
    }

    #[test]
    fn sidecar_import_keeps_download_address() {
        let catalog = Catalog::open_in_memory().unwrap();
        let inserted = catalog
            .insert_with_source(&item("/lib/Imports/2026/a.mp4"), URL, None)
            .unwrap();
        catalog
            .record_media_source(inserted.id, "https://other.example/page", None)
            .unwrap();
        assert!(
            catalog.find_prior_download(URL).unwrap().is_some(),
            "a sidecar's address must not replace the one Lumina downloaded from"
        );
    }

    #[test]
    fn trashed_download_is_not_prior() {
        let catalog = Catalog::open_in_memory().unwrap();
        let inserted = catalog
            .insert_with_source(&item("/lib/Imports/2026/a.mp4"), URL, None)
            .unwrap();
        catalog.set_trashed(inserted.id, true).unwrap();
        assert!(
            catalog.find_prior_download(URL).unwrap().is_none(),
            "a trashed download is downloaded again without a prompt"
        );
    }

    #[test]
    fn v13_sources_backfill_downloaded_flag() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_sources_backfill_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("catalog.db");
        let root = Path::new("/lib");
        {
            let catalog = Catalog::open(&db).unwrap();
            let downloaded = plain(&catalog, "/lib/Imports/2026/2026-11-03/a.mp4");
            let sidecar = plain(&catalog, "/lib/b.mp4");
            catalog
                .record_media_source(downloaded, "https://a.example/1", None)
                .unwrap();
            catalog
                .record_media_source(sidecar, "https://b.example/2", None)
                .unwrap();
            // Back to a v13 catalog: no `downloaded` column.
            catalog
                .conn
                .lock()
                .execute_batch(
                    "ALTER TABLE media_sources DROP COLUMN downloaded; PRAGMA user_version = 13;",
                )
                .unwrap();
        }
        for _ in 0..2 {
            let catalog = Catalog::open(&db).unwrap();
            catalog.backfill_downloaded_sources(root).unwrap();
            assert_eq!(catalog.schema_version().unwrap(), SCHEMA_VERSION);
            assert!(
                catalog
                    .find_prior_download("https://a.example/1")
                    .unwrap()
                    .is_some(),
                "a v13 source under Imports/ counts as a download"
            );
            assert!(
                catalog
                    .find_prior_download("https://b.example/2")
                    .unwrap()
                    .is_none(),
                "a v13 source outside Imports/ came from a sidecar"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn backfill_escapes_like_wildcards_in_the_library_root() {
        let catalog = Catalog::open_in_memory().unwrap();
        let other = plain(&catalog, "/libXa/Imports/a.mp4");
        catalog
            .record_media_source(other, "https://a.example/1", None)
            .unwrap();
        catalog
            .conn
            .lock()
            .execute("UPDATE media_sources SET downloaded = NULL", [])
            .unwrap();
        catalog
            .backfill_downloaded_sources(Path::new("/lib_a"))
            .unwrap();
        assert!(
            catalog
                .find_prior_download("https://a.example/1")
                .unwrap()
                .is_none(),
            "`_` in the library root must match only itself"
        );
    }

    #[test]
    fn downloaded_album_counts_only_downloads() {
        let catalog = Catalog::open_in_memory().unwrap();
        catalog
            .insert_with_source(&item("/lib/Imports/2026/a.mp4"), URL, None)
            .unwrap();
        let sidecar = plain(&catalog, "/lib/b.mp4");
        catalog
            .record_media_source(sidecar, "https://b.example/2", None)
            .unwrap();
        plain(&catalog, "/lib/c.mp4");
        assert_eq!(
            catalog
                .query_system_album_count(SystemAlbum::Downloaded)
                .unwrap(),
            1,
            "Downloaded holds only what Lumina downloaded"
        );
    }

    #[test]
    fn downloaded_album_query_plan() {
        let catalog = Catalog::open_in_memory().unwrap();
        let sql = format!(
            "SELECT COUNT(*) FROM media_items m WHERE {}",
            SystemAlbum::Downloaded.build_where_clause()
        );
        let plan = catalog.explain_query_plan(&sql).unwrap();
        assert!(
            !plan
                .iter()
                .any(|step| step.starts_with("SCAN s") || step.contains("SCAN media_sources")),
            "the Downloaded filter must look sources up by key, got {plan:?}"
        );
    }
}
