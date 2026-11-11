//! Sidebar search suggestions and recent-search history.

use gpui::Context;
use std::collections::HashSet;

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

mod chips;
mod top_match_open;
mod top_matches;
pub(crate) use chips::{
    filter_chips, filter_label, query_bar_parts, query_with_typed, remove_query_tokens, FilterChip,
};
pub(crate) use top_matches::{TopMatch, TopMatchKind, TopMatchesState};

/// Maximum number of committed queries kept in recent-search history.
pub(crate) const MAX_RECENT_SEARCHES: usize = 10;

/// Maximum number of rows rendered in the search suggestion dropdown.
pub(crate) const MAX_SEARCH_SUGGESTIONS: usize = 8;

/// Fields and quick-filter tokens offered when a query is empty or matches one
/// of them. These mirror the query grammar.
const SEARCH_FIELD_SUGGESTIONS: &[&str] = &[
    "is:favorite",
    "is:video",
    "is:raw",
    "is:edited",
    "is:trashed",
    "is:photo",
    "is:unrated",
    "is:untagged",
    "is:unlabeled",
    "has:location",
    "has:tag",
    "has:color",
    "rating:>=4",
    "tag:",
    "camera:",
    "lens:",
    "city:",
    "country:",
    "iso:",
    "year:",
    "month:",
    "date:",
    "f:",
    "color:red",
    "color:orange",
    "color:yellow",
    "color:green",
    "color:blue",
    "color:purple",
    "within:",
];

/// The token currently being typed: the last whitespace-separated word.
///
/// A trailing space starts a new token and therefore matches every suggestion.
pub(crate) fn current_search_token(query: &str) -> &str {
    if query.chars().next_back().is_some_and(char::is_whitespace) {
        ""
    } else {
        query.split_whitespace().next_back().unwrap_or("")
    }
}

/// Case-insensitive substring match so tag:tr finds tag:travel.
pub(crate) fn search_suggestion_matches(suggestion: &str, token: &str) -> bool {
    let token = token.trim();
    if token.is_empty() {
        return true;
    }
    suggestion.to_lowercase().contains(&token.to_lowercase())
}

/// Formats a cached tag as a query token, quoting tags that contain spaces.
pub(crate) fn tag_search_suggestion(tag: &str) -> String {
    let tag = tag.trim();
    if tag.chars().any(char::is_whitespace) || tag.contains('"') {
        format!("tag:\"{}\"", tag.replace('"', ""))
    } else {
        format!("tag:{tag}")
    }
}

fn push_search_suggestion(items: &mut Vec<String>, seen: &mut HashSet<String>, suggestion: &str) {
    if items.len() >= MAX_SEARCH_SUGGESTIONS {
        return;
    }
    let suggestion = suggestion.trim();
    if suggestion.is_empty() {
        return;
    }
    if seen.insert(suggestion.to_lowercase()) {
        items.push(suggestion.to_string());
    }
}

/// Builds the filtered suggestion list from cached state only (no SQL).
///
/// Empty queries show recent searches followed by common operators; typed
/// queries match recent searches, cached tag counts, then operators.
pub(crate) fn search_suggestion_items(
    recent_searches: &[String],
    tag_counts: &[(String, usize)],
    query: &str,
) -> Vec<String> {
    let token = current_search_token(query);
    let mut items: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if token.is_empty() {
        // Keep room for the common operators even with a long recent list.
        for recent in recent_searches.iter().take(5) {
            push_search_suggestion(&mut items, &mut seen, recent);
        }
        for suggestion in SEARCH_FIELD_SUGGESTIONS {
            push_search_suggestion(&mut items, &mut seen, suggestion);
        }
        return items;
    }

    for recent in recent_searches {
        if search_suggestion_matches(recent, token) {
            push_search_suggestion(&mut items, &mut seen, recent);
        }
    }
    for (tag, _) in tag_counts {
        if items.len() >= MAX_SEARCH_SUGGESTIONS {
            break;
        }
        let suggestion = tag_search_suggestion(tag);
        if search_suggestion_matches(&suggestion, token) {
            push_search_suggestion(&mut items, &mut seen, &suggestion);
        }
    }
    for suggestion in SEARCH_FIELD_SUGGESTIONS {
        if items.len() >= MAX_SEARCH_SUGGESTIONS {
            break;
        }
        if search_suggestion_matches(suggestion, token) {
            push_search_suggestion(&mut items, &mut seen, suggestion);
        }
    }
    items
}

