use serde::{Deserialize, Serialize};

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
    /// Parses a user search query string containing keywords and structured tokens.
    /// Example: "sunset in Kyoto camera:Sony is:favorite year:2026 iso:>400 date:2026-09-01..2026-10-10"
    pub fn parse(input: &str) -> Self {
        let mut query = Self::default();
        let tokens = tokenize_query(input);

        for token in tokens {
            if let Some((prefix, value)) = split_prefix(&token) {
                let prefix_lower = prefix.to_lowercase();
                let val_clean = value.trim_matches('"').trim();

                match prefix_lower.as_str() {
                    "year" => {
                        if let Ok(y) = val_clean.parse::<i32>() {
                            query.year = Some(y);
                        }
                    }
                    "month" => {
                        if let Some(m) = parse_month(val_clean) {
                            query.month = Some(m);
                        }
                    }
                    "date" => {
                        if let Some(range) = parse_date_filter(val_clean) {
                            query.date_range = Some(range);
                        }
                    }
                    "camera" | "model" | "make" => {
                        if !val_clean.is_empty() {
                            query.camera = Some(val_clean.to_string());
                        }
                    }
                    "lens" => {
                        if !val_clean.is_empty() {
                            query.lens = Some(val_clean.to_string());
                        }
                    }
                    "city" => {
                        if !val_clean.is_empty() {
                            query.city = Some(val_clean.to_string());
                        }
                    }
                    "country" => {
                        if !val_clean.is_empty() {
                            query.country = Some(val_clean.to_uppercase());
                        }
                    }
                    "iso" => {
                        parse_num_filter(val_clean, &mut query.min_iso, &mut query.max_iso);
                    }
                    "f" | "fnumber" => {
                        parse_float_filter(
                            val_clean,
                            &mut query.min_f_number,
                            &mut query.max_f_number,
                        );
                    }
                    "is" => match val_clean.to_lowercase().as_str() {
                        "favorite" | "fav" => query.is_favorite = Some(true),
                        "video" => query.media_type = Some("video".to_string()),
                        "image" | "photo" => query.media_type = Some("image".to_string()),
                        "raw" => query.media_type = Some("raw".to_string()),
                        "trashed" | "trash" => query.is_trashed = Some(true),
                        _ => {}
                    },
                    "has" => match val_clean.to_lowercase().as_str() {
                        "location" | "geo" | "gps" => query.has_location = Some(true),
                        "tag" | "tags" | "label" => query.has_tag = Some(true),
                        "color" | "color_label" => query.has_color_label = Some(true),
                        "rating" | "stars" => query.min_rating = Some(1),
                        _ => {}
                    },
                    "rating" | "stars" => {
                        parse_num_filter(val_clean, &mut query.min_rating, &mut query.max_rating);
                    }
                    "color" => {
                        let code = match val_clean.to_lowercase().as_str() {
                            "red" | "1" => Some(1),
                            "orange" | "2" => Some(2),
                            "yellow" | "3" => Some(3),
                            "green" | "4" => Some(4),
                            "blue" | "5" => Some(5),
                            "purple" | "6" => Some(6),
                            "none" | "0" => Some(0),
                            _ => None,
                        };
                        if let Some(c) = code {
                            query.color_label = Some(c);
                        }
                    }
                    "tag" | "keyword" => {
                        if !val_clean.is_empty() {
                            query.tag = Some(val_clean.to_lowercase());
                        }
                    }
                    _ => {
                        // Unrecognized prefix treated as free-form text
                        query.fts_terms.push(token);
                    }
                }
            } else {
                let trimmed = token.trim_matches('"').trim();
                if !trimmed.is_empty() {
                    query.fts_terms.push(trimmed.to_string());
                }
            }
        }

        query
    }

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

/// Splits a search query into terms, keeping `"quoted phrases"` (and
/// `tag:"two words"`) together as one term.
pub fn tokenize_query(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' | '\t' if !in_quotes => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    tokens
}

fn split_prefix(token: &str) -> Option<(&str, &str)> {
    if let Some(pos) = token.find(':') {
        let prefix = &token[..pos];
        let val = &token[pos + 1..];
        if !prefix.is_empty() && !val.is_empty() {
            return Some((prefix, val));
        }
    }
    None
}

