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
        "command.about" => "About Lumina",
        "command.empty_trash" => "Empty Trash…",
        "command.restore_all" => "Restore All from Trash",
        "command.merge_all_duplicates" => "Merge All Duplicates",
        "command.retry_error" => "Retry Failed Action",
        "command.show_license" => "Show MIT License",
        "command.show_acknowledgements" => "Show Acknowledgements",
        "command.shortcuts" => "Keyboard Shortcuts",
        "command.toggle_inspector" => "Toggle Inspector",
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
        "duplicates.tab_exact" => "Exact copies",
        "duplicates.tab_exact_hint" => "Files with identical content",
        "duplicates.tab_similar" => "Similar shots",
        "duplicates.tab_similar_hint" => "RAW + JPEG pairs and bursts (⌘K Show Similar Shots)",
        "command.show_map" => "Show Map",
        "command.show_grid" => "Show Grid",
        "command.show_mosaic" => "Show Mosaic",
        "command.select_day" => "Select Day",
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
        "command.set_loop_start" => "Set Loop Start",
        "command.set_loop_end" => "Set Loop End",
        "command.clear_loop_range" => "Clear Loop Range",
        "video.loop_start_set" => "Loop start set to {time}",
        "video.loop_end_set" => "Loop end set to {time}",
        "video.loop_start_after_end" => "The loop start must come before the loop end",
        "video.loop_end_before_start" => "The loop end must come after the loop start",
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
        "command.open_tag_manager" => "Open Tag Manager…",
        "command.add_keyword_alias" => "Add Alias for Keyword…",
        "command.move_keyword" => "Move Keyword…",
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
        "tags.delete_note" => {
            "Removes this keyword and the keywords below it from {items}. The photos stay."
        }
        "command.edit_smart_album" => "Edit Smart Album…",
        "command.delete_smart_album" => "Delete Smart Album…",
        "command.locate_missing" => "Locate Missing File…",
        "command.import_media" => "Import…",
        "command.import_network_video" => "Import Video from URL…",
        "command.toggle_downloads" => "Show Downloads",
        "command.cancel_import" => "Cancel Import",
        "command.back" => "Back",
        "command.forward" => "Forward",
        "command.open_recent" => "Open Recent Library: {name}",
        "command.clear_recents" => "Clear Recent Libraries",
        "command.clear_recent_searches" => "Clear Recent Searches",
        "welcome.title" => "Welcome to Lumina",
        "welcome.intro" => {
            "A fast, private browser for the photos and videos on your Mac: \
            JPEG, HEIC, RAW and video."
        }
        "welcome.in_place_title" => "Works where your photos are",
        "welcome.in_place_body" => {
            "Open a folder and Lumina shows the photos where they already are. \
            Or start a new library, and Lumina copies photos into it."
        }
        "welcome.originals_title" => "Leaves your originals alone",
        "welcome.originals_body" => {
            "Lumina never edits your original files, and nothing is uploaded."
        }
        "welcome.catalog_title" => "Keeps its catalog on this Mac",
        "welcome.catalog_body" => {
            "Dates, tags and albums are kept in Lumina's catalog in Application Support."
        }
        "welcome.open_existing" => "Open Existing Library…",
        "welcome.import_new" => "Import into New Library…",
        "welcome.use_pictures" => "Use Pictures Folder",
        "welcome.sample" => "Explore a Sample Library",
        "welcome.sample_preparing" => "Preparing the Sample Library…",
        "welcome.sample_failed" => "Could not prepare the sample library",
        "welcome.sample_no_folder" => {
            "Could not prepare the sample library: this Mac has no Application Support folder"
        }
        "welcome.footer" => "You can open another library later with File > Open Library… (⌘O).",
        "welcome.recent_heading" => "Recent Libraries",
        "welcome.not_available" => "Not available",
        "welcome.open_recent" => "Open {name}",
        "welcome.remove_recent" => "Remove from Recent Libraries",
        "welcome.remove_recent_named" => "Remove {name} from Recent Libraries",
        "welcome.missing_library" => {
            "Lumina can't find the library “{name}” at {path}. \
            If it is on a drive, connect the drive. If you moved it, open it from its new place."
        }
        "sidebar.albums" => "ALBUMS",
        "sidebar.pinned" => "PINNED",
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
        "album.empty_body" => "Photos and videos you put in this album show here.",
        "sidebar.nav_label_one" => "{label}, 1 item",
        "sidebar.nav_label_other" => "{label}, {n} items",
        "sidebar.rail_tooltip" => "{label} · {n}",
        "sidebar.hide" => "Hide Sidebar",
        "sidebar.show" => "Show Sidebar",
        "search.placeholder" => "Search library",
        "header.items_one" => "1 item",
        "header.items_other" => "{n} items",
        "header.meta" => "{items} · {range}",
        "header.date_range" => "{from} — {to}",
        "date.month_year" => "{month} {year}",
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
        "header.view_mosaic" => "Mosaic",
        "mosaic.day_date" => "%a, %b %-d, %Y",
        "mosaic.undated" => "Undated",
        "mosaic.select" => "Select",
        "mosaic.select_updating" => "Updating…",
        "mosaic.select_day_label" => "Select {} items from {}",
        "mosaic.select_day_label_one" => "Select 1 item from {}",
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
