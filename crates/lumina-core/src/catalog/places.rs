//! Places the catalog holds: cities with their item counts, for the search
//! header's Place top matches.

use anyhow::Result;
use rusqlite::params;

use super::Catalog;

/// A city in the catalog, its country code (empty when unknown) and how
/// many active (not trashed, not hidden) items were taken there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceCount {
    pub city: String,
    pub country_code: String,
    pub count: usize,
}

impl Catalog {
    /// Cities whose name starts with `prefix` (case-insensitive), most
    /// items first, at most `limit`. A prefix with no letters or digits
    /// matches nothing.
    ///
    /// The search index finds the candidate rows (an FTS5 prefix query on
    /// its `city` column), so the cost follows the matching items, not the
    /// library; the join keeps active items, and LIKE keeps the name's
    /// start (FTS matches any word of "New York").
    pub fn matching_places(&self, prefix: &str, limit: usize) -> Result<Vec<PlaceCount>> {
        let prefix = prefix.trim();
        if !prefix.chars().any(char::is_alphanumeric) || limit == 0 {
            return Ok(Vec::new());
        }
        let fts = format!("city : \"{}\"*", prefix.replace('"', "\"\""));
        let mut pattern = String::with_capacity(prefix.len() + 1);
        for ch in prefix.chars() {
            if matches!(ch, '%' | '_' | '\\') {
                pattern.push('\\');
            }
            pattern.push(ch);
        }
        pattern.push('%');
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            r#"SELECT m.city, COALESCE(m.country_code, ''), COUNT(*) AS n
               FROM media_search_fts f
               CROSS JOIN media_items m ON m.id = f.rowid
               WHERE media_search_fts MATCH ?1
                 AND m.is_trashed = 0 AND m.is_hidden = 0
                 AND m.city LIKE ?2 ESCAPE '\'
               GROUP BY m.city, m.country_code
               ORDER BY n DESC, m.city ASC
               LIMIT ?3;"#,
        )?;
        let rows = stmt.query_map(params![fts, pattern, limit as i64], |row| {
            Ok(PlaceCount {
                city: row.get(0)?,
                country_code: row.get(1)?,
                count: row.get::<_, i64>(2)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MediaId, MediaItem, MediaType};
    use std::path::PathBuf;

    fn item(index: usize, city: &str, country: &str) -> MediaItem {
        let file_name = format!("IMG_{index:04}.JPG");
        MediaItem {
            id: MediaId(0),
            path: PathBuf::from(format!("/library/{file_name}")),
            parent_dir: "/library".to_string(),
            file_name,
            title: None,
            file_size: 1,
            mtime_epoch_ms: 1_642_200_000_000,
            media_type: MediaType::Image,
            width: 1,
            height: 1,
            duration_ms: None,
            timestamp_epoch_ms: 1_642_200_000_000,
            year: 2022,
            month: 1,
            day: 15,
            latitude: None,
            longitude: None,
            altitude: None,
            country_code: Some(country.to_string()),
            admin1_region: None,
            city: Some(city.to_string()),
            make: None,
            model: None,
            lens: None,
            focal_length: None,
            f_number: None,
            iso: None,
            exposure_time: None,
            is_favorite: false,
            is_hidden: false,
            is_trashed: false,
        }
    }

    #[test]
    fn matching_places_returns_city_country_and_count() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_places_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let catalog = Catalog::open(dir.join("catalog.db")).expect("catalog opens");
        let mut items = vec![
            item(1, "Lisbon", "PT"),
            item(2, "Lisbon", "PT"),
            item(3, "Lisbon", "PT"),
            item(4, "Porto", "PT"),
        ];
        // A trashed Lisbon item does not count.
        let mut trashed = item(5, "Lisbon", "PT");
        trashed.is_trashed = true;
        items.push(trashed);
        catalog
            .insert_or_update_batch(&items)
            .expect("items insert");

        let places = catalog.matching_places("lis", 4).expect("query runs");
        assert_eq!(
            places,
            vec![PlaceCount {
                city: "Lisbon".to_string(),
                country_code: "PT".to_string(),
                count: 3,
            }],
            "a case-insensitive prefix finds the city with its active count"
        );
        assert!(
            catalog.matching_places("", 4).unwrap().is_empty(),
            "no prefix, no places"
        );
        assert!(
            catalog.matching_places("l%", 4).unwrap().is_empty(),
            "LIKE wildcards in the text are literal"
        );
        assert!(
            catalog.matching_places("\"%", 4).unwrap().is_empty(),
            "quotes and symbols alone match nothing and do not break the query"
        );
        assert!(
            catalog.matching_places("lis\"", 4).unwrap().is_empty(),
            "a quote inside the text is escaped for MATCH and kept for the name"
        );
        assert!(
            catalog.matching_places("orto", 4).unwrap().is_empty(),
            "the name's start, not the middle"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
