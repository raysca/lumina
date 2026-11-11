//! Hand-picked albums: one `albums` row per album and one `album_items` row
//! per member. Members follow the view's [`SortSpec`]; there is no manual
//! position (plan 5, owner answer 1). Both sides of `album_items` cascade, so
//! purging media or deleting an album never leaves orphan rows. Trash is a
//! flag, so a trashed member keeps its membership and returns with it.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

use super::Catalog;
use crate::text::{clean_text, MAX_NAME_CHARS};
use crate::{MediaId, MediaItem, SortSpec};

/// A hand-picked album. Its members live in `album_items`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub created_at_epoch_ms: i64,
    pub pinned: bool,
}

/// What an album card needs beyond the album row. "Active" is not trashed
/// and not hidden, the rule the album counts use, so a card never shows a
/// cover its count leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlbumCard {
    /// The album this card is for.
    pub album_id: i64,
    /// The newest active member by capture time, the larger id on a tie;
    /// `None` when the album has no active member.
    pub cover: Option<MediaId>,
    /// The cover's capture time; `None` when there is no cover or the cover
    /// is undated (a capture time of 0, as in `view_scope`).
    pub newest_epoch_ms: Option<i64>,
}

/// The SQL [`Catalog::album_cards`] runs.
///
/// Per album, a correlated `ORDER BY … LIMIT 1` seeks that album's members
/// through the `album_items` primary key (`SEARCH ai USING PRIMARY KEY
/// (album_id=?)`), looks each media row up by id, sorts that album's active
/// members and takes the first; the cover row is then read by id. `m.id DESC` breaks
/// timestamp ties deterministically, which `MAX()` with bare columns would
/// not. Nothing is materialised.
///
/// Measured against a `ROW_NUMBER() OVER (PARTITION BY album_id)` window
/// over all memberships (125k memberships in 50 albums, in-memory, release
/// build): 7.9 ms here against 48.6 ms for the window, which sorts every
/// album in full and needed a `MATERIALIZED` CTE to avoid an automatic
/// index over every ranked row.
///
/// Cost: one pass over every active membership plus one PK lookup per
/// member; that is inherent, since every album needs its newest member.
/// ponytail: O(all memberships) per call, once per album refresh on a
/// background thread; store a cover per album if that stops being cheap.
const ALBUM_CARDS_SQL: &str = r#"
    SELECT a.id, c.id, c.timestamp_epoch_ms
    FROM albums a
    LEFT JOIN media_items c ON c.id = (
        SELECT m.id
        FROM album_items ai
        JOIN media_items m ON m.id = ai.media_id
        WHERE ai.album_id = a.id AND m.is_trashed = 0 AND m.is_hidden = 0
        ORDER BY m.timestamp_epoch_ms DESC, m.id DESC
        LIMIT 1
    )
    ORDER BY a.created_at_epoch_ms ASC, a.id ASC;
    "#;

/// Adds one member only while its media row exists. `OR IGNORE` skips a
/// member already there but not a foreign-key failure, so without the
/// `SELECT` an id purged meanwhile would abort a restore or an undo.
const INSERT_MEMBER_SQL: &str = "INSERT OR IGNORE INTO album_items (album_id, media_id)
     SELECT ?1, id FROM media_items WHERE id = ?2;";

const ALBUM_COLUMNS: &str = "id, title, created_at_epoch_ms, pinned";

fn map_album(row: &rusqlite::Row) -> rusqlite::Result<Album> {
    Ok(Album {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at_epoch_ms: row.get(2)?,
        pinned: row.get::<_, i64>(3)? != 0,
    })
}

/// `title` cleaned to one line; an album needs a name.
fn album_title(title: &str) -> Result<String> {
    let title = clean_text(title, MAX_NAME_CHARS);
    if title.is_empty() {
        bail!("an album needs a name");
    }
    Ok(title)
}

impl Catalog {
    /// Creates an empty album.
    pub fn create_album(&self, title: &str) -> Result<Album> {
        self.create_album_with(title, &[])
    }

