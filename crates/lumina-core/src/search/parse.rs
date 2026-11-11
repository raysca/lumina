//! Query text parsing: tokenizing and the `prefix:value` filter grammar.

use super::StructuredQuery;
use crate::catalog::keywords::filter_value;

impl StructuredQuery {
    /// Parses a user search query string containing keywords and structured tokens.
    /// Example: "sunset in Kyoto camera:Sony is:favorite year:2026 iso:>400 date:2026-09-01..2026-10-10"
    ///
    /// A leading `-` negates a word, a quoted phrase, `tag:`, `is:favorite|edited`
    /// and `has:location|tag|color` (not `within:`). Bad filter values and unsupported
    /// negations land in [`StructuredQuery::unrecognized`].
    pub fn parse(input: &str) -> Self {
        let mut query = Self::default();

        for token in tokenize_query(input) {
            if let Some(rest) = token.strip_prefix('-') {
                query.parse_negated(&token, rest);
            } else if let Some((prefix, value)) = split_prefix(&token) {
                let prefix = prefix.to_lowercase();
                let val_clean = value.trim_matches('"').trim();
                if !is_filter_prefix(&prefix) {
                    // Unknown prefixes (`foo:bar`, `12:30`) are free text.
                    query.fts_terms.push(token);
                } else if !query.apply_filter(&prefix, val_clean) {
                    query.unrecognized.push(token);
                }
            } else {
                let trimmed = token.trim_matches('"').trim();
                if !trimmed.is_empty() {
                    query.fts_terms.push(trimmed.to_string());
                }
            }
        }

        query
    }

    /// Handles a `-token`. `token` is the whole token as typed, `rest` is it
    /// without the leading `-`.
    fn parse_negated(&mut self, token: &str, rest: &str) {
        let Some((prefix, value)) = split_prefix(rest) else {
            let text = rest.trim_matches('"').trim();
            if !text.is_empty() {
                self.fts_exclude.push(text.to_string());
            }
            return;
        };
        let value = value.trim_matches('"').trim().to_lowercase();
        let prefix = prefix.to_lowercase();
        match (prefix.as_str(), value.as_str()) {
            ("tag" | "keyword", tag) if !tag.is_empty() => {
                self.exclude_tags.push(filter_value(tag))
            }
            ("is", "favorite" | "favourite" | "fav") => self.is_favorite = Some(false),
            ("is", "edited") => self.is_edited = Some(false),
            ("has", "location" | "geo" | "gps") => self.has_location = Some(false),
            ("has", "tag" | "tags" | "label") => self.has_tag = Some(false),
            ("has", "color" | "color_label") => self.has_color_label = Some(false),
            _ if is_filter_prefix(&prefix) => self.unrecognized.push(token.to_string()),
            // ponytail: unknown prefixes are text when positive, so negated ones are excluded text
            _ => self.fts_exclude.push(rest.to_string()),
        }
    }

    /// Applies one known-prefix `prefix:value` token. Returns false when the
    /// value is not understood.
    fn apply_filter(&mut self, prefix: &str, val: &str) -> bool {
        match prefix {
            "year" => set(&mut self.year, val.parse::<i32>().ok()),
            "month" => set(&mut self.month, parse_month(val)),
            "date" => set(&mut self.date_range, parse_date_filter(val)),
            "camera" | "model" | "make" => set(&mut self.camera, non_empty(val)),
            "lens" => set(&mut self.lens, non_empty(val)),
            "city" => set(&mut self.city, non_empty(val)),
            "country" => set(&mut self.country, non_empty(val).map(|c| c.to_uppercase())),
            "iso" => parse_num_filter(val, &mut self.min_iso, &mut self.max_iso),
            "f" | "fnumber" => {
                parse_float_filter(val, &mut self.min_f_number, &mut self.max_f_number)
            }
            "rating" | "stars" => parse_num_filter(val, &mut self.min_rating, &mut self.max_rating),
            "tag" | "keyword" => set(&mut self.tag, non_empty(val).map(|t| filter_value(&t))),
            "is" => {
                match val.to_lowercase().as_str() {
                    "favorite" | "favourite" | "fav" => self.is_favorite = Some(true),
                    "video" => self.media_type = Some("video".to_string()),
                    "image" | "photo" => self.media_type = Some("image".to_string()),
                    "raw" => self.media_type = Some("raw".to_string()),
                    "trashed" | "trash" => self.is_trashed = Some(true),
                    "untagged" => self.has_tag = Some(false),
                    "unrated" => self.max_rating = Some(0),
                    "unlabeled" => self.has_color_label = Some(false),
                    "pick" => self.flag = Some(crate::Flag::Pick),
                    "reject" => self.flag = Some(crate::Flag::Reject),
                    "unflagged" => self.flag = Some(crate::Flag::Unflagged),
                    "edited" => self.is_edited = Some(true),
                    _ => return false,
                }
                true
            }
            "has" => {
                match val.to_lowercase().as_str() {
                    "location" | "geo" | "gps" => self.has_location = Some(true),
                    "tag" | "tags" | "label" => self.has_tag = Some(true),
                    "color" | "color_label" => self.has_color_label = Some(true),
                    "rating" | "stars" => self.min_rating = Some(1),
                    _ => return false,
                }
                true
            }
            "color" => {
                let code = match val.to_lowercase().as_str() {
                    "red" | "1" => 1,
                    "orange" | "2" => 2,
                    "yellow" | "3" => 3,
                    "green" | "4" => 4,
                    "blue" | "5" => 5,
                    "purple" | "6" => 6,
                    "none" | "0" => {
                        self.has_color_label = Some(false);
                        return true;
                    }
                    _ => return false,
                };
                self.color_label = Some(code);
                true
            }
            "within" => set(&mut self.within, parse_within(val)),
            _ => false,
        }
    }
}

