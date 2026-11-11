//! Filter chips: one label and icon per filter token, shared by the
//! Library's chip row (Design 01) and the search query bar (Design 03).

use super::is_filter_token;
use crate::icons::Icon;

/// One chip of the filter row (and of Design 03's query bar): its label, its
/// icon, and the indexes of the `tokenize_query` tokens it stands for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FilterChip {
    pub label: String,
    pub icon: Icon,
    pub tokens: Vec<usize>,
}

/// The label and icon of one filter token, None for free text. Read from
/// the parsed fields, one arm per family; families the design does not
/// name reuse the explanation footer's phrase.
pub(crate) fn filter_label(token: &str) -> Option<(String, Icon)> {
    use crate::i18n::t;
    if !is_filter_token(token) {
        return None;
    }
    let p = lumina_core::StructuredQuery::parse(token);
    let explained = || p.explain().join(", ");
    Some(if let Some(month) = p.month {
        (t(&format!("date.month_short_{month}")), Icon::Calendar)
    } else if let Some(year) = p.year {
        (year.to_string(), Icon::Calendar)
    } else if p.date_range.is_some() {
        (explained(), Icon::Calendar)
    } else if let Some(place) = p.city.as_deref().or(p.country.as_deref()) {
        (place.to_string(), Icon::MapPin)
    } else if p.within.is_some() || p.has_location.is_some() {
        (explained(), Icon::MapPin)
    } else if let Some(camera) = &p.camera {
        (camera.clone(), Icon::Camera)
    } else if let Some(lens) = &p.lens {
        (lens.clone(), Icon::Aperture)
    } else if let (Some(n @ 1..), None) = (p.min_rating, p.max_rating) {
        (t("filter.rated").replace("{n}", &n.to_string()), Icon::Star)
    } else if p.min_rating.is_some() || p.max_rating.is_some() {
        (explained(), Icon::Star)
    } else if let Some(kind) = p.media_type.as_deref() {
        match kind {
            "video" => (t("filter.videos"), Icon::Video),
            "raw" => (t("filter.raw"), Icon::FileImage),
            _ => (t("filter.photos"), Icon::Image),
        }
    } else if p.is_favorite.is_some() {
        (explained(), Icon::Heart)
    } else if p.flag.is_some() {
        (explained(), Icon::Check)
    } else if p.is_trashed.is_some() {
        (explained(), Icon::Trash)
    } else if p.min_iso.is_some()
        || p.max_iso.is_some()
        || p.min_f_number.is_some()
        || p.max_f_number.is_some()
    {
        (explained(), Icon::Aperture)
    } else if let Some(tag) = p
        .tag
        .as_deref()
        .or(p.exclude_tags.first().map(String::as_str))
    {
        let label = format!("#{tag}");
        if p.tag.is_some() {
            (label, Icon::Tag)
        } else {
            (format!("−{label}"), Icon::Tag)
        }
    } else if !p.unrecognized.is_empty() {
        // A misspelled filter searches nothing; the chip shows it as typed.
        (token.to_string(), Icon::SlidersHorizontal)
    } else {
        // Colour labels and `has:` tags.
        (explained(), Icon::Tag)
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
            filter_label(token).map(|(label, icon)| FilterChip {
                label,
                icon,
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
