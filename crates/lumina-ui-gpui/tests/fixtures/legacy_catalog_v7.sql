-- Catalog layout written by an early Lumina build (PRAGMA user_version 7),
-- captured from a real catalog: schema only, no media or user data.
-- Used to prove that old catalogs upgrade and every write path still works.

CREATE TABLE app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

CREATE TABLE media_captions (
                media_id INTEGER PRIMARY KEY,
                caption TEXT NOT NULL,
                FOREIGN KEY (media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

CREATE TABLE media_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL UNIQUE,
                parent_dir TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                mtime_epoch_ms INTEGER NOT NULL,

                -- Dimensions & Media Classification
                media_type TEXT NOT NULL CHECK(media_type IN ('image', 'video', 'raw')),
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                duration_ms INTEGER DEFAULT NULL,

                -- EXIF Temporal & Spatial Attributes
                timestamp_epoch_ms INTEGER NOT NULL,
                year INTEGER NOT NULL,
                month INTEGER NOT NULL,
                day INTEGER NOT NULL,
                latitude REAL DEFAULT NULL,
                longitude REAL DEFAULT NULL,
                altitude REAL DEFAULT NULL,

                -- Reverse Geocoded Metadata
                country_code TEXT DEFAULT NULL,
                admin1_region TEXT DEFAULT NULL,
                city TEXT DEFAULT NULL,

                -- Camera Hardware Metadata
                make TEXT DEFAULT NULL,
                model TEXT DEFAULT NULL,
                lens TEXT DEFAULT NULL,
                focal_length REAL DEFAULT NULL,
                f_number REAL DEFAULT NULL,
                iso INTEGER DEFAULT NULL,
                exposure_time TEXT DEFAULT NULL,

                -- State Flags
                is_favorite INTEGER NOT NULL DEFAULT 0 CHECK(is_favorite IN (0, 1)),
                is_hidden INTEGER NOT NULL DEFAULT 0 CHECK(is_hidden IN (0, 1)),
                is_trashed INTEGER NOT NULL DEFAULT 0 CHECK(is_trashed IN (0, 1))
            , rating INTEGER NOT NULL DEFAULT 0, color_label INTEGER NOT NULL DEFAULT 0, title TEXT, rotation_quarters INTEGER NOT NULL DEFAULT 0);

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

CREATE TABLE media_tags (
                media_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                PRIMARY KEY (media_id, tag),
                FOREIGN KEY(media_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

CREATE TABLE smart_albums (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                icon_name TEXT NOT NULL DEFAULT 'folder-heart',
                match_mode TEXT NOT NULL CHECK(match_mode IN ('all', 'any')),
                rule_definition_json TEXT NOT NULL,
                created_at_epoch_ms INTEGER NOT NULL
            );

CREATE TABLE timeline_buckets (
                bucket_type TEXT NOT NULL CHECK(bucket_type IN ('year', 'month', 'day')),
                bucket_key TEXT NOT NULL,
                start_timestamp INTEGER NOT NULL,
                end_timestamp INTEGER NOT NULL,
                item_count INTEGER NOT NULL,
                cover_media_id INTEGER,
                PRIMARY KEY (bucket_type, bucket_key),
                FOREIGN KEY(cover_media_id) REFERENCES media_items(id) ON DELETE SET NULL
            );

CREATE INDEX idx_folder_listing 
                ON media_items(parent_dir, file_name) 
                WHERE is_trashed = 0;

CREATE INDEX idx_geo_lookup 
                ON media_items(latitude, longitude) 
                WHERE latitude IS NOT NULL;

CREATE INDEX idx_media_camera ON media_items(make, model);

CREATE INDEX idx_media_favorite ON media_items(is_favorite, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;

CREATE INDEX idx_media_location ON media_items(country_code, admin1_region, city);

CREATE INDEX idx_media_tags_media_id ON media_tags(media_id);

CREATE INDEX idx_media_tags_tag ON media_tags(tag);

CREATE INDEX idx_media_timestamp ON media_items(timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0;

CREATE INDEX idx_media_trashed ON media_items(is_trashed, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 1;

CREATE INDEX idx_media_type ON media_items(media_type);

CREATE INDEX idx_media_year_month ON media_items(year DESC, month DESC);

CREATE INDEX idx_timeline_active 
                ON media_items(timestamp_epoch_ms DESC, id DESC) 
                WHERE is_trashed = 0 AND is_hidden = 0;

CREATE TRIGGER media_fts_ad BEFORE DELETE ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        'delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.make, old.model, old.lens, CAST(old.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = old.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = old.id), '')
                    );
                END;

CREATE TRIGGER media_fts_ai AFTER INSERT ON media_items BEGIN
                    INSERT INTO media_search_fts(
                        rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption
                    ) VALUES (
                        new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.make, new.model, new.lens, CAST(new.year AS TEXT),
                        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = new.id), ''),
                        COALESCE((SELECT caption FROM media_captions WHERE media_id = new.id), '')
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

CREATE TRIGGER trg_media_delete AFTER DELETE ON media_items BEGIN
                INSERT INTO media_search_fts(media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES('delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.model);
            END;

CREATE TRIGGER trg_media_insert AFTER INSERT ON media_items BEGIN
                INSERT INTO media_search_fts(rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES (new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.model);
            END;

CREATE TRIGGER trg_media_update AFTER UPDATE ON media_items BEGIN
                INSERT INTO media_search_fts(media_search_fts, rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES('delete', old.id, old.file_name, old.parent_dir, old.city, old.admin1_region, old.country_code, old.model);
                INSERT INTO media_search_fts(rowid, file_name, parent_dir, city, admin1_region, country_code, model)
                VALUES (new.id, new.file_name, new.parent_dir, new.city, new.admin1_region, new.country_code, new.model);
            END;

PRAGMA user_version = 7;
