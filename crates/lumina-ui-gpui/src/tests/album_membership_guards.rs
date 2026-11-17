//! Guards around album membership: a year filter
//! never shows the library under an album, Remove records only what it
//! removed, and Trash offers no Add to Album.

use super::albums::album_with_members;
use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click, seed_library,
    type_text,
};
use crate::views::sidebar::AlbumSelection;
use lumina_core::{MediaId, SortSpec};

fn members(engine: &lumina_core::LuminaEngine, album: i64) -> Vec<MediaId> {
    let mut ids = super::albums::member_ids(engine, album, SortSpec::default());
    ids.sort_by_key(|id| id.0);
    ids
}

#[core::prelude::v1::test]
fn palette_year_in_album_scopes_to_library_and_back_returns() {
    let (dir, engine) = seed_library("album_year_scope", 3);
    let (album, ids) = album_with_members(&engine, "Trip", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let year = view.update(vcx, |app, _| app.timeline_year_buckets[0].0);

    click(vcx, leak(format!("album-row-{}", album.id)));
    // The rail is hidden in an album; the palette's year target filters.
    view.update(vcx, |app, cx| app.set_active_year(Some(year), cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            (app.selection.clone(), app.active_year),
            (AlbumSelection::AllLibrary, Some(year)),
            "a year shows the whole library, so the sidebar must say Library"
        );
        assert_eq!(app.get_active_count(), 3, "every item of that year");
    });

    // ⌫ there opens the Delete dialog; it never edits the album.
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, cx| {
        assert!(app.delete_confirm_state.is_some(), "⌫ outside an album");
        app.close_delete_modal(cx);
    });
    assert_eq!(members(&engine, album.id), ids, "membership unchanged");

    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            (app.selection.clone(), app.active_year),
            (AlbumSelection::Album(album.id), None),
            "Back returns to the album"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn remove_records_only_ids_it_removed() {
    let (dir, engine) = seed_library("album_remove_non_member", 3);
    let (album, ids) = album_with_members(&engine, "Trip", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let toast = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()))
    };

    click(vcx, leak(format!("album-row-{}", album.id)));
    focus_app(vcx, &view);
    for via_palette in [false, true] {
        keystrokes(vcx, "right");
        let selected = view.update(vcx, |app, _| app.selected_item.as_ref().unwrap().id);
        // The selected item leaves the album behind the app (another window),
        // so the selection holds a non-member.
        engine.remove_from_album(album.id, &[selected]).unwrap();
        let kept = members(&engine, album.id);
        if via_palette {
            keystrokes(vcx, "cmd-k");
            type_text(vcx, "remove from album");
            keystrokes(vcx, "enter");
        } else {
            keystrokes(vcx, "backspace");
        }
        assert_eq!(
            toast(&view, vcx).as_deref(),
            Some("Not in “Trip”"),
            "an honest toast when nothing was removed (palette: {via_palette})"
        );
        view.update(vcx, |app, _| {
            assert!(
                app.undo.undo_stack.is_empty(),
                "a remove that removed nothing records no undo"
            );
        });
        keystrokes(vcx, "cmd-z");
        assert_eq!(
            members(&engine, album.id),
            kept,
            "⌘Z never adds a non-member to the album (palette: {via_palette})"
        );
        assert!(!kept.contains(&selected));
    }
    assert!(ids.iter().all(|id| engine.get_item_by_id(*id).is_some()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn add_to_album_hidden_in_trash() {
    let (dir, engine) = seed_library("album_trash_hidden", 2);
    engine.create_album_with("Trip", &[]).unwrap();
    let first = engine.get_item_slice(0..1)[0].id;
    engine.batch_set_trashed(&[first], true).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| app.select_album(AlbumSelection::Trash, cx));
    render_ui(vcx);

    right_click(vcx, leak(format!("media-card-{}", first.0)));
    assert!(vcx.debug_bounds("context-menu").is_some());
    assert!(
        vcx.debug_bounds("context-add-to-album").is_none(),
        "Trash offers no Add to Album"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.open_command_palette(cx);
        app.command_query = "add to album".into();
        let offered = app
            .palette_rows()
            .iter()
            .any(|row| row.title == crate::i18n::t("command.add_to_album"));
        assert!(!offered, "nor does the palette");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn restored_album_with_year_opens_on_library() {
    use crate::app::types::UiMessage;
    use crate::config::{LastView, RestoredSession};
    use lumina_core::EngineEvent;
    let (dir, engine) = seed_library("album_year_restore", 2);
    let album = engine.create_album_with("Trip", &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        let year = app.timeline_year_buckets[0].0;
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = true;
        }
        // A session saved before a year was scoped to the library.
        app.pending_restore = Some(RestoredSession {
            view: LastView::Album(album.id),
            search_query: String::new(),
            active_year: Some(year),
            selected_media_id: None,
            scroll_anchor_id: None,
        });
        let total = app.grid_state.as_ref().map_or(0, |s| s.total_count);
        app.handle_ui_message(
            UiMessage::Engine(EngineEvent::ScanComplete { total_items: total }),
            cx,
        );
        assert_eq!(
            (app.selection.clone(), app.active_year),
            (AlbumSelection::AllLibrary, Some(year)),
            "an album never shows the library's year view"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
