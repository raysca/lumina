//! What the builder calls fields, operators and the match mode, and each
//! field's icon.

use lumina_core::{MatchMode, RuleField, RuleOperator};

use crate::icons::Icon;

pub fn field_label(field: &RuleField) -> &'static str {
    match field {
        RuleField::MediaType => "Kind",
        RuleField::IsFavorite => "Favorite",
        RuleField::Date => "Date Captured",
        RuleField::Tag => "Keywords",
        RuleField::Rating => "Rating",
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
        RuleField::Flag => "Flag",
    }
}

/// The 14 px icon in a row's field picker; the chips' icon, but Kind shows
/// a photo like the value it picks.
pub fn field_icon(field: &RuleField) -> Icon {
    match field {
        RuleField::MediaType => Icon::Image,
        field => crate::app::rule_label::rule_icon(field),
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

pub fn match_label(mode: MatchMode) -> &'static str {
    match mode {
        MatchMode::All => "all",
        MatchMode::Any => "any",
    }
}
