use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    All,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleField {
    Tag,
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
    Rating,
    ColorLabel,
    /// City, region or country code (contains / does not contain).
    Place,
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
    AtLeast,
    AtMost,
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
    pub fn new(
        title: String,
        icon_name: String,
        match_mode: MatchMode,
        rules: Vec<SmartAlbumRule>,
    ) -> Self {
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
            return Ok((
                "m.is_trashed = 0 AND m.is_hidden = 0".to_string(),
                Vec::new(),
            ));
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

        let combined = format!(
            "({}) AND m.is_trashed = 0 AND m.is_hidden = 0",
            clauses.join(joiner)
        );
        Ok((combined, params))
    }

    pub fn synthesize_query(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<(String, Vec<rusqlite::types::Value>)> {
        self.synthesize_query_sorted(limit, offset, crate::SortSpec::default())
    }

    pub fn synthesize_query_sorted(
        &self,
        limit: usize,
        offset: usize,
        sort: crate::SortSpec,
    ) -> Result<(String, Vec<rusqlite::types::Value>)> {
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
                m.is_favorite, m.is_hidden, m.is_trashed, m.title
            FROM media_items m
            WHERE {}
            ORDER BY {}
            LIMIT ? OFFSET ?;
            "#,
            where_clause,
            sort.order_by("m.")
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
        RuleField::Tag => unreachable!(
            "RuleField::Tag rules are compiled via relational subqueries in compile_rule"
        ),
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
        RuleField::Rating => "m.rating",
        RuleField::ColorLabel => "m.color_label",
        RuleField::Place => unreachable!("RuleField::Place rules are compiled in compile_rule"),
    }
}

fn compile_rule(rule: &SmartAlbumRule) -> Result<(String, Vec<rusqlite::types::Value>)> {
    if rule.field == RuleField::Tag {
        return match &rule.operator {
            RuleOperator::Equals => {
                let s = rule_value_to_string(&rule.value)?;
                let clean = s.trim().to_lowercase();
                Ok((
                    "m.id IN (SELECT media_id FROM media_tags WHERE tag = ?)".to_string(),
                    vec![clean.into()],
                ))
            }
            RuleOperator::NotEquals => {
                let s = rule_value_to_string(&rule.value)?;
                let clean = s.trim().to_lowercase();
                Ok((
                    "m.id NOT IN (SELECT media_id FROM media_tags WHERE tag = ?)".to_string(),
                    vec![clean.into()],
                ))
            }
            RuleOperator::Contains => {
                let s = rule_value_to_string(&rule.value)?;
                let clean = s.trim().to_lowercase();
                let escaped = clean
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                Ok((
                    "m.id IN (SELECT media_id FROM media_tags WHERE tag LIKE ? ESCAPE '\\')"
                        .to_string(),
                    vec![format!("%{}%", escaped).into()],
                ))
            }
            RuleOperator::NotContains => {
                let s = rule_value_to_string(&rule.value)?;
                let clean = s.trim().to_lowercase();
                let escaped = clean
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                Ok((
                    "m.id NOT IN (SELECT media_id FROM media_tags WHERE tag LIKE ? ESCAPE '\\')"
                        .to_string(),
                    vec![format!("%{}%", escaped).into()],
                ))
            }
            RuleOperator::IsEmpty => Ok((
                "m.id NOT IN (SELECT media_id FROM media_tags)".to_string(),
                Vec::new(),
            )),
            RuleOperator::IsNotEmpty => Ok((
                "m.id IN (SELECT media_id FROM media_tags)".to_string(),
                Vec::new(),
            )),
            _ => Err(anyhow!("Unsupported operator for Tag field")),
        };
    }

    if rule.field == RuleField::Place {
        let negate = match rule.operator {
            RuleOperator::Contains => false,
            RuleOperator::NotContains => true,
            _ => return Err(anyhow!("Place supports contains / does not contain")),
        };
        let pattern = like_pattern(&rule_value_to_string(&rule.value)?);
        let any = "(m.city LIKE ? ESCAPE '\\' OR m.admin1_region LIKE ? ESCAPE '\\' \
                   OR m.country_code LIKE ? ESCAPE '\\')";
        let clause = if negate {
            format!("NOT COALESCE({any}, 0)")
        } else {
            any.to_string()
        };
        return Ok((
            clause,
            vec![
                pattern.clone().into(),
                pattern.clone().into(),
                pattern.into(),
            ],
        ));
    }

    // "Photo" is every still: plain images and RAW files.
    if rule.field == RuleField::MediaType && rule.value == RuleValue::Text("photo".into()) {
        return match rule.operator {
            RuleOperator::Equals => Ok(("m.media_type IN ('image', 'raw')".into(), Vec::new())),
            RuleOperator::NotEquals => {
                Ok(("m.media_type NOT IN ('image', 'raw')".into(), Vec::new()))
            }
            _ => Err(anyhow!("Kind supports is / is not")),
        };
    }

    if rule.field == RuleField::Date {
        if let Some(compiled) = compile_calendar_date(rule)? {
            return Ok(compiled);
        }
    }

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
            let escaped = s
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            Ok((
                format!("{} LIKE ? ESCAPE '\\'", col),
                vec![format!("%{}%", escaped).into()],
            ))
        }
        RuleOperator::NotContains => {
            let s = rule_value_to_string(&rule.value)?;
            let escaped = s
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_");
            Ok((
                format!("({} IS NULL OR {} NOT LIKE ? ESCAPE '\\')", col, col),
                vec![format!("%{}%", escaped).into()],
            ))
        }
        RuleOperator::GreaterThan => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} > ?", col), vec![val]))
        }
        RuleOperator::LessThan => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} < ?", col), vec![val]))
        }
        RuleOperator::AtLeast => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} >= ?", col), vec![val]))
        }
        RuleOperator::AtMost => {
            let val = rule_value_to_sqlite(&rule.value)?;
            Ok((format!("{} <= ?", col), vec![val]))
        }
        RuleOperator::Between => match &rule.value {
            RuleValue::Range(min, max) => Ok((
                format!("{} >= ? AND {} <= ?", col, col),
                vec![(*min).into(), (*max).into()],
            )),
            RuleValue::Text(s) => {
                if let Some((min_s, max_s)) = s.split_once("..") {
                    let min: f64 = min_s
                        .trim()
                        .parse()
                        .map_err(|_| anyhow!("Invalid range min: {}", min_s))?;
                    let max: f64 = max_s
                        .trim()
                        .parse()
                        .map_err(|_| anyhow!("Invalid range max: {}", max_s))?;
                    Ok((
                        format!("{} >= ? AND {} <= ?", col, col),
                        vec![min.into(), max.into()],
                    ))
                } else {
                    Err(anyhow!("Invalid range format for Between: {}", s))
                }
            }
            _ => Err(anyhow!(
                "Between operator requires Range or 'min..max' string"
            )),
        },
        RuleOperator::InTheLast => {
            // E.g. "30d", "7d", "1y", or integer days
            let ms_delta = parse_duration_ms(&rule.value)?;
            let cutoff = Utc::now().timestamp_millis() - ms_delta;
            Ok((format!("{} >= ?", col), vec![cutoff.into()]))
        }
        RuleOperator::IsEmpty => Ok((format!("({} IS NULL OR {} = '')", col, col), Vec::new())),
        RuleOperator::IsNotEmpty => Ok((
            format!("({} IS NOT NULL AND {} != '')", col, col),
            Vec::new(),
        )),
    }
}