/// Prefixes the parser treats as filters (anything else is free text).
fn is_filter_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "year"
            | "month"
            | "date"
            | "camera"
            | "model"
            | "make"
            | "lens"
            | "city"
            | "country"
            | "iso"
            | "f"
            | "fnumber"
            | "rating"
            | "stars"
            | "tag"
            | "keyword"
            | "is"
            | "has"
            | "color"
            | "within"
    )
}

/// Stores `value` in `slot` when present; returns whether it was.
fn set<T>(slot: &mut Option<T>, value: Option<T>) -> bool {
    let ok = value.is_some();
    if ok {
        *slot = value;
    }
    ok
}

fn non_empty(val: &str) -> Option<String> {
    (!val.is_empty()).then(|| val.to_string())
}

/// Splits a search query into terms, keeping `"quoted phrases"` (and
/// `tag:"two words"`) together as one term.
pub fn tokenize_query(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current.push(ch);
            }
            ' ' | '\t' if !in_quotes => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_string());
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    tokens
}

fn split_prefix(token: &str) -> Option<(&str, &str)> {
    if let Some(pos) = token.find(':') {
        let prefix = &token[..pos];
        let val = &token[pos + 1..];
        if !prefix.is_empty() && !val.is_empty() {
            return Some((prefix, val));
        }
    }
    None
}

/// Parses `lat1,lon1,lat2,lon2` into `[south, west, north, east]`. Each
/// part must be a number, latitudes in -90..=90 and longitudes in
/// -180..=180; anything else is not understood. Latitudes are put in
/// order; longitudes are kept as typed, so `west > east` wraps the
/// antimeridian.
fn parse_within(val: &str) -> Option<[f64; 4]> {
    let parts: Vec<f64> = val
        .split(',')
        .map(|p| p.trim().parse::<f64>().ok())
        .collect::<Option<_>>()?;
    let [lat1, lon1, lat2, lon2] = parts[..] else {
        return None;
    };
    let lat_ok = |v: f64| (-90.0..=90.0).contains(&v);
    let lon_ok = |v: f64| (-180.0..=180.0).contains(&v);
    (lat_ok(lat1) && lat_ok(lat2) && lon_ok(lon1) && lon_ok(lon2))
        .then(|| [lat1.min(lat2), lon1, lat1.max(lat2), lon2])
}

