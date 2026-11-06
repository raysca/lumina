//! Smart album builder model: one editable row per condition.
//!
//! Rows keep the value as typed text and convert to core rules on demand. A
//! row whose value is missing or malformed converts to `None`, which disables
//! OK and outlines that row. No GPUI here, so the rules are unit-tested.

use lumina_core::{
    parse_rule_date, split_date_range, MatchMode, RuleField, RuleOperator, RuleValue,
    SmartAlbumRule,
};

/// Fields offered in the field popup, in Photos-like order. Older albums may
/// hold other fields (Year, City…); those still display, convert and save.
pub const FIELDS: &[RuleField] = &[
    RuleField::MediaType,
    RuleField::IsFavorite,
    RuleField::Date,
    RuleField::Tag,
    RuleField::Rating,
    RuleField::ColorLabel,
    RuleField::Camera,
    RuleField::Lens,
    RuleField::FNumber,
    RuleField::Iso,
    RuleField::FocalLength,
    RuleField::Filename,
    RuleField::Place,
];

const KINDS: &[(&str, &str)] = &[("photo", "Photo"), ("video", "Video")];
/// Kind values saved by the old builder: still valid, shown, never offered.
const LEGACY_KINDS: &[(&str, &str)] = &[("image", "Photo (non-RAW)"), ("raw", "RAW")];
const YES_NO: &[(&str, &str)] = &[("true", "Yes"), ("false", "No")];
const STARS: &[(&str, &str)] = &[
    ("1", "★"),
    ("2", "★★"),
    ("3", "★★★"),
    ("4", "★★★★"),
    ("5", "★★★★★"),
];
/// Same order and numbering as the inspector's label dots.
pub const LABELS: &[(&str, &str)] = &[
    ("1", "Red"),
    ("2", "Orange"),
    ("3", "Yellow"),
    ("4", "Green"),
    ("5", "Blue"),
    ("6", "Purple"),
];

/// How a row's value is entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// The operator needs no value ("is empty").
    None,
    Choice(&'static [(&'static str, &'static str)]),
    /// One of the library's tags, picked from a popup.
    Tags,
    Text,
    Number,
    /// `YYYY-MM-DD`
    Date,
    /// `YYYY-MM-DD to YYYY-MM-DD`
    DateRange,
    /// A count of days, weeks or months ("in the last").
    Period,
}

impl ValueKind {
    /// Typed into the control rather than picked from a popup.
    pub fn is_typed(self) -> bool {
        matches!(
            self,
            Self::Text | Self::Number | Self::Date | Self::DateRange | Self::Period
        )
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Date => "YYYY-MM-DD",
            Self::DateRange => "YYYY-MM-DD to YYYY-MM-DD",
            Self::Number | Self::Period => "number",
            Self::Tags => "choose a tag",
            _ => "text",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodUnit {
    Days,
    Weeks,
    Months,
}

pub const UNITS: &[(PeriodUnit, &str)] = &[
    (PeriodUnit::Days, "days"),
    (PeriodUnit::Weeks, "weeks"),
    (PeriodUnit::Months, "months"),
];

impl PeriodUnit {
    fn suffix(self) -> &'static str {
        match self {
            Self::Days => "d",
            Self::Weeks => "w",
            Self::Months => "mo",
        }
    }

    pub fn label(self) -> &'static str {
        UNITS
            .iter()
            .find(|(u, _)| *u == self)
            .map_or("days", |u| u.1)
    }
}

pub fn field_label(field: &RuleField) -> &'static str {
    match field {
        RuleField::MediaType => "Kind",
        RuleField::IsFavorite => "Favorite",
        RuleField::Date => "Date",
        RuleField::Tag => "Tag",
        RuleField::Rating => "Stars",
        RuleField::ColorLabel => "Color label",
        RuleField::Camera => "Camera",
        RuleField::CameraModel => "Camera model",
        RuleField::Lens => "Lens",
        RuleField::FNumber => "Aperture",
        RuleField::Iso => "ISO",
        RuleField::FocalLength => "Focal length",
        RuleField::Filename => "Filename",
        RuleField::Place => "Place",
        RuleField::CameraMake => "Camera make",
        RuleField::Year => "Year",
        RuleField::Month => "Month",
        RuleField::City => "City",
        RuleField::CountryCode => "Country code",
        RuleField::FileSize => "File size (bytes)",
        RuleField::DurationMs => "Duration (ms)",
        RuleField::Query => "Search",
    }
}

