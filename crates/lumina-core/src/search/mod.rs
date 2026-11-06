use serde::{Deserialize, Serialize};

mod parse;

pub use parse::tokenize_query;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StructuredQuery {
    pub fts_terms: Vec<String>,
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub date_range: Option<(i64, i64)>, // epoch ms
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub min_iso: Option<u32>,
    pub max_iso: Option<u32>,
    pub min_f_number: Option<f64>,
    pub max_f_number: Option<f64>,
    pub media_type: Option<String>, // 'image', 'video', 'raw'
    pub is_favorite: Option<bool>,
    pub is_trashed: Option<bool>,
    pub has_location: Option<bool>,
    pub has_tag: Option<bool>,
    pub tag: Option<String>,
    pub min_rating: Option<u32>,
    pub max_rating: Option<u32>,
    pub color_label: Option<u8>,
    pub has_color_label: Option<bool>,
}

impl StructuredQuery {
    /// Builds a parameterized SQL WHERE clause and parameters.
    pub fn build_sql(&self) -> (String, Vec<rusqlite::types::Value>) {
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<rusqlite::types::Value> = Vec::new();

        // Status flags: default to non-trashed and visible unless is:trashed requested
        if let Some(trashed) = self.is_trashed {
            clauses.push("m.is_trashed = ?".to_string());
            params.push((trashed as i32).into());
        } else {
            clauses.push("m.is_trashed = 0".to_string());
            clauses.push("m.is_hidden = 0".to_string());
        }

        if let Some(fav) = self.is_favorite {
            clauses.push("m.is_favorite = ?".to_string());
            params.push((fav as i32).into());
        }

        if let Some(ref m_type) = self.media_type {
            clauses.push("m.media_type = ?".to_string());
            params.push(m_type.clone().into());
        }

        if let Some(y) = self.year {
            clauses.push("m.year = ?".to_string());
            params.push(y.into());
        }

        if let Some(m) = self.month {
            clauses.push("m.month = ?".to_string());
            params.push((m as i64).into());
        }

        if let Some((start, end)) = self.date_range {
            clauses.push("m.timestamp_epoch_ms >= ? AND m.timestamp_epoch_ms <= ?".to_string());
            params.push(start.into());
            params.push(end.into());
        }

        if let Some(ref cam) = self.camera {
            let escaped = cam
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            clauses.push("(m.make LIKE ? ESCAPE '\\' OR m.model LIKE ? ESCAPE '\\')".to_string());
            let pattern = format!("%{}%", escaped);
            params.push(pattern.clone().into());
            params.push(pattern.into());
        }

        if let Some(ref lens) = self.lens {
            let escaped = lens
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            clauses.push("m.lens LIKE ? ESCAPE '\\'".to_string());
            params.push(format!("%{}%", escaped).into());
        }

        if let Some(ref city) = self.city {
            let escaped = city
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            clauses.push("m.city LIKE ? ESCAPE '\\'".to_string());
            params.push(format!("%{}%", escaped).into());
        }

        if let Some(ref country) = self.country {
            clauses.push("m.country_code = ?".to_string());
            params.push(country.clone().into());
        }

        if let Some(min_iso) = self.min_iso {
            clauses.push("m.iso >= ?".to_string());
            params.push((min_iso as i64).into());
        }

        if let Some(max_iso) = self.max_iso {
            clauses.push("m.iso <= ?".to_string());
            params.push((max_iso as i64).into());
        }

        if let Some(min_f) = self.min_f_number {
            clauses.push("m.f_number >= ?".to_string());
            params.push(min_f.into());
        }

        if let Some(max_f) = self.max_f_number {
            clauses.push("m.f_number <= ?".to_string());
            params.push(max_f.into());
        }

        if let Some(has_loc) = self.has_location {
            if has_loc {
                clauses.push("m.latitude IS NOT NULL AND m.longitude IS NOT NULL".to_string());
            } else {
                clauses.push("m.latitude IS NULL".to_string());
            }
        }

        if let Some(has_tag) = self.has_tag {
            if has_tag {
                clauses.push("EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id)".to_string());
            } else {
                clauses.push(
                    "NOT EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id)".to_string(),
                );
            }
        }

        if let Some(ref tag) = self.tag {
            clauses.push(
                "EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id AND tag = ?)".to_string(),
            );
            params.push(tag.clone().into());
        }

        if let Some(min_r) = self.min_rating {
            clauses.push("m.rating >= ?".to_string());
            params.push((min_r as i64).into());
        }

        if let Some(max_r) = self.max_rating {
            clauses.push("m.rating <= ?".to_string());
            params.push((max_r as i64).into());
        }

        if let Some(c) = self.color_label {
            clauses.push("m.color_label = ?".to_string());
            params.push((c as i64).into());
        }

        if let Some(has_c) = self.has_color_label {
            if has_c {
                clauses.push("m.color_label > 0".to_string());
            } else {
                clauses.push("m.color_label = 0".to_string());
            }
        }

        // FTS match clause if free-text terms exist
        let fts_query = build_fts5_expression(&self.fts_terms);
        if !fts_query.is_empty() {
            clauses.push("media_search_fts MATCH ?".to_string());
            params.push(fts_query.into());
        }

        let where_sql = if clauses.is_empty() {
            "1".to_string()
        } else {
            clauses.join(" AND ")
        };

        (where_sql, params)
    }