    /// Creates an album holding `members`, in one transaction: a failed
    /// member insert leaves no album behind. Ids no longer in the catalog
    /// are skipped.
    pub fn create_album_with(&self, title: &str, members: &[MediaId]) -> Result<Album> {
        let title = album_title(title)?;
        let now_ms = Utc::now().timestamp_millis();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let id = tx
            .query_row(
                "INSERT INTO albums (title, created_at_epoch_ms) VALUES (?1, ?2) RETURNING id;",
                params![title, now_ms],
                |row| row.get(0),
            )
            .context("creating an album")?;
        {
            let mut stmt = tx.prepare_cached(INSERT_MEMBER_SQL)?;
            for media in members {
                stmt.execute(params![id, media.0 as i64])
                    .context("adding to the new album")?;
            }
        }
        tx.commit()?;
        Ok(Album {
            id,
            title,
            created_at_epoch_ms: now_ms,
            pinned: false,
        })
    }

    /// Renames an album; false when no album has `id`.
    pub fn rename_album(&self, id: i64, title: &str) -> Result<bool> {
        let title = album_title(title)?;
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE albums SET title = ?1 WHERE id = ?2;",
                params![title, id],
            )
            .context("renaming an album")?;
        Ok(changed > 0)
    }

    /// Deletes an album and returns it with every member (trashed and hidden
    /// ones too), so [`Self::restore_album`] can undo it. `None` when no
    /// album has `id`. The media themselves are untouched.
    pub fn delete_album(&self, id: i64) -> Result<Option<(Album, Vec<MediaId>)>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let album = tx
            .query_row(
                &format!("SELECT {ALBUM_COLUMNS} FROM albums WHERE id = ?1;"),
                params![id],
                map_album,
            )
            .optional()?;
        let Some(album) = album else {
            return Ok(None);
        };
        let members = {
            let mut stmt = tx.prepare_cached(
                "SELECT media_id FROM album_items WHERE album_id = ?1 ORDER BY media_id;",
            )?;
            let rows = stmt.query_map(params![id], |row| row.get::<_, i64>(0))?;
            rows.map(|row| row.map(|id| MediaId(id as u64)))
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        // ON DELETE CASCADE removes the album_items rows.
        tx.execute("DELETE FROM albums WHERE id = ?1;", params![id])
            .context("deleting an album")?;
        tx.commit()?;
        Ok(Some((album, members)))
    }

    /// Re-inserts a deleted album under its old id with its members (undo of
    /// [`Self::delete_album`]). Members purged since are skipped.
    pub fn restore_album(&self, album: &Album, members: &[MediaId]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            &format!("INSERT INTO albums ({ALBUM_COLUMNS}) VALUES (?1, ?2, ?3, ?4);"),
            params![
                album.id,
                album.title,
                album.created_at_epoch_ms,
                album.pinned as i64
            ],
        )
        .context("restoring an album")?;
        {
            let mut stmt = tx.prepare_cached(INSERT_MEMBER_SQL)?;
            for media in members {
                stmt.execute(params![album.id, media.0 as i64])
                    .context("restoring album members")?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every album with its count of active (not trashed, not hidden)
    /// members, oldest album first, in one query for the sidebar refresh.
    pub fn list_albums_with_counts(&self) -> Result<Vec<(Album, usize)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT a.id, a.title, a.created_at_epoch_ms, a.pinned, COUNT(m.id)
            FROM albums a
            LEFT JOIN album_items ai ON ai.album_id = a.id
            LEFT JOIN media_items m
                ON m.id = ai.media_id AND m.is_trashed = 0 AND m.is_hidden = 0
            GROUP BY a.id
            ORDER BY a.created_at_epoch_ms ASC, a.id ASC;
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((map_album(row)?, row.get::<_, i64>(4)? as usize))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// One [`AlbumCard`] per album, in [`Self::list_albums_with_counts`]
    /// order.
    pub fn album_cards(&self) -> Result<Vec<AlbumCard>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(ALBUM_CARDS_SQL)?;
        let rows = stmt.query_map([], |row| {
            Ok(AlbumCard {
                album_id: row.get(0)?,
                cover: row.get::<_, Option<i64>>(1)?.map(|id| MediaId(id as u64)),
                // A capture time of 0 is undated: keep the cover, drop the date.
                newest_epoch_ms: row.get::<_, Option<i64>>(2)?.filter(|&ts| ts != 0),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Adds media to an album and returns how many were not members yet.
    /// Ids no longer in the catalog are skipped, so undoing a remove cannot
    /// fail on an item purged meanwhile. An unknown album is an error.
    pub fn add_to_album(&self, id: i64, media: &[MediaId]) -> Result<usize> {
        Ok(self.add_new_to_album(id, media)?.len())
    }

    /// [`Self::add_to_album`], returning the ids that were not members yet
    /// (in `media` order), so undo removes only what the add put in.
    pub fn add_new_to_album(&self, id: i64, media: &[MediaId]) -> Result<Vec<MediaId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut added = Vec::new();
        {
            let mut stmt = tx.prepare_cached(INSERT_MEMBER_SQL)?;
            for item in media {
                if stmt
                    .execute(params![id, item.0 as i64])
                    .context("adding to an album")?
                    > 0
                {
                    added.push(*item);
                }
            }
        }
        tx.commit()?;
        Ok(added)
    }

    /// Removes media from an album (never from the catalog) and returns how
    /// many were members.
    pub fn remove_from_album(&self, id: i64, media: &[MediaId]) -> Result<usize> {
        Ok(self.remove_members_from_album(id, media)?.len())
    }

    /// [`Self::remove_from_album`], returning the ids that were members (in
    /// `media` order), so undo adds back only what the remove took out.
    pub fn remove_members_from_album(&self, id: i64, media: &[MediaId]) -> Result<Vec<MediaId>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut removed = Vec::new();
        {
            let mut stmt = tx
                .prepare_cached("DELETE FROM album_items WHERE album_id = ?1 AND media_id = ?2;")?;
            for item in media {
                if stmt
                    .execute(params![id, item.0 as i64])
                    .context("removing from an album")?
                    > 0
                {
                    removed.push(*item);
                }
            }
        }
        tx.commit()?;
        Ok(removed)
    }

    /// The SQL [`Self::query_album_sorted`] runs: `?1` album id, `?2` limit,
    /// `?3` offset. Public so the query plan can be checked.
    ///
    /// The members are found through the album's primary key, so no media
    /// index can give the order and every page sorts the whole album. The
    /// subquery sorts only ids and sort keys, and full rows are read for
    /// the one page. For a 20k-member album (release build) that took the
    /// first page from 11.4 ms to 2.6 ms, and 4.7 ms near the last page,
    /// against the 8 ms album gate.
    /// ponytail: the sort is still per page, O(members); a big album needs
    /// seek pagination (last row's sort key) if it outgrows the gate.
    pub fn album_slice_sql(sort: SortSpec) -> String {
        format!(
            r#"
            SELECT
                m.id, m.file_path, m.parent_dir, m.file_name, m.file_size, m.mtime_epoch_ms,
                m.media_type, m.width, m.height, m.duration_ms,
                m.timestamp_epoch_ms, m.year, m.month, m.day,
                m.latitude, m.longitude, m.altitude,
                m.country_code, m.admin1_region, m.city,
                m.make, m.model, m.lens, m.focal_length, m.f_number, m.iso, m.exposure_time,
                m.is_favorite, m.is_hidden, m.is_trashed, m.title
            FROM media_items m
            WHERE m.id IN (
                SELECT am.id FROM album_items ai
                JOIN media_items am ON am.id = ai.media_id
                WHERE ai.album_id = ?1 AND am.is_trashed = 0 AND am.is_hidden = 0
                ORDER BY {}
                LIMIT ?2 OFFSET ?3
            )
            ORDER BY {};
            "#,
            sort.order_by("am."),
            sort.order_by("m.")
        )
    }

    /// A page of an album's active members in `sort` order.
    pub fn query_album_sorted(
        &self,
        id: i64,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&Self::album_slice_sql(sort))?;
        Self::map_query_rows(&mut stmt, params![id, limit as i64, offset as i64])
    }

    /// How many active members an album has.
    pub fn query_album_count(&self, id: i64) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            r#"
            SELECT COUNT(*) FROM album_items ai
            JOIN media_items m ON m.id = ai.media_id
            WHERE ai.album_id = ?1 AND m.is_trashed = 0 AND m.is_hidden = 0;
            "#,
            params![id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Pins or unpins an album; false when no album has `id`.
    pub fn set_album_pinned(&self, id: i64, pinned: bool) -> Result<bool> {
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE albums SET pinned = ?1 WHERE id = ?2;",
                params![pinned as i64, id],
            )
            .context("pinning an album")?;
        Ok(changed > 0)
    }
}

#[cfg(test)]
mod tests;