/// Records query as the newest recent search.
///
/// Returns true when a non-empty query was recorded. Entries are trimmed,
/// deduplicated case-insensitively, and bounded to MAX_RECENT_SEARCHES.
pub(crate) fn record_recent_search_entry(entries: &mut Vec<String>, query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }
    let key = trimmed.to_lowercase();
    entries.retain(|entry| entry.trim().to_lowercase() != key);
    entries.insert(0, trimmed.to_string());
    entries.truncate(MAX_RECENT_SEARCHES);
    true
}

/// Replaces the token being typed with suggestion, preserving earlier tokens.
pub(crate) fn apply_search_suggestion_text(query: &str, suggestion: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return suggestion.to_string();
    }
    if query.chars().next_back().is_some_and(char::is_whitespace) {
        return format!("{trimmed} {suggestion}");
    }
    let mut parts: Vec<&str> = trimmed.split_whitespace().collect();
    parts.pop();
    parts.push(suggestion);
    parts.join(" ")
}

/// Quick-filter families: a new token replaces the one already there.
#[derive(PartialEq)]
enum FilterFamily {
    Favorite,
    Rating,
    Colour,
    MediaType,
    Flag,
}

/// The family of one parsed token, read from the fields the core parser set
/// (no second grammar here).
fn filter_family(parsed: &lumina_core::StructuredQuery) -> Option<FilterFamily> {
    if parsed.is_favorite.is_some() {
        Some(FilterFamily::Favorite)
    } else if parsed.min_rating.is_some() || parsed.max_rating.is_some() {
        Some(FilterFamily::Rating)
    } else if parsed.color_label.is_some() || parsed.has_color_label.is_some() {
        Some(FilterFamily::Colour)
    } else if parsed.media_type.is_some() {
        Some(FilterFamily::MediaType)
    } else if parsed.flag.is_some() {
        Some(FilterFamily::Flag)
    } else {
        None
    }
}

/// Whether the parser treats `token` as a filter rather than text: it adds
/// no free-text term or exclusion. Misspelled filters count (they are not
/// searched); `-word` does not.
fn is_filter_token(token: &str) -> bool {
    let parsed = lumina_core::StructuredQuery::parse(token);
    parsed.fts_terms.is_empty() && parsed.fts_exclude.is_empty()
}

/// `query`'s tokens, an unclosed quote closed first so a token appended
/// after them stays outside it.
pub(crate) fn query_tokens_closed(query: &str) -> Vec<String> {
    if query.matches('"').count() % 2 == 1 {
        lumina_core::search::tokenize_query(&format!("{query}\""))
    } else {
        lumina_core::search::tokenize_query(query)
    }
}

/// Toggles a quick-filter `token` in `query` (D8). A token with the same
/// parsed effect is removed; otherwise tokens of the same family (favorite,
/// rating, colour, media type, flag) are replaced and `token` is appended. Quoted phrases
/// and free text are kept.
pub(crate) fn toggle_query_token(query: &str, token: &str) -> String {
    use lumina_core::StructuredQuery;
    let wanted = StructuredQuery::parse(token);
    let family = filter_family(&wanted);
    let mut tokens = query_tokens_closed(query);
    let before = tokens.len();
    tokens.retain(|existing| StructuredQuery::parse(existing) != wanted);
    if tokens.len() == before {
        if family.is_some() {
            tokens.retain(|existing| filter_family(&StructuredQuery::parse(existing)) != family);
        }
        tokens.push(token.to_string());
    }
    tokens.join(" ")
}

/// Clear Filters: drops every filter token, negated and misspelled ones
/// included, and keeps free text and `-word` exclusions.
pub(crate) fn clear_query_filters(query: &str) -> String {
    let mut tokens = lumina_core::search::tokenize_query(query);
    tokens.retain(|token| !is_filter_token(token));
    tokens.join(" ")
}

