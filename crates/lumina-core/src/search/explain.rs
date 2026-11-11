//! Plain-English phrases describing what a parsed query searches for.

use super::StructuredQuery;

impl StructuredQuery {
    /// One short phrase per active filter, in a stable order, for the search
    /// field's explanation footer. Typed values are inserted verbatim.
    pub fn explain(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(self.fts_terms.iter().map(|t| format!("Matches “{t}”")));
        out.extend(self.fts_exclude.iter().map(|t| format!("Not “{t}”")));
        if let Some(fav) = self.is_favorite {
            out.push(if fav { "Favorites" } else { "Not favorites" }.into());
        }
        if let Some(kind) = &self.media_type {
            out.push(match kind.as_str() {
                "video" => "Videos".into(),
                "image" => "Photos (not RAW)".into(),
                "raw" => "RAW photos".into(),
                other => format!("Type {other}"),
            });
        }
        if let Some(trashed) = self.is_trashed {
            out.push(
                if trashed {
                    "Recently deleted"
                } else {
                    "Not deleted"
                }
                .into(),
            );
        }
        if let Some(y) = self.year {
            out.push(format!("Year {y}"));
        }
        if let Some(m) = self.month {
            out.push(
                MONTHS
                    .get((m as usize).wrapping_sub(1))
                    .map_or(format!("Month {m}"), |n| n.to_string()),
            );
        }
        if let Some((start, end)) = self.date_range {
            out.push(match (start != 0, end != i64::MAX) {
                (true, true) if day(start) == day(end) => format!("On {}", day(start)),
                (true, true) => format!("From {} to {}", day(start), day(end)),
                (true, false) => format!("From {}", day(start)),
                (false, true) => format!("Until {}", day(end)),
                (false, false) => "Any date".into(),
            });
        }
        let contains = [
            ("Camera", &self.camera),
            ("Lens", &self.lens),
            ("City", &self.city),
        ];
        for (label, value) in contains {
            if let Some(v) = value {
                out.push(format!("{label} contains “{v}”"));
            }
        }
        if let Some(c) = &self.country {
            out.push(format!("Country {c}"));
        }
        let iso = |n: u32| n.to_string();
        out.extend(range("ISO ", self.min_iso, self.max_iso, iso));
        let f = |n: f64| {
            format!("{:.2}", n)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        };
        // The parser stores `f:2.8` as 2.75..2.85 and nudges strict bounds by
        // 0.01 (`f:>2.8` is min 2.81). Undo both so the phrase reads the way
        // it was typed.
        // ponytail: hundredths heuristic; an explicit `f:>=2.81` reads
        // "above 2.8". Record the operator in the parser if that matters.
        let hundredths = |n: f64| ((n * 100.0).round() as i64).rem_euclid(10);
        let mid = |lo: f64, hi: f64| (lo + hi) / 2.0;
        let fstop = |n: f64| ((n * 10.0).round() / 10.0 - n).abs() < 1e-9;
        let low = |lo: f64| match hundredths(lo) {
            1 => format!("above f/{}", f(lo - 0.01)),
            _ => format!("f/{} or higher", f(lo)),
        };
        let high = |hi: f64| match hundredths(hi) {
            9 => format!("below f/{}", f(hi + 0.01)),
            _ => format!("f/{} or lower", f(hi)),
        };
        match (self.min_f_number, self.max_f_number) {
            (Some(lo), Some(hi)) if (hi - lo - 0.1).abs() < 1e-9 && fstop(mid(lo, hi)) => {
                out.push(format!("f/{}", f(mid(lo, hi))))
            }
            (Some(lo), Some(hi)) if hundredths(lo) != 1 && hundredths(hi) != 9 => {
                out.push(format!("f/{} to {}", f(lo), f(hi)))
            }
            (Some(lo), Some(hi)) => out.push(format!("{}, {}", low(lo), high(hi))),
            (Some(lo), None) => out.push(low(lo)),
            (None, Some(hi)) => out.push(high(hi)),
            (None, None) => {}
        }
        match (self.min_rating, self.max_rating) {
            (None | Some(0), Some(0)) => out.push("Unrated".into()),
            (lo, hi) => out.extend(range("Rated ", lo, hi, |n| n.to_string())),
        }
        if let Some(c) = self.color_label {
            let name = COLORS
                .get((c as usize).wrapping_sub(1))
                .copied()
                .unwrap_or("?");
            out.push(format!("Color {name}"));
        }
        if let Some(has) = self.has_color_label {
            out.push(
                if has {
                    "Color labeled"
                } else {
                    "No color label"
                }
                .into(),
            );
        }
        if let Some(flag) = self.flag {
            out.push(
                match flag {
                    crate::Flag::Pick => "Picks",
                    crate::Flag::Reject => "Rejects",
                    crate::Flag::Unflagged => "Unflagged",
                }
                .into(),
            );
        }
        if let Some([south, west, north, east]) = self.within {
            out.push(format!("Within {south}, {west} and {north}, {east}"));
        }
        if let Some(loc) = self.has_location {
            out.push(
                if loc {
                    "With location"
                } else {
                    "Without location"
                }
                .into(),
            );
        }
        if let Some(edited) = self.is_edited {
            out.push(if edited { "Edited" } else { "Not edited" }.into());
        }
        if let Some(uhd) = self.is_4k {
            out.push(if uhd { "4K" } else { "Not 4K" }.into());
        }
        if let Some(tagged) = self.has_tag {
            out.push(if tagged { "Tagged" } else { "Untagged" }.into());
        }
        if let Some(tag) = &self.tag {
            out.push(format!("Tagged “{tag}”"));
        }
        out.extend(
            self.exclude_tags
                .iter()
                .map(|t| format!("Not tagged “{t}”")),
        );
        if !self.unrecognized.is_empty() {
            out.push(format!("Not understood: {}", self.unrecognized.join(", ")));
        }
        out
    }
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Color label names by code (1 = red), as the `color:` filter spells them.
const COLORS: [&str; 6] = ["Red", "Orange", "Yellow", "Green", "Blue", "Purple"];

/// The UTC calendar day of an epoch-ms bound, as `date:` takes it.
fn day(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map_or_else(|| ms.to_string(), |d| d.format("%Y-%m-%d").to_string())
}

/// "ISO 800", "ISO 100 to 400", "ISO 801 or higher" or "ISO 400 or lower".
fn range<T: PartialEq + Copy>(
    label: &str,
    min: Option<T>,
    max: Option<T>,
    fmt: impl Fn(T) -> String,
) -> Option<String> {
    Some(match (min, max) {
        (Some(lo), Some(hi)) if lo == hi => format!("{label}{}", fmt(lo)),
        (Some(lo), Some(hi)) => format!("{label}{} to {}", fmt(lo), fmt(hi)),
        (Some(lo), None) => format!("{label}{} or higher", fmt(lo)),
        (None, Some(hi)) => format!("{label}{} or lower", fmt(hi)),
        (None, None) => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> Vec<String> {
        StructuredQuery::parse(text).explain()
    }

    #[test]
    fn explain_phrases_cover_every_field() {
        // Exhaustive on purpose (no `..`): a new field fails to compile here
        // until it gets a phrase and a case below.
        let StructuredQuery {
            fts_terms: _,
            fts_exclude: _,
            exclude_tags: _,
            unrecognized: _,
            year: _,
            month: _,
            date_range: _,
            camera: _,
            lens: _,
            city: _,
            country: _,
            min_iso: _,
            max_iso: _,
            min_f_number: _,
            max_f_number: _,
            media_type: _,
            is_favorite: _,
            is_trashed: _,
            has_location: _,
            has_tag: _,
            tag: _,
            min_rating: _,
            max_rating: _,
            color_label: _,
            has_color_label: _,
            flag: _,
            within: _,
            is_edited: _,
            is_4k: _,
        } = StructuredQuery::default();

        assert!(
            StructuredQuery::default().explain().is_empty(),
            "an empty query explains nothing"
        );
        let cases: &[(&str, &str, &str)] = &[
            ("fts_terms", "sunset", "Matches “sunset”"),
            ("fts_exclude", "-beach", "Not “beach”"),
            ("exclude_tags", "-tag:work", "Not tagged “work”"),
            ("unrecognized", "is:fovorite", "Not understood: is:fovorite"),
            ("year", "year:2026", "Year 2026"),
            ("month", "month:mar", "March"),
            (
                "date_range",
                "date:2026-09-01..2026-10-10",
                "From 2026-09-01 to 2026-10-10",
            ),
            ("camera", "camera:Sony", "Camera contains “Sony”"),
            ("lens", "lens:50mm", "Lens contains “50mm”"),
            ("city", "city:Kyoto", "City contains “Kyoto”"),
            ("country", "country:jp", "Country JP"),
            ("min_iso", "iso:>800", "ISO 801 or higher"),
            ("max_iso", "iso:<=400", "ISO 400 or lower"),
            ("min_f_number", "f:>=2.8", "f/2.8 or higher"),
            ("max_f_number", "f:<=4", "f/4 or lower"),
            ("media_type", "is:video", "Videos"),
            ("is_favorite", "is:favorite", "Favorites"),
            ("is_trashed", "is:trashed", "Recently deleted"),
            ("has_location", "has:location", "With location"),
            ("has_tag", "is:untagged", "Untagged"),
            ("tag", "tag:travel", "Tagged “travel”"),
            ("min_rating", "rating:>=4", "Rated 4 or higher"),
            ("max_rating", "is:unrated", "Unrated"),
            ("color_label", "color:red", "Color Red"),
            ("has_color_label", "is:unlabeled", "No color label"),
            ("flag", "is:pick", "Picks"),
            ("flag", "is:reject", "Rejects"),
            ("flag", "is:unflagged", "Unflagged"),
            ("is_edited", "is:edited", "Edited"),
            ("is_4k", "is:4k", "4K"),
            (
                "within",
                "within:34.5,135,35.5,136",
                "Within 34.5, 135 and 35.5, 136",
            ),
        ];
        for (field, text, phrase) in cases {
            let got = q(text);
            assert!(
                got.iter().any(|p| p == phrase),
                "{field}: {text:?} must explain as {phrase:?}, got {got:?}"
            );
        }
    }

    #[test]
    fn explain_combines_bounds_and_quotes_user_text_verbatim() {
        assert_eq!(q("iso:800"), ["ISO 800"]);
        assert_eq!(q("f:2.8"), ["f/2.8"]);
        assert_eq!(
            q("f:>=2.8 f:<=2.9"),
            ["f/2.8 to 2.9"],
            "a 0.1-wide range is not an exact f-stop"
        );
        assert_eq!(q("f:>2.8"), ["above f/2.8"], "no 2.81 leaks through");
        assert_eq!(q("f:<4"), ["below f/4"]);
        assert_eq!(q("rating:3"), ["Rated 3"]);
        assert_eq!(q("iso:>=100 iso:<=400"), ["ISO 100 to 400"]);
        assert_eq!(q("date:>=2026-01-01"), ["From 2026-01-01"]);
        assert_eq!(q("date:<2026-01-01"), ["Until 2025-12-31"]);
        assert_eq!(q("is:photo"), ["Photos (not RAW)"]);
        assert_eq!(q("-is:favorite"), ["Not favorites"]);
        assert_eq!(
            q(r#"camera:"Sony A7IV" "two words""#),
            ["Matches “two words”", "Camera contains “Sony A7IV”"]
        );
        assert_eq!(
            q("is:fovorite rating:abc"),
            ["Not understood: is:fovorite, rating:abc"]
        );
    }
}
