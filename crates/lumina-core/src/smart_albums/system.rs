use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemAlbum {
    Favorites,
    Videos,
    Raw,
    Screenshots,
    LongExposures,
    Panoramas,
    Bursts,
    RecentlyDeleted,
    Places,
}

impl SystemAlbum {
    pub fn all() -> &'static [SystemAlbum] {
        &[
            SystemAlbum::Favorites,
            SystemAlbum::Videos,
            SystemAlbum::Raw,
            SystemAlbum::Screenshots,
            SystemAlbum::LongExposures,
            SystemAlbum::Panoramas,
            SystemAlbum::Bursts,
            SystemAlbum::RecentlyDeleted,
            SystemAlbum::Places,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            SystemAlbum::Favorites => "Favorites",
            SystemAlbum::Videos => "Videos",
            SystemAlbum::Raw => "RAW Media",
            SystemAlbum::Screenshots => "Screenshots",
            SystemAlbum::LongExposures => "Long Exposures",
            SystemAlbum::Panoramas => "Panoramas",
            SystemAlbum::Bursts => "Bursts",
            SystemAlbum::RecentlyDeleted => "Recently Deleted",
            SystemAlbum::Places => "Places",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            SystemAlbum::Favorites => "heart",
            SystemAlbum::Videos => "video",
            SystemAlbum::Raw => "aperture",
            SystemAlbum::Screenshots => "camera",
            SystemAlbum::LongExposures => "clock",
            SystemAlbum::Panoramas => "maximize",
            SystemAlbum::Bursts => "layers",
            SystemAlbum::RecentlyDeleted => "trash",
            SystemAlbum::Places => "map-pin",
        }
    }

    pub fn build_where_clause(&self) -> &'static str {
        match self {
            SystemAlbum::Favorites => {
                "m.is_favorite = 1 AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Videos => {
                "m.media_type = 'video' AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Raw => {
                "(m.media_type = 'raw' OR m.file_name LIKE '%.arw' OR m.file_name LIKE '%.cr3' OR m.file_name LIKE '%.cr2' OR m.file_name LIKE '%.nef' OR m.file_name LIKE '%.dng' OR m.file_name LIKE '%.rw2' OR m.file_name LIKE '%.orf' OR m.file_name LIKE '%.raf') AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Screenshots => {
                "(m.file_name LIKE '%screen%' OR m.file_name LIKE '%Screen%' OR m.file_name LIKE '%Screenshot%' OR m.file_name LIKE '%Screen Shot%' OR (m.make IS NULL AND m.model IS NULL AND m.lens IS NULL AND m.focal_length IS NULL AND m.iso IS NULL AND m.width > 0 AND m.height > 0 AND ((CAST(m.width AS REAL)/m.height >= 1.5 AND CAST(m.width AS REAL)/m.height <= 2.4) OR (CAST(m.height AS REAL)/m.width >= 1.5 AND CAST(m.height AS REAL)/m.width <= 2.4)))) AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::LongExposures => {
                "m.exposure_time IS NOT NULL AND m.exposure_time != '' AND m.exposure_time NOT LIKE '%/%' AND CAST(m.exposure_time AS REAL) >= 1.0 AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Panoramas => {
                "m.width > 0 AND m.height > 0 AND (CAST(m.width AS REAL) / m.height >= 2.8 OR CAST(m.height AS REAL) / m.width >= 2.8) AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Bursts => {
                "(m.file_name LIKE '%burst%' OR m.file_name LIKE '%BURST%') AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::RecentlyDeleted => {
                "m.is_trashed = 1"
            }
            SystemAlbum::Places => {
                "m.latitude IS NOT NULL AND m.longitude IS NOT NULL AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
        }
    }

    pub fn synthesize_query(
        &self,
        limit: usize,
        offset: usize,
    ) -> (String, Vec<rusqlite::types::Value>) {
        let where_clause = self.build_where_clause();
        let sql = format!(
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
            WHERE {}
            ORDER BY m.timestamp_epoch_ms DESC, m.id DESC
            LIMIT ?1 OFFSET ?2;
            "#,
            where_clause
        );

        let params = vec![(limit as i64).into(), (offset as i64).into()];

        (sql, params)
    }

    pub fn synthesize_count_query(&self) -> (String, Vec<rusqlite::types::Value>) {
        let where_clause = self.build_where_clause();
        let sql = format!(
            r#"
            SELECT COUNT(*)
            FROM media_items m
            WHERE {};
            "#,
            where_clause
        );

        (sql, Vec::new())
    }
}