fn is_numeric(field: &RuleField) -> bool {
    matches!(
        field,
        RuleField::FNumber
            | RuleField::Iso
            | RuleField::FocalLength
            | RuleField::Year
            | RuleField::Month
            | RuleField::FileSize
            | RuleField::DurationMs
    )
}

/// Operators offered for a field, in popup order.
pub fn operators(field: &RuleField) -> &'static [RuleOperator] {
    use RuleOperator::*;
    match field {
        RuleField::MediaType | RuleField::ColorLabel => &[Equals, NotEquals],
        RuleField::IsFavorite | RuleField::Query => &[Equals],
        RuleField::Date => &[Equals, LessThan, GreaterThan, Between, InTheLast],
        RuleField::Tag => &[Equals, NotEquals, Contains, IsEmpty, IsNotEmpty],
        RuleField::Rating => &[Equals, AtLeast, AtMost],
        f if is_numeric(f) => &[Equals, GreaterThan, LessThan],
        _ => &[Contains, NotContains],
    }
}

pub fn operator_label(field: &RuleField, op: &RuleOperator) -> &'static str {
    let date = *field == RuleField::Date;
    match op {
        // Matches the compile error's wording for a saved search.
        RuleOperator::Equals if *field == RuleField::Query => "matches",
        RuleOperator::Equals => "is",
        RuleOperator::NotEquals => "is not",
        RuleOperator::Contains => "contains",
        RuleOperator::NotContains => "does not contain",
        RuleOperator::GreaterThan if date => "is after",
        RuleOperator::GreaterThan => "is greater than",
        RuleOperator::LessThan if date => "is before",
        RuleOperator::LessThan => "is less than",
        RuleOperator::Between => "is in the range",
        RuleOperator::InTheLast => "is in the last",
        RuleOperator::IsEmpty => "is empty",
        RuleOperator::IsNotEmpty => "is not empty",
        RuleOperator::AtLeast => "is at least",
        RuleOperator::AtMost => "is at most",
    }
}

pub fn value_kind(field: &RuleField, op: &RuleOperator) -> ValueKind {
    match (field, op) {
        (_, RuleOperator::IsEmpty | RuleOperator::IsNotEmpty) => ValueKind::None,
        (RuleField::MediaType, _) => ValueKind::Choice(KINDS),
        (RuleField::IsFavorite, _) => ValueKind::Choice(YES_NO),
        (RuleField::Rating, _) => ValueKind::Choice(STARS),
        (RuleField::ColorLabel, _) => ValueKind::Choice(LABELS),
        (RuleField::Date, RuleOperator::Between) => ValueKind::DateRange,
        (RuleField::Date, RuleOperator::InTheLast) => ValueKind::Period,
        (RuleField::Date, _) => ValueKind::Date,
        (RuleField::Tag, RuleOperator::Equals | RuleOperator::NotEquals) => ValueKind::Tags,
        (f, _) if is_numeric(f) => ValueKind::Number,
        _ => ValueKind::Text,
    }
}

/// One condition row as the user edits it.
#[derive(Debug, Clone, PartialEq)]
pub struct DraftRow {
    pub field: RuleField,
    pub operator: RuleOperator,
    /// The value as typed, or the key of the chosen option.
    pub text: String,
    pub unit: PeriodUnit,
}

impl DraftRow {
    /// A fresh row for `field` with its first operator and default value.
    pub fn new(field: RuleField) -> Self {
        let operator = operators(&field)[0].clone();
        let mut row = Self {
            field,
            operator,
            text: String::new(),
            unit: PeriodUnit::Days,
        };
        row.text = row.default_text();
        row
    }