    /// Synthesizes the full query with SELECT columns and optional FTS5 JOIN.
    pub fn synthesize_query(
        &self,
        limit: usize,
        offset: usize,
    ) -> (String, Vec<rusqlite::types::Value>) {
        self.synthesize_query_sorted(limit, offset, crate::SortSpec::default())
    }

    pub fn synthesize_query_sorted(
        &self,
        limit: usize,
        offset: usize,
        sort: crate::SortSpec,
    ) -> (String, Vec<rusqlite::types::Value>) {
        let (where_clause, mut params) = self.build_sql();
        let fts_query = build_fts5_expression(&self.fts_terms);

        let sql = if !fts_query.is_empty() {
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
                FROM media_search_fts
                JOIN media_items m ON media_search_fts.rowid = m.id
                WHERE {}
                ORDER BY {}
                LIMIT ? OFFSET ?;
                "#,
                where_clause,
                sort.order_by("m.")
            )
        } else {
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
                WHERE {}
                ORDER BY {}
                LIMIT ? OFFSET ?;
                "#,
                where_clause,
                sort.order_by("m.")
            )
        };

        params.push((limit as i64).into());
        params.push((offset as i64).into());

        (sql, params)
    }

    pub fn synthesize_count_query(&self) -> (String, Vec<rusqlite::types::Value>) {
        let (where_clause, params) = self.build_sql();
        let fts_query = build_fts5_expression(&self.fts_terms);

        let sql = if !fts_query.is_empty() {
            format!(
                r#"
                SELECT COUNT(*)
                FROM media_search_fts
                JOIN media_items m ON media_search_fts.rowid = m.id
                WHERE {};
                "#,
                where_clause
            )
        } else {
            format!(
                r#"
                SELECT COUNT(*)
                FROM media_items m
                WHERE {};
                "#,
                where_clause
            )
        };

        (sql, params)
    }
}

fn build_fts5_expression(terms: &[String]) -> String {
    let escaped_terms: Vec<String> = terms
        .iter()
        .map(|t| t.replace(['"', '*'], "").trim().to_string())
        .filter(|clean| !clean.is_empty())
        .map(|clean| format!("\"{}\"*", clean))
        .collect();

    escaped_terms.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_and_special_fts_terms() {
        let terms = vec![
            "".to_string(),
            "\"\"".to_string(),
            "*".to_string(),
            "valid".to_string(),
        ];
        let expr = build_fts5_expression(&terms);
        assert_eq!(expr, "\"valid\"*");

        let empty_terms: Vec<String> = vec!["*".to_string(), "\"\"".to_string()];
        let expr_empty = build_fts5_expression(&empty_terms);
        assert_eq!(expr_empty, "");
    }

    #[test]
    fn test_has_tag_sql_clauses() {
        let q_untagged = StructuredQuery {
            has_tag: Some(false),
            ..Default::default()
        };
        let (sql_untagged, _) = q_untagged.build_sql();
        assert!(
            sql_untagged.contains("NOT EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id)")
        );

        let q_tagged = StructuredQuery {
            has_tag: Some(true),
            ..Default::default()
        };
        let (sql_tagged, _) = q_tagged.build_sql();
        assert!(sql_tagged.contains("EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id)"));

        let q_tag = StructuredQuery {
            tag: Some("vacation".to_string()),
            ..Default::default()
        };
        let (sql_tag, params) = q_tag.build_sql();
        assert!(
            sql_tag.contains("EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id AND tag = ?)")
        );
        assert_eq!(params.len(), 1);
    }
}
