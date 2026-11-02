use anyhow::{Context, Result};
use rusqlite::params;

use super::{Catalog, SCHEMA_VERSION};
use crate::{MediaId, TimelineBucket};

impl Catalog {
    pub(crate) fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA busy_timeout = 5000;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -64000;
            PRAGMA mmap_size = 268435456;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS media_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT UNIQUE NOT NULL,
                parent_dir TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                mtime_epoch_ms INTEGER NOT NULL,
                
                media_type TEXT NOT NULL DEFAULT 'image',
                width INTEGER NOT NULL DEFAULT 0,
                height INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                
                timestamp_epoch_ms INTEGER NOT NULL,
                year INTEGER NOT NULL,
                month INTEGER NOT NULL,
                day INTEGER NOT NULL,
                
                latitude REAL,
                longitude REAL,
                altitude REAL,
                
                country_code TEXT,
                admin1_region TEXT,
                city TEXT,
                
                make TEXT,
                model TEXT,
                lens TEXT,
                focal_length REAL,
                f_number REAL,
                iso INTEGER,
                exposure_time TEXT,
                
                is_favorite INTEGER NOT NULL DEFAULT 0,
                is_hidden INTEGER NOT NULL DEFAULT 0,
                is_trashed INTEGER NOT NULL DEFAULT 0,
                rating INTEGER NOT NULL DEFAULT 0,
                color_label INTEGER NOT NULL DEFAULT 0,
                title TEXT,
                rotation_quarters INTEGER NOT NULL DEFAULT 0,
                added_epoch_ms INTEGER,
                date_edited INTEGER NOT NULL DEFAULT 0,
                place_edited INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_media_timestamp ON media_items(timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;
            CREATE INDEX IF NOT EXISTS idx_media_year_month ON media_items(year DESC, month DESC);
            CREATE INDEX IF NOT EXISTS idx_media_location ON media_items(country_code, admin1_region, city);
            CREATE INDEX IF NOT EXISTS idx_media_camera ON media_items(make, model);
            CREATE INDEX IF NOT EXISTS idx_media_type ON media_items(media_type);
            CREATE INDEX IF NOT EXISTS idx_media_trashed ON media_items(is_trashed, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 1;
            CREATE INDEX IF NOT EXISTS idx_media_favorite ON media_items(is_favorite, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;
            CREATE INDEX IF NOT EXISTS idx_media_rating ON media_items(rating DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;
            CREATE INDEX IF NOT EXISTS idx_media_file_name ON media_items(file_name COLLATE NOCASE ASC, id ASC) WHERE is_trashed = 0 AND is_hidden = 0;
            CREATE INDEX IF NOT EXISTS idx_media_file_size ON media_items(file_size DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;

            CREATE TABLE IF NOT EXISTS timeline_buckets (
                bucket_type TEXT NOT NULL,
                bucket_key TEXT NOT NULL,
                start_timestamp INTEGER NOT NULL,
                end_timestamp INTEGER NOT NULL,
                item_count INTEGER NOT NULL,
                cover_media_id INTEGER,
                PRIMARY KEY (bucket_type, bucket_key)
            );

            CREATE TABLE IF NOT EXISTS smart_albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                icon_name TEXT NOT NULL DEFAULT 'folder',
                match_mode TEXT NOT NULL DEFAULT 'all',
                rules_json TEXT NOT NULL,
                created_at_epoch_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS media_tags (
                media_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (media_id, tag),
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_media_tags_tag ON media_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_media_tags_media_id ON media_tags(media_id);

            CREATE TABLE IF NOT EXISTS app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS media_captions (
                media_id INTEGER PRIMARY KEY,
                caption TEXT NOT NULL,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS media_hashes (
                media_id INTEGER PRIMARY KEY,
                file_size INTEGER NOT NULL,
                content_hash TEXT NOT NULL,
                computed_epoch_ms INTEGER NOT NULL,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_media_hashes_hash ON media_hashes(content_hash);
            CREATE INDEX IF NOT EXISTS idx_media_hashes_size ON media_hashes(file_size);

            CREATE TRIGGER IF NOT EXISTS media_hashes_invalidate_au
            AFTER UPDATE OF file_size, mtime_epoch_ms ON media_items
            BEGIN
                DELETE FROM media_hashes WHERE media_id = new.id;
            END;

            CREATE VIRTUAL TABLE IF NOT EXISTS media_search_fts USING fts5(
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
                content='',
                tokenize='unicode61 remove_diacritics 2'
            );

            CREATE TRIGGER IF NOT EXISTS media_fts_ai AFTER INSERT ON media_items BEGIN
                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                ) VALUES (
                    new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
                );
            END;

            CREATE TRIGGER IF NOT EXISTS media_fts_ad BEFORE DELETE ON media_items BEGIN
                INSERT INTO media_search_fts(
                    media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                ) VALUES (
                    'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                );
            END;

            CREATE TRIGGER IF NOT EXISTS media_fts_au AFTER UPDATE ON media_items BEGIN
                INSERT INTO media_search_fts(
                    media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                ) VALUES (
                    'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                );
                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                ) VALUES (
                    new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                    COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
                );
            END;
            "#,
        )?;

        // Ensure existing databases have the rating and color_label columns on media_items
        let has_rating: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('media_items') WHERE name = 'rating'")?
            .exists([])?;
        if !has_rating {
            let _ = conn.execute(
                "ALTER TABLE media_items ADD COLUMN rating INTEGER NOT NULL DEFAULT 0;",
                [],
            );
        }

        let has_color_label: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('media_items') WHERE name = 'color_label'")?
            .exists([])?;
        if !has_color_label {
            let _ = conn.execute(
                "ALTER TABLE media_items ADD COLUMN color_label INTEGER NOT NULL DEFAULT 0;",
                [],
            );
        }

        let has_title: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('media_items') WHERE name = 'title'")?
            .exists([])?;
        if !has_title {
            let _ = conn.execute("ALTER TABLE media_items ADD COLUMN title TEXT;", []);
        }

        // Display-only viewer rotation, stored as clockwise quarter turns (0..=3).
        // Existing libraries default to 0, which preserves their current framing.
        let has_rotation: bool = conn
            .prepare(
                "SELECT 1 FROM pragma_table_info('media_items') WHERE name = 'rotation_quarters'",
            )?
            .exists([])?;
        if !has_rotation {
            conn.execute(
                "ALTER TABLE media_items ADD COLUMN rotation_quarters INTEGER NOT NULL DEFAULT 0;",
                [],
            )?;
        }

        // When the file first entered the catalog (Get Info "Added to Lumina").
        // Rows from before v9 stay NULL: the real date is unknown.
        let has_added: bool = conn
            .prepare(
                "SELECT 1 FROM pragma_table_info('media_items') WHERE name = 'added_epoch_ms'",
            )?
            .exists([])?;
        if !has_added {
            conn.execute(
                "ALTER TABLE media_items ADD COLUMN added_epoch_ms INTEGER;",
                [],
            )
            .context("adding media_items.added_epoch_ms")?;
        }

        // v10: user-edited date/place flags, so a re-read of a changed file
        // keeps the edit. Edits made before v10 are unflagged: unknowable.
        for column in ["date_edited", "place_edited"] {
            let exists: bool = conn
                .prepare("SELECT 1 FROM pragma_table_info('media_items') WHERE name = ?1")?
                .exists([column])?;
            if !exists {
                conn.execute(
                    &format!(
                        "ALTER TABLE media_items ADD COLUMN {column} INTEGER NOT NULL DEFAULT 0;"
                    ),
                    [],
                )
                .with_context(|| format!("adding media_items.{column}"))?;
            }
        }

        // Catalogs from the first smart-album engine named the rules column
        // `rule_definition_json`; `CREATE TABLE IF NOT EXISTS` never fixed it,
        // so saving or listing smart albums failed. The JSON format is
        // unchanged, so a rename keeps every saved album.
        let has_legacy_rules: bool = conn
            .prepare(
                "SELECT 1 FROM pragma_table_info('smart_albums') WHERE name = 'rule_definition_json'",
            )?
            .exists([])?;
        if has_legacy_rules {
            conn.execute(
                "ALTER TABLE smart_albums RENAME COLUMN rule_definition_json TO rules_json;",
                [],
            )
            .context("migrating smart_albums.rule_definition_json to rules_json")?;
        }

        // The same early builds indexed search through trg_media_* triggers.
        // The media_fts_* triggers above replaced them; the old ones left
        // stale entries in the contentless index, rebuilt once below.
        let had_legacy_triggers: bool = conn
            .prepare(
                "SELECT 1 FROM sqlite_master WHERE type = 'trigger'
                 AND name IN ('trg_media_insert', 'trg_media_update', 'trg_media_delete')",
            )?
            .exists([])?;

        // Check if existing media_search_fts table has the 'tags' column (for migration from older schemas)
        let has_tags_col: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('media_search_fts') WHERE name = 'tags'")?
            .exists([])?;

        if !has_tags_col {
            conn.execute_batch(
                r#"
                DROP TRIGGER IF EXISTS media_fts_ai;
                DROP TRIGGER IF EXISTS media_fts_ad;
                DROP TRIGGER IF EXISTS media_fts_au;
                DROP TABLE IF EXISTS media_search_fts;

                CREATE VIRTUAL TABLE media_search_fts USING fts5(
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
                    content='',
                    tokenize='unicode61 remove_diacritics 2'
                );

                CREATE TRIGGER media_fts_ai AFTER INSERT ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
                    );
                END;

                CREATE TRIGGER media_fts_ad BEFORE DELETE ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                    );
                END;

                CREATE TRIGGER media_fts_au AFTER UPDATE ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                    );
                    INSERT INTO media_search_fts(
                        rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
                    );
                END;

                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                )
                SELECT
                    m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE(c.caption, '')
                FROM media_items m
                LEFT JOIN media_captions c ON c.media_id = m.id;
                "#,
            )?;
        }

        // Check if existing media_search_fts table has the 'caption' column (for migration from older schemas)
        let has_caption_col: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('media_search_fts') WHERE name = 'caption'")?
            .exists([])?;

        if !has_caption_col {
            conn.execute_batch(
                r#"
                DROP TRIGGER IF EXISTS media_fts_ai;
                DROP TRIGGER IF EXISTS media_fts_ad;
                DROP TRIGGER IF EXISTS media_fts_au;
                DROP TABLE IF EXISTS media_search_fts;

                CREATE VIRTUAL TABLE media_search_fts USING fts5(
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
                    content='',
                    tokenize='unicode61 remove_diacritics 2'
                );

                CREATE TRIGGER media_fts_ai AFTER INSERT ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
                    );
                END;

                CREATE TRIGGER media_fts_ad BEFORE DELETE ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                    );
                END;

                CREATE TRIGGER media_fts_au AFTER UPDATE ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                    );
                    INSERT INTO media_search_fts(
                        rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
                    );
                END;

                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                )
                SELECT
                    m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE(c.caption, '')
                FROM media_items m
                LEFT JOIN media_captions c ON c.media_id = m.id;
                "#,
            )?;
        }

        if had_legacy_triggers {
            // One transaction: if the rebuild fails, the triggers stay and the
            // next open retries it instead of leaving search half-empty.
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(
                r#"
                DROP TRIGGER IF EXISTS trg_media_insert;
                DROP TRIGGER IF EXISTS trg_media_update;
                DROP TRIGGER IF EXISTS trg_media_delete;
                INSERT INTO media_search_fts(media_search_fts) VALUES('delete-all');
                INSERT INTO media_search_fts(
                    rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                )
                SELECT
                    m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, m.model, m.lens, CAST(m.year AS TEXT),
                    COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''),
                    COALESCE(c.caption, '')
                FROM media_items m
                LEFT JOIN media_captions c ON c.media_id = m.id;
                "#,
            )
            .context("rebuilding the search index after dropping legacy triggers")?;
            tx.commit()?;
        }

        // Stamp the schema version so future migrations can dispatch on it.
        let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        Ok(())
    }

    /// Recomputes pre-aggregated timeline buckets (years, months, days) for instant pinch-to-zoom.
    pub fn compute_timeline_buckets(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        self.compute_timeline_buckets_in_conn(&mut conn)
    }

    pub(crate) fn compute_timeline_buckets_in_conn(
        &self,
        conn: &mut rusqlite::Connection,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        Self::rebuild_timeline_buckets(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Rewrites every timeline bucket on `conn`; callers own the transaction,
    /// so an edit and its buckets can commit together.
    pub(crate) fn rebuild_timeline_buckets(tx: &rusqlite::Connection) -> Result<()> {
        tx.execute("DELETE FROM timeline_buckets;", [])?;

        // Year buckets
        tx.execute(
            r#"
            INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
            SELECT 
                'year',
                printf('%04d', year),
                MIN(timestamp_epoch_ms),
                MAX(timestamp_epoch_ms),
                COUNT(*),
                MAX(id)
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            GROUP BY year;
            "#,
            [],
        )?;

        // Month buckets
        tx.execute(
            r#"
            INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
            SELECT 
                'month',
                printf('%04d-%02d', year, month),
                MIN(timestamp_epoch_ms),
                MAX(timestamp_epoch_ms),
                COUNT(*),
                MAX(id)
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            GROUP BY year, month;
            "#,
            [],
        )?;

        // Day buckets
        tx.execute(
            r#"
            INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
            SELECT 
                'day',
                printf('%04d-%02d-%02d', year, month, day),
                MIN(timestamp_epoch_ms),
                MAX(timestamp_epoch_ms),
                COUNT(*),
                MAX(id)
            FROM media_items
            WHERE is_trashed = 0 AND is_hidden = 0
            GROUP BY year, month, day;
            "#,
            [],
        )?;

        Ok(())
    }

    pub fn get_timeline_buckets(&self, bucket_type: &str) -> Result<Vec<TimelineBucket>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id
            FROM timeline_buckets
            WHERE bucket_type = ?1
            ORDER BY bucket_key DESC;
            "#,
        )?;

        let rows = stmt.query_map(params![bucket_type], |row| {
            let b_type: String = row.get(0)?;
            let b_key: String = row.get(1)?;
            let start_ts: i64 = row.get(2)?;
            let end_ts: i64 = row.get(3)?;
            let count: i64 = row.get(4)?;
            let cover_id: Option<i64> = row.get(5)?;

            Ok(TimelineBucket {
                bucket_type: b_type,
                bucket_key: b_key,
                start_timestamp: start_ts,
                end_timestamp: end_ts,
                item_count: count as usize,
                cover_media_id: cover_id.map(|id| MediaId(id as u64)),
            })
        })?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }
}