    pub fn kind(&self) -> ValueKind {
        value_kind(&self.field, &self.operator)
    }

    fn default_text(&self) -> String {
        match self.kind() {
            ValueKind::Choice(options) => options[0].0.to_string(),
            ValueKind::Date => chrono::Local::now().format("%Y-%m-%d").to_string(),
            ValueKind::Period => "30".to_string(),
            _ => String::new(),
        }
    }

    /// Changes the operator; the value resets when its kind changes.
    pub fn set_operator(&mut self, operator: RuleOperator) {
        let before = self.kind();
        self.operator = operator;
        let after = self.kind();
        let keeps_text = before == after
            || (matches!(before, ValueKind::Text | ValueKind::Tags)
                && matches!(after, ValueKind::Text | ValueKind::Tags));
        if !keeps_text {
            self.text = self.default_text();
        }
    }

    /// Text shown in the value control.
    pub fn display_value(&self) -> String {
        match self.kind() {
            ValueKind::Choice(options) => options
                .iter()
                .chain(LEGACY_KINDS)
                .find(|(key, _)| *key == self.text)
                .map_or_else(|| self.text.clone(), |(_, label)| label.to_string()),
            _ => self.text.clone(),
        }
    }

    /// The core rule, or `None` while the value is missing or malformed.
    pub fn to_rule(&self) -> Option<SmartAlbumRule> {
        let text = self.text.trim();
        let value = match self.kind() {
            ValueKind::None => RuleValue::Text(String::new()),
            ValueKind::Choice(options) => {
                let legacy = self.field == RuleField::MediaType
                    && LEGACY_KINDS.iter().any(|(k, _)| *k == text);
                if !legacy {
                    options.iter().find(|(key, _)| *key == text)?;
                }
                match self.field {
                    RuleField::IsFavorite => RuleValue::Bool(text == "true"),
                    RuleField::MediaType => RuleValue::Text(text.to_string()),
                    _ => RuleValue::Integer(text.parse().ok()?),
                }
            }
            ValueKind::Tags | ValueKind::Text if text.is_empty() => return None,
            ValueKind::Tags | ValueKind::Text => RuleValue::Text(text.to_string()),
            ValueKind::Number => match text.parse::<i64>() {
                Ok(n) => RuleValue::Integer(n),
                Err(_) => RuleValue::Number(text.parse::<f64>().ok().filter(|n| n.is_finite())?),
            },
            ValueKind::Date => {
                parse_rule_date(text)?;
                RuleValue::Text(text.to_string())
            }
            ValueKind::DateRange => {
                split_date_range(text)?;
                RuleValue::Text(text.to_string())
            }
            ValueKind::Period => {
                let n: u32 = text.parse().ok().filter(|n| *n > 0)?;
                RuleValue::Text(format!("{n}{}", self.unit.suffix()))
            }
        };
        Some(SmartAlbumRule {
            field: self.field.clone(),
            operator: self.operator.clone(),
            value,
        })
    }

    /// Rebuilds the editable row for a saved rule.
    pub fn from_rule(rule: &SmartAlbumRule) -> Self {
        let mut row = Self {
            field: rule.field.clone(),
            operator: rule.operator.clone(),
            text: String::new(),
            unit: PeriodUnit::Days,
        };
        row.text = match &rule.value {
            RuleValue::Text(s) if row.kind() == ValueKind::Period => {
                let s = s.trim().to_lowercase();
                let (count, unit) = if let Some(n) = s.strip_suffix("mo") {
                    (n.to_string(), PeriodUnit::Months)
                } else if let Some(n) = s.strip_suffix('w') {
                    (n.to_string(), PeriodUnit::Weeks)
                } else {
                    (s.trim_end_matches('d').to_string(), PeriodUnit::Days)
                };
                row.unit = unit;
                count
            }
            RuleValue::Text(s) => s.clone(),
            RuleValue::Integer(n) => n.to_string(),
            RuleValue::Number(n) => n.to_string(),
            RuleValue::Bool(b) => b.to_string(),
            RuleValue::Range(min, max) => format!("{min}..{max}"),
        };
        row
    }
}

