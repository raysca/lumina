//! Toolbar search suggestions and recent-search history.

use gpui::Context;
use std::collections::HashSet;

use crate::app::LuminaApp;

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

    /// Reopens the dropdown and clears the keyboard highlight.
    pub(crate) fn reset_search_suggestions(&mut self) {
        self.search_suggestions.dismissed = false;
        self.search_suggestions.selected = None;
    }

    /// Hides the dropdown without leaving the search field.
    pub(crate) fn dismiss_search_suggestions(&mut self) {
        self.search_suggestions.dismissed = true;
        self.search_suggestions.selected = None;
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
        self.active_year = None;
        self.clear_selection(cx);
        self.record_navigation_change(departing);
        self.dismiss_search_suggestions();
        cx.notify();
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
        assert!(search_suggestion_items(&[], &[], "f:").contains(&"f:".to_string()));
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
