//! Top matches under the search header:
//! places, smart albums, months and tags that the query's free text names.
//! Computed off the UI thread when the query or the catalog changes, and
//! cached by both.

use gpui::Context;
use lumina_core::{PlaceCount, StructuredQuery, TimelineBucket};

use super::is_filter_token;
use crate::app::LuminaApp;

/// How long typing must pause before the catalog is asked again.
const REFRESH_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(150);

/// At most this many cards show.
pub(crate) const MAX_TOP_MATCHES: usize = 4;

/// What a card stands for, in display order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TopMatchKind {
    Place {
        city: String,
        country: String,
    },
    Album(i64),
    /// A month (`month` set) or a whole year.
    Moment {
        year: i32,
        month: Option<u32>,
    },
    Tag(String),
}

impl TopMatchKind {
    fn order(&self) -> u8 {
        match self {
            TopMatchKind::Place { .. } => 0,
            TopMatchKind::Album(_) => 1,
            TopMatchKind::Moment { .. } => 2,
            TopMatchKind::Tag(_) => 3,
        }
    }

    /// The selector stem: `top-match-{kind}-{n}`.
    pub(crate) fn selector(&self) -> &'static str {
        match self {
            TopMatchKind::Place { .. } => "place",
            TopMatchKind::Album(_) => "album",
            TopMatchKind::Moment { month: Some(_), .. } => "month",
            TopMatchKind::Moment { month: None, .. } => "year",
            TopMatchKind::Tag(_) => "tag",
        }
    }
}

/// One card: what it opens, its title and item count, and the query words
/// it was found from (a click replaces them with tokens).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TopMatch {
    pub kind: TopMatchKind,
    pub title: String,
    pub count: usize,
    pub words: Vec<String>,
}

/// The query words and catalog revision the cards were found for.
type MatchesKey = (Vec<String>, u64);

/// The cached cards and the keyboard highlight.
#[derive(Debug, Default)]
pub struct TopMatchesState {
    /// The words and catalog revision `matches` were computed for.
    pub(crate) key: Option<MatchesKey>,
    pub(crate) matches: Vec<TopMatch>,
    pub(crate) in_flight: bool,
    /// The card ↓ from the bar highlights for ↵.
    pub(crate) highlight: Option<usize>,
}

/// The query's free-text words (not filters, not `-exclusions`), unquoted,
/// two characters or longer.
pub(crate) fn query_text_words(query: &str) -> Vec<String> {
    lumina_core::search::tokenize_query(query)
        .into_iter()
        .filter(|token| !is_filter_token(token) && !token.starts_with('-'))
        .map(|token| token.trim_matches('"').to_string())
        .filter(|word| word.chars().count() >= 2)
        .collect()
}

/// A month name ("june", "jun"), as the parser reads `month:` (no digits:
/// a bare "6" is text, not June).
fn word_month(word: &str) -> Option<u32> {
    if word.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    StructuredQuery::parse(&format!("month:{word}")).month
}

