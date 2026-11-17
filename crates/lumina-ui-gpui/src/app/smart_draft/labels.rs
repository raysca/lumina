//! What the builder calls fields, operators and the match mode, and each
//! field's icon.

use lumina_core::{MatchMode, RuleField, RuleOperator};

use crate::i18n::{plural, t};
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
        RuleOperator::AnyOf => "include any of",
    }
}

/// A camera as people say it, from its EXIF make and model ("" or spaces
/// when missing); `None` when both are blank. Makers repeat their name in
/// the model, often only its first word ("NIKON CORPORATION" + "NIKON
/// D850"), so the make is dropped then. Shared by the inspector and the
/// smart album "Camera is" list so both name a camera the same way.
pub fn camera_name(make: &str, model: &str) -> Option<String> {
    match (make.trim(), model.trim()) {
        ("", "") => None,
        (make, "") => Some(make.to_string()),
        ("", model) => Some(model.to_string()),
        (make, model) => {
            let brand = make.split_whitespace().next().unwrap_or(make);
            if model.to_lowercase().starts_with(&brand.to_lowercase()) {
                Some(model.to_string())
            } else {
                Some(format!("{make} {model}"))
            }
        }
    }
}

/// The library's cameras grouped by [`camera_name`], ignoring case, in
/// first-seen order: each group is its name and its stored pairs flattened
/// (`[make, model, make, model, …]`), the "Camera is" rule's value. So
/// "Canon" / "canon", or a missing vs an empty make, list once and the rule
/// matches every spelling.
pub fn camera_groups(cameras: &[(String, String)]) -> Vec<(String, Vec<String>)> {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (make, model) in cameras {
        let Some(name) = camera_name(make, model) else {
            continue;
        };
        let pair = [make.clone(), model.clone()];
        // ponytail: linear search, the library has a handful of cameras.
        match groups
            .iter_mut()
            .find(|(n, _)| n.to_lowercase() == name.to_lowercase())
        {
            // A NULL and an empty make both read "": one seek is enough.
            Some((_, pairs)) if pairs.chunks(2).any(|p| p == pair) => {}
            Some((_, pairs)) => pairs.extend(pair),
            None => groups.push((name, pair.to_vec())),
        }
    }
    groups
}

pub fn match_label(mode: MatchMode) -> &'static str {
    match mode {
        MatchMode::All => "all",
        MatchMode::Any => "any",
    }
}

/// The Rating row's words after its stars: "4 stars & up", "2 stars or
/// fewer", "3 stars". `value` is the row's text ("1".."5").
pub fn rating_value_label(op: &RuleOperator, value: &str) -> String {
    let stars = plural("smart_album.stars", value.parse().unwrap_or(0));
    match op {
        RuleOperator::AtLeast => t("smart_album.rating_at_least").replace("{stars}", &stars),
        RuleOperator::AtMost => t("smart_album.rating_at_most").replace("{stars}", &stars),
        _ => stars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_groups_merge_spellings_once_each() {
        let pair = |make: &str, model: &str| (make.to_string(), model.to_string());
        let groups = camera_groups(&[
            // A NULL and an empty make both read "".
            pair("", "iPhone 15"),
            pair("", "iPhone 15"),
            pair("Canon", "Canon EOS R5"),
            pair("canon", "Canon EOS R5"),
        ]);
        assert_eq!(
            groups,
            vec![
                (
                    "iPhone 15".to_string(),
                    vec![String::new(), "iPhone 15".into()]
                ),
                (
                    "Canon EOS R5".to_string(),
                    vec![
                        "Canon".into(),
                        "Canon EOS R5".into(),
                        "canon".into(),
                        "Canon EOS R5".into()
                    ]
                ),
            ],
            "one group per name, each stored pair once"
        );
    }

    #[test]
    fn camera_name_drops_a_make_the_model_repeats() {
        assert_eq!(
            camera_name("NIKON CORPORATION", "NIKON D850").as_deref(),
            Some("NIKON D850")
        );
        assert_eq!(
            camera_name("FUJIFILM", "X-T5").as_deref(),
            Some("FUJIFILM X-T5")
        );
        assert_eq!(camera_name("", "iPhone 15").as_deref(), Some("iPhone 15"));
        assert_eq!(camera_name("Leica", " ").as_deref(), Some("Leica"));
        assert_eq!(camera_name(" ", ""), None, "no camera, no name");
    }

    #[test]
    fn rating_label_follows_the_operator_and_counts_stars() {
        use RuleOperator as O;
        assert_eq!(rating_value_label(&O::AtLeast, "4"), "4 stars & up");
        assert_eq!(rating_value_label(&O::AtMost, "2"), "2 stars or fewer");
        assert_eq!(rating_value_label(&O::Equals, "3"), "3 stars");
        assert_eq!(
            rating_value_label(&O::AtLeast, "1"),
            "1 star & up",
            "singular"
        );
    }
}
