use anyhow::Result;
use rusqlite::params;

use super::Catalog;
use crate::{MediaId, TimelineBucket};

impl Catalog {
    pub(crate) fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
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
                color_label INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_media_timestamp ON media_items(timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;
            CREATE INDEX IF NOT EXISTS idx_media_year_month ON media_items(year DESC, month DESC);
            CREATE INDEX IF NOT EXISTS idx_media_location ON media_items(country_code, admin1_region, city);
            CREATE INDEX IF NOT EXISTS idx_media_camera ON media_items(make, model);
            CREATE INDEX IF NOT EXISTS idx_media_type ON media_items(media_type);
            CREATE INDEX IF NOT EXISTS idx_media_trashed ON media_items(is_trashed, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 1;
            CREATE INDEX IF NOT EXISTS idx_media_favorite ON media_items(is_favorite, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;

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

        tx.commit()?;
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
