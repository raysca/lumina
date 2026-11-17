//! Running a command.

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::views::inspector::InspectorTab;
use crate::views::viewer_state::ViewerRotation;

use super::CommandId;
use crate::app::LuminaApp;

impl LuminaApp {
    pub fn execute_command(&mut self, id: CommandId, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.command_query.clear();
        match id {
            CommandId::OpenLibrary => self.pick_folder_dialog(cx),
            CommandId::Rescan => self.rescan_library(cx),
            CommandId::Preferences => self.open_preferences(cx),
            CommandId::ResetPreferences => self.prompt_reset_preferences(cx),
            CommandId::SettingsDownloads => {
                self.open_preferences_at(crate::app::preferences::PreferencesTab::Downloads, cx)
            }
            CommandId::About => self.open_about(cx),
            CommandId::ShowLicense => self.open_bundled_doc("LICENSE", cx),
            CommandId::EmptyTrash => self.prompt_empty_trash(cx),
            CommandId::RestoreAll => self.restore_all_trash(cx),
            CommandId::MergeAllDuplicates => self.merge_all_duplicates(cx),
            CommandId::SkipDuplicateGroup => self.duplicates_skip_shown(cx),
            CommandId::MergeDuplicateGroup => self.duplicates_merge_shown(cx),
            CommandId::AutoSelectBest => self.duplicates_auto_best(cx),
            CommandId::SkipAllDuplicateGroups => self.duplicates_skip_all(cx),
            CommandId::ShowExactCopies => {
                self.show_duplicates_tab(crate::app::stacks::DuplicatesTab::Exact, cx)
            }
            CommandId::ShowSimilarShots => {
                self.show_duplicates_tab(crate::app::stacks::DuplicatesTab::Similar, cx)
            }
            CommandId::ShowMap => self.show_map(cx),
            CommandId::ShowAlbums => self.show_albums_overview(cx),
            CommandId::ShowAlbumsTab => {
                self.show_albums_tab(crate::app::albums_overview::AlbumsTab::Albums, cx)
            }
            CommandId::ShowSmartAlbumsTab => {
                self.show_albums_tab(crate::app::albums_overview::AlbumsTab::SmartAlbums, cx)
            }
            CommandId::ShowGrid => self.set_browse_layout(crate::config::BrowseLayout::Grid, cx),
            CommandId::ShowMosaic => {
                self.set_browse_layout(crate::config::BrowseLayout::Mosaic, cx)
            }
            CommandId::SelectDay => self.select_mosaic_day(cx),
            CommandId::ShowYears => self.set_group_level(crate::config::GroupLevel::Years, cx),
            CommandId::ShowMonths => self.set_group_level(crate::config::GroupLevel::Months, cx),
            CommandId::ShowDays => self.set_group_level(crate::config::GroupLevel::Days, cx),
            CommandId::ShowOnMap => self.show_item_on_map(cx),
            CommandId::RetryError => self.retry_error(cx),
            CommandId::ShowAcknowledgements => self.open_bundled_doc("THIRD_PARTY_NOTICES.md", cx),
            CommandId::Shortcuts => self.toggle_shortcuts_overlay(cx),
            CommandId::ToggleSidebar => self.toggle_sidebar(cx),
            CommandId::ToggleInspector => self.toggle_inspector(cx),
            CommandId::ToggleAlbumPanel => self.toggle_album_panel(cx),
            CommandId::GetInfo => self.get_info(cx),
            CommandId::ExportSelection => {
                self.export_pick_folder(lumina_core::ExportPreset::Originals, cx)
            }
            CommandId::ExportJpeg => {
                self.export_pick_folder(lumina_core::ExportPreset::Jpeg2048, cx)
            }
            // Anchored at the window centre: the palette has no item to point at.
            #[cfg(target_os = "macos")]
            CommandId::Share => self.share_selection(None, cx),
            #[cfg(not(target_os = "macos"))]
            CommandId::Share => {}
            CommandId::ImportXmp => self.import_sidecars_selected(cx),
            CommandId::SortDate => self.set_sort_field(SortField::DateTaken, cx),
            CommandId::SortName => self.set_sort_field(SortField::FileName, cx),
            CommandId::SortSize => self.set_sort_field(SortField::FileSize, cx),
            CommandId::SortRating => self.set_sort_field(SortField::Rating, cx),
            CommandId::SortAsc => self.set_sort_direction(SortDirection::Ascending, cx),
            CommandId::SortDesc => self.set_sort_direction(SortDirection::Descending, cx),
            CommandId::ToggleFit => {
                self.card_fit_mode = !self.card_fit_mode;
                self.save_config();
                cx.notify();
            }
            // On the map they zoom the map, as ⌘= / ⌘- do.
            CommandId::ZoomInGrid if self.map_shown() => self.zoom_map(1, cx),
            CommandId::ZoomOutGrid if self.map_shown() => self.zoom_map(-1, cx),
            CommandId::ZoomInGrid => self.zoom_in_grid(cx),
            CommandId::ZoomOutGrid => self.zoom_out_grid(cx),
            CommandId::ViewerZoom(preset) => self.apply_viewer_zoom_preset(preset, cx),
            CommandId::QuickFilter(token) => self.toggle_quick_filter(token, cx),
            CommandId::Videos4k => self.toggle_videos_4k(cx),
            CommandId::ClearFilters => self.clear_quick_filters(cx),
            CommandId::ClearSearch => self.clear_search(cx),
            CommandId::AddFilter => self.open_filter_menu(false, cx),
            CommandId::Rate(rating) => {
                self.set_rating_selected(rating, cx);
            }
            CommandId::RateMenu => self.toggle_rate_popover(cx),
            CommandId::Flag(flag) => {
                self.set_flag_selected(flag, cx);
            }
            CommandId::DeleteRejected => self.delete_rejected(cx),
            CommandId::Compare => self.open_compare(cx),
            CommandId::Slideshow => self.start_slideshow_deferred(cx),
            CommandId::SlideshowLoop => self.toggle_slideshow_loop(cx),
            CommandId::SlideshowInterval(secs) => self.set_slideshow_interval(secs, cx),
            CommandId::ToggleFavorite if self.selected_indices.len() > 1 => {
                self.batch_toggle_favorite(cx)
            }
            CommandId::ToggleFavorite => self.toggle_favorite_selected(cx),
            CommandId::RotateLeft => self.rotate_selection(ViewerRotation::Left, cx),
            CommandId::RotateRight => self.rotate_selection(ViewerRotation::Right, cx),
            CommandId::EditPhoto => self.enter_edit_mode(cx),
            CommandId::RevertToOriginal => self.revert_selection_to_original(cx),
            CommandId::EditInExternalEditor => self.edit_selected_externally(false, cx),
            CommandId::EditCopyInExternalEditor => self.edit_selected_externally(true, cx),
            CommandId::TogglePip => self.toggle_pip(cx),
            CommandId::PipPlayPause => self.toggle_pip_playback(cx),
            CommandId::SetLoopStart => self.set_loop_in(cx),
            CommandId::SetLoopEnd => self.set_loop_out(cx),
            CommandId::ClearLoopRange => self.clear_loop_range(cx),
            CommandId::VolumeUp => self.nudge_video_volume(true, cx),
            CommandId::VolumeDown => self.nudge_video_volume(false, cx),
            CommandId::PreviousFrame => self.step_video_frame(-1, cx),
            CommandId::NextFrame => self.step_video_frame(1, cx),
            CommandId::NewAlbum => self.new_album(cx),
            CommandId::NewAlbumWithSelection => self.new_album_with_selection(cx),
            CommandId::RenameAlbum => self.rename_selected_album(cx),
            CommandId::DeleteAlbum => self.delete_selected_album(cx),
            CommandId::AddToAlbum => self.open_album_picker(cx),
            CommandId::RemoveFromAlbum => self.remove_selection_from_album(cx),
            CommandId::SetAlbumCover => self.set_album_cover_from_selection(cx),
            CommandId::AddFromLibrary => self.add_from_library(cx),
            CommandId::PinAlbum => self.set_open_album_pinned(true, cx),
            CommandId::UnpinAlbum => self.set_open_album_pinned(false, cx),
            CommandId::DuplicateSmartAlbum => self.duplicate_selected_smart_album(cx),
            CommandId::ExportSmartAlbumRules => self.export_selected_smart_album_rules(cx),
            CommandId::ImportSmartAlbumRules => self.import_smart_album_rules(cx),
            CommandId::NewSmartAlbum => self.open_album_builder(cx),
            CommandId::SaveSearch => self.save_current_search_as_album(cx),
            CommandId::EditSmartAlbum => self.edit_selected_smart_album(cx),
            CommandId::DeleteSmartAlbum => self.delete_selected_smart_album(cx),
            CommandId::LocateMissing => self.locate_missing_selected(cx),
            CommandId::ImportMedia => self.pick_import_folders(cx),
            CommandId::ImportNetworkVideo => self.open_network_import_dialog(cx),
            CommandId::ToggleDownloads => self.toggle_downloads_panel(cx),
            CommandId::ShowAllDownloads => self.show_downloads_screen(cx),
            CommandId::PauseAllDownloads => self.pause_all_downloads(cx),
            CommandId::ResumeAllDownloads => self.resume_all_downloads(cx),
            CommandId::RetryFailedDownloads => self.retry_all_downloads(cx),
            CommandId::ClearFinishedDownloads => self.clear_finished_downloads(cx),
            CommandId::CancelImport => self.cancel_running_import(cx),
            CommandId::AdjustDate => {
                self.open_get_info_at(InspectorTab::Info, cx);
                self.start_date_input(cx);
            }
            CommandId::EditLocation => {
                self.open_get_info_at(InspectorTab::Location, cx);
                self.start_location_input(cx);
            }
            CommandId::RemoveLocation if self.selected_indices.len() <= 1 => {
                self.remove_location_selected(cx)
            }
            CommandId::RemoveLocation => self.set_error(
                "Remove Location works on one item at a time.".to_string(),
                cx,
            ),
            CommandId::OpenTagManager => self.open_tag_manager(cx),
            CommandId::SortTags => {
                if self.tag_manager.sort_menu.is_none() {
                    self.toggle_tag_sort_menu(cx);
                }
            }
            CommandId::AddKeywordAlias => self.start_alias_input(cx),
            CommandId::MoveKeyword => self.start_keyword_move(cx),
            CommandId::MergeKeyword => self.start_tag_merge(cx),
            CommandId::SaveKeywordSmart => self.save_command_keyword_as_album(cx),
            CommandId::RenameKeyword => self.start_keyword_rename(cx),
            CommandId::DeleteKeyword => self.delete_command_keyword(cx),
            CommandId::Back => self.navigate_back(cx),
            CommandId::Forward => self.navigate_forward(cx),
            CommandId::ClearRecents => self.clear_recents(cx),
            CommandId::ClearRecentSearches => self.clear_recent_searches(cx),
            CommandId::ApplyTemplate => self.start_template_pick(cx),
            CommandId::ManageTemplates => self.open_template_editor(cx),
        }
    }
}
