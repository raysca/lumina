//! Filter chips: one label and icon per filter token, shared by the
//! Library's chip row (Design 01) and the search query bar (Design 03).

use super::is_filter_token;
use crate::icons::Icon;

/// One chip of the filter row (and of Design 03's query bar): its label, its
/// icon, its family, and the indexes of the `tokenize_query` tokens it
/// stands for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FilterChip {
    pub label: String,
    pub icon: Icon,
    pub family: ChipFamily,
    pub tokens: Vec<usize>,
}

/// The token family that picks a chip's connector word in the query bar
/// (Design 03 decision 3), read from the parsed fields like the label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ChipFamily {
    /// `city:` / `country:`: "in".
    Place,
    /// `date:` / `month:` / `year:`: "during".
    Date,
    /// `camera:` / `lens:`: "with".
    Gear,
    /// A lone minimum rating (`rating:>=4`): "rated", then "4+".
    Rated(u32),
    /// Everything else, media type included: no connector.
    Other,
}

impl ChipFamily {
    /// The connector word shown before the chip, None for no word.
    pub(crate) fn connector(self) -> Option<String> {
        use crate::i18n::t;
        match self {
            ChipFamily::Place => Some(t("search.connector_in")),
            ChipFamily::Date => Some(t("search.connector_during")),
            ChipFamily::Gear => Some(t("search.connector_with")),
            ChipFamily::Rated(_) => Some(t("search.connector_rated")),
            ChipFamily::Other => None,
        }
    }
}

impl FilterChip {
    /// The label in the query bar: the row's label, except that a rated
    /// chip drops the word its connector already says ("rated 4+").
    pub(crate) fn bar_label(&self) -> String {
        match self.family {
            ChipFamily::Rated(n) => {
                crate::i18n::t("search.rating_value").replace("{n}", &n.to_string())
            }
            _ => self.label.clone(),
        }
    }
}

/// The label, icon and family of one filter token, None for free text. Read from
/// the parsed fields, one arm per family; families the design does not
/// name reuse the explanation footer's phrase.
pub(crate) fn filter_label(token: &str) -> Option<(String, Icon, ChipFamily)> {
    use crate::i18n::t;
    use ChipFamily::*;
    if !is_filter_token(token) {
        return None;
    }
    let p = lumina_core::StructuredQuery::parse(token);
    let explained = || p.explain().join(", ");
    Some(if let Some(month) = p.month {
        (
            t(&format!("date.month_short_{month}")),
            Icon::Calendar,
            Date,
        )
    } else if let Some(year) = p.year {
        (year.to_string(), Icon::Calendar, Date)
    } else if p.date_range.is_some() {
        (explained(), Icon::Calendar, Date)
    } else if let Some(place) = p.city.as_deref().or(p.country.as_deref()) {
        (place.to_string(), Icon::MapPin, Place)
    } else if p.within.is_some() || p.has_location.is_some() {
        (explained(), Icon::MapPin, Other)
    } else if let Some(camera) = &p.camera {
        (camera.clone(), Icon::Camera, Gear)
    } else if let Some(lens) = &p.lens {
        (lens.clone(), Icon::Aperture, Gear)
    } else if let (Some(n @ 1..), None) = (p.min_rating, p.max_rating) {
        (
            t("filter.rated").replace("{n}", &n.to_string()),
            Icon::Star,
            Rated(n),
        )
    } else if p.min_rating.is_some() || p.max_rating.is_some() {
        (explained(), Icon::Star, Other)
    } else if let Some(kind) = p.media_type.as_deref() {
        match kind {
            "video" => (t("filter.videos"), Icon::Video, Other),
            "raw" => (t("filter.raw"), Icon::FileImage, Other),
            _ => (t("filter.photos"), Icon::Image, Other),
        }
    } else if p.is_edited.is_some() {
        (explained(), Icon::Pencil, Other)
    } else if p.is_favorite.is_some() {
        (explained(), Icon::Heart, Other)
    } else if p.flag.is_some() {
        (explained(), Icon::Check, Other)
    } else if p.is_trashed.is_some() {
        (explained(), Icon::Trash, Other)
    } else if p.min_iso.is_some()
        || p.max_iso.is_some()
        || p.min_f_number.is_some()
        || p.max_f_number.is_some()
    {
        (explained(), Icon::Aperture, Other)
    } else if let Some(tag) = p
        .tag
        .as_deref()
        .or(p.exclude_tags.first().map(String::as_str))
    {
        let label = format!("#{tag}");
        if p.tag.is_some() {
            (label, Icon::Tag, Other)
        } else {
            (format!("−{label}"), Icon::Tag, Other)
        }
    } else if !p.unrecognized.is_empty() {
        // A misspelled filter searches nothing; the chip shows it as typed.
        (token.to_string(), Icon::SlidersHorizontal, Other)
    } else {
        // Colour labels and `has:` tags.
        (explained(), Icon::Tag, Other)
    })
}

