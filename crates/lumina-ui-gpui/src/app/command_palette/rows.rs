//! The rows the palette lists for the typed query.

use gpui::*;
use lumina_core::DownloadState;

use crate::icons::Icon;
use crate::views::sidebar::AlbumSelection;

use super::{commands, Command, CommandId, MAX_COMMANDS};
use crate::app::LuminaApp;
use crate::i18n::t;

/// What a palette row does when run.
#[derive(Debug, Clone, PartialEq)]
pub enum PaletteTarget {
    Command(CommandId),
    Go(AlbumSelection),
    Year(i32),
    /// Open Recent Library: switches to that library.
    Library(std::path::PathBuf),
    /// Apply Metadata Template…: applies template `index` to the selection.
    Template(usize),
    /// Audio Track: {name}: plays track `index` of the open video.
    AudioTrack(usize),
}

/// One rendered palette row: a command, or a GO TO destination.
#[derive(Debug, Clone)]
pub struct PaletteRow {
    pub target: PaletteTarget,
    pub title: String,
    pub shortcut: &'static str,
    pub icon: Icon,
    /// Stable `debug_selector`.
    pub selector: String,
}

impl PaletteRow {
    pub fn is_go_to(&self) -> bool {
        matches!(
            self.target,
            PaletteTarget::Go(_) | PaletteTarget::Year(_) | PaletteTarget::Library(_)
        )
    }

    /// The heading the row is listed under.
    pub fn group(&self) -> &'static str {
        match self.target {
            PaletteTarget::Command(_) | PaletteTarget::AudioTrack(_) => "COMMANDS",
            PaletteTarget::Template(_) => "TEMPLATES",
            _ => "GO TO",
        }
    }
}

/// Every typed word must appear in the title or keywords ("name sort" finds
/// Sort by Name).
fn matches(words: &[String], title: &str, keywords: &str) -> bool {
    let haystack = format!("{} {keywords}", title.to_lowercase());
    words.iter().all(|word| haystack.contains(word.as_str()))
}

impl LuminaApp {
    /// A command's palette title; the sidebar toggle says what it will do.
    fn command_title(&self, command: &Command) -> String {
        if command.id == CommandId::ToggleSidebar {
            crate::app::panels::sidebar_toggle_title(self.sidebar_collapsed)
        } else if command.id == CommandId::ToggleAlbumPanel {
            crate::views::album_page::panel::toggle_title(self.config.album_panel_hidden)
        } else {
            command.title()
        }
    }

