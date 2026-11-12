//! The default collections, in sidebar order. The expanded rows render from
//! this one list, so a new collection or visibility rule lives in one place.

use gpui::Context;
use lumina_core::SystemAlbum;

use super::AlbumSelection;
use crate::icons::Icon;
use crate::LuminaApp;

/// One default collection as the sidebar shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavEntry {
    /// Stable, label-independent name (e.g. `favorites`).
    pub key: &'static str,
    pub label: &'static str,
    pub icon: Icon,
    pub selection: AlbumSelection,
    pub count: usize,
    /// Draw the count in the accent: something here wants attention.
    pub attention: bool,
}

/// The visible default collections, in order. Downloaded and Missing appear
/// only while their count is above 0.
pub(crate) fn nav_entries(app: &LuminaApp) -> Vec<NavEntry> {
    let system = |album| app.system_album_counts.get(&album).copied().unwrap_or(0);
    // Before a library is open the rows show sample counts.
    let live = |count, sample| {
        if app.grid_state.is_some() {
            count
        } else {
            sample
        }
    };
    let downloaded = system(SystemAlbum::Downloaded);
    let missing = system(SystemAlbum::Missing);
    let entry = |key, label, icon, selection, count| NavEntry {
        key,
        label,
        icon,
        selection,
        count,
        attention: false,
    };
    [
        Some(entry(
            "all",
            "All Media",
            Icon::Images,
            AlbumSelection::AllLibrary,
            live(app.total_library_count, 22),
        )),
        Some(entry(
            "favorites",
            "Favorites",
            Icon::Heart,
            AlbumSelection::System(SystemAlbum::Favorites),
            live(system(SystemAlbum::Favorites), 3),
        )),
        Some(entry(
            "videos",
            "Videos",
            Icon::Video,
            AlbumSelection::System(SystemAlbum::Videos),
            live(system(SystemAlbum::Videos), 7),
        )),
        Some(entry(
            "recently-added",
            "Recently Added",
            Icon::Clock,
            AlbumSelection::System(SystemAlbum::RecentlyAdded),
            live(system(SystemAlbum::RecentlyAdded), 5),
        )),
        // Only once something was downloaded from a link.
        (downloaded > 0).then(|| {
            entry(
                "downloaded",
                "Downloaded",
                Icon::ArrowDownToLine,
                AlbumSelection::System(SystemAlbum::Downloaded),
                downloaded,
            )
        }),
        Some(entry(
            "screenshots",
            "Screenshots",
            Icon::Camera,
            AlbumSelection::System(SystemAlbum::Screenshots),
            live(system(SystemAlbum::Screenshots), 2),
        )),
        // TAGS › Keywords: the Tags page, counting distinct keywords.
        Some(entry(
            "keywords",
            "Keywords",
            Icon::Hash,
            AlbumSelection::Tags,
            live(app.library_tag_counts.len(), 3),
        )),
        Some(entry(
            "places",
            "Places",
            Icon::MapPin,
            AlbumSelection::System(SystemAlbum::Places),
            system(SystemAlbum::Places),
        )),
        // Copies waiting for review: the count is drawn as attention.
        Some(NavEntry {
            attention: app.duplicate_items_count > 0,
            ..entry(
                "duplicates",
                "Duplicates",
                Icon::Copy,
                AlbumSelection::Duplicates,
                app.duplicate_items_count,
            )
        }),
        // Only while something is missing: there is nothing to do otherwise.
        (missing > 0).then(|| {
            entry(
                "missing",
                "Missing",
                Icon::TriangleAlert,
                AlbumSelection::System(SystemAlbum::Missing),
                missing,
            )
        }),
        Some(entry(
            "recently-deleted",
            "Recently Deleted",
            Icon::Trash,
            AlbumSelection::Trash,
            live(system(SystemAlbum::RecentlyDeleted), 1),
        )),
        // A built-in smart album, listed under Smart Albums.
        Some(entry(
            "untagged",
            "Untagged",
            Icon::Tag,
            AlbumSelection::Untagged,
            app.untagged_count,
        )),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// What clicking an entry does: clicking Untagged again goes back to the
/// whole library; Keywords opens the Tags page fresh. `select_album` itself
/// clears the year and the search, and records one Back step.
pub(crate) fn open_entry(
    this: &mut LuminaApp,
    selection: &AlbumSelection,
    cx: &mut Context<LuminaApp>,
) {
    match selection {
        AlbumSelection::Untagged if this.selection == AlbumSelection::Untagged => {
            this.select_album(AlbumSelection::AllLibrary, cx);
        }
        AlbumSelection::Tags => this.open_tag_manager(cx),
        other => this.select_album(other.clone(), cx),
    }
}