/// Whether `query` has free text (a term, phrase or `-word`), which shows
/// the Search screen (Design 03).
pub(crate) fn query_has_text(query: &str) -> bool {
    lumina_core::search::tokenize_query(query)
        .iter()
        .any(|token| !is_filter_token(token))
}

/// Whether Clear Filters would change `query`.
pub(crate) fn query_has_filters(query: &str) -> bool {
    lumina_core::search::tokenize_query(query)
        .iter()
        .any(|token| is_filter_token(token))
}

impl LuminaApp {
    ///
    /// Reads only cached recent searches and cached tag counts, never SQL.
    pub(crate) fn search_suggestion_items(&self) -> Vec<String> {
        search_suggestion_items(
            &self.config.recent_searches,
            &self.library_tag_counts,
            &self.search_query,
        )
    }

    /// Reopens the dropdown and clears the keyboard highlight and the
    /// query bar's chip selection.
    pub(crate) fn reset_search_suggestions(&mut self) {
        self.search_suggestions.dismissed = false;
        self.search_suggestions.selected = None;
        self.query_bar.chip = None;
        self.top_matches.highlight = None;
    }

    /// Hides the dropdown without leaving the search field.
    pub(crate) fn dismiss_search_suggestions(&mut self) {
        self.search_suggestions.dismissed = true;
        self.search_suggestions.selected = None;
        self.query_bar.chip = None;
    }

    /// Moves the suggestion highlight without editing the query.
    pub(crate) fn move_search_suggestion(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.search_suggestions.dismissed {
            return;
        }
        let count = self.search_suggestion_items().len();
        if count == 0 {
            return;
        }
        let next = match self.search_suggestions.selected {
            None => {
                if delta > 0 {
                    0
                } else {
                    count - 1
                }
            }
            Some(index) => (index as isize + delta).clamp(0, count as isize - 1) as usize,
        };
        self.search_suggestions.selected = Some(next);
        cx.notify();
    }

    /// Applies the suggestion at `index`, keeping the field focused so the user
    /// can keep refining the query.
    pub(crate) fn apply_search_suggestion_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let items = self.search_suggestion_items();
        let Some(suggestion) = items.get(index).cloned() else {
            return;
        };
        let departing = self.navigation_snapshot();
        self.search_query = apply_search_suggestion_text(&self.search_query, &suggestion);
        // Before clearing the year: the helper snapshots the album as left.
        self.scope_search_to_library();
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
        self.dismiss_search_suggestions();
        cx.notify();
    }

    /// D11: a non-empty query searches the whole library, so the sidebar
    /// moves to Library. The album being left stays one Back step away even
    /// for a typed query, which records no arrival: syncing the album makes
    /// it the trail's current entry, and Back's own sync adds the query.
    pub(crate) fn scope_search_to_library(&mut self) {
        if self.selection == AlbumSelection::AllLibrary || self.search_query.trim().is_empty() {
            return;
        }
        let mut album = self.navigation_snapshot();
        // Only an empty query can sit under an album (this helper and
        // `select_album` keep it so), so the album was left with none.
        album.search_query.clear();
        self.navigation_history.sync_pending(album);
        self.selection = AlbumSelection::AllLibrary;
        self.stacks.reset();
    }

    /// Whether the sidebar row for `selection` shows as active. While a
    /// query shows no row does: the results span the library, even though
    /// the selection stays `AllLibrary` for Back and navigation.
    pub(crate) fn sidebar_highlights(&self, selection: &AlbumSelection) -> bool {
        self.search_query.is_empty() && &self.selection == selection
    }

    /// Runs a quick filter: toggles its token in the query (D8).
    pub(crate) fn toggle_quick_filter(&mut self, token: &str, cx: &mut Context<Self>) {
        let query = toggle_query_token(&self.search_query, token);
        self.set_search_query(&query, cx);
    }

    /// Removes every filter token from the query, keeping its text.
    pub(crate) fn clear_quick_filters(&mut self, cx: &mut Context<Self>) {
        let query = clear_query_filters(&self.search_query);
        self.set_search_query(&query, cx);
    }

    /// The explanation footer text: the parsed query's phrases joined, or
    /// None for an empty query.
    pub(crate) fn search_explanation(&self) -> Option<String> {
        let phrases = lumina_core::StructuredQuery::parse(&self.search_query).explain();
        (!phrases.is_empty()).then(|| phrases.join(" · "))
    }

    /// Whether a suggestion row came from the recent-search history.
    pub(crate) fn is_recent_search(&self, suggestion: &str) -> bool {
        self.config
            .recent_searches
            .iter()
            .any(|recent| recent.trim().eq_ignore_ascii_case(suggestion))
    }

    /// Forgets the recent-search history. A failed save restores it (the
    /// banner is set by `save_config`).
    pub(crate) fn clear_recent_searches(&mut self, cx: &mut Context<Self>) {
        let before = std::mem::take(&mut self.config.recent_searches);
        if !self.save_config() {
            self.config.recent_searches = before;
        }
        self.search_suggestions.selected = None;
        cx.notify();
    }

    /// Records the current query as a committed recent search when non-empty.
    pub(crate) fn record_current_search(&mut self) {
        let query = self.search_query.clone();
        if record_recent_search_entry(&mut self.config.recent_searches, &query) {
            self.save_config();
        }
    }

    /// Commits the current query: records history and unfocuses the field.
    pub(crate) fn commit_search(&mut self, cx: &mut Context<Self>) {
        self.record_current_search();
        self.search_focused = false;
        self.search_suggestions.dismissed = false;
        self.search_suggestions.selected = None;
        cx.notify();
    }
}

