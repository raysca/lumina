//! What a top-match card does: a click, or ↓ / ↑ / ↵ / Esc from the bar.

use gpui::Context;

use super::query_tokens_closed;
use super::top_matches::{query_text_words, TopMatchKind};
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

/// `month:` values as the rest of the app writes them (`month:jun`).
const MONTH_TOKENS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `query` with the first token equal to `words[0]` replaced by `tokens`
/// and the other `words` dropped. Tokens are added at the end when the
/// words are gone.
pub(super) fn replace_words(query: &str, words: &[String], tokens: &[String]) -> String {
    let mut out = query_tokens_closed(query);
    let mut at = None;
    for word in words {
        if let Some(i) = out
            .iter()
            .position(|token| token.trim_matches('"').eq_ignore_ascii_case(word))
        {
            out.remove(i);
            at = Some(at.map_or(i, |a: usize| a.min(i)));
        }
    }
    let at = at.unwrap_or(out.len());
    for (offset, token) in tokens.iter().enumerate() {
        out.insert(at + offset, token.clone());
    }
    out.join(" ")
}

/// A `field:value` token, the value quoted when it has spaces.
pub(super) fn field_token(field: &str, value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("{field}:\"{}\"", value.replace('"', ""))
    } else {
        format!("{field}:{value}")
    }
}

impl LuminaApp {
    /// Opens card `index`: a place or month becomes tokens in place of the
    /// words it came from, a smart album or tag opens in the sidebar.
    pub(crate) fn open_top_match(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(found) = self.top_matches().get(index).cloned() else {
            return;
        };
        // A card from before the last keystroke whose words have since gone
        // would land its tokens somewhere unasked; it does nothing instead.
        let words = query_text_words(&self.search_query);
        if !found
            .words
            .iter()
            .all(|word| words.iter().any(|w| w.eq_ignore_ascii_case(word)))
        {
            return;
        }
        self.top_matches.highlight = None;
        self.blur_search(cx);
        match found.kind {
            TopMatchKind::Place { city, .. } => {
                let query = replace_words(
                    &self.search_query,
                    &found.words,
                    &[field_token("city", &city)],
                );
                self.set_search_query(&query, cx);
            }
            TopMatchKind::Moment { year, month } => {
                let mut tokens: Vec<String> = month
                    .and_then(|m| MONTH_TOKENS.get(m.checked_sub(1)? as usize))
                    .map(|m| format!("month:{m}"))
                    .into_iter()
                    .collect();
                tokens.push(format!("year:{year}"));
                let query = replace_words(&self.search_query, &found.words, &tokens);
                self.set_search_query(&query, cx);
            }
            TopMatchKind::Album(id) => self.select_album(AlbumSelection::User(id), cx),
            TopMatchKind::Tag(tag) => self.select_album(AlbumSelection::Tag(tag), cx),
        }
        cx.notify();
    }

    /// ↓ / ↑ / ↵ / Esc on the cards from the focused bar, once the
    /// suggestions are out of the way (Esc hides them, or none show).
    /// True when the key was used.
    pub(crate) fn top_match_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let count = self.top_matches().len();
        let highlight = self.top_matches.highlight.filter(|&i| i < count);
        let suggestions_shown =
            !self.search_suggestions.dismissed && !self.search_suggestion_items().is_empty();
        match key {
            "down" | "up" if count > 0 && !suggestions_shown => {
                let next = match (highlight, key) {
                    (None, "down") => Some(0),
                    (None, _) => Some(count - 1),
                    (Some(i), "down") => Some((i + 1).min(count - 1)),
                    (Some(0), _) => None,
                    (Some(i), _) => Some(i - 1),
                };
                self.top_matches.highlight = next;
                if next.is_some() {
                    // One thing selected at a time: the card, not a chip.
                    self.query_bar.chip = None;
                }
                cx.notify();
            }
            "enter" | "return" if highlight.is_some() => {
                self.open_top_match(highlight.unwrap_or_default(), cx);
            }
            "escape" if highlight.is_some() => {
                self.top_matches.highlight = None;
                cx.notify();
            }
            _ => return false,
        }
        true
    }
}