    /// Commands that would do nothing right now stay out of the list.
    pub(crate) fn command_applies(&self, id: CommandId) -> bool {
        match id {
            CommandId::RetryError => self.error_retry.is_some(),
            CommandId::Back => self.can_navigate_back() && !self.compare_holds_view(),
            CommandId::Forward => self.can_navigate_forward() && !self.compare_holds_view(),
            CommandId::ClearRecents => !self.config.recent_libraries.is_empty(),
            CommandId::ClearRecentSearches => !self.config.recent_searches.is_empty(),
            CommandId::SaveSearch => self.search_save_enabled(),
            CommandId::NewAlbumWithSelection => {
                self.selected_item.is_some() || !self.selected_indices.is_empty()
            }
            CommandId::AddToAlbum => {
                self.selection != AlbumSelection::Trash
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::RemoveFromAlbum => {
                matches!(self.selection, AlbumSelection::Album(_))
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::SetAlbumCover => {
                self.album_page_album().is_some() && self.single_selected_id().is_some()
            }
            CommandId::AddFromLibrary => self.empty_album_shown(),
            CommandId::RenameAlbum | CommandId::DeleteAlbum => self.palette_album().is_some(),
            // The panel is hidden on these pages; the toggle does nothing.
            CommandId::ToggleInspector => {
                !self.non_media_page_shown() && !self.duplicates_review_shown()
            }
            // Already there.
            CommandId::OpenTagManager => !self.tags_page_shown(),
            CommandId::SortTags => self.tags_page_shown(),
            // An open album's tiles always fill.
            CommandId::ToggleFit => !self.album_tiles(),
            CommandId::ToggleAlbumPanel => self.album_panel_fits(),
            CommandId::CheckForUpdates => crate::app::updates::supported() && !self.updates.busy(),
            CommandId::RestartToUpdate => {
                matches!(
                    self.updates.phase,
                    crate::app::updates::UpdatePhase::Ready(_)
                )
            }
            CommandId::AutoUpdatesOn => {
                crate::app::updates::supported() && !self.config.auto_update_check
            }
            CommandId::AutoUpdatesOff => {
                crate::app::updates::supported() && self.config.auto_update_check
            }
            CommandId::PinAlbum => self.open_album_pinned() == Some(false),
            CommandId::UnpinAlbum => self.open_album_pinned() == Some(true),
            CommandId::DuplicateSmartAlbum | CommandId::ExportSmartAlbumRules => {
                matches!(self.selection, AlbumSelection::User(_))
            }
            CommandId::ImportSmartAlbumRules => self.grid_state.is_some(),
            CommandId::ClearFilters => crate::app::search::query_has_filters(&self.search_query),
            CommandId::ClearSearch => !self.search_query.trim().is_empty(),
            // Only in Videos (plain or 4K): elsewhere it would replace a
            // typed search with the 4K one.
            CommandId::Videos4k => self.videos_view(),
            CommandId::EmptyTrash | CommandId::RestoreAll => self
                .system_album_counts
                .get(&lumina_core::SystemAlbum::RecentlyDeleted)
                .is_some_and(|&n| n > 0),
            // Not when the review shows every group skipped: it would merge nothing.
            CommandId::MergeAllDuplicates => {
                self.duplicate_items_count > 0
                    && (!self.duplicates_review_shown() || self.duplicates_to_review())
            }
            CommandId::SkipDuplicateGroup
            | CommandId::MergeDuplicateGroup
            | CommandId::AutoSelectBest
            | CommandId::SkipAllDuplicateGroups => {
                self.duplicates_review_shown()
                    && self.duplicates_to_review()
                    && !self.is_fullscreen_viewer
            }
            CommandId::ShowExactCopies | CommandId::ShowSimilarShots => {
                self.grid_state.is_some() && !self.compare_holds_view()
            }
            CommandId::ShowMap => !self.map_blocked() && !self.map_shown(),
            CommandId::ShowAlbums => {
                self.grid_state.is_some()
                    && !self.compare_holds_view()
                    && !self.albums_overview_shown()
            }
            // Listed on either tab, so typing a tab's name always finds it.
            CommandId::ShowAlbumsTab | CommandId::ShowSmartAlbumsTab => {
                self.grid_state.is_some() && !self.compare_holds_view()
            }
            CommandId::ShowGrid => self.grid_state.is_some() && !self.view_mode_on().1,
            CommandId::ShowMosaic => self.grid_state.is_some() && !self.view_mode_on().0,
            CommandId::SelectDay => {
                self.mosaic_shown()
                    && self.sort.field == lumina_core::SortField::DateTaken
                    && !self.is_fullscreen_viewer
                    && self.mosaic_layout_current()
            }
            CommandId::ShowYears | CommandId::ShowMonths | CommandId::ShowDays => {
                let level = match id {
                    CommandId::ShowYears => crate::config::GroupLevel::Years,
                    CommandId::ShowMonths => crate::config::GroupLevel::Months,
                    _ => crate::config::GroupLevel::Days,
                };
                self.group_levels_shown()
                    && !self.is_fullscreen_viewer
                    && self.config.group_level != level
            }
            CommandId::ShowOnMap => !self.map_blocked() && self.selected_item_location().is_some(),
            CommandId::DeleteRejected => {
                self.grid_state.is_some()
                    && !self.compare_holds_view()
                    && self.view_query().reject_scope().is_some()
            }
            CommandId::Compare => self.grid_state.is_some() && !self.is_fullscreen_viewer,
            CommandId::Slideshow => self.grid_state.is_some() && !self.compare_holds_view(),
            CommandId::SlideshowInterval(secs) => self.config.slideshow_interval_secs != secs,
            CommandId::ToggleDownloads => !self.downloads.entries.is_empty(),
            CommandId::ShowAllDownloads => {
                self.grid_state.is_some()
                    && !self.compare_holds_view()
                    && !self.downloads_screen_shown()
            }
            // Placeholders (insert not back yet) aren't paused, so they don't count.
            CommandId::PauseAllDownloads => self.downloads.entries.iter().any(|e| {
                e.row.id > 0
                    && matches!(e.row.state, DownloadState::Running | DownloadState::Waiting)
            }),
            CommandId::ResumeAllDownloads => self.any_download_in(&[DownloadState::Paused]),
            CommandId::RetryFailedDownloads => self
                .downloads
                .entries
                .iter()
                .any(|e| e.row.state == DownloadState::Failed && e.row.can_retry),
            CommandId::ClearFinishedDownloads => self.any_download_in(&[
                DownloadState::Done,
                DownloadState::Failed,
                DownloadState::Duplicate,
            ]),
            CommandId::ExportSelection | CommandId::ExportJpeg => self.jobs.export.is_none(),
            // Similar shots holds no selection, so it offers no Share….
            CommandId::Share => {
                cfg!(target_os = "macos")
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::ImportXmp => self.jobs.xmp.is_none(),
            CommandId::AddKeywordAlias
            | CommandId::MoveKeyword
            | CommandId::MergeKeyword
            | CommandId::RenameKeyword
            | CommandId::DeleteKeyword => self.command_keyword().is_some(),
            CommandId::SaveKeywordSmart => {
                self.tags_page_shown() && self.tag_manager.focused.is_some()
            }
            CommandId::RotateLeft | CommandId::RotateRight | CommandId::RevertToOriginal => {
                self.selected_item.is_some() || !self.selected_indices.is_empty()
            }
            CommandId::EditInExternalEditor | CommandId::EditCopyInExternalEditor => {
                self.selected_item.is_some()
                    && (self.is_fullscreen_viewer || self.selected_indices.len() <= 1)
            }
            CommandId::RateMenu => {
                self.is_fullscreen_viewer
                    && self.slideshow.is_none()
                    && self.edits.session.is_none()
            }
            CommandId::EditPhoto => self
                .selected_item
                .as_ref()
                .is_some_and(|item| item.media_type != lumina_core::MediaType::Video),
            CommandId::ApplyTemplate => {
                !self.config.templates.is_empty()
                    && (self.selected_item.is_some() || !self.selected_indices.is_empty())
            }
            CommandId::SetLoopStart
            | CommandId::SetLoopEnd
            | CommandId::ClearLoopRange
            | CommandId::VolumeUp
            | CommandId::VolumeDown
            | CommandId::PreviousFrame
            | CommandId::NextFrame => self.loop_range_applies(),
            CommandId::ViewerZoom(_) => {
                self.is_fullscreen_viewer
                    && self
                        .selected_item
                        .as_ref()
                        .is_some_and(|item| item.media_type != lumina_core::MediaType::Video)
            }
            _ => true,
        }
    }

    /// COMMANDS (at most eight) then GO TO, filtered by the query.
    pub fn palette_rows(&self) -> Vec<PaletteRow> {
        let words: Vec<String> = self
            .command_query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if self.templates.palette_pick {
            return self
                .config
                .templates
                .iter()
                .enumerate()
                .filter(|(_, template)| matches(&words, &template.name, "template"))
                .map(|(index, template)| PaletteRow {
                    target: PaletteTarget::Template(index),
                    title: template.name.clone(),
                    shortcut: "",
                    icon: Icon::Tag,
                    selector: format!("palette-template-{}", template.name),
                })
                .collect();
        }
        let mut rows: Vec<PaletteRow> = commands()
            .iter()
            .filter(|c| {
                self.command_applies(c.id) && matches(&words, &self.command_title(c), c.keywords)
            })
            .take(MAX_COMMANDS)
            .map(|c| PaletteRow {
                target: PaletteTarget::Command(c.id),
                title: self.command_title(c),
                shortcut: c.shortcut,
                icon: if c.id == CommandId::ToggleSidebar {
                    crate::app::panels::sidebar_toggle_icon(self.sidebar_collapsed)
                } else {
                    c.icon()
                },
                selector: format!("command-item-{}", c.key),
            })
            .collect();
        // The open video's audio choice (§11: the Audio menu's keyboard path),
        // within the same row budget as the commands.
        if self.video.player.is_some() && self.video.tracks.has_audio_choice() {
            let playing = self.current_audio_track();
            let audio = (0..self.video.tracks.audio.len())
                .map(|index| PaletteRow {
                    target: PaletteTarget::AudioTrack(index),
                    title: t("command.audio_track")
                        .replace("{name}", &self.video.tracks.audio_name(index)),
                    shortcut: "",
                    icon: if playing == Some(index) {
                        Icon::Check
                    } else {
                        Icon::Volume
                    },
                    selector: format!("palette-audio-track-{index}"),
                })
                .filter(|row| matches(&words, &row.title, "video sound language"))
                .take(MAX_COMMANDS.saturating_sub(rows.len()))
                .collect::<Vec<_>>();
            rows.extend(audio);
        }

        let go = |selection: AlbumSelection, title: String, icon: Icon, slug: String| PaletteRow {
            target: PaletteTarget::Go(selection),
            title,
            shortcut: "",
            icon,
            selector: format!("palette-go-{slug}"),
        };
        // open_library_inner puts every opened library first, so while one
        // is open the first recent is it.
        let skip_current = usize::from(self.grid_state.is_some());
        let mut go_to: Vec<PaletteRow> = self
            .config
            .recent_libraries
            .iter()
            .enumerate()
            .skip(skip_current)
            .filter(|(_, path)| !self.recent_unavailable(path))
            .map(|(i, path)| PaletteRow {
                target: PaletteTarget::Library(path.clone()),
                title: t("command.open_recent")
                    .replace("{name}", &crate::app::recents::library_name(path)),
                shortcut: "",
                icon: Icon::Folder,
                selector: format!("palette-recent-{i}"),
            })
            .collect();
        go_to.extend([
            go(
                AlbumSelection::AllLibrary,
                "All Media".into(),
                Icon::Images,
                "all".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
                "Favorites".into(),
                Icon::Heart,
                "favorites".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Videos),
                "Videos".into(),
                Icon::Video,
                "videos".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::RecentlyAdded),
                "Recently Added".into(),
                Icon::Clock,
                "recently added new".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Downloaded),
                "Downloaded".into(),
                Icon::ArrowDownToLine,
                "downloaded downloads url".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Screenshots),
                "Screenshots".into(),
                Icon::Camera,
                "screenshots".into(),
            ),
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Places),
                "Places".into(),
                Icon::MapPin,
                "places".into(),
            ),
            // Listed even at 0, like Downloaded: the palette is a keyboard
            // path, so its rows do not come and go with the counts.
            go(
                AlbumSelection::System(lumina_core::SystemAlbum::Missing),
                "Missing".into(),
                Icon::TriangleAlert,
                "missing".into(),
            ),
            go(
                AlbumSelection::Trash,
                "Recently Deleted".into(),
                Icon::Trash,
                "trash".into(),
            ),
            go(
                AlbumSelection::Untagged,
                "Untagged".into(),
                Icon::Tag,
                "untagged".into(),
            ),
            go(
                AlbumSelection::Duplicates,
                "Duplicates".into(),
                Icon::Copy,
                "duplicates".into(),
            ),
        ]);
        go_to.extend(
            self.timeline_year_buckets
                .iter()
                .map(|(year, _)| PaletteRow {
                    target: PaletteTarget::Year(*year),
                    title: year.to_string(),
                    shortcut: "",
                    icon: Icon::Image,
                    selector: format!("palette-go-year-{year}"),
                }),
        );
        go_to.extend(self.library_tag_counts.iter().map(|(tag, _)| {
            go(
                AlbumSelection::Tag(tag.clone()),
                format!("#{tag}"),
                Icon::Tag,
                format!("tag-{tag}"),
            )
        }));
        go_to.extend(self.albums.list.iter().map(|album| {
            go(
                AlbumSelection::Album(album.id),
                album.title.clone(),
                Icon::Image,
                format!("curated-{}", album.id),
            )
        }));
        go_to.extend(self.user_smart_albums.iter().map(|album| {
            go(
                AlbumSelection::User(album.id),
                album.title.clone(),
                Icon::Sparkles,
                format!("album-{}", album.id),
            )
        }));
        rows.extend(
            go_to
                .into_iter()
                .filter(|row| matches(&words, &row.title, "go to album")),
        );
        rows
    }
}
