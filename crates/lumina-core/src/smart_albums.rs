use anyhow::{anyhow, Result};
use chrono::Utc;
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
                "(m.file_name LIKE '%screen%' OR m.file_name LIKE '%Screen%' OR m.file_name LIKE '%Screenshot%' OR m.file_name LIKE '%Screen Shot%' OR (m.lens IS NULL AND m.focal_length IS NULL AND m.iso IS NULL AND m.width > 0 AND m.height > 0 AND ((CAST(m.width AS REAL)/m.height >= 1.5 AND CAST(m.width AS REAL)/m.height <= 2.4) OR (CAST(m.height AS REAL)/m.width >= 1.5 AND CAST(m.height AS REAL)/m.width <= 2.4)))) AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::LongExposures => {
                "m.exposure_time IS NOT NULL AND m.exposure_time != '' AND NOT (m.exposure_time LIKE '1/%' AND m.exposure_time NOT LIKE '1/1' AND m.exposure_time NOT LIKE '1/0%') AND (CAST(m.exposure_time AS REAL) >= 1.0 OR m.exposure_time NOT LIKE '1/%') AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Panoramas => {
                "m.width > 0 AND m.height > 0 AND (CAST(m.width AS REAL) / m.height >= 2.8 OR CAST(m.height AS REAL) / m.width >= 2.8) AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::Bursts => {
                "m.id IN (SELECT b1.id FROM media_items b1 JOIN media_items b2 ON b1.id != b2.id AND b1.model IS NOT NULL AND b1.model = b2.model AND abs(b1.timestamp_epoch_ms - b2.timestamp_epoch_ms) <= 400 WHERE b1.is_trashed = 0) AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
            SystemAlbum::RecentlyDeleted => {
                "m.is_trashed = 1"
            }
            SystemAlbum::Places => {
                "m.latitude IS NOT NULL AND m.longitude IS NOT NULL AND m.is_trashed = 0 AND m.is_hidden = 0"
            }
        }
    }

    pub fn synthesize_query(&self, limit: usize, offset: usize) -> (String, Vec<rusqlite::types::Value>) {
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
                m.is_favorite, m.is_hidden, m.is_trashed
            FROM media_items m
            WHERE {}
            ORDER BY m.timestamp_epoch_ms DESC, m.id DESC
            LIMIT ?1 OFFSET ?2;
            "#,
            where_clause
        );

        let params = vec![
            (limit as i64).into(),
            (offset as i64).into(),
        ];

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    All,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleField {
    MediaType,
    IsFavorite,
    Year,
    Month,
    City,
    CountryCode,
    CameraMake,
    CameraModel,
    Lens,
    Iso,
    FNumber,
    FocalLength,
    Date,
    Filename,
    FileSize,
    DurationMs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOperator {
    Equals,
    NotEquals,
    Contains,
    NotContains,
    GreaterThan,
    LessThan,
    Between,
    InTheLast,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleValue {
    Text(String),
    Number(f64),
    Integer(i64),
    Bool(bool),
    Range(f64, f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartAlbumRule {
    pub field: RuleField,
    pub operator: RuleOperator,
    pub value: RuleValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAlbum {
    pub id: i64,
    pub title: String,
    pub icon_name: String,
    pub match_mode: MatchMode,
    pub rules: Vec<SmartAlbumRule>,
    pub created_at_epoch_ms: i64,
}

impl SmartAlbum {
    pub fn new(title: String, icon_name: String, match_mode: MatchMode, rules: Vec<SmartAlbumRule>) -> Self {
        Self {
            id: 0,
            title,
            icon_name,
            match_mode,
            rules,
            created_at_epoch_ms: Utc::now().timestamp_millis(),
        }
    }

    /// Compiles the rules into a parameterized SQL WHERE clause and bindings.
    pub fn build_sql(&self) -> Result<(String, Vec<rusqlite::types::Value>)> {
        if self.rules.is_empty() {
            return Ok(("m.is_trashed = 0 AND m.is_hidden = 0".to_string(), Vec::new()));
        }

        let mut clauses = Vec::new();
        let mut params = Vec::new();

        for rule in &self.rules {
            let (clause, rule_params) = compile_rule(rule)?;
            clauses.push(clause);
            params.extend(rule_params);
        }

        let joiner = match self.match_mode {
            MatchMode::All => " AND ",
            MatchMode::Any => " OR ",
        };

        let combined = format!("({}) AND m.is_trashed = 0 AND m.is_hidden = 0", clauses.join(joiner));
        Ok((combined, params))
    }

    pub fn synthesize_query(&self, limit: usize, offset: usize) -> Result<(String, Vec<rusqlite::types::Value>)> {
        let (where_clause, mut params) = self.build_sql()?;

        let sql = format!(
            r#"
            SELECT 
                m.id, m.file_path, m.parent_dir, m.file_name, m.file_size, m.mtime_epoch_ms,
                m.media_type, m.width, m.height, m.duration_ms,
                m.timestamp_epoch_ms, m.year, m.month, m.day,
                m.latitude, m.longitude, m.altitude,
                m.country_code, m.admin1_region, m.city,
                m.make, m.model, m.lens, m.focal_length, m.f_number, m.iso, m.exposure_time,
                m.is_favorite, m.is_hidden, m.is_trashed
            FROM media_items m
            WHERE {}
            ORDER BY m.timestamp_epoch_ms DESC, m.id DESC
            LIMIT ? OFFSET ?;
            "#,
            where_clause
        );

        params.push((limit as i64).into());
        params.push((offset as i64).into());

        Ok((sql, params))
    }

    pub fn synthesize_count_query(&self) -> Result<(String, Vec<rusqlite::types::Value>)> {
        let (where_clause, params) = self.build_sql()?;

        let sql = format!(
            r#"
            SELECT COUNT(*)
            FROM media_items m
            WHERE {};
            "#,
            where_clause
        );

        Ok((sql, params))
    }
}

fn column_for_field(field: &RuleField) -> &'static str {
    match field {
        RuleField::MediaType => "m.media_type",
        RuleField::IsFavorite => "m.is_favorite",
        RuleField::Year => "m.year",
        RuleField::Month => "m.month",
        RuleField::City => "m.city",
        RuleField::CountryCode => "m.country_code",
        RuleField::CameraMake => "m.make",
        RuleField::CameraModel => "m.model",
        RuleField::Lens => "m.lens",
        RuleField::Iso => "m.iso",
        RuleField::FNumber => "m.f_number",
        RuleField::FocalLength => "m.focal_length",
        RuleField::Date => "m.timestamp_epoch_ms",
        RuleField::Filename => "m.file_name",
        RuleField::FileSize => "m.file_size",
        RuleField::DurationMs => "m.duration_ms",
    }
}

fn compile_rule(rule: &SmartAlbumRule) -> Result<(String, Vec<rusqlite::types::Value>)> {
    let col = column_for_field(&rule.field);

    match &rule.operator {
        RuleOperator::Equals => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} = ?", col), vec![val]))
        }
        RuleOperator::NotEquals => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("({} IS NULL OR {} != ?)", col, col), vec![val]))
        }
        RuleOperator::Contains => {
            let s = rule_value_to_string(&rule.value)?;
            Ok((format!("{} LIKE ?", col), vec![format!("%{}%", s).into()]))
        }
        RuleOperator::NotContains => {
            let s = rule_value_to_string(&rule.value)?;
            Ok((format!("({} IS NULL OR {} NOT LIKE ?)", col, col), vec![format!("%{}%", s).into()]))
        }
        RuleOperator::GreaterThan => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} > ?", col), vec![val]))
        }
        RuleOperator::LessThan => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} < ?", col), vec![val]))
        }
        RuleOperator::Between => {
            match &rule.value {
                RuleValue::Range(min, max) => {
                    Ok((format!("{} >= ? AND {} <= ?", col, col), vec![(*min).into(), (*max).into()]))
                }
                RuleValue::Text(s) => {
                    if let Some((min_s, max_s)) = s.split_once("..") {
                        let min: f64 = min_s.trim().parse().map_err(|_| anyhow!("Invalid range min: {}", min_s))?;
                        let max: f64 = max_s.trim().parse().map_err(|_| anyhow!("Invalid range max: {}", max_s))?;
                        Ok((format!("{} >= ? AND {} <= ?", col, col), vec![min.into(), max.into()]))
                    } else {
                        Err(anyhow!("Invalid range format for Between: {}", s))
                    }
                }
                _ => Err(anyhow!("Between operator requires Range or 'min..max' string")),
            }
        }
        RuleOperator::InTheLast => {
            // E.g. "30d", "7d", "1y", or integer days
            let ms_delta = parse_duration_ms(&rule.value)?;
            let cutoff = Utc::now().timestamp_millis() - ms_delta;
            Ok((format!("{} >= ?", col), vec![cutoff.into()]))
        }
        RuleOperator::IsEmpty => {
            Ok((format!("({} IS NULL OR {} = '')", col, col), Vec::new()))
        }
        RuleOperator::IsNotEmpty => {
            Ok((format!("({} IS NOT NULL AND {} != '')", col, col), Vec::new()))
        }
    }
}

