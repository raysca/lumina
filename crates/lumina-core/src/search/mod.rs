use crate::catalog::keywords::{subtree_clause, subtree_params};
use serde::{Deserialize, Serialize};

mod explain;
mod parse;

pub use parse::tokenize_query;

/// A parsed search query: free text plus structured filters.
///
/// Fields added after the first release carry `#[serde(default)]` so older
/// serialized queries still deserialize.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StructuredQuery {
    pub fts_terms: Vec<String>,
    /// Free text to exclude (`-word`, `-"two words"`), with the same prefix
    /// matching as inclusion: `-beach` also hides "beaches".
    #[serde(default)]
    pub fts_exclude: Vec<String>,
    /// Tags to exclude (`-tag:x`), lowercased like `tag`.
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    /// Filter tokens that were not understood, as typed (`is:fovorite`,
    /// `rating:abc`, `-is:video`). They add no clause and are not searched.
    #[serde(default)]
    pub unrecognized: Vec<String>,
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
    /// `is:pick`, `is:reject`, `is:unflagged`.
    #[serde(default)]
    pub flag: Option<crate::Flag>,
}

/// How the free-text FTS match is written into the WHERE clause.
#[derive(Clone, Copy)]
enum FtsMatch {
    /// `media_search_fts MATCH ?`: the query joins `media_search_fts`.
    Joined,
    /// `m.id IN (SELECT rowid …)`: for SQL that selects from `media_items`
    /// alone, such as a smart album. `sorted` adds the unary `+` that SQL
    /// with an ORDER BY needs; a count leaves it off so FTS drives.
    Subquery { sorted: bool },
}

impl StructuredQuery {
    /// Builds a parameterized SQL WHERE clause and parameters. The FTS match
    /// needs `media_search_fts` joined, as [`Self::synthesize_query`] does.
    pub fn build_sql(&self) -> (String, Vec<rusqlite::types::Value>) {
        self.where_clause(FtsMatch::Joined)
    }

    /// The same WHERE clause as [`Self::build_sql`], but the FTS match is an
    /// `m.id IN (SELECT rowid …)` subquery, so it runs against `media_items m`
    /// alone (smart album `Query` rules). Pass `sorted` when the SQL has an
    /// ORDER BY; leave it false for a count.
    pub fn build_where_subquery(&self, sorted: bool) -> (String, Vec<rusqlite::types::Value>) {
        self.where_clause(FtsMatch::Subquery { sorted })
    }

    /// The one clause builder behind `build_sql` and `build_where_subquery`,
    /// so a search and a saved search cannot drift apart.
    fn where_clause(&self, fts_match: FtsMatch) -> (String, Vec<rusqlite::types::Value>) {
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

        // `tag:x` and `-tag:x` share one subtree test, so both cover x/...
        if let Some(ref tag) = self.tag {
            clauses.push(format!(
                "EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id AND {})",
                subtree_clause("tag")
            ));
            params.extend(subtree_params(tag));
        }

        for tag in &self.exclude_tags {
            clauses.push(format!(
                "NOT EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id AND {})",
                subtree_clause("tag")
            ));
            params.extend(subtree_params(tag));
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

        if let Some(flag) = self.flag {
            clauses.push("m.flag = ?".to_string());
            params.push((flag as i64).into());
        }

        // Exclusions are one OR expression over the FTS index, so the planner
        // runs a single indexed subquery however many words are excluded.
        let exclude = fts5_terms(&self.fts_exclude).join(" OR ");
        if !exclude.is_empty() {
            clauses.push(
                "m.id NOT IN (SELECT rowid FROM media_search_fts WHERE media_search_fts MATCH ?)"
                    .to_string(),
            );
            params.push(exclude.into());
        }

        // FTS match clause if free-text terms exist
        let fts_query = build_fts5_expression(&self.fts_terms);
        if !fts_query.is_empty() {
            clauses.push(match fts_match {
                FtsMatch::Joined => "media_search_fts MATCH ?".to_string(),
                // Sorted: the unary `+` stops the planner from driving the
                // query from the FTS rowids, which forces a temp B-tree sort;
                // instead it walks the sort index and probes the matched set.
                // ponytail: a very rare term walks the whole timeline index;
                // drive from FTS when the result set is known to be small.
                // A count has no ORDER BY, so it drives from the FTS rowids.
                FtsMatch::Subquery { sorted } => format!(
                    "{}m.id IN (SELECT rowid FROM media_search_fts WHERE media_search_fts MATCH ?)",
                    if sorted { "+" } else { "" }
                ),
            });
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

/// Escapes each term as a literal FTS5 prefix phrase (`"term"*`), dropping
/// quotes and `*` so typed text can never become FTS5 syntax.
fn fts5_terms(terms: &[String]) -> Vec<String> {
    terms
        .iter()
        .map(|t| t.replace(['"', '*'], "").trim().to_string())
        .filter(|clean| !clean.is_empty())
        .map(|clean| format!("\"{}\"*", clean))
        .collect()
}

fn build_fts5_expression(terms: &[String]) -> String {
    fts5_terms(terms).join(" ")
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
        assert!(sql_tag.contains(&format!(
            "EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id AND {})",
            subtree_clause("tag")
        )));
        assert_eq!(params.len(), 4, "the subtree test binds the tag four times");
    }

    #[test]
    fn color_none_and_unlabeled_are_one_clause() {
        for text in ["color:none", "color:0", "is:unlabeled", "-has:color"] {
            let q = StructuredQuery::parse(text);
            assert_eq!(q.color_label, None, "{text:?} must not set a label value");
            assert_eq!(q.has_color_label, Some(false), "{text:?}");
            let (sql, _) = q.build_sql();
            assert_eq!(
                sql.matches("color_label").count(),
                1,
                "{text:?} must give exactly one color clause, got {sql}"
            );
        }
        assert_eq!(
            StructuredQuery::parse("color:none").build_sql(),
            StructuredQuery::parse("is:unlabeled").build_sql()
        );
    }
}