const DAY_MS: i64 = 86_400_000;

/// Parses `YYYY-MM-DD` to the epoch ms of that day's start. Capture times are
/// stored as naive local time in UTC (see the scanner), so UTC midnight is the
/// day boundary that matches the timeline's year/month/day columns.
pub fn parse_rule_date(s: &str) -> Option<i64> {
    let date = chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()?;
    Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
}

/// Splits a date range written as `a..b` or `a to b`.
pub fn split_date_range(s: &str) -> Option<(i64, i64)> {
    let (a, b) = s.split_once("..").or_else(|| s.split_once(" to "))?;
    let (a, b) = (parse_rule_date(a)?, parse_rule_date(b)?);
    Some((a.min(b), a.max(b)))
}

/// Date rules with calendar-day text values: "is", "before", "after" and
/// "in the range" cover whole days. Returns `None` for other date rules
/// (numeric epoch values, "in the last"), which compile generically.
fn compile_calendar_date(
    rule: &SmartAlbumRule,
) -> Result<Option<(String, Vec<rusqlite::types::Value>)>> {
    let RuleValue::Text(text) = &rule.value else {
        return Ok(None);
    };
    let col = "m.timestamp_epoch_ms";
    let day = || parse_rule_date(text).ok_or_else(|| anyhow!("Invalid date: {text}"));
    Ok(Some(match rule.operator {
        RuleOperator::Equals => {
            let start = day()?;
            (
                format!("{col} >= ? AND {col} < ?"),
                vec![start.into(), (start + DAY_MS).into()],
            )
        }
        RuleOperator::LessThan => (format!("{col} < ?"), vec![day()?.into()]),
        RuleOperator::GreaterThan => (format!("{col} >= ?"), vec![(day()? + DAY_MS).into()]),
        RuleOperator::Between => {
            let (start, end) =
                split_date_range(text).ok_or_else(|| anyhow!("Invalid date range: {text}"))?;
            (
                format!("{col} >= ? AND {col} < ?"),
                vec![start.into(), (end + DAY_MS).into()],
            )
        }
        _ => return Ok(None),
    }))
}

fn like_pattern(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{}%", escaped)
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
                let d: f64 = stripped
                    .parse()
                    .map_err(|_| anyhow!("Invalid days: {}", s))?;
                Ok((d * 86_400_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('h') {
                let h: f64 = stripped
                    .parse()
                    .map_err(|_| anyhow!("Invalid hours: {}", s))?;
                Ok((h * 3_600_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('w') {
                let w: f64 = stripped
                    .parse()
                    .map_err(|_| anyhow!("Invalid weeks: {}", s))?;
                Ok((w * 7.0 * 86_400_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix("mo") {
                // ponytail: a month is 30 days; calendar months if anyone notices.
                let mo: f64 = stripped
                    .parse()
                    .map_err(|_| anyhow!("Invalid months: {}", s))?;
                Ok((mo * 30.0 * 86_400_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('m') {
                let m: f64 = stripped
                    .parse()
                    .map_err(|_| anyhow!("Invalid minutes: {}", s))?;
                Ok((m * 60_000.0) as i64)
            } else if let Some(stripped) = s.strip_suffix('y') {
                let y: f64 = stripped
                    .parse()
                    .map_err(|_| anyhow!("Invalid years: {}", s))?;
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
