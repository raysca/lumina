//! Short chip labels for smart album rules ("ISO > 1600", "RAW"), one per
//! rule, for the smart album cards and the empty smart album.

use lumina_core::{RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use crate::app::smart_draft::{field_label, DraftRow};
use crate::icons::Icon;

/// A rule as a chip: field, a short operator and the value, as the builder
/// shows them ("ISO > 1600", "Stars ≥ ★★★★", "Date last 30 days").
pub(crate) fn rule_label(rule: &SmartAlbumRule) -> String {
    let row = DraftRow::from_rule(rule);
    let value = row.display_value();
    let field = match rule.field {
        RuleField::FNumber => "ƒ",
        ref field => field_label(field),
    };
    match (&rule.field, &rule.operator) {
        // The value says it all: "RAW", "Video".
        (RuleField::MediaType, RuleOperator::Equals) => value,
        (RuleField::MediaType, RuleOperator::NotEquals) => format!("≠ {value}"),
        (RuleField::IsFavorite, _) if rule.value == RuleValue::Bool(false) => {
            format!("Not {}", field.to_lowercase())
        }
        (RuleField::IsFavorite, _) => field.to_string(),
        (RuleField::Query, _) => format!("“{value}”"),
        (_, op) => {
            let date = rule.field == RuleField::Date;
            let op = match op {
                RuleOperator::Equals => "=",
                RuleOperator::NotEquals => "≠",
                RuleOperator::GreaterThan if date => "after",
                RuleOperator::LessThan if date => "before",
                RuleOperator::GreaterThan => ">",
                RuleOperator::LessThan => "<",
                RuleOperator::AtLeast => "≥",
                RuleOperator::AtMost => "≤",
                RuleOperator::Contains => "has",
                RuleOperator::NotContains => "lacks",
                RuleOperator::Between => "in",
                RuleOperator::IsEmpty => return format!("No {}", field.to_lowercase()),
                RuleOperator::IsNotEmpty => return format!("Any {}", field.to_lowercase()),
                RuleOperator::InTheLast => {
                    return format!("{field} last {value} {}", row.unit.label())
                }
            };
            format!("{field} {op} {value}")
        }
    }
}

/// The chip's 10 px icon for a rule's field.
pub(crate) fn rule_icon(field: &RuleField) -> Icon {
    match field {
        RuleField::Tag => Icon::Tag,
        RuleField::MediaType => Icon::FileImage,
        RuleField::IsFavorite => Icon::Heart,
        RuleField::Year | RuleField::Month | RuleField::Date => Icon::Calendar,
        RuleField::City | RuleField::CountryCode | RuleField::Place => Icon::MapPin,
        RuleField::CameraMake | RuleField::CameraModel | RuleField::Camera | RuleField::Lens => {
            Icon::Camera
        }
        RuleField::Iso | RuleField::FNumber | RuleField::FocalLength => Icon::Aperture,
        RuleField::Filename | RuleField::FileSize => Icon::Image,
        RuleField::DurationMs => Icon::Clock,
        RuleField::Rating => Icon::Star,
        RuleField::ColorLabel | RuleField::Flag => Icon::Check,
        RuleField::Query => Icon::Search,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(field: RuleField, operator: RuleOperator, value: RuleValue) -> SmartAlbumRule {
        SmartAlbumRule {
            field,
            operator,
            value,
        }
    }

    #[test]
    fn rule_labels_are_short_chips() {
        use RuleField as F;
        use RuleOperator as O;
        use RuleValue as V;
        let cases = [
            (rule(F::Iso, O::GreaterThan, V::Integer(1600)), "ISO > 1600"),
            (rule(F::FNumber, O::LessThan, V::Number(2.5)), "ƒ < 2.5"),
            (rule(F::MediaType, O::Equals, V::Text("raw".into())), "RAW"),
            (
                rule(F::MediaType, O::NotEquals, V::Text("video".into())),
                "≠ Video",
            ),
            (rule(F::IsFavorite, O::Equals, V::Bool(true)), "Favorite"),
            (
                rule(F::IsFavorite, O::Equals, V::Bool(false)),
                "Not favorite",
            ),
            (rule(F::Rating, O::AtLeast, V::Integer(4)), "Stars ≥ ★★★★"),
            (rule(F::Flag, O::Equals, V::Integer(1)), "Flag = Pick"),
            (
                rule(F::Date, O::InTheLast, V::Text("30d".into())),
                "Date last 30 days",
            ),
            (
                rule(F::Date, O::GreaterThan, V::Text("2024-01-01".into())),
                "Date after 2024-01-01",
            ),
            (rule(F::Tag, O::IsEmpty, V::Text(String::new())), "No tag"),
            (
                rule(F::Tag, O::IsNotEmpty, V::Text(String::new())),
                "Any tag",
            ),
            (
                rule(
                    F::Date,
                    O::Between,
                    V::Text("2024-01-01 to 2024-02-01".into()),
                ),
                "Date in 2024-01-01 to 2024-02-01",
            ),
            (
                rule(F::Place, O::Contains, V::Text("Lisbon".into())),
                "Place has Lisbon",
            ),
            (
                rule(F::Query, O::Equals, V::Text("sunset".into())),
                "“sunset”",
            ),
        ];
        for (rule, want) in cases {
            assert_eq!(rule_label(&rule), want, "chip for {rule:?}");
        }
    }
}