/// A four-digit year.
fn word_year(word: &str) -> Option<i32> {
    (word.len() == 4 && word.chars().all(|c| c.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
        .filter(|year| (1800..=2200).contains(year))
}

/// What the cards are found in: everything but places is already cached
/// app state; `places` looks a word up in the catalog.
pub(crate) struct MatchSources<'a> {
    pub places: &'a dyn Fn(&str) -> Vec<PlaceCount>,
    /// User smart albums: id, title, cached count.
    pub albums: &'a [(i64, String, usize)],
    /// Month buckets, newest first (`bucket_key` "2025-06").
    pub months: &'a [TimelineBucket],
    /// Year buckets (`bucket_key` "2025").
    pub years: &'a [TimelineBucket],
    pub tags: &'a [(String, usize)],
}

/// The cards for `words`: one of each kind first (Place, Smart Album,
/// Moment, Tag), then more in that order, at most [`MAX_TOP_MATCHES`].
pub(crate) fn compute_top_matches(words: &[String], sources: &MatchSources) -> Vec<TopMatch> {
    let mut found: Vec<TopMatch> = Vec::new();
    for word in words {
        for place in (sources.places)(word) {
            let kind = TopMatchKind::Place {
                city: place.city.clone(),
                country: place.country_code.clone(),
            };
            if found.iter().all(|m| m.kind != kind) {
                let title = if place.country_code.is_empty() {
                    place.city
                } else {
                    format!("{}, {}", place.city, place.country_code)
                };
                found.push(TopMatch {
                    kind,
                    title,
                    count: place.count,
                    words: vec![word.clone()],
                });
            }
        }
    }
    for word in words {
        let lower = word.to_lowercase();
        for (id, title, count) in sources.albums {
            let kind = TopMatchKind::Album(*id);
            if title.to_lowercase().contains(&lower) && found.iter().all(|m| m.kind != kind) {
                found.push(TopMatch {
                    kind,
                    title: title.clone(),
                    count: *count,
                    words: vec![word.clone()],
                });
            }
        }
    }
    let year = words.iter().find_map(|w| word_year(w).map(|y| (w, y)));
    if let Some((month_word, month)) = words.iter().find_map(|w| word_month(w).map(|m| (w, m))) {
        let newest = sources.months.iter().find(|bucket| {
            let key = format!("-{month:02}");
            bucket.bucket_key.ends_with(&key)
                && year.is_none_or(|(_, y)| bucket.bucket_key.starts_with(&format!("{y:04}-")))
        });
        if let Some(bucket) = newest {
            let y: i32 = bucket.bucket_key[..4].parse().unwrap_or_default();
            let mut used = vec![month_word.clone()];
            used.extend(year.map(|(w, _)| w.clone()));
            found.push(TopMatch {
                kind: TopMatchKind::Moment {
                    year: y,
                    month: Some(month),
                },
                title: crate::i18n::month_year(y, month),
                count: bucket.item_count,
                words: used,
            });
        }
    } else if let Some((year_word, y)) = year {
        if let Some(bucket) = sources
            .years
            .iter()
            .find(|bucket| bucket.bucket_key == format!("{y:04}"))
        {
            found.push(TopMatch {
                kind: TopMatchKind::Moment {
                    year: y,
                    month: None,
                },
                title: y.to_string(),
                count: bucket.item_count,
                words: vec![year_word.clone()],
            });
        }
    }
    for word in words {
        let lower = word.to_lowercase();
        for (tag, count) in sources.tags {
            let kind = TopMatchKind::Tag(tag.clone());
            if tag.to_lowercase().starts_with(&lower) && found.iter().all(|m| m.kind != kind) {
                found.push(TopMatch {
                    kind,
                    title: tag.clone(),
                    count: *count,
                    words: vec![word.clone()],
                });
            }
        }
    }
    // One of each kind first, then the rest, then back in kind order.
    let mut firsts: Vec<usize> = Vec::new();
    for order in 0..4 {
        if let Some(i) = found.iter().position(|m| m.kind.order() == order) {
            firsts.push(i);
        }
    }
    let rest = (0..found.len()).filter(|i| !firsts.contains(i));
    let mut picked: Vec<usize> = firsts
        .iter()
        .copied()
        .chain(rest)
        .take(MAX_TOP_MATCHES)
        .collect();
    picked.sort_by_key(|&i| (found[i].kind.order(), i));
    picked.into_iter().map(|i| found[i].clone()).collect()
}

impl LuminaApp {
    /// The cards for the current query. The last fetch's cards stay up
    /// while a newer one runs, so the grid does not jump on each keystroke;
    /// none show without free text or off the Search screen.
    pub(crate) fn top_matches(&self) -> &[TopMatch] {
        if query_text_words(&self.search_query).is_empty() || !self.search_screen_shown() {
            return &[];
        }
        &self.top_matches.matches
    }

    /// Starts the matches fetch when the query's words or the catalog
    /// changed: after [`REFRESH_DEBOUNCE`], with the words current then.
    /// Places need the catalog; the rest is cached app state, copied into
    /// the task.
    pub(crate) fn refresh_top_matches(&mut self, cx: &mut Context<Self>) {
        if self.top_matches.in_flight
            || !self.search_screen_shown()
            || self.top_match_key_now().is_none()
        {
            return;
        }
        if query_text_words(&self.search_query).is_empty() {
            self.top_matches.key = self.top_match_key_now();
            self.top_matches.matches.clear();
            self.top_matches.highlight = None;
            return;
        }
        self.top_matches.in_flight = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REFRESH_DEBOUNCE).await;
            let Ok(Some(job)) = this.update(cx, |app, _| app.top_match_job()) else {
                return;
            };
            let (key, matches) = cx.background_executor().spawn(job).await;
            let _ = this.update(cx, |app, cx| {
                app.top_matches.in_flight = false;
                app.top_matches.key = Some(key);
                app.top_matches.matches = matches;
                app.top_matches.highlight = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// The cache key for now, or None when it is already current (or there
    /// is no library).
    fn top_match_key_now(&self) -> Option<MatchesKey> {
        let engine = &self.grid_state.as_ref()?.engine;
        let key = (query_text_words(&self.search_query), engine.data_revision());
        (self.top_matches.key.as_ref() != Some(&key)).then_some(key)
    }

    /// The fetch for the words typed by the time the debounce ends, or
    /// None (clearing `in_flight`) when there is nothing new to look up.
    fn top_match_job(
        &mut self,
    ) -> Option<impl std::future::Future<Output = (MatchesKey, Vec<TopMatch>)>> {
        let key = self
            .top_match_key_now()
            .filter(|(words, _)| !words.is_empty());
        let (Some(key), Some(state)) = (key, self.grid_state.as_ref()) else {
            self.top_matches.in_flight = false;
            return None;
        };
        let engine = state.engine.clone();
        let albums: Vec<(i64, String, usize)> = self
            .user_smart_albums
            .iter()
            .map(|album| {
                let count = self.user_album_counts.get(&album.id).copied();
                (album.id, album.title.clone(), count.unwrap_or(0))
            })
            .collect();
        let tags = self.library_tag_counts.clone();
        // ponytail: albums and tags are not in the key; a rename shows on
        // the next query change or catalog write.
        Some(async move {
            let places = |word: &str| engine.matching_places(word, MAX_TOP_MATCHES);
            let months = engine.get_timeline_buckets("month");
            let years = engine.get_timeline_buckets("year");
            let matches = compute_top_matches(
                &key.0,
                &MatchSources {
                    places: &places,
                    albums: &albums,
                    months: &months,
                    years: &years,
                    tags: &tags,
                },
            );
            (key, matches)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::top_match_open::{field_token, replace_words};
    use super::*;

    fn bucket(key: &str, count: usize) -> TimelineBucket {
        TimelineBucket {
            bucket_type: if key.len() == 4 { "year" } else { "month" }.to_string(),
            bucket_key: key.to_string(),
            start_timestamp: 0,
            end_timestamp: 0,
            item_count: count,
            cover_media_id: None,
        }
    }

    #[core::prelude::v1::test]
    fn matches_come_one_per_kind_first_and_at_most_four() {
        let places = |word: &str| {
            if word == "lisbon" {
                (0..3)
                    .map(|i| PlaceCount {
                        city: format!("Lisbon{i}"),
                        country_code: "PT".to_string(),
                        count: 3 - i,
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };
        let albums = [(7, "Lisbon trips".to_string(), 12)];
        let months = [bucket("2025-06", 40), bucket("2024-06", 9)];
        let tags = [("lisbon-tram".to_string(), 2)];
        let words = vec!["lisbon".to_string(), "june".to_string()];
        let found = compute_top_matches(
            &words,
            &MatchSources {
                places: &places,
                albums: &albums,
                months: &months,
                years: &[],
                tags: &tags,
            },
        );
        let kinds: Vec<&str> = found.iter().map(|m| m.kind.selector()).collect();
        assert_eq!(
            kinds,
            vec!["place", "album", "month", "tag"],
            "one of each kind beats a second place"
        );
        assert_eq!(found[2].title, "Jun 2025", "the newest June");
        assert_eq!(found[2].words, vec!["june".to_string()]);

        let words = vec!["june".to_string(), "2024".to_string()];
        let found = compute_top_matches(
            &words,
            &MatchSources {
                places: &places,
                albums: &[],
                months: &months,
                years: &[],
                tags: &[],
            },
        );
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].kind,
            TopMatchKind::Moment {
                year: 2024,
                month: Some(6)
            },
            "a year in the text picks that June"
        );
        assert_eq!(
            replace_words(
                "tram june 2024",
                &found[0].words,
                &["month:jun".into(), "year:2024".into()]
            ),
            "tram month:jun year:2024"
        );
        assert_eq!(field_token("city", "New York"), "city:\"New York\"");
    }

    #[core::prelude::v1::test]
    fn only_free_text_words_are_looked_up() {
        assert_eq!(
            query_text_words(r#"lisbon city:Porto -beach "golden hour" a"#),
            vec!["lisbon".to_string(), "golden hour".to_string()],
            "filters, exclusions and one-letter words are not matched"
        );
    }
}
