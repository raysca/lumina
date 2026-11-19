use crate::search::SqlParts;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::rules_sql::compile_rule;

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
            SELECT {columns}
            FROM media_items m
            WHERE {where_clause}
            ORDER BY {order}
            LIMIT ? OFFSET ?;
            "#,
            columns = crate::sql::media_columns("m."),
            where_clause = &where_clause,
            order = sort.order_by("m.")
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
