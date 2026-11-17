//! Captions, and the search index row that carries them.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

use super::Catalog;
use crate::text::{clean_caption, MAX_TEXT_CHARS};
use crate::MediaId;

impl Catalog {
    pub fn set_caption(&self, id: MediaId, caption: &str) -> Result<()> {
        self.set_captions(&[(id, caption)])
    }

    /// Sets each item's caption in one transaction; blank removes it.
    /// Captions are cleaned, keeping line breaks (see [`crate::text::clean_caption`]).
    pub fn set_captions(&self, changes: &[(MediaId, &str)]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for &(id, caption) in changes {
            Self::write_caption(&tx, id, caption)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn write_caption(conn: &rusqlite::Connection, id: MediaId, caption: &str) -> Result<()> {
        let media_id = id.0 as i64;
        let cleaned = clean_caption(caption, MAX_TEXT_CHARS);
        let trimmed = cleaned.as_str();

        // The caption currently reflected by the FTS index is needed to remove
        // the contentless FTS5 row before it is rewritten.
        let previous_caption: Option<String> = conn
            .query_row(
                "SELECT caption FROM media_captions WHERE media_id = ?1;",
                params![media_id],
                |row| row.get(0),
            )
            .optional()?;

        if trimmed.is_empty() {
            conn.execute(
                "DELETE FROM media_captions WHERE media_id = ?1;",
                params![media_id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO media_captions (media_id, caption) VALUES (?1, ?2) ON CONFLICT(media_id) DO UPDATE SET caption = excluded.caption;",
                params![media_id, trimmed],
            )?;
        }

        Self::refresh_fts_caption_row(conn, media_id, previous_caption.as_deref())?;
        Ok(())
    }

    /// Rewrites the contentless FTS5 row for `media_id` after a caption change.
    ///
    /// Contentless FTS5 rows can only be removed by supplying the column values
    /// that were originally indexed, so `previous_caption` must be the caption
    /// the FTS row was built from. The row is then reinserted using the current
    /// media metadata and caption, which keeps edits and clears searchable.
    fn refresh_fts_caption_row(
        conn: &rusqlite::Connection,
        media_id: i64,
        previous_caption: Option<&str>,
    ) -> Result<()> {
        let row = conn
            .query_row(
                r#"
                SELECT
                    m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code,
                    m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = m.id), '')
                FROM media_items m
                WHERE m.id = ?1;
                "#,
                params![media_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?;

        let Some((
            file_name,
            parent_dir,
            city,
            admin1_region,
            country_code,
            make,
            model,
            lens,
            year,
            tags,
            caption,
        )) = row
        else {
            return Ok(());
        };

        conn.execute(
            r#"
            INSERT INTO media_search_fts(
                media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
            ) VALUES ('delete', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
            "#,
            params![
                media_id,
                file_name,
                parent_dir,
                city,
                admin1_region,
                country_code,
                make,
                model,
                lens,
                year,
                tags,
                previous_caption.unwrap_or("")
            ],
        )?;

        conn.execute(
            r#"
            INSERT INTO media_search_fts(
                rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12);
            "#,
            params![
                media_id,
                file_name,
                parent_dir,
                city,
                admin1_region,
                country_code,
                make,
                model,
                lens,
                year,
                tags,
                caption
            ],
        )?;

        Ok(())
    }

    /// One item's caption, if it has one.
    pub fn get_caption(&self, id: MediaId) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT caption FROM media_captions WHERE media_id = ?1;",
                params![id.0 as i64],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn get_captions_map(&self) -> Result<HashMap<MediaId, String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT media_id, caption FROM media_captions;")?;
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let caption: String = row.get(1)?;
            Ok((MediaId(id as u64), caption))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
