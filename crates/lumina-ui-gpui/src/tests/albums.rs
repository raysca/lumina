//! Hand-picked albums: the sidebar section, the album
//! view, New Album (⌘N), rename, delete and their undo. Every data-changing
//! test reads the catalog rows back, not only the app state.

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library, type_text,
};
use crate::app::types::UiMessage;
use crate::config::{LastView, RestoredSession};
use crate::views::sidebar::AlbumSelection;
use lumina_core::{Album, EngineEvent, LuminaEngine, MediaId, SortDirection, SortField, SortSpec};

/// The albums in the catalog as (title, active member count).
pub(super) fn catalog_albums(engine: &LuminaEngine) -> Vec<(String, usize)> {
    engine
        .list_albums_with_counts()
        .into_iter()
        .map(|(album, count)| (album.title, count))
        .collect()
}

pub(super) fn only_album(engine: &LuminaEngine) -> Album {
    let albums = engine.list_albums_with_counts();
    assert_eq!(albums.len(), 1, "exactly one album exists: {albums:?}");
    albums[0].0.clone()
}

/// Ids of an album's active members, in the view's sort order.
pub(super) fn member_ids(engine: &LuminaEngine, id: i64, sort: SortSpec) -> Vec<MediaId> {
    engine
        .query_album_sorted(id, 100, 0, sort)
        .into_iter()
        .map(|item| item.id)
        .collect()
}

pub(super) fn album_with_members(
    engine: &LuminaEngine,
    title: &str,
    count: usize,
) -> (Album, Vec<MediaId>) {
    let album = engine.create_album_with(title, &[]).unwrap();
    let ids: Vec<MediaId> = engine
        .get_item_slice(0..count)
        .into_iter()
        .map(|item| item.id)
        .collect();
    engine.add_to_album(album.id, &ids).unwrap();
    (album, ids)
}

