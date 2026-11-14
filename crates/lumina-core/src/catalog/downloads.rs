//! The URL download queue, stored per library so it survives a restart.
//! Progress lives in memory only; rows change on state transitions.

use anyhow::{anyhow, Result};
use rusqlite::params;

use super::Catalog;
use crate::MediaId;

/// Where a queued URL download stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    Waiting,
    Running,
    /// Interrupted by quitting (or a crash); resumes only when asked.
    Paused,
    Failed,
    Done,
    /// Already in the library, found once the link was inspected.
    Duplicate,
}

impl DownloadState {
    fn as_str(self) -> &'static str {
        match self {
            DownloadState::Waiting => "waiting",
            DownloadState::Running => "running",
            DownloadState::Paused => "paused",
            DownloadState::Failed => "failed",
            DownloadState::Done => "done",
            DownloadState::Duplicate => "duplicate",
        }
    }

    fn parse(text: &str) -> Result<Self> {
        Ok(match text {
            "waiting" => DownloadState::Waiting,
            "running" => DownloadState::Running,
            "paused" => DownloadState::Paused,
            "failed" => DownloadState::Failed,
            "done" => DownloadState::Done,
            "duplicate" => DownloadState::Duplicate,
            other => return Err(anyhow!("unknown download state {other:?}")),
        })
    }
}

/// One row of the download list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRow {
    pub id: i64,
    /// As pasted.
    pub url: String,
    /// The address the site reported once inspected.
    pub canonical_url: Option<String>,
    pub title: Option<String>,
    pub state: DownloadState,
    /// What went wrong, written for people.
    pub error: Option<String>,
    pub can_retry: bool,
    /// Not a foreign key: purging the media keeps the record.
    pub media_id: Option<MediaId>,
    pub added_epoch_ms: i64,
    pub finished_epoch_ms: Option<i64>,
    /// Computed on read: `media_id` is a non-trashed item.
    pub in_library: bool,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

const SELECT_ROWS: &str = "SELECT q.id, q.url, q.canonical_url, q.title, q.state, q.error,
        q.can_retry, q.media_id, q.added_epoch_ms, q.finished_epoch_ms,
        EXISTS (SELECT 1 FROM media_items m WHERE m.id = q.media_id AND m.is_trashed = 0)
    FROM download_queue q";

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<(DownloadRow, String)> {
    Ok((
        DownloadRow {
            id: row.get(0)?,
            url: row.get(1)?,
            canonical_url: row.get(2)?,
            title: row.get(3)?,
            state: DownloadState::Waiting,
            error: row.get(5)?,
            can_retry: row.get::<_, i64>(6)? != 0,
            media_id: row.get::<_, Option<i64>>(7)?.map(|id| MediaId(id as u64)),
            added_epoch_ms: row.get(8)?,
            finished_epoch_ms: row.get(9)?,
            in_library: row.get::<_, i64>(10)? != 0,
        },
        row.get(4)?,
    ))
}

impl Catalog {
    fn download_by_id(conn: &rusqlite::Connection, id: i64) -> Result<DownloadRow> {
        let (mut row, state) = conn.query_row(
            &format!("{SELECT_ROWS} WHERE q.id = ?1"),
            params![id],
            map_row,
        )?;
        row.state = DownloadState::parse(&state)?;
        Ok(row)
    }