/// Converts every row, or `None` when any row is invalid.
pub fn rules_from_rows(rows: &[DraftRow]) -> Option<Vec<SmartAlbumRule>> {
    rows.iter().map(DraftRow::to_rule).collect()
}

/// A keyboard-focusable control in the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftFocus {
    Title,
    Match,
    Field(usize),
    Operator(usize),
    Value(usize),
    Unit(usize),
    Remove(usize),
    Add(usize),
    Cancel,
    Save,
}

/// An open option popup and its highlighted option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftPopup {
    pub target: DraftFocus,
    pub highlight: usize,
}

/// Tab order for the current rows.
pub fn focus_order(rows: &[DraftRow]) -> Vec<DraftFocus> {
    let mut order = vec![DraftFocus::Title, DraftFocus::Match];
    for (i, row) in rows.iter().enumerate() {
        order.push(DraftFocus::Field(i));
        order.push(DraftFocus::Operator(i));
        if row.kind() != ValueKind::None {
            order.push(DraftFocus::Value(i));
        }
        if row.kind() == ValueKind::Period {
            order.push(DraftFocus::Unit(i));
        }
        order.push(DraftFocus::Remove(i));
        order.push(DraftFocus::Add(i));
    }
    // The empty state's "Add Condition" button.
    if rows.is_empty() {
        order.push(DraftFocus::Add(0));
    }
    order.extend([DraftFocus::Cancel, DraftFocus::Save]);
    order
}

pub fn match_label(mode: MatchMode) -> &'static str {
    match mode {
        MatchMode::All => "all",
        MatchMode::Any => "any",
    }
}