/// The chips for `query`'s filter tokens, in query order. A lone `month:`
/// and a lone `year:` merge into one date chip ("Jun 2025") where the first
/// of them stands.
pub(crate) fn filter_chips(query: &str) -> Vec<FilterChip> {
    use lumina_core::StructuredQuery;
    let tokens = lumina_core::search::tokenize_query(query);
    let mut chips: Vec<FilterChip> = tokens
        .iter()
        .enumerate()
        .filter_map(|(i, token)| {
            filter_label(token).map(|(label, icon, family)| FilterChip {
                label,
                icon,
                family,
                tokens: vec![i],
            })
        })
        .collect();
    // One token sets one field, so a token with a month is a lone `month:`.
    let parsed: Vec<StructuredQuery> = tokens.iter().map(|t| StructuredQuery::parse(t)).collect();
    let month = parsed
        .iter()
        .enumerate()
        .find_map(|(i, p)| p.month.map(|m| (i, m)));
    let year = parsed
        .iter()
        .enumerate()
        .find_map(|(i, p)| p.year.map(|y| (i, y)));
    if let (Some((m, month)), Some((y, year))) = (month, year) {
        let (first, second) = (m.min(y), m.max(y));
        chips.retain(|chip| chip.tokens != [second]);
        if let Some(chip) = chips.iter_mut().find(|chip| chip.tokens == [first]) {
            chip.label = crate::i18n::month_year(year, month);
            chip.tokens.push(second);
        }
    }
    chips
}

