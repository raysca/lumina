//! Localization scaffolding.
//!
//! Only English (en-US) ships today, but user-facing command labels go through
//! this catalog so additional locales can be added without touching call sites.
//! Select a locale with LUMINA_LANG (currently only "en-US" is understood).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    EnUs,
}

pub fn current_locale() -> Locale {
    match std::env::var("LUMINA_LANG").unwrap_or_default().as_str() {
        "en" | "en-US" | "en_US" => Locale::EnUs,
        _ => Locale::EnUs,
    }
}

/// Translates a catalog key, falling back to the key itself.
pub fn t(key: &str) -> String {
    match current_locale() {
        Locale::EnUs => en_us(key).unwrap_or(key).to_string(),
    }
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

/// The page header's meta line: "1 item", "48,291 items".
pub fn items(n: usize) -> String {
    if n == 1 {
        t("header.items_one")
    } else {
        t("header.items_other").replace("{n}", &count(n))
    }
}

/// The duplicates review's group count: "1 group", "128 groups".
pub fn duplicate_groups(n: usize) -> String {
    if n == 1 {
        t("duplicates.groups_one")
    } else {
        t("duplicates.groups_other").replace("{n}", &count(n))
    }
}

/// The Albums overview's counts: "1 album", "24 albums".
pub fn albums(n: usize) -> String {
    if n == 1 {
        t("albums.count_one")
    } else {
        t("albums.count_other").replace("{n}", &count(n))
    }
}

/// "1 smart album", "12 smart albums".
pub fn smart_albums(n: usize) -> String {
    if n == 1 {
        t("albums.smart_count_one")
    } else {
        t("albums.smart_count_other").replace("{n}", &count(n))
    }
}

/// A star count: "1 star", "4 stars" (a rating star's aria label).
pub fn stars(n: i64) -> String {
    if n == 1 {
        t("smart_album.star_one")
    } else {
        t("smart_album.stars_other").replace("{n}", &n.to_string())
    }
}

/// The Tags page's count: "1 keyword", "612 keywords".
pub fn keywords(n: usize) -> String {
    if n == 1 {
        t("tags.keywords_one")
    } else {
        t("tags.keywords_other").replace("{n}", &count(n))
    }
}

/// The Videos view's count: "1 clip", "4,183 clips".
pub fn clips(n: usize) -> String {
    if n == 1 {
        t("header.clips_one")
    } else {
        t("header.clips_other").replace("{n}", &count(n))
    }
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
/// duration when none is known. `size` is already formatted.
pub fn clips_meta(n: usize, duration_ms: u64, size: &str) -> String {
    let clips = clips(n);
    match hours_minutes(duration_ms) {
        Some(duration) => t("header.clips_meta")
            .replace("{clips}", &clips)
            .replace("{duration}", &duration)
            .replace("{size}", size),
        None => t("header.clips_size")
            .replace("{clips}", &clips)
            .replace("{size}", size),
    }
}

/// The filter row's count: "1 match", "1,284 matches".
pub fn matches(n: usize) -> String {
    if n == 1 {
        t("filter.matches_one")
    } else {
        t("filter.matches_other").replace("{n}", &count(n))
    }
}

/// The search header's count: "1 result", "1,284 results".
pub fn results(n: usize) -> String {
    if n == 1 {
        t("search.results_one")
    } else {
        t("search.results_other").replace("{n}", &count(n))
    }
}

/// The import result banner title: "Imported 1,284 of 1,310. 26 files
/// failed."
pub fn import_failed(copied: usize, total: usize, failed: usize) -> String {
    let key = if failed == 1 {
        "import.failed_one"
    } else {
        "import.failed_other"
    };
    t(key)
        .replace("{copied}", &count(copied))
        .replace("{total}", &count(total))
        .replace("{n}", &count(failed))
}

/// "Jun 2018" (en-US short month and year); the year alone for a month
/// outside 1-12.
pub fn month_year(year: i32, month: u32) -> String {
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
        return items(n);
    };
    let range = if span.first == span.last {
        month_year(span.first.0, span.first.1)
    } else {
        t("header.date_range")
            .replace("{from}", &month_year(span.first.0, span.first.1))
            .replace("{to}", &month_year(span.last.0, span.last.1))
    };
    t("header.meta")
        .replace("{items}", &items(n))
        .replace("{range}", &range)
}

/// An album header's meta line: "284 items · 212 photos, 72 videos · 14 —
/// 18 Jun 2025". The count `n` alone until `stats` land, "0 items" for an
/// empty album; no range when no member is dated.
/// Once stats land the count is theirs, so the line never mixes a newer
/// count with older kinds.
pub fn album_meta(n: usize, stats: Option<lumina_core::AlbumStats>) -> String {
    let Some(stats) = stats else {
        return items(n);
    };
    let n = stats.photos + stats.videos;
    if n == 0 {
        return items(0);
    }
    let plural = |n: usize, key: &str| {
        let form = if n == 1 { "one" } else { "other" };
        t(&format!("album_page.{key}_{form}")).replace("{n}", &count(n))
    };
    let mut parts = vec![
        items(n),
        t("album_page.kinds")
            .replace("{photos}", &plural(stats.photos, "photos"))
            .replace("{videos}", &plural(stats.videos, "videos")),
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

fn en_us(key: &str) -> Option<&'static str> {
    Some(match key {
        "command.open_library" => "Open Library…",
        "command.rescan" => "Rescan Library",
        "command.preferences" => "Preferences…",
        "command.reset_preferences" => "Reset Settings to Defaults…",
        "command.about" => "About Lumina",
        "command.empty_trash" => "Empty Trash…",
        "command.restore_all" => "Restore All from Trash",
        "command.merge_all_duplicates" => "Merge All Duplicates",
        "command.retry_error" => "Retry Failed Action",
        "error_banner.dismiss" => "Dismiss",
        "error_banner.dismiss_tooltip" => "Dismiss (Esc)",
        "command.show_license" => "Show MIT License",
        "command.show_acknowledgements" => "Show Acknowledgements",
        "command.shortcuts" => "Keyboard Shortcuts",
        "command.toggle_inspector" => "Toggle Inspector",
        "command.toggle_album_panel" => "Toggle Album Settings",
        "command.get_info" => "Get Info",
        "command.export" => "Export Originals…",
        "command.export_jpeg" => "Export as JPEG…",
        "command.share" => "Share…",
        "share.missing_one" => "The selected file is missing, so there is nothing to share",
        "share.missing_many" => "The {} selected files are missing, so there is nothing to share",
        "share.too_many" => "Select {} items or fewer to share",
        "share.no_window" => "Sharing needs the Lumina window in front. Click it and try again",
        "share.dock_tooltip" => "Share the selected originals",
        "command.import_xmp" => "Import XMP Metadata",
        "command.sort_date" => "Sort by Date Taken",
        "command.sort_name" => "Sort by Name",
        "command.sort_size" => "Sort by File Size",
        "command.sort_rating" => "Sort by Rating",
        "command.sort_asc" => "Sort Ascending",
        "command.sort_desc" => "Sort Descending",
        "command.toggle_fit" => "Toggle Fit / Square Cards",
        "command.zoom_in" => "Zoom Grid In",
        "command.zoom_out" => "Zoom Grid Out",
        // ponytail: chrono's %b is always the English month (no locale
        // feature); fine while en-US is the only locale, swap for the
        // date.month_short_* keys when another ships.
        "grid.caption_date" => "%-d %b %Y",
        "grid.badge_video" => "Video, {}",
        "grid.badge_playing" => "In Picture in Picture, {}",
        "grid.zoom_in" => "Larger thumbnails (⌘+)",
        "grid.zoom_out" => "Smaller thumbnails (⌘-)",
        "grid.zoom_limit" => "Not available at this size",
        "command.zoom_fit" => "Zoom to Fit",
        "command.zoom_fill" => "Zoom to Fill",
        "command.zoom_actual" => "Actual Size",
        "command.zoom_200" => "Zoom 200%",
        "command.zoom_400" => "Zoom 400%",
        "zoom.preset_fit" => "Fit",
        "zoom.preset_fill" => "Fill",
        "zoom.preset_actual" => "1:1",
        "zoom.preset_fit_hint" => "Fit (⌘0)",
        "zoom.preset_fill_hint" => "Fill (⌘3)",
        "zoom.preset_actual_hint" => "Actual size (⌘1)",
        "command.filter_favorites" => "Show Favorites",
        "command.filter_videos" => "Show Videos",
        "command.filter_rating_1" => "Show Rated ★ 1+",
        "command.filter_rating_2" => "Show Rated ★ 2+",
        "command.filter_rating_3" => "Show Rated ★ 3+",
        "command.filter_rating_4" => "Show Rated ★ 4+",
        "command.filter_rating_5" => "Show Rated ★ 5+",
        "command.filter_unrated" => "Show Unrated",
        "command.filter_color_red" => "Show Color: Red",
        "command.filter_color_orange" => "Show Color: Orange",
        "command.filter_color_yellow" => "Show Color: Yellow",
        "command.filter_color_green" => "Show Color: Green",
        "command.filter_color_blue" => "Show Color: Blue",
        "command.filter_color_purple" => "Show Color: Purple",
        "command.filter_unlabeled" => "Show Unlabeled",
        "command.filter_clear" => "Clear Filters",
        "command.clear_search" => "Clear Search",
        "command.add_filter" => "Add Filter…",
        "command.rate_1" => "Rate 1 Star",
        "command.rate_2" => "Rate 2 Stars",
        "command.rate_3" => "Rate 3 Stars",
        "command.rate_4" => "Rate 4 Stars",
        "command.rate_5" => "Rate 5 Stars",
        "command.clear_rating" => "Clear Rating",
        "command.rate_menu" => "Rate…",
        "command.flag_pick" => "Pick",
        "command.flag_reject" => "Reject",
        "command.flag_clear" => "Remove Flag",
        "command.delete_rejected" => "Delete Rejected…",
        "command.compare" => "Compare",
        "command.slideshow" => "Slideshow",
        "command.slideshow_loop" => "Slideshow: Loop",
        "command.slideshow_interval_3" => "Slideshow Interval: 3 Seconds",
        "command.slideshow_interval_5" => "Slideshow Interval: 5 Seconds",
        "command.slideshow_interval_10" => "Slideshow Interval: 10 Seconds",
        "slideshow.paused" => "Paused",
        "slideshow.empty" => "Nothing to show here",
        "slideshow.loop_on" => "Slideshow loops at the end",
        "slideshow.loop_off" => "Slideshow stops at the end",
        "command.show_exact_copies" => "Show Exact Copies",
        "command.show_similar_shots" => "Show Similar Shots",
        "duplicates.title" => "Duplicates",
        "duplicates.tab_exact_hint" => "Files with identical content",
        "duplicates.tab_similar" => "Similar shots",
        "duplicates.tab_similar_hint" => "RAW + JPEG pairs and bursts (⌘K Show Similar Shots)",
        "duplicates.review_title" => "Review Duplicates",
        "duplicates.groups_one" => "1 group",
        "duplicates.groups_other" => "{n} groups",
        "duplicates.recoverable" => "{} recoverable",
        "duplicates.copies" => "{} copies",
        "duplicates.groups_head" => "GROUPS",
        "duplicates.position" => "{} of {}",
        "duplicates.group_position" => "group {} of {}",
        "duplicates.groups_list" => "Duplicate groups",
        "duplicates.kind_exact" => "Exact",
        "duplicates.kind_exact_match" => "Exact match",
        "duplicates.merge_groups_one" => "Merge 1 group",
        "duplicates.merge_groups_other" => "Merge {n} groups",
        "duplicates.merge_groups_hint" => "Keep the chosen copy in every group and move the others to Recently Deleted",
        "duplicates.merge_group" => "Merge group",
        "duplicates.merge_group_hint" => "Keep the chosen copy and move the others to Recently Deleted",
        "duplicates.keeping" => "Keeping 1 of {}",
        "duplicates.moves_one" => "1 copy moves to Recently Deleted",
        "duplicates.moves_other" => "{} copies move to Recently Deleted",
        "duplicates.frees" => "· frees {}",
        "duplicates.best" => "BEST",
        "duplicates.best_hint" => "Highest rating, then favourite, then earliest",
        "duplicates.keeping_this" => "Keeping this copy",
        "duplicates.keep_this" => "Keep this copy",
        "duplicates.skip_group" => "Skip group",
        "duplicates.with_key" => "{} ({})",
        "command.skip_group" => "Skip Group",
        "command.merge_group" => "Merge Group",
        "command.auto_select_best" => "Auto-select Best",
        "command.skip_all_groups" => "Skip All Groups",
        "duplicates.skip_group_hint" => "Leave these copies as they are for now",
        "duplicates.skip_all" => "Skip all",
        "duplicates.skip_all_hint" => "Leave every group as it is for now. Skipped groups come back the next time you open Duplicates.",
        "duplicates.all_skipped" => "All groups skipped",
        "duplicates.all_skipped_body" => "Skipped groups were left as they are and come back the next time you open Duplicates.",
        "duplicates.auto_best" => "Auto-select best",
        "duplicates.auto_best_hint" => "Keep the best copy in every group: highest rating, then favourite, then earliest",
        "duplicates.prev" => "Previous group",
        "duplicates.next" => "Next group",
        "duplicates.original" => "ORIGINAL",
        "duplicates.copy_n" => "COPY {}",
        "duplicates.fact_size" => "File size",
        "duplicates.fact_format" => "Format",
        "duplicates.fact_edits" => "Edits",
        "duplicates.fact_source" => "Source",
        "duplicates.fact_captured" => "Captured",
        "duplicates.fact_folder" => "Folder",
        "duplicates.fact_dimensions" => "Dimensions",
        "duplicates.fact_name" => "Name",
        "duplicates.fact_title" => "Title",
        "duplicates.no_differences" => "No differences",
        "duplicates.one_difference" => "1 difference",
        "duplicates.differences" => "{} differences",
        "duplicates.kind_all" => "All",
        "duplicates.sorted" => "Sorted by space saved",
        "duplicates.edited" => "Edited",
        "duplicates.not_edited" => "None",
        "command.show_map" => "Show Map",
        "command.show_albums" => "Show Albums",
        "culling.delete_rejected_unavailable" => {
            "Delete Rejected works in a photo view, not in Recently Deleted, Duplicates or the Albums page"
        }
        "command.show_albums_tab" => "Show Albums Tab",
        "command.show_smart_albums_tab" => "Show Smart Albums Tab",
        "command.show_grid" => "Show Grid",
        "command.show_mosaic" => "Show Mosaic",
        "command.select_day" => "Select Day",
        "command.show_years" => "Show Years",
        "command.show_months" => "Show Months",
        "command.show_days" => "Show Days",
        "zoom.years" => "Years",
        "zoom.months" => "Months",
        "zoom.days" => "Days",
        "command.show_on_map" => "Show on Map",
        "map.label" => "Map of photo locations",
        "map.empty_title" => "No photos with a location yet",
        "map.empty_body" => "Photos and videos with a location, from the camera or set in the inspector, show here.",
        "map.load_failed" => "Could not read photo locations. Choose Retry, or reopen the library if it keeps failing.",
        "map.error_title" => "Could not read photo locations",
        "map.error_body" => "Choose Retry, or show the grid and then the map, to try again.",
        "map.cluster_one" => "1 item near {lat}, {lon}",
        "map.cluster_other" => "{n} items near {lat}, {lon}",
        "map.zoom_in" => "Zoom In (⌘=)",
        "map.zoom_out" => "Zoom Out (⌘-)",
        "map.hint" => "Tab or arrows: focus a place · Enter: show its photos · ⌘= ⌘-: zoom · Drag: pan · Esc: grid",
        "stacks.loading" => "Looking for similar shots…",
        "stacks.empty_title" => "No similar shots",
        "stacks.empty_body" => "RAW + JPEG pairs and bursts from one camera show up here.",
        "stacks.error_title" => "Could not look for similar shots",
        "stacks.error_body" => "The library could not be read. Try again; if it keeps failing, reopen the library.",
        "stacks.read_failed" => "Could not read this stack: the library could not be read. Try again.",
        "stacks.changed" => "This stack changed since it was listed, so nothing was flagged. The list was refreshed.",
        "stacks.title" => "{} · {} items",
        "stacks.more" => "+{} more",
        "stacks.no_slideshow" => "There is nothing to play here. Open Exact copies or another view for a slideshow.",
        "stacks.with_shortcut" => "{} ({})",
        "stacks.retry" => "Try Again",
        "stacks.kind_pair" => "RAW + JPEG",
        "stacks.kind_burst" => "Burst",
        "stacks.keep_picks" => "Keep picks, reject the rest",
        "stacks.reject_others" => "Reject all but the first",
        "stacks.pick_first" => "Pick at least one item in this stack first",
        "stacks.nothing_to_reject" => "Nothing left to reject in this stack",
        "stacks.hint" => "↑ ↓ move · Enter keep picks · ⇧Enter keep the first · Rejects wait for Delete Rejected…",
        "compare.title" => "Compare",
        "compare.close" => "Close Compare",
        "compare.hint" => "← → focus · P X U, 0–5 mark · ⌫ remove · Enter view",
        "compare.too_few" => "Select 2 to 9 items to compare",
        "compare.too_many" => "Compare up to 9 items",
        "compare.history_outside" => "Close Compare to undo or redo that change",
        "compare.pane_gone" => "That item is no longer in the library",
        "command.filter_picks" => "Show Picks",
        "command.filter_rejected" => "Show Rejected",
        "context.flag" => "Flag",
        "command.toggle_favorite" => "Toggle Favorite",
        "command.rotate_left" => "Rotate Left",
        "command.rotate_right" => "Rotate Right",
        "command.edit_photo" => "Edit Photo",
        "command.revert_to_original" => "Revert to Original",
        "command.edit_external" => "Edit in External Editor",
        "command.edit_copy_external" => "Edit a Copy in External Editor",
        "command.toggle_pip" => "Toggle Picture-in-Picture",
        "command.pip_play_pause" => "Play/Pause Picture-in-Picture",
        // GPUI has no text-transform: the badge is uppercase in the string.
        "pip.badge" => "PICTURE IN PICTURE",
        "pip.expand" => "Expand to viewer",
        "pip.close" => "Close",
        "pip.play" => "Play (Space)",
        "pip.pause" => "Pause (Space)",
        "pip.back" => "Back 5 seconds (⌥←)",
        "pip.forward" => "Forward 5 seconds (⌥→)",
        "pip.previous_chapter" => "Previous chapter (⌥←)",
        "pip.next_chapter" => "Next chapter (⌥→)",
        "pip.mute" => "Mute (M)",
        "pip.unmute" => "Unmute (M)",
        "pip.seek" => "Playback position",
        "pip.window_title" => "Picture-in-Picture",
        "pip.main_window_closed" => "Main library window closed",
        "pip.loading" => "Loading video…",
        "pip.unplayable" => "This video can't be played",
        "command.set_loop_start" => "Set Loop Start",
        "command.set_loop_end" => "Set Loop End",
        "command.clear_loop_range" => "Clear Loop Range",
        "command.volume_up" => "Volume Up",
        "command.volume_down" => "Volume Down",
        "command.previous_frame" => "Previous Frame",
        "command.next_frame" => "Next Frame",
        "video.loop_start_set" => "Loop start set to {time}",
        "video.loop_end_set" => "Loop end set to {time}",
        "video.loop_start_after_end" => "The loop start must come before the loop end",
        "video.loop_end_before_start" => "The loop end must come after the loop start",
        "video.play" => "Play (Space)",
        "video.pause" => "Pause (Space)",
        "video.back_5" => "Back 5 seconds (←)",
        "video.forward_5" => "Forward 5 seconds (→)",
        "video.speed" => "Playback speed",
        "video.mute" => "Mute (M)",
        "video.volume" => "Volume (↑/↓)",
        "video.unmute" => "Unmute (M)",
        "video.loop" => "Loop (L)",
        "video.fullscreen" => "Full screen (⌃⌘F)",
        "video.audio_menu" => "Audio",
        "video.audio_menu_tooltip" => "Audio track",
        "video.audio_track_unnamed" => "Track {n}",
        "command.audio_track" => "Audio Track: {name}",
        "video.chapter_jump" => "Chapter: {name}",
        "video.chapter_unnamed" => "Chapter {n}",
        "command.new_album" => "New Album",
        "command.new_album_with_selection" => "New Album with Selection",
        "command.rename_album" => "Rename Album…",
        "command.delete_album" => "Delete Album…",
        "command.add_to_album" => "Add to Album…",
        "command.remove_from_album" => "Remove from Album",
        "command.set_album_cover" => "Set as Album Cover",
        "command.add_from_library" => "Add from Library",
        "command.pin_album" => "Pin Album",
        "command.unpin_album" => "Unpin Album",
        "command.duplicate_smart_album" => "Duplicate Smart Album",
        "command.export_smart_album_rules" => "Export Smart Album Rules…",
        "command.import_smart_album_rules" => "Import Smart Album Rules…",
        "command.new_smart_album" => "New Smart Album…",
        "command.save_search" => "Save Search as Smart Album…",
        "command.adjust_date" => "Adjust Date Taken…",
        "command.edit_location" => "Edit Location…",
        "command.remove_location" => "Remove Location",
        "command.open_tag_manager" => "Open Tags",
        "tags.title" => "Tags",
        "tags.keywords_one" => "1 keyword",
        "tags.keywords_other" => "{n} keywords",
        "tags.meta" => "{keywords} · {percent}% of library tagged",
        "tags.tab_keywords" => "Keywords",
        "tags.filter_placeholder" => "Filter tags",
        "tags.sort_most_used" => "Most used",
        "tags.sort_az" => "A–Z",
        "tags.sort_menu" => "Sort tags",
        "command.sort_tags" => "Sort Tags…",
        "command.add_keyword_alias" => "Add Alias for Keyword…",
        "command.move_keyword" => "Move Keyword…",
        "command.rename_keyword" => "Rename Keyword…",
        "command.delete_keyword" => "Delete Keyword",
        "tags.col_tag" => "TAG",
        "tags.col_usage" => "USAGE",
        "tags.aliases_one" => "+1 alias",
        "tags.aliases_other" => "+{n} aliases",
        "tags.row_menu" => "More actions for {tag}",
        "tags.row_rename" => "Rename",
        "tags.row_move" => "Move to…",
        "tags.row_merge" => "Merge into…",
        "tags.merge_sub" => "Combines items, keeps its name as an alias",
        "tags.merge_hint" => "Type to filter, ↑↓ to choose, ↵ to merge.",
        "tags.merge_none" => "No other keyword matches.",
        "tags.merge_placeholder" => "Filter keywords",
        "tags.merge_targets" => "Keywords to merge into",
        "tags.merged_into" => "Merged \"{source}\" into \"{target}\"",
        "command.merge_keyword" => "Merge Tag Into…",
        "tags.row_save_smart" => "Save as Smart Album",
        "tags.save_smart_sub" => "Updates itself as new matches arrive",
        "command.save_keyword_smart" => "Save Tag as Smart Album",
        "tags.row_delete" => "Delete",
        "command.apply_template" => "Apply Metadata Template…",
        "command.manage_templates" => "Metadata Templates…",
        "template.title" => "Metadata Templates",
        "template.intro" => "A template sets creator, copyright and contact details and adds keywords. Applying it changes only the fields it sets.",
        "template.new" => "New Template",
        "template.field_name" => "Name",
        "template.field_creator" => "Creator",
        "template.field_copyright" => "Copyright",
        "template.field_usage_terms" => "Usage terms",
        "template.field_contact_email" => "Contact email",
        "template.field_contact_url" => "Contact website",
        "template.field_keywords" => "Keywords",
        "template.save" => "Save",
        "template.cancel" => "Cancel",
        "template.delete" => "Delete",
        "template.keys_hint" => "Tab next field · ↑/↓ other templates · ↵ save · Esc discard",
        "template.saved" => "Saved template “{name}”",
        "template.deleted" => "Deleted template “{name}”",
        "template.error_no_name" => "Give the template a name.",
        "template.error_name_taken" => "Another template has this name.",
        "template.error_restore_name_taken" => {
            "Cannot restore the template: another template has its name. Rename that one, then undo again"
        }
        "template.error_no_selection" => "Select photos or videos to apply a template to.",
        "template.error_apply" => "Could not apply the template",
        "template.error_other_dialog" => "Close the open dialog before editing templates",
        "template.error_unsaved" => "Save your changes (↵) or press Esc to discard them.",
        "template.delete_tooltip" => "Delete this template (⇧⌘⌫)",
        "template.applied_one" => "Applied “{name}” to 1 item",
        "template.applied_other" => "Applied “{name}” to {n} items",
        "template.section" => "TEMPLATE",
        "template.none_yet" => "No templates yet.",
        "template.picker_label" => "Template to apply: {name}. Click for the next one.",
        "template.apply" => "Apply to All",
        "tags.renamed_to" => "Renamed tag to \"{path}\"",
        "tags.moved_to" => "Moved keyword to \"{path}\"",
        "tags.bulk_delete_tooltip" => "Delete {n} keywords and the keywords below them (⌘⌫)",
        "tags.delete_note" => "Removes it from {items}. Your files are untouched.",
        "tags.delete_note_children" => {
            "Removes it and the keywords below it from {items}. Your files are untouched."
        }
        "tags.delete" => "Delete tag",
        "tags.detail_empty" => "Select a tag to see its details.",
        "tags.rename" => "Rename (↵)",
        "tags.touches" => "Changes {items}, including keywords below it. ↵ saves, Esc cancels.",
        "tags.preview" => "PREVIEW",
        "tags.view_all" => "View all",
        "tags.view_all_label" => "Show every item with this tag in the library",
        "tags.no_items" => "No items carry this tag.",
        "tags.aliases" => "ALIASES",
        "tags.aliases_hint" => "Typing any of these in a tag field adds {tag}.",
        "tags.alias_add" => "Add",
        "tags.alias_add_tooltip" => {
            "Add an alias (⇧⌘A): ↵ adds, ⌫ in an empty field removes the last"
        }
        "tags.alias_remove" => "Remove alias {alias}",
        "tags.organise" => "ORGANISE",
        "tags.move_sub" => "Puts it under another keyword (⇧⌘M)",
        "tags.move_hint" => "Type the parent keyword, or leave it empty for the top level.",
        "command.edit_smart_album" => "Edit Smart Album…",
        "command.delete_smart_album" => "Delete Smart Album…",
        "command.locate_missing" => "Locate Missing File…",
        "command.import_media" => "Import…",
        "command.import_network_video" => "Import Video from URL…",
        "command.toggle_downloads" => "Show Downloads",
        "command.show_all_downloads" => "Show All Downloads",
        "downloads.title" => "Downloads",
        "downloads.pause_all" => "Pause all",
        "downloads.resume_all" => "Resume all",
        "downloads.header_active" => "{} active",
        "downloads.show_all" => "Show all downloads",
        "downloads.earlier" => "EARLIER",
        "downloads.clear" => "Clear",
        "downloads.nothing_to_pause" => "Nothing to pause",
        "downloads.clear_completed" => "Clear completed",
        "downloads.nothing_to_clear" => "Nothing to clear",
        "downloads.add_url" => "Add URL",
        "downloads.add_url_tooltip" => "Import Video from URL… (⌥⌘U)",
        "downloads.empty_title" => "No downloads yet",
        "downloads.empty_body" => "Videos you import from a link show here while they download.",
        "downloads.meta_downloading" => "{} downloading",
        "downloads.meta_paused" => "{} paused",
        "downloads.meta_queued" => "{} queued",
        "downloads.meta_failed" => "{} failed",
        "downloads.meta_completed" => "{} completed",
        "downloads.filters" => "Filter downloads",
        "downloads.filter_all" => "All",
        "downloads.filter_downloading" => "Downloading",
        "downloads.filter_queued" => "Queued",
        "downloads.filter_paused" => "Paused",
        "downloads.filter_failed" => "Failed",
        "downloads.filter_completed" => "Completed",
        "downloads.filter_empty" => "No downloads match this filter.",
        "downloads.limits" => "{} at a time",
        "downloads.status_headline" => "Downloading {} of {} items",
        "downloads.status_of" => "{} of {}",
        "downloads.status_speed" => "{}/s combined",
        "downloads.status_remaining" => "about {} remaining",
        "downloads.overall" => "OVERALL",
        "downloads.settings" => "Preferences",
        "downloads.section_active" => "ACTIVE",
        "downloads.section_attention" => "NEEDS ATTENTION",
        "downloads.section_queued" => "QUEUED",
        "downloads.section_completed" => "COMPLETED",
        "downloads.section_active_action" => "Pause all",
        "downloads.section_attention_action" => "Retry all",
        "downloads.section_queued_action" => "Clear queue",
        "downloads.section_completed_action" => "View all",
        "downloads.pause" => "Pause",
        "downloads.resume" => "Resume",
        "downloads.retry" => "Retry",
        "downloads.remove" => "Remove from List",
        "downloads.move_up" => "Move Up",
        "downloads.show_in_library" => "Show in Library",
        "downloads.download_again" => "Download Again",
        "downloads.more" => "More",
        "downloads.row_menu" => "Download actions",
        "downloads.finishing" => "Finishing…",
        "downloads.getting_ready" => "Getting ready…",
        "downloads.paused" => "Paused",
        "downloads.paused_restart" => "Paused · resumes from the start",
        "downloads.next_up_row" => "Next up",
        "downloads.waiting" => "Waiting",
        "downloads.cancel" => "Cancel download",
        "downloads.failed" => "Failed",
        "downloads.next_up" => "next up",
        "downloads.starts_after" => "starts after {}",
        "downloads.added" => "Added to library · {}",
        "downloads.added_plain" => "Added to library",
        "downloads.removed_from_library" => "Removed from library",
        "downloads.already_in_library" => "Already in your library",
        "downloads.already_in_downloads" => "Already in your downloads",
        "downloads.last_tried" => "last tried {}",
        "command.pause_all_downloads" => "Pause All Downloads",
        "command.resume_all_downloads" => "Resume All Downloads",
        "command.retry_failed_downloads" => "Retry Failed Downloads",
        "command.clear_finished_downloads" => "Clear Finished Downloads",
        "command.cancel_import" => "Cancel Import",
        "command.back" => "Back",
        "command.forward" => "Forward",
        "command.open_recent" => "Open Recent Library: {name}",
        "command.clear_recents" => "Clear Recent Libraries",
        "command.clear_recent_searches" => "Clear Recent Searches",
        "welcome.empty_title" => "Your library is empty",
        "welcome.empty_body" => {
            "Point Lumina at a folder or a drive. Nothing is copied or moved: \
            Lumina reads your files where they already live and leaves them there."
        }
        "welcome.choose_folder" => "Choose a Folder…",
        "welcome.hint_watch_title" => "Watch folders",
        "welcome.hint_watch_body" => "Stays in sync as you shoot",
        "welcome.hint_formats_title" => "RAW and video",
        "welcome.hint_formats_body" => "RAW · HEIC · MOV · MP4",
        "welcome.hint_device_title" => "On-device",
        "welcome.hint_device_body" => "Your catalog and places stay on this Mac",
        "welcome.import_new" => "Import into New Library…",
        "welcome.use_pictures" => "Use Pictures Folder",
        "welcome.sample" => "Explore a Sample Library",
        "welcome.sample_preparing" => "Preparing the Sample Library…",
        "welcome.sample_busy" => "Unavailable until the sample library is ready",
        "welcome.sample_failed" => "Could not prepare the sample library",
        "welcome.sample_no_folder" => {
            "Could not prepare the sample library: this Mac has no Application Support folder"
        }
        "welcome.recent_heading" => "RECENT LIBRARIES",
        "welcome.not_available" => "Not available",
        "welcome.open_recent" => "Open {name}",
        "welcome.remove_recent" => "Remove from Recent Libraries",
        "welcome.remove_recent_named" => "Remove {name} from Recent Libraries",
        "welcome.missing_library" => {
            "Lumina can't find the library “{name}” at {path}. \
            If it is on a drive, connect the drive. If you moved it, open it from its new place."
        }
        "welcome.missing_title" => "Can't reach your library",
        "welcome.missing_body_volume" => {
            "The volume isn't mounted, or the folder has moved. Nothing has been changed."
        }
        "welcome.missing_body_folder" => {
            "The folder isn't there, or its drive isn't connected. Nothing has been changed."
        }
        "welcome.missing_body_failed" => {
            "The folder is there, but the library could not be opened. Your photos have not been changed."
        }
        "welcome.missing_retry" => "Retry",
        "welcome.missing_checking" => "Checking…",
        "welcome.missing_choose" => "Choose another library…",
        "welcome.missing_back" => "Back",
        "sidebar.albums" => "ALBUMS",
        "albums.title" => "Albums",
        "albums.tab_smart" => "Smart Albums",
        "albums.count_one" => "1 album",
        "albums.count_other" => "{n} albums",
        "albums.smart_count_one" => "1 smart album",
        "albums.smart_count_other" => "{n} smart albums",
        "albums.sort" => "Sort: {field}",
        "albums.sort_menu" => "Sort albums",
        "albums.sort_name" => "Name",
        "albums.sort_created" => "Date created",
        "albums.sort_newest" => "Newest photo",
        "albums.sort_count" => "Item count",
        "albums.empty" => "No albums yet.",
        "albums.card_menu" => "Album actions",
        "albums.card_rename" => "Rename",
        "albums.card_delete" => "Delete…",
        "albums.manage_rules" => "Manage rules",
        "albums.smart_meta" => "{items} · live",
        "albums.smart_empty" => "No smart albums yet.",
        "albums.smart_card_menu" => "Smart album actions",
        "albums.smart_edit" => "Edit…",
        "albums.more_rules" => "+{n}",
        "sidebar.pinned" => "PINNED",
        "album.cover_failed" => "Could not set the album cover",
        "album.cover_set" => "Set the album cover",
        "album.pin_failed" => "Could not pin album",
        "album.unpin_failed" => "Could not unpin album",
        "album.pinned" => "Pinned album",
        "album.unpinned" => "Unpinned album",
        "smart_album.open_first" => "Open a smart album first",
        "smart_album.copy_title" => "{title} copy",
        "smart_album.duplicate_failed" => "Could not duplicate smart album",
        "smart_album.duplicated" => "Duplicated as “{title}”",
        "smart_album.export_dialog" => "Export Smart Album Rules",
        "smart_album.import_dialog" => "Import Smart Album Rules",
        "smart_album.export_failed" => "Could not export smart album rules",
        "smart_album.exported" => "Exported the rules of “{title}”",
        "smart_album.import_failed" => "Could not import smart album rules",
        "smart_album.imported" => "Imported “{title}”",
        "smart_album.untitled" => "Untitled Smart Album",
        "smart_album.star_one" => "1 star",
        "smart_album.stars_other" => "{n} stars",
        "smart_album.rating_at_least" => "{stars} & up",
        "smart_album.rating_at_most" => "{stars} or fewer",
        "smart_album.cancel" => "Cancel",
        "smart_album.no_matches" => "No items match",
        "smart_album.matches_one" => "1 item matches",
        "smart_album.matches_other" => "{n} items match",
        "smart_album.live_preview" => "LIVE PREVIEW",
        "smart_album.preview_help" => "The first matches, updated as you edit.",
        "smart_album.pin" => "Pin to sidebar",
        "smart_album.create" => "Create Smart Album",
        "smart_album.save" => "Save Smart Album",
        "smart_album.stays_current" => "· stays up to date automatically",
        "smart_album.more" => "more",
        "smart_album.no_tags" => "No tags in this library yet",
        "smart_album.no_cameras" => "No cameras in this library yet",
        "smart_album.preview_empty" => "Nothing matches yet. The album fills in as matching items arrive.",
        "smart_album.close" => "Close without saving (Esc)",
        "smart_album.empty_title" => "Nothing matches these rules yet",
        "smart_album.empty_body" => "This album fills itself. As soon as a photo matches, it appears here.",
        "smart_album.empty_edit" => "Edit rules",
        "smart_album.empty_delete" => "Delete album",
        "search.empty_title_one" => "Nothing matches this filter",
        "search.empty_title_two" => "Nothing matches both filters",
        "search.empty_title_many" => "Nothing matches all {n} filters",
        "search.empty_body_one" => "Try a different search.",
        "search.empty_body_many" => "Try loosening one of them.",
        "search.empty_drop" => "Drop “{term}”",
        "search.empty_drop_tooltip" => "Drop {term}",
        "search.empty_clear_all" => "Clear all filters",
        "number.3" => "three",
        "number.4" => "four",
        "number.5" => "five",
        "number.6" => "six",
        "number.7" => "seven",
        "number.8" => "eight",
        "number.9" => "nine",
        "number.10" => "ten",
        "album.untitled" => "Untitled Album",
        "album.fallback_title" => "Album",
        "album.new_tooltip" => "New album (⌘N)",
        "album.rename_tooltip" => "Rename album",
        "album.delete_tooltip" => "Delete album",
        "album.row_tooltip" => "Click to open · double-click to rename",
        "album.select_first" => "Select photos or videos first",
        "album.create_failed" => "Could not create album",
        "album.finish_edit_first" => "Finish editing the photo first",
        "album.close_dialog_first" => "Close the open dialog first",
        "album.rename_cancel" => "Keep the old name",
        "album.rename_failed" => "Could not rename album",
        "album.delete_failed" => "Could not delete album",
        "album.renamed" => "Renamed album to “{title}”",
        "album.deleted" => "Deleted album",
        "album.delete_title" => "Delete “{title}”?",
        "album.delete_title_unknown" => "Delete this album?",
        "album.delete_body" => "Only the album goes. Its photos and videos stay in your library.",
        "album.delete_cancel" => "Cancel",
        "album.delete_confirm" => "Delete",
        "album.picker_title" => "Add to Album",
        "album.picker_new" => "New Album…",
        "album.picker_hint_add" => "add",
        "album.add_failed" => "Could not add to album",
        "album.remove_failed" => "Could not remove from album",
        "album.added_one" => "Added 1 item to “{album}”",
        "album.added_other" => "Added {n} items to “{album}”",
        "album.removed_one" => "Removed 1 item from “{album}”",
        "album.removed_other" => "Removed {n} items from “{album}”",
        "album.already_in" => "Already in “{album}”",
        "album.not_in" => "Not in “{album}”",
        "album.empty_title" => "“{title}” is empty",
        "album.empty_title_unknown" => "This album is empty",
        "album.empty_body" => "Items stay in your library — an album is just a view of them.",
        "trash.empty_title" => "Nothing waiting to be removed",
        // Emptying asks per run (delete_modal.rs): macOS Trash, keep on disk, or delete.
        "trash.empty_body" => "Deleted items stay here until you restore them or remove them for good. Lumina leaves their original files on disk unless you choose to move them to the macOS Trash or delete them.",
        "album.add_from_library_toast" => "Select items, then use Add to Album (⇧⌘A).",
        "sidebar.nav_label_one" => "{label}, 1 item",
        "sidebar.nav_label_other" => "{label}, {n} items",
        "sidebar.rail_tooltip" => "{label} · {n}",
        "sidebar.hide" => "Hide Sidebar",
        "sidebar.show" => "Show Sidebar",
        "sidebar.smart_albums" => "SMART ALBUMS",
        "sidebar.tags" => "TAGS",
        "sidebar.dormant_search" => "Nothing to search yet",
        "sidebar.dormant_albums" => "No albums yet",
        "sidebar.dormant_smart" => "Save a search to create one",
        "sidebar.dormant_tags" => "Tags appear as you import",
        "sidebar.unavailable" => "Unavailable until a library is open",
        "header.no_media" => "No media yet",
        "search.placeholder" => "Search library",
        "header.items_one" => "1 item",
        "header.items_other" => "{n} items",
        "header.meta" => "{items} · {range}",
        "header.clips_one" => "1 clip",
        "header.clips_other" => "{n} clips",
        "header.clips_meta" => "{clips} · {duration} · {size}",
        "header.clips_size" => "{clips} · {size}",
        "header.hours_minutes" => "{h} h {m} m",
        "header.minutes" => "{m} m",
        "header.date_range" => "{from} — {to}",
        "date.month_year" => "{month} {year}",
        "date.day_month" => "{day} {month}",
        "date.day_month_year" => "{day_month} {year}",
        "album_page.photos_one" => "1 photo",
        "album_page.photos_other" => "{n} photos",
        "album_page.videos_one" => "1 video",
        "album_page.videos_other" => "{n} videos",
        "album_page.kinds" => "{photos}, {videos}",
        "album_page.crumb_tooltip" => "Show Albums",
        "album_page.slideshow_tooltip" => "Slideshow (⌥⌘S)",
        "album_page.more" => "More album actions",
        "album_page.share_tooltip" => "Select items to share",
        "album_panel.show" => "Show Album Settings",
        "album_panel.hide" => "Hide Album Settings",
        "album_panel.key_photo" => "KEY PHOTO",
        "album_panel.key_photo_sub" => "Used as the album’s cover",
        "album_panel.no_key_photo" => "No key photo",
        "album_panel.change" => "Change",
        "album_panel.change_tooltip" => "Pick another item as the album cover",
        "album_panel.change_cancel" => "Cancel",
        "album_panel.change_disabled" => "The album has nothing to use as its cover",
        "album_panel.pick_hint" => "Click an item, or select one and press Return. Esc cancels.",
        "album_panel.delete" => "Delete Album",
        "album_panel.hint_one" => "The 1 item stays in your library.",
        "album_panel.hint_other" => "The {n} items stay in your library.",
        "album_bar.label" => "Selection actions",
        "album_bar.selected" => "{n} selected",
        "album_bar.select_all" => "Select all {n}",
        "album_bar.select_all_tooltip" => "Select every item shown (⌘A)",
        "album_bar.clear" => "Clear",
        "album_bar.clear_tooltip" => "Clear selection (Esc)",
        "album_bar.add" => "Add to Album",
        "album_bar.add_tooltip" => "Add to Album… (⇧⌘A)",
        "album_bar.tag" => "Tag",
        "album_bar.tag_tooltip" => "Tag selection (T)",
        "album_bar.set_cover" => "Set as Cover",
        "album_bar.set_cover_tooltip" => "Use the selected item as the album cover",
        "album_bar.set_cover_disabled" => "Select one item to set as the cover",
        "album_bar.export" => "Export…",
        "album_bar.export_tooltip" => "Export the selected originals…",
        "album_bar.remove" => "Remove",
        "album_bar.remove_tooltip" => "Remove from album (⌫)",
        "album_bar.delete" => "Delete",
        "album_bar.delete_tooltip" => "Move to Trash… (⌘⌫)",
        "timeline.label" => "TIMELINE",
        "timeline.scroll_to" => "Scroll to {year}",
        "timeline.scroll_to_older" => "Scroll to {year} and older",
        "timeline.month" => "{month}, {items}",
        "date.month_short_1" => "Jan",
        "date.month_short_2" => "Feb",
        "date.month_short_3" => "Mar",
        "date.month_short_4" => "Apr",
        "date.month_short_5" => "May",
        "date.month_short_6" => "Jun",
        "date.month_short_7" => "Jul",
        "date.month_short_8" => "Aug",
        "date.month_short_9" => "Sep",
        "date.month_short_10" => "Oct",
        "date.month_short_11" => "Nov",
        "date.month_short_12" => "Dec",
        "header.import" => "Import",
        "header.import_tooltip" => "Import… (⇧⌘I)",
        "header.filter" => "Filter",
        "import.failed_one" => "Imported {copied} of {total}. 1 file failed.",
        "import.failed_other" => "Imported {copied} of {total}. {n} files failed.",
        "import.failed_more" => " and {n} more",
        "filter.matches_one" => "1 match",
        "filter.matches_other" => "{n} matches",
        "filter.clear_all" => "Clear all",
        "filter.add" => "Add filter",
        "filter.remove" => "Remove {label}",
        "filter.rated" => "Rated {n}+",
        "filter.videos" => "Videos",
        "filter.photos" => "Photos",
        "filter.raw" => "RAW",
        "filter.camera" => "Camera…",
        "filter.place" => "Place…",
        "filter.date" => "Date…",
        // The Videos chip row (Design 04 Task 4).
        "filter.all_clips" => "All clips",
        "filter.4k" => "4K",
        "command.filter_videos_4k" => "Filter: 4K Videos",
        "videos.empty_title" => "No clips yet",
        "videos.empty_body" => "Videos in your library show here.",
        "videos.empty_4k_title" => "No 4K clips",
        "videos.empty_4k_body" => "None of these clips is 3840 pixels or more on its long side. Clear all to show every clip.",
        "header.view_mosaic" => "Mosaic",
        "mosaic.day_date" => "%a, %b %-d, %Y",
        "mosaic.undated" => "Undated",
        "mosaic.select" => "Select",
        "mosaic.select_updating" => "Updating…",
        "mosaic.select_day_label" => "Select {} items from {}",
        "mosaic.select_day_label_one" => "Select 1 item from {}",
        "mosaic.select_day_clips_label" => "Select {} clips from {}",
        "mosaic.select_day_clips_label_one" => "Select 1 clip from {}",
        "header.view_grid" => "Grid",
        "header.view_map" => "Map",
        "header.tags" => "Tags",
        "header.info" => "Info",
        "header.downloads" => "Downloads",
        "header.sort" => "Sort: {}",
        "search.shortcut" => "⌘F",
        "search.results_one" => "1 result",
        "search.results_other" => "{n} results",
        "search.connector_in" => "in",
        "search.connector_during" => "during",
        "search.connector_with" => "with",
        "search.connector_rated" => "rated",
        "search.rating_value" => "{n}+",
        "search.header" => "Search",
        "search.clear" => "Clear search",
        "search.narrow_by" => "Narrow by",
        "search.top_matches" => "Top matches",
        "search.match_place" => "Place",
        "search.match_album" => "Smart Album",
        "search.match_moment" => "Moment",
        "search.match_tag" => "Tag",
        "search.match_sub" => "{kind} · {items}",
        "search.match_label" => "{kind}, {title}, {items}",
        "search.narrow_fnumber" => "ƒ < 2.0",
        "search.narrow_video" => "Videos only",
        "search.narrow_rating" => "Rated 4+",
        "search.narrow_raw" => "RAW",
        "search.narrow_edited" => "Edited",
        "search.results_photos" => "Photos",
        "search.results_videos" => "Videos",
        "search.results_mixed" => "Photos & Videos",
        "search.results_sort" => "Sort by {sort}",
        "sort.date_taken" => "Date taken",
        "sort.name" => "Name",
        "sort.file_size" => "File size",
        "sort.rating" => "Rating",
        "search.save_unavailable" => "Unavailable while Compare or Edit shows",
        "command.filter_raw" => "Show RAW",
        "command.filter_edited" => "Show Edited",
        "command.filter_wide_aperture" => "Show ƒ < 2.0",
        "grid.no_preview" => "No preview",
        "grid.preview_failed" => "Lumina can't show a preview of this file. Rescan to try again.",
        "viewer.preview_failed" => "Lumina can't show a preview of this file.",
        "viewer.open_default_hint" => "Press O to open it in its default app.",
        "viewer.missing_preview" => {
            "The original file is missing — you're looking at a cached preview."
        }
        "viewer.missing_no_preview" => "The original file is missing.",
        "viewer.drive_preview" => "“{name}” is disconnected — you're looking at a cached preview.",
        "viewer.drive_no_preview" => "“{name}” is disconnected.",
        "viewer.missing_title" => "Original not available",
        "viewer.missing_body_drive" => {
            "Editing, export and full-size view need the original file. Reconnect the drive, or point Lumina at where the file lives now."
        }
        "viewer.missing_body" => {
            "Editing, export and full-size view need the original file. Point Lumina at where the file lives now."
        }
        "viewer.missing_locate" => "Locate file…",
        "viewer.missing_keep" => "Keep preview",
        "viewer.missing_dismiss" => "Dismiss",
        "playback.failed_title" => "Can't play this clip",
        "playback.failed_body" => {
            "Lumina couldn't open this video. The file may be damaged or use a format macOS can't play."
        }
        "playback.retry" => "Try Again",
        "playback.reveal" => "Reveal in Finder",
        "viewer.info_panel" => "Info",
        "view.all_media" => "All Media",
        "view.untagged" => "Untagged",
        "view.recently_deleted" => "Recently Deleted",
        "viewer.back_tooltip" => "Back to {view} (Esc)",
        "viewer.favorite_tooltip" => "Favorite (F)",
        "viewer.unfavorite_tooltip" => "Unfavorite (F)",
        "viewer.rate_tooltip" => "Rate (1–5)",
        "viewer.rate_hint" => "1–5 rate · same again or 0 clears",
        "viewer.edit_tooltip" => "Edit (E)",
        "viewer.share_tooltip" => "Share",
        "viewer.info_tooltip" => "Info (I)",
        "viewer.pip_tooltip" => "Picture in Picture (⌥P)",
        "viewer.prev_tooltip" => "Previous ([)",
        "viewer.next_tooltip" => "Next (])",
        "jobs.export_running" => "An export is already running",
        "export.videos_as_originals_one" => "{n} video exported as the original",
        "export.videos_as_originals_other" => "{n} videos exported as originals",
        "export.videos_rotation_dropped" => "({n} without Lumina's rotation)",
        "jobs.xmp_running" => "An XMP import is already running",
        "status.exporting_one" => "Exporting {n} item…",
        "status.exporting_other" => "Exporting {n} items…",
        "status.reading_xmp_one" => "Reading XMP for {n} item…",
        "status.reading_xmp_other" => "Reading XMP for {n} items…",
        "inspector.capture" => "CAPTURE",
        "inspector.rating" => "RATING",
        "inspector.location" => "LOCATION",
        "inspector.tags" => "TAGS",
        "inspector.camera" => "Camera",
        "inspector.lens" => "Lens",
        "inspector.exposure" => "Exposure",
        "inspector.focal_length" => "Focal length",
        "inspector.focal_equivalent" => "{} eq.",
        "inspector.chapters" => "CHAPTERS",
        "inspector.playback" => "PLAYBACK",
        "inspector.duration" => "Duration",
        "inspector.resolution" => "Resolution",
        "inspector.tag_add" => "Add",
        "inspector.filter_tag" => "Show items with this tag",
        "inspector.remove_tag" => "Remove tag",
        "inspector.get_info" => "Get Info…",
        "inspector.get_info_tooltip" => "Open the full inspector window",
        "inspector.reveal" => "Reveal",
        "inspector.nothing_selected" => "Nothing selected.",
        "inspector.star_one" => "1 star",
        "inspector.star_other" => "{} stars",
        "inspector.format_label" => "{} file",
        "inspector.format_label_dims" => "{} file, {} pixels",
        "inspector.tag_label" => "Tag {}",
        "inspector.tag_add_label" => "Add a tag",
        "inspector.filter_tag_label" => "Show items tagged {}",
        "inspector.remove_tag_label" => "Remove tag {}",
        "inspector.open_map" => "Open map",
        "inspector.open_map_tooltip" => "Show on the Places map (⌘K Show on Map)",
        "inspector.location_map_label" => "Map: {}",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_keys() {
        assert_eq!(t("command.preferences"), "Preferences…");
        assert_eq!(t("missing.key"), "missing.key");
        assert_eq!(current_locale(), Locale::EnUs);
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
        assert_eq!(items(0), "0 items");
        assert_eq!(items(1), "1 item");
        assert_eq!(items(1_284_503), "1,284,503 items");
    }

    #[test]
    fn import_failed_is_singular_for_one_and_groups_thousands() {
        assert_eq!(import_failed(2, 3, 1), "Imported 2 of 3. 1 file failed.");
        assert_eq!(
            import_failed(1_284, 1_310, 26),
            "Imported 1,284 of 1,310. 26 files failed."
        );
    }

    #[test]
    fn clips_meta_reads_count_hours_minutes_and_size() {
        assert_eq!(clips(1), "1 clip");
        assert_eq!(clips(4_183), "4,183 clips");
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
        assert_eq!(clips_meta(2, 180_000, "1.9 MB"), "2 clips · 3 m · 1.9 MB");
        assert_eq!(
            t("header.clips_size"),
            "{clips} · {size}",
            "the no-duration line has its own key, not the date-range one"
        );
        assert_eq!(
            clips_meta(1, 0, "12 B"),
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
    }
}
