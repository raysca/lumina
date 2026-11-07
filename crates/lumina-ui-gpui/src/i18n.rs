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
        "command.rate_1" => "Rate 1 Star",
        "command.rate_2" => "Rate 2 Stars",
        "command.rate_3" => "Rate 3 Stars",
        "command.rate_4" => "Rate 4 Stars",
        "command.rate_5" => "Rate 5 Stars",
        "command.clear_rating" => "Clear Rating",
        "command.flag_pick" => "Pick",
        "command.flag_reject" => "Reject",
        "command.flag_clear" => "Remove Flag",
        "command.delete_rejected" => "Delete Rejected…",
        "command.compare" => "Compare",
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