/// One line for an album's rules: "Kind is Video and Stars is greater than 3".
pub fn rule_summary(mode: MatchMode, rules: &[SmartAlbumRule]) -> String {
    if rules.is_empty() {
        return "No conditions yet".to_string();
    }
    let joiner = match mode {
        MatchMode::All => " and ",
        MatchMode::Any => " or ",
    };
    rules
        .iter()
        .map(|rule| {
            let row = DraftRow::from_rule(rule);
            let mut text = format!(
                "{} {}",
                field_label(&row.field),
                operator_label(&row.field, &row.operator)
            );
            let value = row.display_value();
            if !value.is_empty() {
                text.push(' ');
                text.push_str(&value);
            }
            if row.operator == RuleOperator::InTheLast {
                text.push(' ');
                text.push_str(row.unit.label());
            }
            text
        })
        .collect::<Vec<_>>()
        .join(joiner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_summary_reads_as_one_sentence() {
        let video = row(RuleField::MediaType, RuleOperator::Equals, "video")
            .to_rule()
            .unwrap();
        let stars = row(RuleField::Rating, RuleOperator::GreaterThan, "3")
            .to_rule()
            .unwrap();
        let summary = rule_summary(MatchMode::Any, &[video, stars]);
        assert_eq!(summary, "Kind is Video or Stars is greater than ★★★");
        assert_eq!(rule_summary(MatchMode::All, &[]), "No conditions yet");
    }

    #[test]
    fn saved_search_row_reads_search_matches() {
        let rule = row(RuleField::Query, RuleOperator::Equals, "sunset")
            .to_rule()
            .unwrap();
        assert_eq!(
            rule_summary(MatchMode::All, &[rule]),
            "Search matches sunset",
            "the row must say \"matches\", as the compile error does"
        );
    }

    fn row(field: RuleField, op: RuleOperator, text: &str) -> DraftRow {
        let mut row = DraftRow::new(field);
        row.set_operator(op);
        row.text = text.to_string();
        row
    }

    #[test]
    fn invalid_values_do_not_convert() {
        assert!(row(RuleField::Tag, RuleOperator::Contains, "  ")
            .to_rule()
            .is_none());
        assert!(row(RuleField::Date, RuleOperator::Equals, "2024-13-01")
            .to_rule()
            .is_none());
        assert!(row(RuleField::Date, RuleOperator::Between, "2024-01-01")
            .to_rule()
            .is_none());
        assert!(row(RuleField::Iso, RuleOperator::GreaterThan, "fast")
            .to_rule()
            .is_none());
        assert!(row(RuleField::Date, RuleOperator::InTheLast, "0")
            .to_rule()
            .is_none());
        assert!(row(RuleField::Tag, RuleOperator::IsEmpty, "")
            .to_rule()
            .is_some());
    }

    #[test]
    fn rows_round_trip_through_rules() {
        let mut period = row(RuleField::Date, RuleOperator::InTheLast, "6");
        period.unit = PeriodUnit::Months;
        let rows = [
            row(RuleField::MediaType, RuleOperator::NotEquals, "video"),
            row(RuleField::IsFavorite, RuleOperator::Equals, "false"),
            row(RuleField::Rating, RuleOperator::AtLeast, "4"),
            row(RuleField::FNumber, RuleOperator::LessThan, "2.8"),
            row(
                RuleField::Date,
                RuleOperator::Between,
                "2024-01-01 to 2024-02-01",
            ),
            period,
        ];
        let rules = rules_from_rows(&rows).expect("all rows valid");
        assert_eq!(rules[1].value, RuleValue::Bool(false));
        assert_eq!(rules[2].value, RuleValue::Integer(4));
        assert_eq!(rules[3].value, RuleValue::Number(2.8));
        assert_eq!(rules[5].value, RuleValue::Text("6mo".into()));
        let back: Vec<DraftRow> = rules.iter().map(DraftRow::from_rule).collect();
        assert_eq!(back, rows, "editing a saved album restores the same rows");
    }

    #[test]
    fn old_builder_kind_values_still_save() {
        for (legacy, label) in [("raw", "RAW"), ("image", "Photo (non-RAW)")] {
            let saved = SmartAlbumRule {
                field: RuleField::MediaType,
                operator: RuleOperator::Equals,
                value: RuleValue::Text(legacy.into()),
            };
            let row = DraftRow::from_rule(&saved);
            assert_eq!(row.display_value(), label);
            assert_eq!(
                row.to_rule(),
                Some(saved),
                "an album saved by the old builder must stay savable"
            );
        }
    }

    #[test]
    fn changing_operator_resets_value_only_when_its_kind_changes() {
        let mut r = row(RuleField::Tag, RuleOperator::Equals, "beach");
        r.set_operator(RuleOperator::Contains);
        assert_eq!(r.text, "beach", "tag text survives is → contains");
        r.set_operator(RuleOperator::IsEmpty);
        assert_eq!(r.text, "");
        let mut d = DraftRow::new(RuleField::Date);
        assert!(d.to_rule().is_some(), "a new date row defaults to today");
        d.set_operator(RuleOperator::InTheLast);
        assert_eq!(d.text, "30");
    }

    #[test]
    fn focus_order_skips_controls_a_row_does_not_have() {
        let one = [DraftRow::new(RuleField::Tag)];
        assert!(
            focus_order(&one).contains(&DraftFocus::Remove(0)),
            "the last row can be removed (v3)"
        );
        assert!(focus_order(&[]).contains(&DraftFocus::Add(0)));
        let rows = [
            row(RuleField::Tag, RuleOperator::IsEmpty, ""),
            row(RuleField::Date, RuleOperator::InTheLast, "7"),
        ];
        let order = focus_order(&rows);
        assert!(!order.contains(&DraftFocus::Value(0)));
        assert!(order.contains(&DraftFocus::Unit(1)));
        assert!(order.contains(&DraftFocus::Remove(0)));
        assert_eq!(order.last(), Some(&DraftFocus::Save));
    }
}