#[core::prelude::v1::test]
fn new_album_from_selection_shows_in_sidebar_with_count() {
    let (dir, engine) = seed_library("album_from_selection", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Keyboard only: two items, then ⌘K New Album with Selection.
    keystrokes(vcx, "right");
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "new album with selection");
    keystrokes(vcx, "enter");

    let album = only_album(&engine);
    assert_eq!(
        engine.query_album_count(album.id),
        2,
        "the new album holds exactly the two selected items"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.renaming,
            Some(album.id),
            "a new album starts in rename mode"
        );
        assert_eq!(
            app.albums.counts.get(&album.id),
            Some(&2),
            "the sidebar count is the member count"
        );
    });
    assert!(
        vcx.debug_bounds("album-rename-field").is_some(),
        "the rename field is drawn in the sidebar"
    );

    type_text(vcx, " Kyoto");
    keystrokes(vcx, "enter");
    render_ui(vcx);
    let renamed = only_album(&engine);
    assert_eq!(
        renamed.title,
        format!("{} Kyoto", album.title),
        "↵ saves the typed name to the catalog"
    );
    assert!(
        vcx.debug_bounds(leak(format!("album-row-{}", album.id)))
            .is_some(),
        "the album has a sidebar row"
    );
    assert!(vcx.debug_bounds("sidebar-albums-header").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_n_creates_empty_album_in_rename_mode() {
    let (dir, engine) = seed_library("album_cmd_n", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // One selected item is the normal grid state: ⌘N must not fill the album.
    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-n");
    let album = only_album(&engine);
    assert_eq!(
        engine.query_album_count(album.id),
        0,
        "⌘N makes an empty album even with an item selected"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.renaming,
            Some(album.id),
            "it starts in rename mode"
        );
        assert_eq!(app.albums.rename_buffer, album.title);
    });

    // Typing goes to the field, not to the grid's shortcuts (F favourites).
    keystrokes(vcx, "cmd-a");
    type_text(vcx, "Favorites trip");
    keystrokes(vcx, "enter");
    assert_eq!(only_album(&engine).title, "Favorites trip");
    assert!(
        engine
            .get_item_slice(0..2)
            .iter()
            .all(|item| !item.is_favorite),
        "letters typed into the rename field never reach the grid"
    );
    view.update(vcx, |app, _| {
        assert!(app.albums.renaming.is_none(), "↵ ends rename mode");
    });

    // Esc leaves the name as it was.
    keystrokes(vcx, "cmd-n");
    type_text(vcx, "x");
    keystrokes(vcx, "escape");
    let titles: Vec<String> = catalog_albums(&engine).into_iter().map(|a| a.0).collect();
    assert_eq!(titles.len(), 2, "the second ⌘N made a second album");
    assert!(
        !titles.iter().any(|t| t.ends_with('x')),
        "Esc discards the typed name: {titles:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_view_lists_members_in_sort_order() {
    let (dir, engine) = seed_library("album_sort", 5);
    // Members 1, 3 and 4 by file name; 0 and 2 stay out.
    let items = engine.get_item_slice_sorted(
        0,
        5,
        SortSpec {
            field: SortField::FileName,
            direction: SortDirection::Ascending,
        },
    );
    let album = engine.create_album_with("Some", &[]).unwrap();
    let members: Vec<MediaId> = [1, 3, 4].iter().map(|&i| items[i].id).collect();
    engine.add_to_album(album.id, &members).unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, leak(format!("album-row-{}", album.id)));

    let shown = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            let count = app.get_active_count();
            app.get_active_slice(0..count)
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>()
        })
    };
    view.update(vcx, |app, cx| {
        assert_eq!(app.selection, AlbumSelection::Album(album.id));
        app.set_sort(SortField::FileName, SortDirection::Ascending, cx);
    });
    assert_eq!(
        shown(&view, vcx),
        members,
        "name ascending lists only members, A to Z"
    );

    view.update(vcx, |app, cx| {
        app.set_sort(SortField::FileName, SortDirection::Descending, cx)
    });
    let mut reversed = members.clone();
    reversed.reverse();
    assert_eq!(
        shown(&view, vcx),
        reversed,
        "the album follows the sort direction"
    );
    assert_eq!(
        shown(&view, vcx),
        member_ids(
            &engine,
            album.id,
            SortSpec {
                field: SortField::FileName,
                direction: SortDirection::Descending
            }
        ),
        "the grid shows the catalog's album query"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_count_follows_trash_and_restore() {
    let (dir, engine) = seed_library("album_trash_count", 3);
    let (album, ids) = album_with_members(&engine, "Trip", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        assert_eq!(app.albums.counts.get(&album.id), Some(&2));
        app.select_album(AlbumSelection::Album(album.id), cx);
        app.select_index(0, cx);
        app.trash_or_restore_selected(cx);
        assert_eq!(
            app.albums.counts.get(&album.id),
            Some(&1),
            "a trashed member leaves the sidebar count"
        );
    });
    assert_eq!(engine.query_album_count(album.id), 1, "the catalog agrees");

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.counts.get(&album.id),
            Some(&2),
            "restoring it puts the count back"
        );
    });
    assert_eq!(
        member_ids(&engine, album.id, SortSpec::default()).len(),
        ids.len(),
        "the restored item is still a member"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_rename_undo() {
    let (dir, engine) = seed_library("album_rename_undo", 1);
    let album = engine.create_album_with("Before", &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // The row's pencil starts rename mode.
    click(vcx, leak(format!("album-rename-{}", album.id)));
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.renaming, Some(album.id))
    });
    keystrokes(vcx, "cmd-a");
    type_text(vcx, "After");
    keystrokes(vcx, "enter");
    assert_eq!(only_album(&engine).title, "After");

    keystrokes(vcx, "cmd-z");
    assert_eq!(
        only_album(&engine).title,
        "Before",
        "⌘Z restores the old name"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.list[0].title, "Before",
            "the sidebar follows the undo"
        );
    });
    keystrokes(vcx, "cmd-shift-z");
    assert_eq!(only_album(&engine).title, "After", "⇧⌘Z renames it again");

    // The palette reaches rename for the open album.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx)
    });
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "rename album");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.renaming,
            Some(album.id),
            "Rename Album… opens the field"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_selection_persists_across_restart_token() {
    assert_eq!(LastView::Album(9).token(), "album:9");
    assert_eq!(LastView::from_token("album:9"), LastView::Album(9));
    assert_eq!(LastView::from_token("album:x"), LastView::All);
    assert_eq!(LastView::from_token("album:0"), LastView::All);
    assert_eq!(
        LastView::from_selection(&AlbumSelection::Album(9)),
        LastView::Album(9)
    );
    assert_ne!(
        LastView::Album(9).token(),
        LastView::User(9).token(),
        "albums and smart albums are separate id spaces"
    );

    let (dir, engine) = seed_library("album_restore", 2);
    let album = engine.create_album_with("Trip", &[]).unwrap();
    for (saved, expected) in [
        (LastView::Album(album.id), AlbumSelection::Album(album.id)),
        (LastView::Album(album.id + 100), AlbumSelection::AllLibrary),
    ] {
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = app_with_library(&mut cx, engine.clone());
        view.update(vcx, |app, cx| {
            if let Some(state) = app.grid_state.as_mut() {
                state.is_scanning = true;
            }
            app.pending_restore = Some(RestoredSession {
                view: LastView::from_token(&saved.token()),
                search_query: String::new(),
                active_year: None,
                selected_media_id: None,
                scroll_anchor_id: None,
            });
            let total = app.grid_state.as_ref().map_or(0, |s| s.total_count);
            app.handle_ui_message(
                UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
                cx,
            );
            assert_eq!(
                app.selection, expected,
                "{saved:?} restores as {expected:?}; an unknown album falls back to All"
            );
        });
    }
    let _ = std::fs::remove_dir_all(&dir);
}