#[cfg(test)]
mod search_suggestion_tests {
    use super::*;

    #[core::prelude::v1::test]
    fn recent_searches_trim_dedupe_and_bound() {
        let mut entries = Vec::new();
        for index in 0..12 {
            assert!(record_recent_search_entry(
                &mut entries,
                &format!("query {index}")
            ));
        }
        assert_eq!(
            entries.len(),
            MAX_RECENT_SEARCHES,
            "recent searches must stay bounded"
        );
        assert_eq!(entries[0], "query 11", "the newest query must be first");

        assert!(record_recent_search_entry(&mut entries, "  QUERY 11  "));
        assert_eq!(
            entries.len(),
            MAX_RECENT_SEARCHES,
            "re-recording an existing query must not grow the history"
        );
        assert_eq!(
            entries[0], "QUERY 11",
            "the recorded query must be trimmed and keep its newest casing"
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.to_lowercase() == "query 11")
                .count(),
            1,
            "deduplication must be case-insensitive"
        );

        assert!(
            !record_recent_search_entry(&mut entries, "   "),
            "empty queries must not be recorded"
        );
    }

    #[core::prelude::v1::test]
    fn empty_query_shows_recent_searches_and_common_operators() {
        let recent = vec!["camera:sony".to_string()];
        let items = search_suggestion_items(&recent, &[], "");
        assert_eq!(
            items.first().map(String::as_str),
            Some("camera:sony"),
            "recent searches come first for an empty query"
        );
        assert!(
            items.iter().any(|item| item == "is:favorite"),
            "common operators must fill the rest of an empty-query list"
        );
        assert!(items.len() <= MAX_SEARCH_SUGGESTIONS);
    }

    #[core::prelude::v1::test]
    fn suggestions_include_matching_tags_and_filter_by_token() {
        let tags = vec![
            ("travel".to_string(), 4),
            ("trip 2024".to_string(), 2),
            ("family".to_string(), 1),
        ];
        let items = search_suggestion_items(&[], &tags, "tag:");
        assert!(items.contains(&"tag:travel".to_string()));
        assert!(items.contains(&"tag:\"trip 2024\"".to_string()));

        let filtered = search_suggestion_items(&[], &tags, "tag:fam");
        assert_eq!(
            filtered.first().map(String::as_str),
            Some("tag:family"),
            "matching tags must be filtered by the typed token"
        );

        let operators = search_suggestion_items(&[], &tags, "is:");
        assert!(
            operators.iter().all(|item| item.contains("is:")),
            "operator suggestions must respect the typed token: {operators:?}"
        );
        assert!(operators.contains(&"is:favorite".to_string()));
        assert!(!operators.contains(&"tag:travel".to_string()));
    }

    #[core::prelude::v1::test]
    fn applying_a_suggestion_replaces_only_the_current_token() {
        assert_eq!(apply_search_suggestion_text("", "is:video"), "is:video");
        assert_eq!(
            apply_search_suggestion_text("camera:sony", "is:video"),
            "is:video"
        );
        assert_eq!(
            apply_search_suggestion_text("camera:sony tag:tr", "tag:travel"),
            "camera:sony tag:travel"
        );
        assert_eq!(
            apply_search_suggestion_text("camera:sony ", "tag:travel"),
            "camera:sony tag:travel"
        );
    }

    #[core::prelude::v1::test]
    fn color_suggestions_offered_for_color_prefix() {
        let items = search_suggestion_items(&[], &[], "color:");
        for color in ["red", "orange", "yellow", "green", "blue", "purple"] {
            assert!(
                items.contains(&format!("color:{color}")),
                "typing color: must offer color:{color}, got {items:?}"
            );
        }
        for token in ["is:photo", "is:unrated", "is:untagged", "is:unlabeled"] {
            assert!(
                search_suggestion_items(&[], &[], token).contains(&token.to_string()),
                "{token} must be suggested when typed"
            );
        }
        assert!(search_suggestion_items(&[], &[], "mon").contains(&"month:".to_string()));
        assert!(
            search_suggestion_items(&[], &[], "is:e").contains(&"is:edited".to_string()),
            "typing is:e offers is:edited"
        );
        assert!(search_suggestion_items(&[], &[], "f:").contains(&"f:".to_string()));
    }

    #[core::prelude::v1::test]
    fn toggle_query_token_adds_removes_and_replaces() {
        assert_eq!(toggle_query_token("", "is:favorite"), "is:favorite");
        assert_eq!(
            toggle_query_token("kyoto is:favorite", "is:favorite"),
            "kyoto",
            "a token already present toggles off"
        );
        assert_eq!(
            toggle_query_token("kyoto is:fav", "is:favorite"),
            "kyoto",
            "an alias with the same parsed effect toggles off"
        );
        assert_eq!(
            toggle_query_token("kyoto is:unrated", "rating:>=3"),
            "kyoto rating:>=3",
            "rating:>=3 replaces is:unrated (one rating family)"
        );
        assert_eq!(
            toggle_query_token("stars:>=2 kyoto", "rating:>=4"),
            "kyoto rating:>=4",
            "a rating replaces another rating"
        );
        assert_eq!(
            toggle_query_token("is:unlabeled", "color:red"),
            "color:red",
            "color:red replaces is:unlabeled (one colour family)"
        );
        assert_eq!(
            toggle_query_token("is:video", "is:photo"),
            "is:photo",
            "media types replace each other"
        );
        assert_eq!(
            toggle_query_token("\"red door\" -beach is:video", "color:red"),
            "\"red door\" -beach is:video color:red",
            "quoted phrases, exclusions and other families survive"
        );
        assert_eq!(
            clear_query_filters("\"red door\" -beach is:video -tag:x rating:>=2 is:fovorite"),
            "\"red door\" -beach",
            "Clear Filters drops every filter, negated and misspelled ones too, and keeps text"
        );
        assert_eq!(
            toggle_query_token("-is:favorite kyoto", "is:favorite"),
            "kyoto is:favorite",
            "Show Favorites replaces a negated favorite (one favorite family)"
        );
        assert_eq!(
            toggle_query_token("\"red door", "is:video"),
            "\"red door\" is:video",
            "an unclosed quote is closed so the token stays outside it"
        );
        assert!(query_has_filters("kyoto -is:favorite"));
        assert!(!query_has_filters("kyoto -beach \"a b\""));
    }

    #[core::prelude::v1::test]
    fn current_token_handles_trailing_space() {
        assert_eq!(current_search_token(""), "");
        assert_eq!(current_search_token("camera:sony tag:tr"), "tag:tr");
        assert_eq!(
            current_search_token("tag:tr "),
            "",
            "a trailing space starts a new token"
        );
    }
}