fn rule_value_to_sqlite(v: &RuleValue) -> Result<rusqlite::types::Value> {
    match v {
        RuleValue::Text(s) => Ok(s.clone().into()),
        RuleValue::Number(n) => Ok((*n).into()),
        RuleValue::Integer(i) => Ok((*i).into()),
        RuleValue::Bool(b) => Ok((if *b { 1i64 } else { 0i64 }).into()),
        RuleValue::Range(min, _) => Ok((*min).into()),
    }
}

fn rule_value_to_string(v: &RuleValue) -> Result<String> {
    match v {
        RuleValue::Text(s) => Ok(s.clone()),
        RuleValue::Number(n) => Ok(n.to_string()),
        RuleValue::Integer(i) => Ok(i.to_string()),
        RuleValue::Bool(b) => Ok(b.to_string()),
        RuleValue::Range(min, max) => Ok(format!("{}..{}", min, max)),
    }
}

fn parse_duration_ms(v: &RuleValue) -> Result<i64> {
    match v {
        RuleValue::Integer(days) => Ok(days * 86_400_000),
        RuleValue::Number(days) => Ok((days * 86_400_000.0) as i64),
        RuleValue::Text(s) => {
            let s = s.trim().to_lowercase();
            if let Some(stripped) = s.strip_suffix('d') {
                let d: f64 = stripped.parse().map_err(|_| anyhow!("Invalid days: {}", s))?;
                Ok((d * 86_400_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('h') {
                let h: f64 = stripped.parse().map_err(|_| anyhow!("Invalid hours: {}", s))?;
                Ok((h * 3_600_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('m') {
                let m: f64 = stripped.parse().map_err(|_| anyhow!("Invalid minutes: {}", s))?;
                Ok((m * 60_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('y') {
                let y: f64 = stripped.parse().map_err(|_| anyhow!("Invalid years: {}", s))?;
                Ok((y * 365.0 * 86_400_000.0) as i64)
            } else if let Ok(d) = s.parse::<f64>() {
                Ok((d * 86_400_000.0) as i64)
            } else {
                Err(anyhow!("Cannot parse duration: {}", s))
            }
        }
        _ => Err(anyhow!("Unsupported value type for duration")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_albums_sql_generation() {
        for album in SystemAlbum::all() {
            let (query_sql, params) = album.synthesize_query(50, 0);
            assert!(query_sql.contains("LIMIT ?1 OFFSET ?2"));
            assert_eq!(params.len(), 2);

            let (count_sql, count_params) = album.synthesize_count_query();
            assert!(count_sql.contains("SELECT COUNT(*)"));
            assert_eq!(count_params.len(), 0);
        }
    }

    #[test]
    fn test_smart_album_rule_compilation() {
        let rules = vec![
            SmartAlbumRule {
                field: RuleField::IsFavorite,
                operator: RuleOperator::Equals,
                value: RuleValue::Bool(true),
            },
            SmartAlbumRule {
                field: RuleField::CameraMake,
                operator: RuleOperator::Contains,
                value: RuleValue::Text("Sony".to_string()),
            },
            SmartAlbumRule {
                field: RuleField::Iso,
                operator: RuleOperator::GreaterThan,
                value: RuleValue::Integer(800),
            },
        ];

        let album = SmartAlbum::new(
            "High ISO Sony Favorites".to_string(),
            "heart".to_string(),
            MatchMode::All,
            rules,
        );

        let (sql, params) = album.synthesize_query(20, 0).unwrap();
        assert!(sql.contains("m.is_favorite = ?"));
        assert!(sql.contains("m.make LIKE ?"));
        assert!(sql.contains("m.iso > ?"));
        assert_eq!(params.len(), 5); // 3 rule params + limit + offset
    }

    #[test]
    fn test_smart_album_in_the_last_operator() {
        let rule = SmartAlbumRule {
            field: RuleField::Date,
            operator: RuleOperator::InTheLast,
            value: RuleValue::Text("30d".to_string()),
        };

        let album = SmartAlbum::new(
            "Recent 30 Days".to_string(),
            "clock".to_string(),
            MatchMode::All,
            vec![rule],
        );

        let (where_clause, params) = album.build_sql().unwrap();
        assert!(where_clause.contains("m.timestamp_epoch_ms >= ?"));
        assert_eq!(params.len(), 1);
    }
}