    /// Queues `url` as Waiting, at the end of the queue.
    pub fn add_download(&self, url: &str) -> Result<DownloadRow> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO download_queue (url, state, can_retry, added_epoch_ms) VALUES (?1, 'waiting', 1, ?2)",
            params![url, Self::next_added_ms(&conn)?],
        )?;
        Self::download_by_id(&conn, conn.last_insert_rowid())
    }

    /// Never earlier than the newest row, so the queue order is the order
    /// of adding even within one millisecond.
    fn next_added_ms(conn: &rusqlite::Connection) -> Result<i64> {
        let newest: Option<i64> = conn.query_row(
            "SELECT MAX(added_epoch_ms) FROM download_queue",
            [],
            |row| row.get(0),
        )?;
        Ok(newest.map_or(now_ms(), |newest| now_ms().max(newest + 1)))
    }

    /// What inspecting the link found; leaves the state alone, so it can
    /// never undo a finished row.
    pub fn set_download_title(&self, id: i64, canonical_url: &str, title: &str) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE download_queue SET canonical_url = ?2, title = ?3 WHERE id = ?1",
            params![id, canonical_url, title],
        )?;
        Ok(())
    }

    /// Writes every field of `row` except its id, URL and position. A known
    /// title or canonical URL is kept when `row` has none: the job thread's
    /// copy of a row predates its inspection.
    pub fn update_download(&self, row: &DownloadRow) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE download_queue SET canonical_url = COALESCE(?2, canonical_url),
                 title = COALESCE(?3, title), state = ?4, error = ?5,
                 can_retry = ?6, media_id = ?7, finished_epoch_ms = ?8
             WHERE id = ?1",
            params![
                row.id,
                row.canonical_url,
                row.title,
                row.state.as_str(),
                row.error,
                row.can_retry as i64,
                row.media_id.map(|id| id.0 as i64),
                row.finished_epoch_ms,
            ],
        )?;
        Ok(())
    }

    /// Retry and Resume: back to Waiting, at the end of the queue.
    pub fn requeue_download(&self, id: i64) -> Result<DownloadRow> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE download_queue SET state = 'waiting', error = NULL, can_retry = 1,
                 finished_epoch_ms = NULL, added_epoch_ms = ?2
             WHERE id = ?1",
            params![id, Self::next_added_ms(&conn)?],
        )?;
        Self::download_by_id(&conn, id)
    }

    pub fn remove_download(&self, id: i64) -> Result<()> {
        self.conn
            .lock()
            .execute("DELETE FROM download_queue WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Clear finished: removes Done, Failed and Duplicate rows. The media
    /// they point at stays in the library.
    pub fn clear_finished_downloads(&self) -> Result<usize> {
        Ok(self.conn.lock().execute(
            "DELETE FROM download_queue WHERE state IN ('done', 'failed', 'duplicate')",
            [],
        )?)
    }

    /// Clear completed (the Downloads screen): removes Done and Duplicate
    /// rows only. Failed rows stay: they hold the link for Retry.
    pub fn clear_completed_downloads(&self) -> Result<usize> {
        Ok(self.conn.lock().execute(
            "DELETE FROM download_queue WHERE state IN ('done', 'duplicate')",
            [],
        )?)
    }

    /// Every row, in queue order.
    pub fn list_downloads(&self) -> Result<Vec<DownloadRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!("{SELECT_ROWS} ORDER BY q.added_epoch_ms, q.id"))?;
        let rows = stmt.query_map([], map_row)?;
        rows.map(|row| {
            let (mut row, state) = row?;
            row.state = DownloadState::parse(&state)?;
            Ok(row)
        })
        .collect()
    }

    /// At library open: anything left Running or Waiting was interrupted by
    /// a quit or crash and waits for the user to resume it.
    pub fn recover_downloads(&self) -> Result<usize> {
        Ok(self.conn.lock().execute(
            "UPDATE download_queue SET state = 'paused' WHERE state IN ('running', 'waiting')",
            [],
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;

    fn states(catalog: &Catalog) -> Vec<(String, DownloadState)> {
        catalog
            .list_downloads()
            .unwrap()
            .into_iter()
            .map(|row| (row.url, row.state))
            .collect()
    }

    #[test]
    fn downloads_list_in_added_order() {
        let catalog = Catalog::open_in_memory().unwrap();
        for url in ["https://a/1", "https://a/2", "https://a/3"] {
            let row = catalog.add_download(url).unwrap();
            assert_eq!(row.state, DownloadState::Waiting);
            assert_eq!(row.url, url);
        }
        let urls: Vec<String> = states(&catalog).into_iter().map(|(u, _)| u).collect();
        assert_eq!(urls, ["https://a/1", "https://a/2", "https://a/3"]);
    }

    #[test]
    fn requeue_moves_to_the_end_and_clears_error() {
        let catalog = Catalog::open_in_memory().unwrap();
        let mut first = catalog.add_download("https://a/1").unwrap();
        catalog.add_download("https://a/2").unwrap();
        first.state = DownloadState::Failed;
        first.error = Some("The connection dropped".into());
        first.can_retry = false;
        catalog.update_download(&first).unwrap();

        let requeued = catalog.requeue_download(first.id).unwrap();
        assert_eq!(requeued.state, DownloadState::Waiting);
        assert_eq!(requeued.error, None, "Retry clears the old reason");
        assert!(requeued.can_retry);
        let urls: Vec<String> = states(&catalog).into_iter().map(|(u, _)| u).collect();
        assert_eq!(
            urls,
            ["https://a/2", "https://a/1"],
            "Retry puts the download at the end of the queue"
        );
    }

    #[test]
    fn update_round_trips_every_field() {
        let catalog = Catalog::open_in_memory().unwrap();
        let mut row = catalog.add_download("https://a/1").unwrap();
        row.canonical_url = Some("https://a/canonical".into());
        row.title = Some("Talk".into());
        row.state = DownloadState::Done;
        row.media_id = Some(MediaId(7));
        row.finished_epoch_ms = Some(42);
        catalog.update_download(&row).unwrap();
        let read = catalog.list_downloads().unwrap().remove(0);
        assert_eq!(read.canonical_url.as_deref(), Some("https://a/canonical"));
        assert_eq!(read.title.as_deref(), Some("Talk"));
        assert_eq!(read.state, DownloadState::Done);
        assert_eq!(read.media_id, Some(MediaId(7)));
        assert_eq!(read.finished_epoch_ms, Some(42));
        assert!(!read.in_library, "media 7 does not exist");
    }

    #[test]
    fn clear_completed_keeps_failed_and_active_rows() {
        let catalog = Catalog::open_in_memory().unwrap();
        for (url, state) in [
            ("https://a/waiting", DownloadState::Waiting),
            ("https://a/running", DownloadState::Running),
            ("https://a/paused", DownloadState::Paused),
            ("https://a/failed", DownloadState::Failed),
            ("https://a/done", DownloadState::Done),
            ("https://a/duplicate", DownloadState::Duplicate),
        ] {
            let mut row = catalog.add_download(url).unwrap();
            row.state = state;
            catalog.update_download(&row).unwrap();
        }
        assert_eq!(catalog.clear_completed_downloads().unwrap(), 2);
        let left: Vec<DownloadState> = states(&catalog).into_iter().map(|(_, s)| s).collect();
        assert_eq!(
            left,
            [
                DownloadState::Waiting,
                DownloadState::Running,
                DownloadState::Paused,
                DownloadState::Failed
            ],
            "Clear completed removes only Done and Duplicate rows; a failed row keeps its link"
        );
    }

    #[test]
    fn clear_finished_keeps_active_rows() {
        let catalog = Catalog::open_in_memory().unwrap();
        for (url, state) in [
            ("https://a/waiting", DownloadState::Waiting),
            ("https://a/running", DownloadState::Running),
            ("https://a/paused", DownloadState::Paused),
            ("https://a/failed", DownloadState::Failed),
            ("https://a/done", DownloadState::Done),
            ("https://a/duplicate", DownloadState::Duplicate),
        ] {
            let mut row = catalog.add_download(url).unwrap();
            row.state = state;
            catalog.update_download(&row).unwrap();
        }
        assert_eq!(catalog.clear_finished_downloads().unwrap(), 3);
        let left: Vec<DownloadState> = states(&catalog).into_iter().map(|(_, s)| s).collect();
        assert_eq!(
            left,
            [
                DownloadState::Waiting,
                DownloadState::Running,
                DownloadState::Paused
            ],
            "Clear finished never touches unfinished downloads"
        );
    }

    #[test]
    fn recover_pauses_running_and_waiting() {
        let catalog = Catalog::open_in_memory().unwrap();
        let waiting = catalog.add_download("https://a/1").unwrap();
        let mut running = catalog.add_download("https://a/2").unwrap();
        running.state = DownloadState::Running;
        catalog.update_download(&running).unwrap();
        let mut done = catalog.add_download("https://a/3").unwrap();
        done.state = DownloadState::Done;
        catalog.update_download(&done).unwrap();

        assert_eq!(catalog.recover_downloads().unwrap(), 2);
        let rows = catalog.list_downloads().unwrap();
        assert_eq!(rows[0].id, waiting.id);
        assert_eq!(rows[0].state, DownloadState::Paused);
        assert_eq!(rows[1].state, DownloadState::Paused);
        assert_eq!(rows[2].state, DownloadState::Done, "finished rows stay");
    }

    #[test]
    fn remove_download_deletes_only_that_row() {
        let catalog = Catalog::open_in_memory().unwrap();
        let first = catalog.add_download("https://a/1").unwrap();
        catalog.add_download("https://a/2").unwrap();
        catalog.remove_download(first.id).unwrap();
        let urls: Vec<String> = states(&catalog).into_iter().map(|(u, _)| u).collect();
        assert_eq!(urls, ["https://a/2"]);
    }

    #[test]
    fn in_library_false_after_trash_and_purge() {
        let catalog = Catalog::open_in_memory().unwrap();
        let item = crate::catalog::tests_support::video_item("/lib/Imports/a.mp4");
        let inserted = catalog
            .insert_with_source(&item, "https://a/1", None)
            .unwrap();
        let mut row = catalog.add_download("https://a/1").unwrap();
        row.state = DownloadState::Done;
        row.media_id = Some(inserted.id);
        catalog.update_download(&row).unwrap();
        assert!(catalog.list_downloads().unwrap()[0].in_library);

        catalog.set_trashed(inserted.id, true).unwrap();
        assert!(
            !catalog.list_downloads().unwrap()[0].in_library,
            "a trashed download is no longer in the library"
        );

        catalog
            .conn
            .lock()
            .execute(
                "DELETE FROM media_items WHERE id = ?1",
                [inserted.id.0 as i64],
            )
            .unwrap();
        let rows = catalog.list_downloads().unwrap();
        assert_eq!(rows.len(), 1, "purging the media keeps the queue record");
        assert!(!rows[0].in_library);
    }

    #[test]
    fn download_queue_survives_reopen() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_download_queue_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("catalog.db");
        {
            let catalog = Catalog::open(&db).unwrap();
            let mut row = catalog.add_download("https://a/1").unwrap();
            row.state = DownloadState::Failed;
            row.error = Some("This video is private".into());
            catalog.update_download(&row).unwrap();
        }
        let catalog = Catalog::open(&db).unwrap();
        let rows = catalog.list_downloads().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, DownloadState::Failed);
        assert_eq!(rows[0].error.as_deref(), Some("This video is private"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
