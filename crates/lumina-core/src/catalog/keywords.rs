//! Keyword paths. A hierarchy lives in the flat tag string
//! (`places/europe/paris`), so search, smart albums, the search index and
//! sidecars keep one tag model. Writes store exact paths; filters match a
//! keyword and everything below it.

use anyhow::{bail, Result};
use rusqlite::types::Value;

use crate::text::{clean_text, MAX_NAME_CHARS};

/// The canonical form of a keyword path: each `/` segment cleaned to one
/// line, trimmed and lowercased, empty segments dropped. Refuses a path that
/// is empty after cleaning or longer than [`MAX_NAME_CHARS`] (cutting it
/// would cut a segment and store a different keyword).
pub fn normalize_path(raw: &str) -> Result<String> {
    let path = clean_segments(raw);
    if path.is_empty() {
        bail!("a keyword cannot be empty");
    }
    if path.chars().count() > MAX_NAME_CHARS {
        bail!("a keyword can be at most {MAX_NAME_CHARS} characters, including its parents");
    }
    Ok(path)
}

/// True when `raw` has no segment left after cleaning (`""`, `"/"`, `" / "`).
pub(crate) fn is_blank_path(raw: &str) -> bool {
    clean_segments(raw).is_empty()
}

/// A filter value (`tag:`, a smart-album Tag rule, the tag view) in the form
/// tags are stored, so `tag:Places / Europe/` finds `places/europe`. A value
/// [`normalize_path`] refuses is matched as typed (trimmed, lowercased), so a
/// tag stored before paths were capped can still be filtered on.
pub(crate) fn filter_value(raw: &str) -> String {
    normalize_path(raw).unwrap_or_else(|_| raw.trim().to_lowercase())
}

fn clean_segments(raw: &str) -> String {
    // A segment's byte length bounds its character count, so this cap never
    // cuts; the whole-path cap in `normalize_path` is the one that refuses.
    raw.split('/')
        .map(|segment| clean_text(segment, segment.len()).to_lowercase())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// SQL that is true when `column` holds `path` or a keyword below it. Bind
/// [`subtree_params`] in order (anonymous `?`, so it composes with builders
/// that push positional values).
///
/// One index range plus a segment check: the range `[p, p || '0')` holds `p`,
/// `p/...` and also siblings such as `p-old` or `p.x` (every byte below `'0'`,
/// and `'/'` is 0x2F), so the `substr` test drops those. A single range,
/// rather than an `OR` of two, keeps one index seek on `media_tags`.
pub(crate) fn subtree_clause(column: &str) -> String {
    format!(
        "({column} >= ? AND {column} < ? || '0' \
         AND ({column} = ? OR substr({column}, length(?) + 1, 1) = '/'))"
    )
}

/// The four values [`subtree_clause`] binds, all `path`.
pub(crate) fn subtree_params(path: &str) -> [Value; 4] {
    let v = || Value::Text(path.to_string());
    [v(), v(), v(), v()]
}

/// The path one level up, or `None` for a top-level keyword.
pub fn parent_of(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

/// The last segment of `path` (the whole path for a top-level keyword).
pub fn leaf(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, leaf)| leaf)
}

/// `path` relative to `root` (`""` or `"/..."`) when it is `root` or below it.
pub fn subtree_suffix<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    let rest = path.strip_prefix(root)?;
    (rest.is_empty() || rest.starts_with('/')).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtree_suffix_matches_the_root_and_children_not_siblings() {
        assert_eq!(subtree_suffix("a", "a"), Some(""));
        assert_eq!(subtree_suffix("a/x", "a"), Some("/x"));
        assert_eq!(
            subtree_suffix("ax", "a"),
            None,
            "a sibling is not in the subtree"
        );
        assert_eq!(subtree_suffix("b/a", "a"), None);
    }

    #[test]
    fn normalize_path_trims_and_drops_empty_segments() {
        assert_eq!(
            normalize_path(" Places // Europe/ ").expect("valid path"),
            "places/europe",
            "segments are trimmed and lowercased, empty ones dropped"
        );
        assert_eq!(
            normalize_path("a\u{202E}b / c\nd").expect("valid path"),
            "a b/c d",
            "each segment is cleaned to one line"
        );
        assert!(
            normalize_path(" / // ").is_err(),
            "a path with no segments is refused"
        );
    }

    #[test]
    fn normalize_path_accepts_exactly_the_cap() {
        let at_cap = format!("{}/{}", "a".repeat(49), "b".repeat(50));
        assert_eq!(at_cap.chars().count(), MAX_NAME_CHARS);
        assert_eq!(normalize_path(&at_cap).expect("at the cap"), at_cap);
        assert!(
            normalize_path(&format!("{at_cap}c")).is_err(),
            "one character over the cap is refused, not cut"
        );
    }

    #[test]
    fn parent_and_leaf_split_on_the_last_separator() {
        assert_eq!(parent_of("places/europe/paris"), Some("places/europe"));
        assert_eq!(parent_of("places"), None);
        assert_eq!(leaf("places/europe/paris"), "paris");
        assert_eq!(leaf("places"), "places");
    }

    #[test]
    fn subtree_clause_has_one_placeholder_per_param() {
        assert_eq!(
            subtree_clause("tag").matches('?').count(),
            subtree_params("x").len(),
            "every ? needs exactly one bound value"
        );
    }
}