fn parse_month(s: &str) -> Option<u32> {
    if let Ok(m) = s.parse::<u32>() {
        if (1..=12).contains(&m) {
            return Some(m);
        }
    }
    match s.to_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn parse_date_filter(val: &str) -> Option<(i64, i64)> {
    if let Some((start_s, end_s)) = val.split_once("..") {
        let start_opt = chrono::NaiveDate::parse_from_str(start_s.trim(), "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc().timestamp_millis());
        let end_opt = chrono::NaiveDate::parse_from_str(end_s.trim(), "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(23, 59, 59))
            .map(|dt| dt.and_utc().timestamp_millis() + 999);
        if start_opt.is_none() && end_opt.is_none() {
            return None;
        }
        return Some((start_opt.unwrap_or(0), end_opt.unwrap_or(i64::MAX)));
    }
    if let Some(stripped) = val.strip_prefix(">=") {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let start = d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
            return Some((start, i64::MAX));
        }
    } else if let Some(stripped) = val.strip_prefix('>') {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let start = d.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis() + 1000;
            return Some((start, i64::MAX));
        }
    } else if let Some(stripped) = val.strip_prefix("<=") {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let end = d.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis() + 999;
            return Some((0, end));
        }
    } else if let Some(stripped) = val.strip_prefix('<') {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(stripped.trim(), "%Y-%m-%d") {
            let end = d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis() - 1;
            return Some((0, end));
        }
    } else if let Ok(d) = chrono::NaiveDate::parse_from_str(val.trim(), "%Y-%m-%d") {
        let start = d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis();
        let end = d.and_hms_opt(23, 59, 59)?.and_utc().timestamp_millis() + 999;
        return Some((start, end));
    }
    None
}

/// Parses `>=n`, `>n`, `<=n`, `<n` or `n` into the bounds. Returns false
/// (bounds untouched) when the value is not a number.
fn parse_num_filter(s: &str, min: &mut Option<u32>, max: &mut Option<u32>) -> bool {
    let num = |v: &str| v.parse::<u32>().ok();
    if let Some(rest) = s.strip_prefix(">=") {
        set(min, num(rest))
    } else if let Some(rest) = s.strip_prefix('>') {
        set(min, num(rest).map(|n| n.saturating_add(1)))
    } else if let Some(rest) = s.strip_prefix("<=") {
        set(max, num(rest))
    } else if let Some(rest) = s.strip_prefix('<') {
        set(max, num(rest).map(|n| n.saturating_sub(1)))
    } else if let Some(n) = num(s) {
        *min = Some(n);
        *max = Some(n);
        true
    } else {
        false
    }
}

