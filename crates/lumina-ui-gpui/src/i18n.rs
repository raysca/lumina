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
        "command.toggle_sidebar" => "Toggle Sidebar",
        "command.toggle_inspector" => "Toggle Inspector",
        "command.get_info" => "Get Info",
        "command.export" => "Export Selection…",
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
        "command.filter_all" => "Filter: All",
        "command.filter_favorites" => "Filter: Favorites",
        "command.filter_videos" => "Filter: Videos",
        "command.filter_three_stars" => "Filter: ★ 3+",
        "command.filter_labeled" => "Filter: Labeled",
        "command.rate_1" => "Rate 1 Star",
        "command.rate_2" => "Rate 2 Stars",
        "command.rate_3" => "Rate 3 Stars",
        "command.rate_4" => "Rate 4 Stars",
        "command.rate_5" => "Rate 5 Stars",
        "command.clear_rating" => "Clear Rating",
        "command.toggle_favorite" => "Toggle Favorite",
        "command.toggle_pip" => "Toggle Picture-in-Picture",
        "command.pip_play_pause" => "Play/Pause Picture-in-Picture",
        "command.new_smart_album" => "New Smart Album…",
        "command.adjust_date" => "Adjust Date Taken…",
        "command.edit_location" => "Edit Location…",
        "command.remove_location" => "Remove Location",
        "command.open_tag_manager" => "Open Tag Manager…",
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
        "grid.no_preview" => "No preview",
        "grid.preview_failed" => "Lumina can't show a preview of this file. Rescan to try again.",
        "viewer.preview_failed" => "Lumina can't show a preview of this file.",
        "viewer.open_default_hint" => "Press O to open it in its default app.",
        "jobs.export_running" => "An export is already running",
        "jobs.xmp_running" => "An XMP import is already running",
        "status.exporting_one" => "Exporting {n} item…",
        "status.exporting_other" => "Exporting {n} items…",
        "status.reading_xmp_one" => "Reading XMP for {n} item…",
        "status.reading_xmp_other" => "Reading XMP for {n} items…",
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
}