/// `query` without the tokens at `indexes` (from [`FilterChip::tokens`]).
pub(crate) fn remove_query_tokens(query: &str, indexes: &[usize]) -> String {
    lumina_core::search::tokenize_query(query)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !indexes.contains(i))
        .map(|(_, token)| token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The query bar's two halves (Design 03): the chips, then the typed text
/// the search input edits (free text, plus the token being typed). While
/// `typing` (the user is typing in the bar), a last token with no space
/// after it, or inside an unclosed quote, is still being typed, so it stays
/// text: `city:Lis` turns into a chip on the space, `camera:"Sony ` on the
/// closing quote and a space. The typed text keeps a trailing space so the
/// next word can start; the query keeps its grammar either way.
pub(crate) fn query_bar_parts(query: &str, typing: bool) -> (Vec<FilterChip>, String) {
    let tokens = lumina_core::search::tokenize_query(query);
    let trailing_space = query.ends_with(char::is_whitespace);
    let ends_open = !trailing_space || query.matches('"').count() % 2 == 1;
    let closed = tokens.len() - usize::from(typing && ends_open && !tokens.is_empty());
    // The open token is last, so the closed tokens keep their indexes.
    let chips = filter_chips(&tokens[..closed].join(" "));
    let in_chip = |i: &usize| chips.iter().any(|chip| chip.tokens.contains(i));
    let mut typed = tokens
        .iter()
        .enumerate()
        .filter(|(i, _)| !in_chip(i))
        .map(|(_, token)| token.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if !typed.is_empty() && trailing_space {
        typed.push(' ');
    }
    (chips, typed)
}

/// The query after the bar's typed text became `typed`: the chips of
/// `query` (as [`query_bar_parts`] split it), then `typed`.
pub(crate) fn query_with_typed(query: &str, typing: bool, typed: &str) -> String {
    let tokens = lumina_core::search::tokenize_query(query);
    let (chips, _) = query_bar_parts(query, typing);
    let mut indexes: Vec<usize> = chips.iter().flat_map(|chip| chip.tokens.clone()).collect();
    indexes.sort_unstable();
    let mut out = indexes
        .into_iter()
        .map(|i| tokens[i].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if !out.is_empty() && !typed.is_empty() {
        out.push(' ');
    }
    out.push_str(typed);
    out
}

#[cfg(test)]
mod query_bar_tests {
    use super::*;

    #[core::prelude::v1::test]
    fn the_bar_splits_chips_from_typed_text_and_back() {
        let (chips, typed) = query_bar_parts("tram city:Lisbon month:jun year:2025", false);
        assert_eq!(chips.len(), 2, "place and the merged date: {chips:?}");
        assert_eq!(typed, "tram", "free text is the typed half");
        assert_eq!(
            query_with_typed("tram city:Lisbon", false, "tram x"),
            "city:Lisbon tram x",
            "the chips come first, the edited text after"
        );
        let (chips, typed) = query_bar_parts("tram city:Lis", true);
        assert!(chips.is_empty(), "the token being typed is no chip yet");
        assert_eq!(typed, "tram city:Lis");
        let (chips, typed) = query_bar_parts("city:Lisbon ", true);
        assert_eq!(chips.len(), 1, "a space closes the token into a chip");
        assert_eq!(typed, "", "no typed text left");
        assert_eq!(
            query_bar_parts("tram ", true).1,
            "tram ",
            "the trailing space survives for the next word"
        );
        assert_eq!(query_with_typed("city:Lisbon ", true, "x"), "city:Lisbon x");
        let (chips, typed) = query_bar_parts(r#"camera:"Sony "#, true);
        assert!(chips.is_empty(), "an unclosed quote keeps the token open");
        assert_eq!(
            typed, r#"camera:"Sony "#,
            "with its space, for the next word"
        );
        let (chips, typed) = query_bar_parts("year:2025 month:jun", true);
        assert_eq!(chips.len(), 1, "an open month leaves the year a chip");
        assert_eq!(typed, "month:jun");
    }

    #[core::prelude::v1::test]
    fn each_family_has_its_connector() {
        let bar = |query: &str| {
            filter_chips(query)
                .into_iter()
                .map(|chip| (chip.family.connector(), chip.bar_label()))
                .collect::<Vec<_>>()
        };
        let word = |w: &str| Some(w.to_string());
        assert_eq!(
            bar(r#"country:PT date:2025-06-01 camera:"X-T5" rating:>=4 is:video"#),
            vec![
                (word("in"), "PT".to_string()),
                (word("during"), filter_label("date:2025-06-01").unwrap().0),
                (word("with"), "X-T5".to_string()),
                (word("rated"), "4+".to_string()),
                (None, "Videos".to_string()),
            ],
            "place in, date during, gear with, rated n+, media type bare"
        );
        assert_eq!(
            filter_chips("rating:>=4")[0].label,
            "Rated 4+",
            "the Library chip row keeps its own label"
        );
        let (label, icon, _) = filter_label("is:edited").expect("is:edited is a filter");
        assert_eq!(
            (label.as_str(), icon),
            ("Edited", Icon::Pencil),
            "is:edited reads Edited with the pencil, like its Narrow chip"
        );
    }
}