fn parse_month(s: &str) -> Option<u32> {
    if let Ok(m) = s.parse::<u32>() {
        if (1..=12).contains(&m) {
            return Some(m);
        }
    }
    match s.to_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn parse_date_filter(val: &str) -> Option<(i64, i64)> {
    if let Some((start_s, end_s)) = val.split_once("..") {
        let start_opt = chrono::NaiveDate::parse_from_str(start_s.trim(), "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().timestamp_millis());
        let end_opt = chrono::NaiveDate::parse_from_str(end_s.trim(), "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .map(|dt| dt.and_utc().timestamp_millis() + 999);
        if start_opt.is_none() && end_opt.is_none() {
            return None;
        }
        return Some((start_opt.unwrap_or(0), end_opt.unwrap_or(i64::MAX)));
    }
    if let Some(stripped) = val.strip_prefix(">=") {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let start = d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
            return Some((start, i64::MAX));
        }
    } else if let Some(stripped) = val.strip_prefix('>') {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let start = d.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis() + 1000;
            return Some((start, i64::MAX));
        }
    } else if let Some(stripped) = val.strip_prefix("<=") {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let end = d.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis() + 999;
            return Some((0, end));
        }
    } else if let Some(stripped) = val.strip_prefix('<') {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let end = d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis() - 1;
            return Some((0, end));
        }
    } else if let Ok(d) = chrono::NaiveDate::parse_from_str(val.trim(), "%Y-%m-%d") {
        let start = d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
        let end = d.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis() + 999;
        return Some((start, end));
    }
    None
}

fn parse_num_filter(s: &str, min: &mut Option<u32>, max: &mut Option<u32>) {
    if let Some(stripped) = s.strip_prefix(">=") {
        if let Ok(n) = stripped.parse::<u32>() {
            *min = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('>') {
        if let Ok(n) = stripped.parse::<u32>() {
            *min = Some(n.saturating_add(1));
        }
    } else if let Some(stripped) = s.strip_prefix("<=") {
        if let Ok(n) = stripped.parse::<u32>() {
            *max = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('<') {
        if let Ok(n) = stripped.parse::<u32>() {
            *max = Some(n.saturating_sub(1));
        }
    } else if let Ok(n) = s.parse::<u32>() {
        *min = Some(n);
        *max = Some(n);
    }
}

fn parse_float_filter(s: &str, min: &mut Option<f64>, max: &mut Option<f64>) {
    if let Some(stripped) = s.strip_prefix(">=") {
        if let Ok(n) = stripped.parse::<f64>() {
            *min = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('>') {
        if let Ok(n) = stripped.parse::<f64>() {
            *min = Some(n + 0.01);
        }
    } else if let Some(stripped) = s.strip_prefix("<=") {
        if let Ok(n) = stripped.parse::<f64>() {
            *max = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('<') {
        if let Ok(n) = stripped.parse::<f64>() {
            *max = Some(n - 0.01);
        }
    } else if let Ok(n) = s.parse::<f64>() {
        *min = Some(n - 0.05);
        *max = Some(n + 0.05);
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
    fn test_parse_structured_query() {
        let q = StructuredQuery::parse(
            r#"sunset in Kyoto camera:"Sony A7IV" is:favorite year:2026 iso:>400 f:<2.8 date:2026-09-01..2026-10-10"#,
        );

        assert_eq!(q.fts_terms, vec!["sunset", "in", "Kyoto"]);
        assert_eq!(q.camera.as_deref(), Some("Sony A7IV"));
        assert_eq!(q.is_favorite, Some(true));
        assert_eq!(q.year, Some(2026));
        assert_eq!(q.min_iso, Some(401));
        assert!(q.max_f_number.is_some());
        assert!(q.date_range.is_some());
    }

    #[test]
    fn test_date_range_parsing() {
        let r = parse_date_filter("2026-01-01..2026-01-02").unwrap();
        assert!(r.0 < r.1);

        let r_exact = parse_date_filter("2026-05-10").unwrap();
        assert!(r_exact.0 < r_exact.1);
    }

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
