//! The command table.

use crate::views::viewer_state::ViewerZoomPreset;

use super::{Command, CommandId};

/// Shorthand for one row of [`commands`].
const fn cmd(
    id: CommandId,
    key: &'static str,
    shortcut: &'static str,
    keywords: &'static str,
) -> Command {
    Command {
        id,
        key,
        shortcut,
        keywords,
    }
}

/// Every palette command in listing order: id, catalog key, shortcut and
/// search keywords, one row each.
#[rustfmt::skip]
pub fn commands() -> &'static [Command] {
    use CommandId::*;
    const TABLE: &[Command] = &[
        cmd(OpenLibrary, "command.open_library", "⌘O", "folder choose switch library"),
        cmd(ImportMedia, "command.import_media", "⇧⌘I", "import copy add files folder card camera"),
        cmd(ImportNetworkVideo, "command.import_network_video", "⌥⌘U", "import video url youtube hls download stream network link web"),
        cmd(ToggleDownloads, "command.toggle_downloads", "⌥⌘L", "downloads queue progress url video"),
        cmd(ShowAllDownloads, "command.show_all_downloads", "", "downloads screen list queue url video"),
        cmd(PauseAllDownloads, "command.pause_all_downloads", "", "downloads stop queue url video"),
        cmd(ResumeAllDownloads, "command.resume_all_downloads", "", "downloads continue start queue url video"),
        cmd(RetryFailedDownloads, "command.retry_failed_downloads", "", "downloads again error try queue"),
        cmd(ClearFinishedDownloads, "command.clear_finished_downloads", "", "downloads done completed remove list"),
        cmd(CancelImport, "command.cancel_import", "", "import stop abort copy"),
        cmd(Rescan, "command.rescan", "⌘R", "refresh index scan"),
        cmd(Preferences, "command.preferences", "⌘,", "settings preferences config cache"),
        cmd(SettingsDownloads, "command.settings_downloads", "", "preferences concurrency simultaneous url video"),
        cmd(ResetPreferences, "command.reset_preferences", "", "settings preferences restore default layout sort sidebar columns"),
        cmd(About, "command.about", "", "version info"),
        cmd(EmptyTrash, "command.empty_trash", "", "trash recently deleted delete permanently"),
        cmd(RestoreAll, "command.restore_all", "", "trash recently deleted undelete recover"),
        cmd(MergeAllDuplicates, "command.merge_all_duplicates", "", "duplicate duplicates merge clean dedup"),
        cmd(SkipDuplicateGroup, "command.skip_group", "S", "duplicates skip group later"),
        cmd(MergeDuplicateGroup, "command.merge_group", "↵", "duplicates merge group keep copy"),
        cmd(AutoSelectBest, "command.auto_select_best", "", "duplicates keeper best copy reset"),
        cmd(SkipAllDuplicateGroups, "command.skip_all_groups", "", "duplicates skip all groups later"),
        cmd(ShowExactCopies, "command.show_exact_copies", "", "duplicates exact copies identical"),
        cmd(ShowSimilarShots, "command.show_similar_shots", "", "duplicates similar shots stacks raw jpeg pairs bursts"),
        cmd(ShowAlbums, "command.show_albums", "", "albums overview collections folders go"),
        cmd(ShowAlbumsTab, "command.show_albums_tab", "", "albums overview tab"),
        cmd(ShowSmartAlbumsTab, "command.show_smart_albums_tab", "", "albums overview tab smart rules"),
        // Before Show Map: "show on map" matches both, and the item wins.
        cmd(ShowOnMap, "command.show_on_map", "", "map places location gps where pin open"),
        cmd(ShowMap, "command.show_map", "", "map places location gps world where"),
        cmd(ShowGrid, "command.show_grid", "", "grid places photos thumbnails map"),
        cmd(SelectDay, "command.select_day", "", "select day date mosaic all items"),
        cmd(ShowYears, "command.show_years", "", "years zoom group mosaic timeline overview"),
        cmd(ShowMonths, "command.show_months", "", "months zoom group mosaic timeline overview"),
        cmd(ShowDays, "command.show_days", "", "days zoom group mosaic timeline"),
        cmd(ShowMosaic, "command.show_mosaic", "", "mosaic layout justified rows days photos thumbnails"),
        cmd(RetryError, "command.retry_error", "", "error again failed"),
        cmd(ShowLicense, "command.show_license", "", "license mit legal about"),
        cmd(ShowAcknowledgements, "command.show_acknowledgements", "", "acknowledgements credits third party notices licenses about"),
        cmd(CheckForUpdates, "command.check_for_updates", "", "update upgrade new version latest release download"),
        cmd(RestartToUpdate, "command.restart_to_update", "", "update upgrade install relaunch new version"),
        cmd(AutoUpdatesOn, "command.auto_updates_on", "", "update automatic daily check enable settings"),
        cmd(AutoUpdatesOff, "command.auto_updates_off", "", "update automatic daily check disable settings"),
        cmd(Shortcuts, "command.shortcuts", "⌘/", "help keys"),
        // Titled by state in `command_title`: Hide or Show Sidebar.
        cmd(ToggleSidebar, "command.toggle_sidebar", crate::app::panels::SIDEBAR_SHORTCUT, "panel hide show toggle sidebar"),
        cmd(ToggleInspector, "command.toggle_inspector", "I", "panel info hide show"),
        // Titled by state in `command_title`: Hide or Show Album Settings.
        cmd(ToggleAlbumPanel, "command.toggle_album_panel", "", "panel album settings hide show"),
        cmd(GetInfo, "command.get_info", "⌘I", "info window details metadata"),
        // Before Export as JPEG…, whose keywords also say "share".
        cmd(Share, "command.share", "", "send airdrop mail messages"),
        cmd(ExportSelection, "command.export", "", "download copy files xmp originals"),
        cmd(ExportJpeg, "command.export_jpeg", "", "download convert jpg smaller share"),
        cmd(ImportXmp, "command.import_xmp", "", "sidecar metadata read"),
        cmd(SortDate, "command.sort_date", "", "order newest oldest"),
        cmd(SortName, "command.sort_name", "", "order alphabetical filename"),
        cmd(SortSize, "command.sort_size", "", "order bytes disk"),
        cmd(SortRating, "command.sort_rating", "", "order stars"),
        cmd(SortAsc, "command.sort_asc", "", "direction increasing"),
        cmd(SortDesc, "command.sort_desc", "", "direction decreasing"),
        cmd(ToggleFit, "command.toggle_fit", "", "cards square aspect"),
        cmd(ZoomInGrid, "command.zoom_in", "⌘+", "columns density bigger"),
        cmd(ZoomOutGrid, "command.zoom_out", "⌘-", "columns density smaller"),
        cmd(ViewerZoom(ViewerZoomPreset::Fit), "command.zoom_fit", "⌘0", "zoom viewer fit window whole reset"),
        cmd(ViewerZoom(ViewerZoomPreset::Actual), "command.zoom_actual", "⌘1", "zoom viewer 100% 1:1 pixels actual"),
        cmd(ViewerZoom(ViewerZoomPreset::TwoX), "command.zoom_200", "⌘2", "zoom viewer 200% 2x magnify"),
        cmd(ViewerZoom(ViewerZoomPreset::Fill), "command.zoom_fill", "⌘3", "zoom viewer fill crop cover"),
        cmd(ViewerZoom(ViewerZoomPreset::FourX), "command.zoom_400", "⌘4", "zoom viewer 400% 4x magnify"),
        cmd(RateMenu, "command.rate_menu", "", "rating stars popover viewer"),
        cmd(Rate(1), "command.rate_1", "1", "rating stars one"),
        cmd(Rate(2), "command.rate_2", "2", "rating stars two"),
        cmd(Rate(3), "command.rate_3", "3", "rating stars three"),
        cmd(Rate(4), "command.rate_4", "4", "rating stars four"),
        cmd(Rate(5), "command.rate_5", "5", "rating stars five"),
        cmd(Rate(0), "command.clear_rating", "0", "rating stars remove unrate"),
        cmd(Flag(lumina_core::Flag::Pick), "command.flag_pick", "P", "flag cull keep select"),
        cmd(Flag(lumina_core::Flag::Reject), "command.flag_reject", "X", "flag cull discard"),
        cmd(Flag(lumina_core::Flag::Unflagged), "command.flag_clear", "U", "flag cull unflag unpick unreject remove"),
        cmd(DeleteRejected, "command.delete_rejected", "", "flag cull reject rejects rejected trash remove"),
        cmd(Compare, "command.compare", "C", "compare survey side by side cull"),
        cmd(Slideshow, "command.slideshow", "⌥⌘S", "slideshow slide show play present full screen"),
        cmd(SlideshowLoop, "command.slideshow_loop", "", "slideshow loop repeat wrap"),
        cmd(SlideshowInterval(3), "command.slideshow_interval_3", "", "slideshow interval seconds speed timing"),
        cmd(SlideshowInterval(5), "command.slideshow_interval_5", "", "slideshow interval seconds speed timing"),
        cmd(SlideshowInterval(10), "command.slideshow_interval_10", "", "slideshow interval seconds speed timing"),
        cmd(ToggleFavorite, "command.toggle_favorite", "F", "heart star like"),
        cmd(RotateLeft, "command.rotate_left", "⇧R", "turn counterclockwise anticlockwise orientation image photo"),
        cmd(RotateRight, "command.rotate_right", "R", "turn clockwise orientation image photo"),
        cmd(EditPhoto, "command.edit_photo", "E", "adjust exposure brightness auto enhance quick edit image"),
        cmd(RevertToOriginal, "command.revert_to_original", "", "reset undo remove edits rotation exposure image photo"),
        cmd(EditInExternalEditor, "command.edit_external", "⌘E", "open app application editor retouch image photo"),
        cmd(EditCopyInExternalEditor, "command.edit_copy_external", "⌥⌘E", "duplicate open app application editor image photo"),
        // After the Rate commands: "rate 3" also matches "Show Rated ★ 3+".
        cmd(QuickFilter("is:favorite"), "command.filter_favorites", "", "filter hearts loved favourites"),
        cmd(QuickFilter("is:pick"), "command.filter_picks", "", "filter flag picked cull"),
        cmd(QuickFilter("is:reject"), "command.filter_rejected", "", "filter flag rejects cull"),
        cmd(QuickFilter("is:video"), "command.filter_videos", "", "filter movies clips"),
        cmd(Videos4k, "command.filter_videos_4k", "", "filter movies clips uhd 2160p resolution"),
        // These quick filters mirror the search header's Narrow by chips.
        cmd(QuickFilter("is:edited"), "command.filter_edited", "", "filter edited cropped rotated adjusted retouched"),
        cmd(QuickFilter("is:raw"), "command.filter_raw", "", "filter raw camera files negatives"),
        cmd(QuickFilter("f:<2"), "command.filter_wide_aperture", "", "filter aperture fnumber wide fast lens bokeh"),
        cmd(QuickFilter("rating:>=1"), "command.filter_rating_1", "", "filter rating stars 1"),
        cmd(QuickFilter("rating:>=2"), "command.filter_rating_2", "", "filter rating stars 2"),
        cmd(QuickFilter("rating:>=3"), "command.filter_rating_3", "", "filter rating stars 3"),
        cmd(QuickFilter("rating:>=4"), "command.filter_rating_4", "", "filter rating stars 4"),
        cmd(QuickFilter("rating:>=5"), "command.filter_rating_5", "", "filter rating stars 5"),
        cmd(QuickFilter("is:unrated"), "command.filter_unrated", "", "filter rating stars none no"),
        cmd(QuickFilter("color:red"), "command.filter_color_red", "", "filter colour color label"),
        cmd(QuickFilter("color:orange"), "command.filter_color_orange", "", "filter colour color label"),
        cmd(QuickFilter("color:yellow"), "command.filter_color_yellow", "", "filter colour color label"),
        cmd(QuickFilter("color:green"), "command.filter_color_green", "", "filter colour color label"),
        cmd(QuickFilter("color:blue"), "command.filter_color_blue", "", "filter colour color label"),
        cmd(QuickFilter("color:purple"), "command.filter_color_purple", "", "filter colour color label"),
        cmd(QuickFilter("is:unlabeled"), "command.filter_unlabeled", "", "filter colour color label none no"),
        cmd(AddFilter, "command.add_filter", "", "filter menu narrow refine"),
        cmd(ClearFilters, "command.filter_clear", "", "filter remove reset show everything all"),
        cmd(ClearSearch, "command.clear_search", "", "search query empty reset remove all filters"),
        cmd(TogglePip, "command.toggle_pip", "⌥P", "pip picture in picture video float detach"),
        cmd(PipPlayPause, "command.pip_play_pause", "", "pip picture in picture video pause resume"),
        cmd(SetLoopStart, "command.set_loop_start", "⌥I", "video loop range in point start"),
        cmd(SetLoopEnd, "command.set_loop_end", "⌥O", "video loop range out point end"),
        cmd(ClearLoopRange, "command.clear_loop_range", "⌥X", "video loop range remove reset"),
        cmd(VolumeUp, "command.volume_up", "↑", "video sound louder audio"),
        cmd(VolumeDown, "command.volume_down", "↓", "video sound quieter softer audio"),
        cmd(PreviousFrame, "command.previous_frame", ",", "video frame step back"),
        cmd(NextFrame, "command.next_frame", ".", "video frame step forward"),
        cmd(NewAlbum, "command.new_album", "⌘N", "album create collection"),
        cmd(NewAlbumWithSelection, "command.new_album_with_selection", "", "album create collection selected items"),
        cmd(RenameAlbum, "command.rename_album", "", "album name title change"),
        cmd(DeleteAlbum, "command.delete_album", "", "album remove delete"),
        cmd(AddToAlbum, "command.add_to_album", "⇧⌘A", "album add put collection selected items"),
        cmd(RemoveFromAlbum, "command.remove_from_album", "⌫", "album remove take out collection selected items"),
        cmd(SetAlbumCover, "command.set_album_cover", "", "album cover key photo thumbnail set selected item"),
        cmd(AddFromLibrary, "command.add_from_library", "", "album add library pick choose items empty"),
        cmd(PinAlbum, "command.pin_album", "", "album smart pin pinned top favorite keep first"),
        cmd(UnpinAlbum, "command.unpin_album", "", "album smart unpin pinned remove"),
        cmd(DuplicateSmartAlbum, "command.duplicate_smart_album", "", "smart album copy duplicate clone rules"),
        cmd(ExportSmartAlbumRules, "command.export_smart_album_rules", "", "smart album rules export save file json share"),
        cmd(ImportSmartAlbumRules, "command.import_smart_album_rules", "", "smart album rules import open file json"),
        cmd(NewSmartAlbum, "command.new_smart_album", "⌥⌘N", "smart album rules filter create"),
        cmd(SaveSearch, "command.save_search", "", "save search query smart album saved"),
        cmd(EditSmartAlbum, "command.edit_smart_album", "", "smart album rules filter change"),
        cmd(DeleteSmartAlbum, "command.delete_smart_album", "", "smart album remove delete"),
        cmd(LocateMissing, "command.locate_missing", "", "missing file find relink reconnect moved"),
        cmd(AdjustDate, "command.adjust_date", "", "date taken time edit change shift offset get info"),
        cmd(EditLocation, "command.edit_location", "", "location gps coordinates place edit get info"),
        cmd(RemoveLocation, "command.remove_location", "", "location gps coordinates place clear delete"),
        cmd(OpenTagManager, "command.open_tag_manager", "", "tag manager keywords manage rename"),
        cmd(SortTags, "command.sort_tags", "", "tags keywords sort order most used count alphabetical a-z"),
        cmd(AddKeywordAlias, "command.add_keyword_alias", "", "tag keyword alias synonym"),
        cmd(MoveKeyword, "command.move_keyword", "", "tag keyword move parent hierarchy reparent"),
        cmd(MergeKeyword, "command.merge_keyword", "", "tag keyword merge combine into alias"),
        cmd(SaveKeywordSmart, "command.save_keyword_smart", "", "tag keyword save smart album rule"),
        cmd(RenameKeyword, "command.rename_keyword", "", "tag keyword rename name"),
        cmd(DeleteKeyword, "command.delete_keyword", "", "tag keyword delete remove"),
        cmd(Back, "command.back", "⌘[", "history previous go return"),
        cmd(Forward, "command.forward", "⌘]", "history next go redo"),
        cmd(ApplyTemplate, "command.apply_template", "", "metadata template preset rights creator copyright apply"),
        cmd(ManageTemplates, "command.manage_templates", "", "metadata template preset rights creator copyright edit new delete"),
        cmd(ClearRecents, "command.clear_recents", "", "forget remove history libraries"),
        cmd(ClearRecentSearches, "command.clear_recent_searches", "", "forget remove history search queries"),
    ];
    TABLE
}

#[cfg(test)]
mod table_tests {
    use super::commands;

    /// Pins the command table: count, order, ids, catalog keys, shortcuts
    /// and keywords. A deliberate change updates the expected values.
    #[core::prelude::v1::test]
    fn command_table_is_pinned() {
        let table = commands();
        let rows: Vec<String> = table
            .iter()
            .map(|c| format!("{:?}|{}|{}|{}", c.id, c.key, c.shortcut, c.keywords))
            .collect();
        // FNV-1a: stable across Rust releases, unlike DefaultHasher.
        let hash = rows
            .join("\n")
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        assert_eq!(table.len(), 157, "command count changed");
        assert_eq!(
            hash,
            15_117_093_722_329_621_587,
            "command table changed:\n{}",
            rows.join("\n")
        );
    }
}