/// Like [`parse_num_filter`] for f-numbers; an exact value matches +-0.05.
fn parse_float_filter(s: &str, min: &mut Option<f64>, max: &mut Option<f64>) -> bool {
    let num = |v: &str| v.parse::<f64>().ok().filter(|n| n.is_finite());
    if let Some(rest) = s.strip_prefix(">=") {
        set(min, num(rest))
    } else if let Some(rest) = s.strip_prefix('>') {
        set(min, num(rest).map(|n| n + 0.01))
    } else if let Some(rest) = s.strip_prefix("<=") {
        set(max, num(rest))
    } else if let Some(rest) = s.strip_prefix('<') {
        set(max, num(rest).map(|n| n - 0.01))
    } else if let Some(n) = num(s) {
        *min = Some(n - 0.05);
        *max = Some(n + 0.05);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_structured_query() {
        let q = StructuredQuery::parse(
            r#"sunset in Kyoto camera:"Sony A7IV" is:favorite year:2026 iso:>400 f:<2.8 date:2026-09-01..2026-10-10"#,
        );

        assert_eq!(q.fts_terms, vec!["sunset", "in", "Kyoto"]);
        assert_eq!(q.camera.as_deref(), Some("Sony A7IV"));
        assert_eq!(q.is_favorite, Some(true));
        assert_eq!(q.year, Some(2026));
        assert_eq!(q.min_iso, Some(401));
        assert!(q.max_f_number.is_some());
        assert!(q.date_range.is_some());
    }

    #[test]
    fn test_date_range_parsing() {
        let r = parse_date_filter("2026-01-01..2026-01-02").unwrap();
        assert!(r.0 < r.1);

        let r_exact = parse_date_filter("2026-05-10").unwrap();
        assert!(r_exact.0 < r_exact.1);
    }

    #[test]
    fn negated_booleans_and_aliases_parse() {
        let q = StructuredQuery::parse("-is:favorite -has:location -has:tag -has:color");
        assert_eq!(
            q.is_favorite,
            Some(false),
            "-is:favorite means not favorite"
        );
        assert_eq!(q.has_location, Some(false), "-has:location");
        assert_eq!(q.has_tag, Some(false), "-has:tag");
        assert_eq!(q.has_color_label, Some(false), "-has:color");
        assert!(q.fts_terms.is_empty() && q.unrecognized.is_empty());

        let q = StructuredQuery::parse("-beach -\"two words\" -tag:Work");
        assert_eq!(q.fts_exclude, vec!["beach", "two words"]);
        assert_eq!(q.exclude_tags, vec!["work"], "-tag: lowercases like tag:");
        assert!(
            q.fts_terms.is_empty(),
            "exclusions are not searched as text"
        );

        assert_eq!(StructuredQuery::parse("is:untagged").has_tag, Some(false));
        assert_eq!(StructuredQuery::parse("is:unrated").max_rating, Some(0));
        assert_eq!(
            StructuredQuery::parse("is:unlabeled").has_color_label,
            Some(false)
        );
        assert_eq!(
            StructuredQuery::parse("is:favourite").is_favorite,
            Some(true)
        );
        assert_eq!(StructuredQuery::parse("is:fav").is_favorite, Some(true));

        for lone in ["-", "-\"\""] {
            assert_eq!(
                StructuredQuery::parse(lone),
                StructuredQuery::default(),
                "{lone:?} is ignored"
            );
        }

        let q = StructuredQuery::parse("-is:video -camera:x");
        assert_eq!(
            q.unrecognized,
            vec!["-is:video", "-camera:x"],
            "unsupported negations are reported"
        );
        assert_eq!(q.media_type, None);
        assert_eq!(q.camera, None);
    }

    #[test]
    fn within_ignores_malformed_value() {
        let q = StructuredQuery::parse("within:35.5,135,34.5,136 is:favorite beach");
        assert_eq!(
            q.within,
            Some([34.5, 135.0, 35.5, 136.0]),
            "latitudes are put in order; longitudes stay as typed"
        );
        assert_eq!(q.is_favorite, Some(true), "within: composes with filters");
        assert_eq!(q.fts_terms, vec!["beach"], "and with free text");
        assert_eq!(
            StructuredQuery::parse("within:-20,170,-10,-170").within,
            Some([-20.0, 170.0, -10.0, -170.0]),
            "west > east is kept: the box wraps the antimeridian"
        );
        assert_eq!(
            StructuredQuery::parse(r#"within:"35, 135, 36, 136""#).within,
            Some([35.0, 135.0, 36.0, 136.0]),
            "a quoted value may contain spaces"
        );
        assert_eq!(
            StructuredQuery::parse("WITHIN:-90,-180,90,180").within,
            Some([-90.0, -180.0, 90.0, 180.0]),
            "the range limits are inclusive and the prefix is case-insensitive"
        );

        for bad in [
            "within:1,2,3",
            "within:1,2,3,4,5",
            "within:a,b,c,d",
            "within:1,,3,4",
            "within:1;2;3;4",
            "within:91,0,0,1",
            "within:0,0,-90.5,1",
            "within:0,181,1,1",
            "within:0,0,1,-180.01",
            "within:nan,0,1,1",
            "within:0,inf,1,1",
            "-within:0,0,1,1",
        ] {
            let q = StructuredQuery::parse(bad);
            assert_eq!(q.within, None, "{bad:?} must add no geo filter");
            assert!(
                q.fts_terms.is_empty() && q.fts_exclude.is_empty(),
                "{bad:?} must never become a text term, got {q:?}"
            );
            assert_eq!(q.unrecognized, vec![bad], "{bad:?} is reported as typed");
            assert_eq!(
                q.build_sql(),
                StructuredQuery::default().build_sql(),
                "{bad:?} adds no clause"
            );
        }
    }

    #[test]
    fn unknown_filter_values_are_reported() {
        let q = StructuredQuery::parse(
            "is:fovorite color:pink color:grey color:gray rating:abc year:20x6 iso:lots f:wide month:smarch date:yesterday has:wings",
        );
        assert_eq!(
            q.unrecognized,
            vec![
                "is:fovorite",
                "color:pink",
                "color:grey",
                "color:gray",
                "rating:abc",
                "year:20x6",
                "iso:lots",
                "f:wide",
                "month:smarch",
                "date:yesterday",
                "has:wings",
            ],
            "every bad value is reported as typed"
        );
        assert!(q.fts_terms.is_empty(), "reported tokens are not searched");
        assert_eq!(
            q.build_sql(),
            StructuredQuery::default().build_sql(),
            "reported tokens add no clause"
        );

        let q = StructuredQuery::parse("foo:bar 12:30");
        assert_eq!(
            q.fts_terms,
            vec!["foo:bar", "12:30"],
            "unknown prefixes stay text"
        );
        assert!(q.unrecognized.is_empty());
    }
}
