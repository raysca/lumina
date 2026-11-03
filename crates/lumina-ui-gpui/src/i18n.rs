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
