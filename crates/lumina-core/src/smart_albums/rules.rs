use crate::search::SqlParts;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::catalog::keywords::{filter_value, subtree_clause, subtree_params};

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
    /// Camera make or model: contains / does not contain text, or is / is
    /// not one `[make, model]` pair.
    Camera,
    /// A saved search: the value is the raw search text, compiled through
    /// [`crate::StructuredQuery`] so the album and the search cannot drift.
    Query,
    /// Culling flag (is / is not): the value is the [`crate::Flag`] number,
    /// 1 pick, -1 reject, 0 unflagged.
    Flag,
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
    /// The item has at least one of the keywords in a [`RuleValue::List`]
    /// (or a keyword below one). Tag only.
    AnyOf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleValue {
    Text(String),
    Number(f64),
    Integer(i64),
    Bool(bool),
    Range(f64, f64),
    /// Several keywords ([`RuleOperator::AnyOf`]). Last, so a stored numeric
    /// `[min, max]` still reads as a `Range`; old rules never hold strings in
    /// an array, so they cannot read as a list.
    List(Vec<String>),
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
    /// Shown in the sidebar's pinned group. Not part of the rules.
    #[serde(default)]
    pub pinned: bool,
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
            created_at_epoch_ms: crate::now_ms(),
            pinned: false,
        }
    }

    /// Compiles the rules into a parameterized SQL WHERE clause and bindings,
    /// for SQL with an ORDER BY (see [`Self::synthesize_count_query`]).
    pub fn build_sql(&self) -> Result<SqlParts> {
        self.where_sql(true)
    }

    /// `sorted` only changes how a `Query` rule's FTS match is written.
    pub(crate) fn where_sql(&self, sorted: bool) -> Result<SqlParts> {
        if self.rules.is_empty() {
            return Ok((
                "m.is_trashed = 0 AND m.is_hidden = 0".to_string(),
                Vec::new(),
            ));
        }

        // Each keyword binds four values; SQLite allows 32,766 per statement.
        // A camera binds one value per stored spelling, a handful, uncounted.
        let keywords: usize = self
            .rules
            .iter()
            .map(|rule| match &rule.value {
                RuleValue::List(items) if rule.field == RuleField::Tag => items.len(),
                _ => 0,
            })
            .sum();
        if keywords > MAX_KEYWORDS {
            return Err(anyhow!("Too many keywords in this album"));
        }

        let mut clauses = Vec::new();
        let mut params = Vec::new();

        for rule in &self.rules {
            let (clause, rule_params) = compile_rule(rule, sorted)?;
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

    pub fn synthesize_query_sorted(
        &self,
        limit: usize,
        offset: usize,
        sort: crate::SortSpec,
    ) -> Result<SqlParts> {
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

    pub fn synthesize_count_query(&self) -> Result<SqlParts> {
        let (where_clause, params) = self.where_sql(false)?;

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

/// The format version a smart album rules file carries.
const PORTABLE_VERSION: u32 = 1;
/// The largest rules file an import reads, in bytes.
pub const MAX_PORTABLE_BYTES: usize = 1024 * 1024;
/// The most rules an imported smart album may have.
pub const MAX_PORTABLE_RULES: usize = 100;
/// The longest text value an imported rule may have, in characters.
pub const MAX_PORTABLE_TEXT_CHARS: usize = 1_000;
/// The most keywords an imported "include any of" rule may list.
const MAX_PORTABLE_LIST: usize = 100;
/// The most "include any of" keywords one album may list across its rules,
/// well under SQLite's 32,766 bound values (four per keyword).
pub const MAX_KEYWORDS: usize = 1_000;

/// A smart album as a rules file holds it: no catalog id, date or pin,
/// which belong to one library.
#[derive(Debug, Clone, PartialEq)]
pub struct PortableSmartAlbum {
    pub title: String,
    pub icon_name: String,
    pub match_mode: MatchMode,
    pub rules: Vec<SmartAlbumRule>,
}

/// The file layout. Unknown keys are refused at both levels, so a file from
/// a newer build never imports as a silently narrower album; the stored
/// `rules_json` keeps reading [`SmartAlbumRule`] as before.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableFile {
    lumina_smart_album: u32,
    title: String,
    icon_name: String,
    match_mode: MatchMode,
    rules: Vec<PortableRule>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableRule {
    field: RuleField,
    operator: RuleOperator,
    value: RuleValue,
}

/// Export Smart Album Rules…: the album's title, icon, match mode and
/// rules as pretty JSON.
pub fn to_portable_json(album: &SmartAlbum) -> Result<String> {
    let file = PortableFile {
        lumina_smart_album: PORTABLE_VERSION,
        title: album.title.clone(),
        icon_name: album.icon_name.clone(),
        match_mode: album.match_mode,
        rules: album
            .rules
            .iter()
            .map(|r| PortableRule {
                field: r.field.clone(),
                operator: r.operator.clone(),
                value: r.value.clone(),
            })
            .collect(),
    };
    Ok(serde_json::to_string_pretty(&file)?)
}

/// Import Smart Album Rules…: parses a rules file and checks that its rules
/// compile, the same check the builder makes before it saves. A file from
/// a newer build (another version, a setting or a rule field this build
/// does not know) is an error, never a silently narrower album. Sizes are
/// bounded, since the file can come from anywhere.
pub fn from_portable_json(json: &str) -> Result<PortableSmartAlbum> {
    if json.len() > MAX_PORTABLE_BYTES {
        return Err(anyhow!(
            "This file is too large to be a smart album rules file"
        ));
    }
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| anyhow!("This is not a Lumina smart album file"))?;
    match value.get("lumina_smart_album").and_then(|v| v.as_u64()) {
        None => return Err(anyhow!("This is not a Lumina smart album file")),
        Some(v) if v > PORTABLE_VERSION as u64 => {
            return Err(anyhow!(
                "This file was made by a newer version of Lumina; update Lumina to import it"
            ))
        }
        Some(v) if v != PORTABLE_VERSION as u64 => {
            return Err(anyhow!(
                "This smart album file has an unknown version ({v})"
            ))
        }
        Some(_) => {}
    }
    let file: PortableFile = serde_json::from_value(value).map_err(|err| {
        log::warn!("smart album rules file refused: {err}");
        anyhow!("This file has a rule or setting this version of Lumina does not know")
    })?;
    if file.rules.len() > MAX_PORTABLE_RULES {
        return Err(anyhow!(
            "This file has more than {MAX_PORTABLE_RULES} rules, too many to import"
        ));
    }
    let long = |text: &String| text.chars().count() > MAX_PORTABLE_TEXT_CHARS;
    let too_long = |v: &RuleValue| match v {
        RuleValue::Text(text) => long(text),
        RuleValue::List(items) => items.len() > MAX_PORTABLE_LIST || items.iter().any(long),
        _ => false,
    };
    if file.rules.iter().any(|r| too_long(&r.value)) {
        return Err(anyhow!("A rule in this file is too long to import"));
    }
    let portable = PortableSmartAlbum {
        title: file.title,
        icon_name: file.icon_name,
        match_mode: file.match_mode,
        rules: file
            .rules
            .into_iter()
            .map(|r| SmartAlbumRule {
                field: r.field,
                operator: r.operator,
                value: r.value,
            })
            .collect(),
    };
    SmartAlbum::new(
        String::new(),
        String::new(),
        portable.match_mode,
        portable.rules.clone(),
    )
    .build_sql()?;
    Ok(portable)
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
        RuleField::Place | RuleField::Camera => {
            unreachable!("multi-column fields are compiled in compile_rule")
        }
        RuleField::Query => unreachable!("RuleField::Query rules are compiled in compile_rule"),
        RuleField::Flag => unreachable!("RuleField::Flag rules are compiled in compile_rule"),
    }
}

fn compile_rule(rule: &SmartAlbumRule, sorted: bool) -> Result<SqlParts> {
    if rule.field == RuleField::Query {
        return compile_query_rule(rule, sorted);
    }
    if rule.field == RuleField::Flag {
        return compile_flag_rule(rule);
    }
    if rule.field == RuleField::Tag {
        return match &rule.operator {
            // Equals / NotEquals cover the keyword and its descendants.
            RuleOperator::Equals | RuleOperator::NotEquals => {
                let clean = filter_value(&rule_value_to_string(&rule.value)?);
                let not = if rule.operator == RuleOperator::NotEquals {
                    "NOT "
                } else {
                    ""
                };
                Ok((
                    format!(
                        "m.id {not}IN (SELECT media_id FROM media_tags WHERE {})",
                        subtree_clause("tag")
                    ),
                    subtree_params(&clean).into(),
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
            RuleOperator::AnyOf => compile_tag_any_of(&rule.value),
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

    if rule.field == RuleField::Camera
        && matches!(
            rule.operator,
            RuleOperator::Equals | RuleOperator::NotEquals
        )
    {
        return compile_camera_is(rule);
    }

    let columns: &[&str] = match rule.field {
        RuleField::Place => &["m.city", "m.admin1_region", "m.country_code"],
        RuleField::Camera => &["m.make", "m.model"],
        _ => &[],
    };
    if !columns.is_empty() {
        let negate = match rule.operator {
            RuleOperator::Contains => false,
            RuleOperator::NotContains => true,
            _ => return Err(anyhow!("this field supports contains / does not contain")),
        };
        let pattern = like_pattern(&rule_value_to_string(&rule.value)?);
        let any = columns
            .iter()
            .map(|col| format!("{col} LIKE ? ESCAPE '\\'"))
            .collect::<Vec<_>>()
            .join(" OR ");
        // NULL columns make the OR NULL; COALESCE keeps "does not contain"
        // true for items with no value at all.
        let clause = if negate {
            format!("NOT COALESCE(({any}), 0)")
        } else {
            format!("({any})")
        };
        let params = columns.iter().map(|_| pattern.clone().into()).collect();
        return Ok((clause, params));
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
        RuleOperator::AnyOf => Err(anyhow!("Only keywords support \"include any of\"")),
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
            let cutoff = crate::now_ms() - ms_delta;
            Ok((format!("{} >= ?", col), vec![cutoff.into()]))
        }
        RuleOperator::IsEmpty => Ok((format!("({} IS NULL OR {} = '')", col, col), Vec::new())),
        RuleOperator::IsNotEmpty => Ok((
            format!("({} IS NOT NULL AND {} != '')", col, col),
            Vec::new(),
        )),
    }
}

/// Tag "include any of": one `IN` over media_tags with a subtree test per
/// keyword, so each OR arm still seeks the tag index (see `subtree_clause`).
fn compile_tag_any_of(value: &RuleValue) -> Result<SqlParts> {
    let RuleValue::List(tags) = value else {
        return Err(anyhow!("\"include any of\" needs a list of keywords"));
    };
    let mut paths: Vec<String> = tags
        .iter()
        .map(|tag| filter_value(tag))
        .filter(|path| !path.is_empty())
        .collect();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err(anyhow!("Choose at least one keyword"));
    }
    let any = vec![subtree_clause("tag"); paths.len()].join(" OR ");
    let params = paths.iter().flat_map(|path| subtree_params(path)).collect();
    Ok((
        format!("m.id IN (SELECT media_id FROM media_tags WHERE {any})"),
        params,
    ))
}

/// Camera "is" / "is not": the value is one or more `[make, model]` pairs as
/// stored (see `Catalog::distinct_cameras`), "" for a missing part. The
/// builder lists spellings of one camera ("Canon" / "canon") under one name
/// and stores every pair it grouped. Each part is compared exactly, case
/// included, so every pair's binary `=` lets the count seek `idx_media_camera`.
fn compile_camera_is(rule: &SmartAlbumRule) -> Result<SqlParts> {
    let RuleValue::List(parts) = &rule.value else {
        return Err(anyhow!("Choose a camera"));
    };
    if parts.is_empty() || !parts.len().is_multiple_of(2) {
        return Err(anyhow!("Choose a camera"));
    }
    let mut params = Vec::new();
    let mut part = |col: &str, value: &str| {
        if value.is_empty() {
            format!("({col} IS NULL OR {col} = '')")
        } else {
            params.push(value.to_string().into());
            format!("{col} = ?")
        }
    };
    let mut pairs = Vec::new();
    for [make, model] in parts.as_chunks::<2>().0 {
        if make.is_empty() && model.is_empty() {
            return Err(anyhow!("Choose a camera"));
        }
        pairs.push(format!(
            "({} AND {})",
            part("m.make", make),
            part("m.model", model)
        ));
    }
    let any = pairs.join(" OR ");
    // A NULL part makes the AND NULL; COALESCE keeps "is not" true for it.
    let clause = if rule.operator == RuleOperator::NotEquals {
        format!("NOT COALESCE(({any}), 0)")
    } else {
        format!("({any})")
    };
    Ok((clause, params))
}

/// A saved search: the raw text parsed exactly as the search field parses it.
/// Refuses what a smart album cannot show or what the parser did not
/// understand, so a saved search never silently shows more than was typed.
fn compile_query_rule(rule: &SmartAlbumRule, sorted: bool) -> Result<SqlParts> {
    let (RuleOperator::Equals, RuleValue::Text(text)) = (&rule.operator, &rule.value) else {
        return Err(anyhow!(
            "A saved search supports only \"matches\" with search text"
        ));
    };
    let query = crate::StructuredQuery::parse(text);
    if query.is_trashed == Some(true) {
        return Err(anyhow!("A smart album cannot show deleted items"));
    }
    if !query.unrecognized.is_empty() {
        return Err(anyhow!("Not understood: {}", query.unrecognized.join(", ")));
    }
    if query == crate::StructuredQuery::default() {
        return Err(anyhow!("A saved search needs some search text"));
    }
    // Parenthesized so the album's AND/OR joiner cannot split the clause.
    let (clause, params) = query.build_where_subquery(sorted);
    Ok((format!("({clause})"), params))
}

/// `m.flag = ?` (never wrapped in a function or `IS NULL`), so the
/// partial `idx_media_flag` can serve an "is" rule.
fn compile_flag_rule(rule: &SmartAlbumRule) -> Result<SqlParts> {
    // Stored rules_json reads a whole number back as `Number`.
    let flag = match rule.value {
        RuleValue::Integer(v @ -1..=1) => v,
        RuleValue::Number(n) if [-1.0, 0.0, 1.0].contains(&n) => n as i64,
        _ => return Err(anyhow!("Flag is pick, reject or unflagged")),
    };
    let clause = match rule.operator {
        RuleOperator::Equals => "m.flag = ?",
        RuleOperator::NotEquals => "m.flag != ?",
        _ => return Err(anyhow!("Flag supports is / is not")),
    };
    Ok((clause.to_string(), vec![flag.into()]))
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
/// "in the range" cover whole days. Returns `None` for numeric epoch values
/// and "in the last", which compile generically; other operators are errors.
fn compile_calendar_date(rule: &SmartAlbumRule) -> Result<Option<SqlParts>> {
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
        RuleOperator::InTheLast => return Ok(None),
        // Comparing epoch ms against text would silently match everything.
        _ => return Err(anyhow!("Date does not support this operator with a date")),
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
        RuleValue::List(_) => Err(anyhow!("This rule takes one value, not a list")),
    }
}

fn rule_value_to_string(v: &RuleValue) -> Result<String> {
    match v {
        RuleValue::Text(s) => Ok(s.clone()),
        RuleValue::Number(n) => Ok(n.to_string()),
        RuleValue::Integer(i) => Ok(i.to_string()),
        RuleValue::Bool(b) => Ok(b.to_string()),
        RuleValue::Range(min, max) => Ok(format!("{}..{}", min, max)),
        RuleValue::List(_) => Err(anyhow!("This rule takes one value, not a list")),
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
