use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use super::{Catalog, SCHEMA_VERSION};
use crate::{MediaId, TimelineBucket};

/// Empties the contentless search index and rebuilds it from the catalog,
/// with the same columns as the `media_fts_*` triggers.
const REBUILD_SEARCH_INDEX_SQL: &str = r#"
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
"#;

/// Whether `table` has a column named `column`.
fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    Ok(conn
        .prepare("SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2")?
        .exists([table, column])?)
}

/// Adds `column` with the definition `ddl` (type and constraints) to
/// `table` unless it is already there, so a migration can run on every open.
/// A failed ALTER is returned, never ignored: queries name every column.
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, ddl: &str) -> Result<()> {
    if !has_column(conn, table, column)? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl};"),
            [],
        )
        .with_context(|| format!("adding {table}.{column}"))?;
    }
    Ok(())
}

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
                place_edited INTEGER NOT NULL DEFAULT 0,
                flag INTEGER NOT NULL DEFAULT 0
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
            CREATE INDEX IF NOT EXISTS idx_media_geo ON media_items(latitude, longitude) WHERE latitude IS NOT NULL AND is_trashed = 0 AND is_hidden = 0;

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
                created_at_epoch_ms INTEGER NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0
            );

            -- v19: hand-picked albums. Members follow the view's sort order
            -- (no position column). Both sides cascade, so purging media or
            -- deleting an album leaves no orphan rows.
            CREATE TABLE IF NOT EXISTS albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                created_at_epoch_ms INTEGER NOT NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                -- v22: the key photo; see the guarded ALTER below.
                cover_media_id INTEGER REFERENCES media_items(id) ON DELETE SET NULL
            );
            CREATE TABLE IF NOT EXISTS album_items (
                album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
                media_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
                PRIMARY KEY (album_id, media_id)
            ) WITHOUT ROWID;
            -- "Albums containing this item", and the media-side cascade.
            CREATE INDEX IF NOT EXISTS idx_album_items_media ON album_items(media_id);

            CREATE TABLE IF NOT EXISTS media_tags (
                media_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (media_id, tag),
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_media_tags_tag ON media_tags(tag);
            CREATE INDEX IF NOT EXISTS idx_media_tags_media_id ON media_tags(media_id);

            -- v16: keyword aliases, resolved when a tag is typed; the alias
            -- itself is never stored on media. Both columns hold normalized
            -- paths, and an alias is never also a keyword path in use.
            CREATE TABLE IF NOT EXISTS tag_aliases (
                alias TEXT PRIMARY KEY,
                tag TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tag_aliases_tag ON tag_aliases(tag);

            -- v17: creator, copyright and contact per item. Not searched,
            -- so no index or trigger; a row with every field NULL is
            -- deleted instead of stored.
            CREATE TABLE IF NOT EXISTS media_rights (
                media_id INTEGER PRIMARY KEY,
                creator TEXT,
                copyright TEXT,
                usage_terms TEXT,
                contact_email TEXT,
                contact_url TEXT,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            -- v18: non-destructive edits per item; rotation stays in
            -- media_items.rotation_quarters. A row with no edit is deleted
            -- instead of stored.
            CREATE TABLE IF NOT EXISTS media_edits (
                media_id INTEGER PRIMARY KEY,
                -- Crop in normalized [0,1] coordinates of the unrotated decoded frame; NULL = none.
                crop_x REAL, crop_y REAL, crop_w REAL, crop_h REAL,
                exposure_ev REAL NOT NULL DEFAULT 0,
                auto_enhance INTEGER NOT NULL DEFAULT 0,
                updated_epoch_ms INTEGER NOT NULL,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

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

            -- v12: where each row's file is. A side table, not media_items
            -- columns: media_fts_au rewrites the search row on every
            -- media_items UPDATE, and marking a folder missing must not.
            -- No row means present, inode unknown.
            CREATE TABLE IF NOT EXISTS media_file_state (
                media_id INTEGER PRIMARY KEY,
                file_inode INTEGER,
                missing_since_epoch_ms INTEGER,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            -- Every row, not only missing ones: a folder going missing looks
            -- up each file's just-added twin (present) by inode.
            CREATE INDEX IF NOT EXISTS idx_file_state_inode ON media_file_state(file_inode);

            -- v13: Source attribution for network imports (URL, uploader, timestamp).
            CREATE TABLE IF NOT EXISTS media_sources (
                media_id INTEGER PRIMARY KEY,
                source_url TEXT NOT NULL,
                uploader TEXT,
                imported_epoch_ms INTEGER NOT NULL,
                -- v14: 1 = Lumina downloaded it; 0 = the URL came from an
                -- imported sidecar; NULL = a v13 row the engine backfills.
                downloaded INTEGER,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_media_sources_url ON media_sources(source_url);

            -- v14: URL download queue. media_id is deliberately not a foreign
            -- key: purging the media keeps the record.
            CREATE TABLE IF NOT EXISTS download_queue (
                id INTEGER PRIMARY KEY,
                url TEXT NOT NULL,
                canonical_url TEXT,
                title TEXT,
                state TEXT NOT NULL,
                error TEXT,
                can_retry INTEGER NOT NULL DEFAULT 1,
                media_id INTEGER,
                added_epoch_ms INTEGER NOT NULL,
                finished_epoch_ms INTEGER
            );

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

        // Columns added after a table's first release. Every add goes
        // through `add_column_if_missing`, so a failed ALTER stops the open
        // instead of leaving a catalog the queries cannot read.
        add_column_if_missing(&conn, "media_items", "rating", "INTEGER NOT NULL DEFAULT 0")?;
        add_column_if_missing(
            &conn,
            "media_items",
            "color_label",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        add_column_if_missing(&conn, "media_items", "title", "TEXT")?;
        // Display-only viewer rotation, stored as clockwise quarter turns (0..=3).
        // Existing libraries default to 0, which preserves their current framing.
        add_column_if_missing(
            &conn,
            "media_items",
            "rotation_quarters",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        // When the file first entered the catalog (Get Info "Added to Lumina").
        // Rows from before v9 stay NULL: the real date is unknown.
        add_column_if_missing(&conn, "media_items", "added_epoch_ms", "INTEGER")?;
        // v10: user-edited date/place flags, so a re-read of a changed file
        // keeps the edit. Edits made before v10 are unflagged: unknowable.
        for column in ["date_edited", "place_edited"] {
            add_column_if_missing(&conn, "media_items", column, "INTEGER NOT NULL DEFAULT 0")?;
        }

        // Catalogs from the first smart-album engine named the rules column
        // `rule_definition_json`; `CREATE TABLE IF NOT EXISTS` never fixed it,
        // so saving or listing smart albums failed. The JSON format is
        // unchanged, so a rename keeps every saved album.
        if has_column(&conn, "smart_albums", "rule_definition_json")? {
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
        if !has_column(&conn, "media_search_fts", "tags")? {
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
        if !has_column(&conn, "media_search_fts", "caption")? {
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
                "#,
            )
            .context("dropping legacy search triggers")?;
            tx.execute_batch(REBUILD_SEARCH_INDEX_SQL)
                .context("rebuilding the search index after dropping legacy triggers")?;
            tx.commit()?;
        }

        // v14: v13 rows keep NULL until `backfill_downloaded_sources`.
        add_column_if_missing(&conn, "media_sources", "downloaded", "INTEGER")?;

        // v19: smart albums can be pinned.
        add_column_if_missing(
            &conn,
            "smart_albums",
            "pinned",
            "INTEGER NOT NULL DEFAULT 0",
        )?;

        // v20: culling flags (-1 reject, 0 unflagged, 1 pick).
        add_column_if_missing(&conn, "media_items", "flag", "INTEGER NOT NULL DEFAULT 0")?;

        // v15: `color:<label>` under the default date sort seeks the label and
        // walks it in timeline order (tie-breaker follows the primary
        // direction) instead of filtering every row of idx_media_timestamp.
        // Created here, not in the table batch above, because the color_label
        // column only exists on old catalogs after the ALTER further up.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_color ON media_items(color_label, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;",
            [],
        )
        .context("creating the colour label index")?;

        // v20: `is:pick` / `is:reject` seek the flag and walk it in timeline
        // order, like the colour index. Not a `WHERE flag != 0` partial index:
        // SQLite cannot prove `flag = ?` implies it, so it would never be
        // chosen. Created after the guarded ALTER that adds `flag`.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_flag ON media_items(flag, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;",
            [],
        )
        .context("creating the culling flag index")?;

        // v21: `is:edited` lists rotated items from this partial index (only
        // rotated rows, so it stays tiny) instead of scanning media_items.
        // Created after the guarded ALTER that adds `rotation_quarters`.
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_media_rotated ON media_items(rotation_quarters) WHERE rotation_quarters != 0;",
            [],
        )
        .context("creating the rotation index")?;

        // v22: an album's key photo. Purging the media sets it to NULL (the
        // connection enables foreign keys above; SQLite allows a REFERENCES
        // column in ADD COLUMN when its default is NULL). A cover removed
        // from the album or trashed stays stored but the cover query skips
        // it. The index keeps the purge's SET NULL lookup off a table scan.
        add_column_if_missing(
            &conn,
            "albums",
            "cover_media_id",
            "INTEGER REFERENCES media_items(id) ON DELETE SET NULL",
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_albums_cover ON albums(cover_media_id) WHERE cover_media_id IS NOT NULL;",
            [],
        )
        .context("creating the album key photo index")?;

        // Stamp the schema version so future migrations can dispatch on it.
        let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        // v16: the tag and duplicate-merge paths used to send the contentless
        // index 'delete's without the caption (or with values that were not
        // indexed), dropping caption words and keeping stale ones. Rebuild it
        // once; one transaction so a failure retries on the next open.
        // A new catalog (version 0) has nothing indexed yet, which is harmless.
        if current < 16 {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(REBUILD_SEARCH_INDEX_SQL)
                .context("repairing the search index")?;
            tx.commit()?;
        }
        // v23: the scanner stored an empty GPS block (0,0) as a position.
        // Clear it unless the user set the place; `media_fts_au` reindexes
        // the cleared place names. A new catalog has no rows, so this is a
        // no-op there, and a rerun finds nothing to clear.
        if current < 23 {
            conn.execute(
                "UPDATE media_items SET latitude = NULL, longitude = NULL, altitude = NULL, \
                 city = NULL, admin1_region = NULL, country_code = NULL \
                 WHERE latitude = 0 AND longitude = 0 AND place_edited = 0;",
                [],
            )
            .context("clearing 0,0 GPS positions")?;
        }
        // v24: the scanner filed an undated row (capture time 0) under
        // 1970-01-01; it now stores year, month and day 0 ("Undated"). One
        // transaction with the bucket rebuild, so a failure retries next open.
        if current < 24 {
            let tx = conn.unchecked_transaction()?;
            let changed = tx
                .execute(
                    "UPDATE media_items SET year = 0, month = 0, day = 0 \
                     WHERE timestamp_epoch_ms = 0 AND year <> 0;",
                    [],
                )
                .context("marking undated items")?;
            if changed > 0 {
                Self::rebuild_timeline_buckets(&tx)?;
            }
            tx.commit()?;
        }
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

        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_column_if_missing_adds_once() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE t (a INTEGER);", []).unwrap();
        add_column_if_missing(&conn, "t", "b", "INTEGER NOT NULL DEFAULT 0").unwrap();
        assert!(
            has_column(&conn, "t", "b").unwrap(),
            "the column must be added"
        );
        add_column_if_missing(&conn, "t", "b", "INTEGER NOT NULL DEFAULT 0")
            .expect("a second run must be a no-op, so migrations stay idempotent");
    }

    #[test]
    fn add_column_if_missing_returns_a_failed_alter() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE VIEW v AS SELECT 1 AS a;", []).unwrap();
        let err = add_column_if_missing(&conn, "v", "rating", "INTEGER")
            .expect_err("an ALTER that SQLite refuses must surface, not be swallowed");
        assert!(
            format!("{err:#}").contains("adding v.rating"),
            "the error must name the column: {err:#}"
        );
    }
}
