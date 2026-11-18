//! Localization scaffolding.
//!
//! Only English (en-US) ships today, but user-facing command labels go through
//! this catalog so additional locales can be added without touching call sites.

mod en_us;

/// Translates a catalog key, falling back to the key itself.
pub fn t(key: &str) -> String {
    en_us::en_us(key).unwrap_or(key).to_string()
}

/// A count grouped by thousands for display, e.g. 48291 -> "48,291" (en-US,
/// the only shipped locale).
pub fn count(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The `{key}_one` text for 1 and `{key}_other` otherwise, with `{n}` as
/// the count grouped by thousands: "1 item", "48,291 items".
pub fn plural(key: &str, n: usize) -> String {
    t(&plural_key(key, n)).replace("{n}", &count(n))
}

/// The catalog key [`plural`] reads for `n`: `{key}_one` or `{key}_other`.
pub fn plural_key(key: &str, n: usize) -> String {
    format!("{key}_{}", if n == 1 { "one" } else { "other" })
}

/// A total running time to the nearest minute: "68 h 12 m", "3 m" under an
/// hour. `None` for zero (no clip has a known duration); any other total
/// reads at least "1 m", so a few seconds of video never shows as nothing.
pub fn hours_minutes(ms: u64) -> Option<String> {
    if ms == 0 {
        return None;
    }
    let minutes = (ms.saturating_add(30_000) / 60_000).max(1);
    let (h, m) = (minutes / 60, minutes % 60);
    Some(if h == 0 {
        t("header.minutes").replace("{m}", &m.to_string())
    } else {
        t("header.hours_minutes")
            .replace("{h}", &count(h as usize))
            .replace("{m}", &m.to_string())
    })
}

/// The Videos meta line: "4,183 clips · 68 h 12 m · 211.0 GB", without the
/// duration when none is known, and "at least 68 h 12 m" when `unknown`
/// clips have no duration yet. `size` is already formatted.
pub fn clips_meta(n: usize, duration_ms: u64, unknown: usize, size: &str) -> String {
    let clips = plural("header.clips", n);
    let duration = hours_minutes(duration_ms).map(|d| {
        if unknown > 0 {
            t("header.at_least").replace("{duration}", &d)
        } else {
            d
        }
    });
    match duration {
        Some(duration) => t("header.clips_meta")
            .replace("{clips}", &clips)
            .replace("{duration}", &duration)
            .replace("{size}", size),
        None => t("header.clips_size")
            .replace("{clips}", &clips)
            .replace("{size}", size),
    }
}

/// The import result banner title: "Imported 1,284 of 1,310. 26 files
/// failed." A copy stopped early (a full disk) never tried the rest, so it
/// counts every file not copied instead: "Imported 3 of 10. 7 files not
/// copied."
pub fn import_failed(copied: usize, total: usize, failed: usize) -> String {
    let not_copied = total.saturating_sub(copied);
    let (key, n) = if not_copied > failed {
        ("import.not_copied", not_copied)
    } else {
        ("import.failed", failed)
    };
    plural(key, n)
        .replace("{copied}", &count(copied))
        .replace("{total}", &count(total))
}

/// A year bucket's label: "2018", or "Undated" for items without a capture
/// time (the catalog files them under year 0).
pub fn year_label(year: i32) -> String {
    if year > 0 {
        year.to_string()
    } else {
        t("date.undated")
    }
}

/// "Jun 2018" (en-US short month and year); the year alone for a month
/// outside 1-12, and "Undated" for year 0.
pub fn month_year(year: i32, month: u32) -> String {
    if year <= 0 {
        return year_label(year);
    }
    if !(1..=12).contains(&month) {
        return year.to_string();
    }
    t("date.month_year")
        .replace("{month}", &t(&format!("date.month_short_{month}")))
        .replace("{year}", &year.to_string())
}

/// The page header's meta line with the view's capture months: "3 items ·
/// Jun 2018 — Nov 2026", one month when they match, the count alone without
/// a span.
pub fn items_in_span(n: usize, span: Option<lumina_core::DateSpan>) -> String {
    let Some(span) = span else {
        return plural("header.items", n);
    };
    let range = if span.first == span.last {
        month_year(span.first.0, span.first.1)
    } else {
        t("header.date_range")
            .replace("{from}", &month_year(span.first.0, span.first.1))
            .replace("{to}", &month_year(span.last.0, span.last.1))
    };
    t("header.meta")
        .replace("{items}", &plural("header.items", n))
        .replace("{range}", &range)
}

/// An album header's meta line: "284 items · 212 photos, 72 videos · 14 —
/// 18 Jun 2025". The count `n` alone until `stats` land, "0 items" for an
/// empty album; no range when no member is dated.
/// Once stats land the count is theirs, so the line never mixes a newer
/// count with older kinds.
pub fn album_meta(n: usize, stats: Option<lumina_core::AlbumStats>) -> String {
    let Some(stats) = stats else {
        return plural("header.items", n);
    };
    let n = stats.photos + stats.videos;
    if n == 0 {
        return plural("header.items", 0);
    }
    let mut parts = vec![
        plural("header.items", n),
        t("album_page.kinds")
            .replace("{photos}", &plural("album_page.photos", stats.photos))
            .replace("{videos}", &plural("album_page.videos", stats.videos)),
    ];
    if let (Some(first), Some(last)) = (stats.first_epoch_ms, stats.last_epoch_ms) {
        parts.extend(day_range(first, last));
    }
    parts.join(" · ")
}

/// Capture times as days, naming each side only as far as it differs:
/// "14 Jun 2025", "14 — 18 Jun 2025", "28 May — 3 Jun 2025", "28 Dec 2024
/// — 3 Jan 2025". Capture times are wall-clock time stored as UTC.
pub fn day_range(first_ms: i64, last_ms: i64) -> Option<String> {
    use chrono::Datelike;
    let first = chrono::DateTime::from_timestamp_millis(first_ms)?.date_naive();
    let last = chrono::DateTime::from_timestamp_millis(last_ms)?.date_naive();
    let day_month = |d: chrono::NaiveDate| {
        t("date.day_month")
            .replace("{day}", &d.day().to_string())
            .replace("{month}", &t(&format!("date.month_short_{}", d.month())))
    };
    let full = |d: chrono::NaiveDate| {
        t("date.day_month_year")
            .replace("{day_month}", &day_month(d))
            .replace("{year}", &d.year().to_string())
    };
    if first == last {
        return Some(full(last));
    }
    let from = if first.year() != last.year() {
        full(first)
    } else if first.month() != last.month() {
        day_month(first)
    } else {
        first.day().to_string()
    };
    Some(
        t("header.date_range")
            .replace("{from}", &from)
            .replace("{to}", &full(last)),
    )
}

/// Accessible name of a collection button, e.g. "Favorites, 22 items".
pub fn nav_aria_label(label: &str, count: usize) -> String {
    t(if count == 1 {
        "sidebar.nav_label_one"
    } else {
        "sidebar.nav_label_other"
    })
    .replace("{label}", label)
    .replace("{n}", &count.to_string())
}

/// [`t`] with each `{}` in the text replaced by the next of `args`.
pub fn t_with(key: &str, args: &[&str]) -> String {
    let mut out = t(key);
    for arg in args {
        out = out.replacen("{}", arg, 1);
    }
    out
}

/// Copy that names the platform's file manager and trash: Finder and the
/// macOS Trash on macOS, plain wording on Linux. Every such string lives
/// here, cfg-selected, so macOS copy stays exactly as it was.
pub mod platform {
    /// The reveal action in menus, the inspector and the playback card.
    #[cfg(not(target_os = "linux"))]
    pub const REVEAL: &str = "Reveal in Finder";
    #[cfg(target_os = "linux")]
    pub const REVEAL: &str = "Show in File Manager";
    /// [`REVEAL`] with its shortcut, for the inspector tooltip.
    #[cfg(not(target_os = "linux"))]
    pub const REVEAL_TOOLTIP: &str = "Reveal in Finder (⇧⌘R)";
    #[cfg(target_os = "linux")]
    pub const REVEAL_TOOLTIP: &str = "Show in File Manager (Ctrl+Shift+R)";
    /// The reveal action in the shortcut sheet and Settings › Downloads.
    #[cfg(not(target_os = "linux"))]
    pub const SHOW: &str = "Show in Finder";
    #[cfg(target_os = "linux")]
    pub const SHOW: &str = "Show in File Manager";
    /// Banner shown when the file manager could not reveal the file.
    #[cfg(not(target_os = "linux"))]
    pub const REVEAL_FAILED: &str = "Could not show this file in Finder.";
    #[cfg(target_os = "linux")]
    pub const REVEAL_FAILED: &str = "Could not reveal this file in the file manager.";
    /// Banner shown when no app is set to open a file's type.
    #[cfg(not(target_os = "linux"))]
    pub const DEFAULT_APP_FAILED: &str =
        "No app could open this file. Choose one with Open With in Finder.";
    #[cfg(target_os = "linux")]
    pub const DEFAULT_APP_FAILED: &str =
        "No app could open this file. Choose an app in your system's Open With menu.";
    /// Delete modal: what Move to Trash does with files on disk.
    #[cfg(not(target_os = "linux"))]
    pub const MOVE_TO_TRASH_DESCRIPTION: &str =
        "Moves the original files to the macOS Trash. Recover them in Finder.";
    #[cfg(target_os = "linux")]
    pub const MOVE_TO_TRASH_DESCRIPTION: &str =
        "Moves the original files to Trash. Recover them from your system's trash.";
    // Emptying asks per run (delete_modal.rs): the system trash, keep on
    // disk, or delete.
    #[cfg(not(target_os = "linux"))]
    pub(super) const TRASH_EMPTY_BODY: &str = "Deleted items stay here until you restore them or remove them for good. Lumina leaves their original files on disk unless you choose to move them to the macOS Trash or delete them.";
    #[cfg(target_os = "linux")]
    pub(super) const TRASH_EMPTY_BODY: &str = "Deleted items stay here until you restore them or remove them for good. Lumina leaves their original files on disk unless you choose to move them to your system's trash or delete them.";
    #[cfg(not(target_os = "linux"))]
    pub(super) const PLAYBACK_FAILED_BODY: &str =
        "Lumina couldn't open this video. The file may be damaged or use a format macOS can't play.";
    #[cfg(target_os = "linux")]
    pub(super) const PLAYBACK_FAILED_BODY: &str =
        "Lumina couldn't open this video. The file may be damaged or use a format this system can't play.";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the Linux port renamed Finder and the macOS Trash in
    /// macOS copy. macOS keeps its wording; only Linux uses plain wording.
    #[test]
    fn platform_copy_names_finder_only_on_macos() {
        let copy = [
            t("playback.reveal"),
            t("playback.failed_body"),
            t("trash.empty_body"),
            platform::REVEAL.into(),
            platform::REVEAL_TOOLTIP.into(),
            platform::SHOW.into(),
            platform::REVEAL_FAILED.into(),
            platform::DEFAULT_APP_FAILED.into(),
            platform::MOVE_TO_TRASH_DESCRIPTION.into(),
        ];
        if cfg!(target_os = "linux") {
            for text in &copy {
                assert!(
                    !text.contains("Finder") && !text.contains("macOS") && !text.contains('⌘'),
                    "Linux copy names neither Finder nor macOS, nor ⌘: {text}"
                );
            }
        } else {
            assert_eq!(t("playback.reveal"), "Reveal in Finder");
            assert_eq!(platform::SHOW, "Show in Finder");
            assert_eq!(
                platform::REVEAL_FAILED,
                "Could not show this file in Finder."
            );
            assert_eq!(
                platform::DEFAULT_APP_FAILED,
                "No app could open this file. Choose one with Open With in Finder."
            );
            assert!(
                t("playback.failed_body").ends_with("a format macOS can't play."),
                "macOS keeps its playback wording"
            );
            assert!(
                t("trash.empty_body").contains("the macOS Trash"),
                "macOS keeps the macOS Trash wording"
            );
            assert!(
                platform::MOVE_TO_TRASH_DESCRIPTION.contains("macOS Trash"),
                "macOS keeps the macOS Trash wording"
            );
        }
    }

    #[test]
    fn known_and_unknown_keys() {
        assert_eq!(t("command.preferences"), "Settings…");
        assert_eq!(t("missing.key"), "missing.key");
    }

    #[test]
    fn smart_album_builder_strings_are_catalogued() {
        assert_eq!(
            t_with("smart_album.cant_run", &["no date"]),
            "These conditions can't run: no date"
        );
        assert_eq!(
            t("smart_album.fix_conditions"),
            "Fix the highlighted conditions"
        );
        assert_eq!(t("smart_album.key_save"), "⏎");
        assert_eq!(t("smart_album.key_cancel"), "Esc");
        assert_eq!(t("smart_album.add_keyword"), "add keyword…");
        assert_eq!(
            t_with("smart_album.remove_keyword", &["beach"]),
            "Remove beach"
        );
    }

    /// The plural keys that replaced inline English keep its exact text.
    #[test]
    fn plural_keys_read_one_and_other() {
        let ungrouped = |key: &str, n: usize| t(&plural_key(key, n)).replace("{n}", &n.to_string());
        assert_eq!(ungrouped("duplicates.merged", 1), "Merged 1 duplicate");
        assert_eq!(
            ungrouped("duplicates.merged", 1200),
            "Merged 1200 duplicates"
        );
        assert_eq!(
            ungrouped("import.cancelled", 2),
            "Import cancelled after 2 items"
        );
        assert_eq!(ungrouped("import.imported", 1), "Imported 1 item");
        assert_eq!(ungrouped("preferences.thumbnails", 0), "0 thumbnails");
        assert_eq!(ungrouped("search.results", 1), "1 result");
        assert_eq!(
            ungrouped("downloads.still_running", 3),
            "3 downloads are still running."
        );
        assert_eq!(plural("smart_album.stars", 1), "1 star");
        assert_eq!(plural("filter.matches", 1_284), "1,284 matches");
    }

    #[test]
    fn count_groups_thousands() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(1_000), "1,000");
        assert_eq!(count(100_000), "100,000");
        assert_eq!(count(48_291), "48,291");
        assert_eq!(count(1_234_567), "1,234,567");
    }

    #[test]
    fn items_is_singular_for_one_and_groups_thousands() {
        assert_eq!(plural("header.items", 0), "0 items");
        assert_eq!(plural("header.items", 1), "1 item");
        assert_eq!(plural("header.items", 1_284_503), "1,284,503 items");
    }

    #[test]
    fn import_failed_is_singular_for_one_and_groups_thousands() {
        assert_eq!(import_failed(2, 3, 1), "Imported 2 of 3. 1 file failed.");
        assert_eq!(
            import_failed(1_284, 1_310, 26),
            "Imported 1,284 of 1,310. 26 files failed."
        );
        assert_eq!(
            import_failed(3, 10, 1),
            "Imported 3 of 10. 7 files not copied.",
            "a copy stopped early counts the untried files too"
        );
        assert_eq!(
            import_failed(2, 3, 0),
            "Imported 2 of 3. 1 file not copied.",
            "one file not copied is singular"
        );
    }

    #[test]
    fn clips_meta_reads_count_hours_minutes_and_size() {
        assert_eq!(plural("header.clips", 1), "1 clip");
        assert_eq!(plural("header.clips", 4_183), "4,183 clips");
        assert_eq!(hours_minutes(0), None, "no known duration, no part");
        assert_eq!(
            hours_minutes(5_000).as_deref(),
            Some("1 m"),
            "seconds round up to a minute"
        );
        assert_eq!(hours_minutes(89_999).as_deref(), Some("1 m"));
        assert_eq!(
            hours_minutes(90_000).as_deref(),
            Some("2 m"),
            "half a minute rounds up"
        );
        assert_eq!(hours_minutes(3_600_000).as_deref(), Some("1 h 0 m"));
        assert_eq!(
            hours_minutes((68 * 60 + 12) * 60_000).as_deref(),
            Some("68 h 12 m")
        );
        assert_eq!(
            clips_meta(2, 180_000, 0, "1.9 MB"),
            "2 clips · 3 m · 1.9 MB"
        );
        assert_eq!(
            clips_meta(3, 180_000, 1, "1.9 MB"),
            "3 clips · at least 3 m · 1.9 MB",
            "an unprobed clip makes the summed duration a lower bound"
        );
        assert_eq!(
            t("header.clips_size"),
            "{clips} · {size}",
            "the no-duration line has its own key, not the date-range one"
        );
        assert_eq!(
            clips_meta(1, 0, 1, "12 B"),
            "1 clip · 12 B",
            "unprobed clips drop the duration, not the size"
        );
    }

    #[test]
    fn album_meta_names_kinds_and_a_day_range() {
        let ms = |y, m, d| {
            chrono::NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp_millis()
        };
        let stats = |photos, videos, first, last| {
            Some(lumina_core::AlbumStats {
                photos,
                videos,
                first_epoch_ms: Some(first),
                last_epoch_ms: Some(last),
                cover: None,
            })
        };
        assert_eq!(
            album_meta(284, stats(212, 72, ms(2025, 6, 14), ms(2025, 6, 18))),
            "284 items · 212 photos, 72 videos · 14 — 18 Jun 2025"
        );
        assert_eq!(
            album_meta(2, stats(1, 1, ms(2025, 5, 28), ms(2025, 6, 3))),
            "2 items · 1 photo, 1 video · 28 May — 3 Jun 2025"
        );
        assert_eq!(
            album_meta(2, stats(2, 0, ms(2024, 12, 28), ms(2025, 1, 3))),
            "2 items · 2 photos, 0 videos · 28 Dec 2024 — 3 Jan 2025"
        );
        assert_eq!(
            album_meta(1, stats(1, 0, ms(2025, 6, 14), ms(2025, 6, 14))),
            "1 item · 1 photo, 0 videos · 14 Jun 2025"
        );
        assert_eq!(
            album_meta(3, None),
            "3 items",
            "the count alone before stats"
        );
        assert_eq!(
            album_meta(5, Some(lumina_core::AlbumStats::default())),
            "0 items",
            "an empty album has no kinds or range, and the count is the stats'"
        );
    }

    #[test]
    fn items_in_span_names_one_month_or_a_range() {
        use lumina_core::DateSpan;
        let span = |first, last| Some(DateSpan { first, last });
        assert_eq!(
            items_in_span(48_291, span((2018, 6), (2026, 11))),
            "48,291 items · Jun 2018 — Nov 2026"
        );
        assert_eq!(
            items_in_span(1, span((2018, 6), (2018, 6))),
            "1 item · Jun 2018"
        );
        assert_eq!(items_in_span(2, None), "2 items", "no span, no range part");
        assert_eq!(
            month_year(2018, 13),
            "2018",
            "a bad month never shows a raw key"
        );
        assert_eq!(month_year(0, 0), "Undated", "year 0 is undated");
        assert_eq!(year_label(0), "Undated");
        assert_eq!(year_label(2018), "2018");
    }
}
