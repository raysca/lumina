//! Query text parsing: tokenizing and the `prefix:value` filter grammar.

use super::StructuredQuery;

impl StructuredQuery {
    /// Parses a user search query string containing keywords and structured tokens.
    /// Example: "sunset in Kyoto camera:Sony is:favorite year:2026 iso:>400 date:2026-09-01..2026-10-10"
    pub fn parse(input: &str) -> Self {
        let mut query = Self::default();
        let tokens = tokenize_query(input);

        for token in tokens {
            if let Some((prefix, value)) = split_prefix(&token) {
                let prefix_lower = prefix.to_lowercase();
                let val_clean = value.trim_matches('"').trim();

                match prefix_lower.as_str() {
                    "year" => {
                        if let Ok(y) = val_clean.parse::<i32>() {
                            query.year = Some(y);
                        }
                    }
                    "month" => {
                        if let Some(m) = parse_month(val_clean) {
                            query.month = Some(m);
                        }
                    }
                    "date" => {
                        if let Some(range) = parse_date_filter(val_clean) {
                            query.date_range = Some(range);
                        }
                    }
                    "camera" | "model" | "make" => {
                        if !val_clean.is_empty() {
                            query.camera = Some(val_clean.to_string());
                        }
                    }
                    "lens" => {
                        if !val_clean.is_empty() {
                            query.lens = Some(val_clean.to_string());
                        }
                    }
                    "city" => {
                        if !val_clean.is_empty() {
                            query.city = Some(val_clean.to_string());
                        }
                    }
                    "country" => {
                        if !val_clean.is_empty() {
                            query.country = Some(val_clean.to_uppercase());
                        }
                    }
                    "iso" => {
                        parse_num_filter(val_clean, &mut query.min_iso, &mut query.max_iso);
                    }
                    "f" | "fnumber" => {
                        parse_float_filter(
                            val_clean,
                            &mut query.min_f_number,
                            &mut query.max_f_number,
                        );
                    }
                    "is" => match val_clean.to_lowercase().as_str() {
                        "favorite" | "fav" => query.is_favorite = Some(true),
                        "video" => query.media_type = Some("video".to_string()),
                        "image" | "photo" => query.media_type = Some("image".to_string()),
                        "raw" => query.media_type = Some("raw".to_string()),
                        "trashed" | "trash" => query.is_trashed = Some(true),
                        _ => {}
                    },
                    "has" => match val_clean.to_lowercase().as_str() {
                        "location" | "geo" | "gps" => query.has_location = Some(true),
                        "tag" | "tags" | "label" => query.has_tag = Some(true),
                        "color" | "color_label" => query.has_color_label = Some(true),
                        "rating" | "stars" => query.min_rating = Some(1),
                        _ => {}
                    },
                    "rating" | "stars" => {
                        parse_num_filter(val_clean, &mut query.min_rating, &mut query.max_rating);
                    }
                    "color" => {
                        let code = match val_clean.to_lowercase().as_str() {
                            "red" | "1" => Some(1),
                            "orange" | "2" => Some(2),
                            "yellow" | "3" => Some(3),
                            "green" | "4" => Some(4),
                            "blue" | "5" => Some(5),
                            "purple" | "6" => Some(6),
                            "none" | "0" => Some(0),
                            _ => None,
                        };
                        if let Some(c) = code {
                            query.color_label = Some(c);
                        }
                    }
                    "tag" | "keyword" => {
                        if !val_clean.is_empty() {
                            query.tag = Some(val_clean.to_lowercase());
                        }
                    }
                    _ => {
                        // Unrecognized prefix treated as free-form text
                        query.fts_terms.push(token);
                    }
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

fn parse_num_filter(s: &str, min: &mut Option<u32>, max: &mut Option<u32>) {
    if let Some(stripped) = s.strip_prefix(">=") {
        if let Ok(n) = stripped.parse::<u32>() {
            *min = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('>') {
        if let Ok(n) = stripped.parse::<u32>() {
            *min = Some(n.saturating_add(1));
        }
    } else if let Some(stripped) = s.strip_prefix("<=") {
        if let Ok(n) = stripped.parse::<u32>() {
            *max = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('<') {
        if let Ok(n) = stripped.parse::<u32>() {
            *max = Some(n.saturating_sub(1));
        }
    } else if let Ok(n) = s.parse::<u32>() {
        *min = Some(n);
        *max = Some(n);
    }
}

fn parse_float_filter(s: &str, min: &mut Option<f64>, max: &mut Option<f64>) {
    if let Some(stripped) = s.strip_prefix(">=") {
        if let Ok(n) = stripped.parse::<f64>() {
            *min = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('>') {
        if let Ok(n) = stripped.parse::<f64>() {
            *min = Some(n + 0.01);
        }
    } else if let Some(stripped) = s.strip_prefix("<=") {
        if let Ok(n) = stripped.parse::<f64>() {
            *max = Some(n);
        }
    } else if let Some(stripped) = s.strip_prefix('<') {
        if let Ok(n) = stripped.parse::<f64>() {
            *max = Some(n - 0.01);
        }
    } else if let Ok(n) = s.parse::<f64>() {
        *min = Some(n - 0.05);
        *max = Some(n + 0.05);
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
}
