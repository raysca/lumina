//! Set as Cover in the album selection bar, the KEY
//! PHOTO section of the album panel, its pick mode and the key photo
//! badge on the cover's grid card.

use super::album_page::{library_with_album, open_album, painted};
use super::ui_automation::{
    app_with_library, bounds_center, click, double_click, focus_app, keystrokes, leak, render_ui,
    type_text,
};
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};

/// The album's stored key photo, read from the catalog.
fn stored_cover(engine: &LuminaEngine, id: i64) -> Option<MediaId> {
    engine
        .list_albums_with_counts()
        .into_iter()
        .find(|(album, _)| album.id == id)
        .and_then(|(album, _)| album.cover_media_id)
}

fn name(engine: &LuminaEngine, id: MediaId) -> String {
    engine
        .get_item_by_id(id)
        .expect("the item exists")
        .file_name
}

fn card(id: MediaId) -> &'static str {
    leak(format!("media-card-{}", id.0))
}

fn badge(vcx: &mut VisualTestContext, id: MediaId) -> bool {
    painted(vcx, format!("key-photo-badge-{}", id.0))
}

fn panel_names(vcx: &mut VisualTestContext, file_name: &str) -> bool {
    painted(vcx, format!("album-panel-key-photo-name-text:{file_name}"))
}

/// Lets the panel's stats reread after a write land and paint: a read in
/// flight during the write lands first, then the next frame rereads.
fn settle(vcx: &mut VisualTestContext) {
    for _ in 0..3 {
        render_ui(vcx);
    }
}

fn picking(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.cover_picking())
}

#[core::prelude::v1::test]
fn set_as_cover_updates_panel_badge_and_undo() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);

    // No key photo stored: the newest member (18 Jun) is the cover.
    assert_eq!(stored_cover(&engine, id), None);
    assert!(
        panel_names(vcx, &name(&engine, ids[1])),
        "the panel shows the cover in use, the newest member"
    );
    assert!(badge(vcx, ids[1]), "the badge marks the cover in use");

    click(vcx, card(ids[0]));
    click(vcx, "album-bar-set-cover");
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[0]),
        "Set as Cover stores the selected item as the key photo"
    );
    settle(vcx);
    assert!(
        panel_names(vcx, &name(&engine, ids[0])),
        "the panel names the new key photo"
    );
    assert!(badge(vcx, ids[0]), "the badge moves to the new key photo");
    assert!(!badge(vcx, ids[1]), "only one card carries the badge");

    keystrokes(vcx, "cmd-z");
    assert_eq!(
        stored_cover(&engine, id),
        None,
        "⌘Z restores the stored key photo (none), not the fallback"
    );
    settle(vcx);
    assert!(panel_names(vcx, &name(&engine, ids[1])));
    assert!(badge(vcx, ids[1]) && !badge(vcx, ids[0]));

    keystrokes(vcx, "cmd-shift-z");
    assert_eq!(stored_cover(&engine, id), Some(ids[0]), "redo reapplies it");
    settle(vcx);
    assert!(panel_names(vcx, &name(&engine, ids[0])));

    // Two selected: the button is disabled and says why.
    click(vcx, card(ids[1]));
    let third = bounds_center(vcx, card(ids[2]));
    vcx.simulate_click(third, crate::tests::ui_automation::primary_modifier());
    render_ui(vcx);
    click(vcx, "album-bar-set-cover");
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[0]),
        "with two selected Set as Cover does nothing"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            2,
            "the click kept the selection"
        );
    });
    let center = bounds_center(vcx, "album-bar-set-cover");
    vcx.simulate_mouse_move(center, None, gpui::Modifiers::default());
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tooltip-Select one item to set as the cover")
            .is_some(),
        "the disabled button explains itself in a tooltip"
    );
}

#[core::prelude::v1::test]
fn change_picks_the_next_item_and_esc_cancels() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_pick");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);

    click(vcx, "album-panel-key-photo-change");
    assert!(picking(&view, vcx), "Change starts picking the cover");
    assert!(
        vcx.debug_bounds("album-panel-key-photo-hint").is_some(),
        "the panel tells the user what to do"
    );
    keystrokes(vcx, "escape");
    assert!(!picking(&view, vcx), "Esc cancels the pick");
    assert_eq!(
        stored_cover(&engine, id),
        None,
        "a cancelled pick sets nothing"
    );

    // A click picks.
    click(vcx, "album-panel-key-photo-change");
    click(vcx, card(ids[2]));
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[2]),
        "the clicked item becomes the cover"
    );
    assert!(!picking(&view, vcx), "picking ends with the pick");
    settle(vcx);
    assert!(badge(vcx, ids[2]));

    // Return picks the selected item, and does not open the viewer.
    click(vcx, card(ids[0]));
    click(vcx, "album-panel-key-photo-change");
    keystrokes(vcx, "enter");
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[0]),
        "↵ picks the selection"
    );
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer, "↵ in pick mode opens no viewer");
    });

    // The pick is undone like Set as Cover.
    keystrokes(vcx, "cmd-z");
    assert_eq!(stored_cover(&engine, id), Some(ids[2]));

    // Leaving the album ends pick mode.
    click(vcx, "album-panel-key-photo-change");
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    settle(vcx);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(id), cx)
    });
    settle(vcx);
    assert!(!picking(&view, vcx), "leaving the album ends the pick");
}

#[core::prelude::v1::test]
fn palette_sets_album_cover_for_one_selected() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_palette");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    use crate::app::command_palette::CommandId;
    view.update(vcx, |app, _| {
        assert!(
            !app.command_applies(CommandId::SetAlbumCover),
            "nothing selected: the command is not offered"
        );
    });
    click(vcx, card(ids[2]));
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "set as album cover");
    keystrokes(vcx, "enter");
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[2]),
        "⌘K Set as Album Cover sets the selected item"
    );
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            !app.command_applies(CommandId::SetAlbumCover),
            "outside an album page the command is not offered"
        );
    });
}

#[core::prelude::v1::test]
fn empty_album_has_no_key_photo() {
    let (_dir, engine, _id, _ids) = library_with_album("album_key_photo_empty");
    let empty = engine.create_album_with("Empty", &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, empty.id);
    assert!(
        panel_names(vcx, "No key photo"),
        "an empty album says it has no key photo"
    );
    click(vcx, "album-panel-key-photo-change");
    assert!(
        !picking(&view, vcx),
        "Change is disabled with nothing to pick"
    );
}

#[core::prelude::v1::test]
fn undo_clears_when_the_old_cover_left_the_album() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_left");
    engine.set_album_cover(id, Some(ids[0])).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);

    click(vcx, card(ids[1]));
    click(vcx, "album-bar-set-cover");
    assert_eq!(stored_cover(&engine, id), Some(ids[1]));
    // The old key photo leaves the album outside this undo stack.
    engine.remove_from_album(id, &[ids[0]]).unwrap();

    keystrokes(vcx, "cmd-z");
    assert_eq!(
        stored_cover(&engine, id),
        None,
        "undo to a key photo that left the album clears the cover"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_none(),
            "the fallback is not an error: {:?}",
            app.error_message
        );
    });
}

#[core::prelude::v1::test]
fn failed_cover_write_shows_the_banner() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_fail");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);

    click(vcx, card(ids[0]));
    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(true));
    click(vcx, "album-bar-set-cover");
    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(false));
    assert_eq!(stored_cover(&engine, id), None, "nothing was written");
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not set the album cover")),
            "the failure is in the error banner: {:?}",
            app.error_message
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed write leaves nothing to undo"
        );
    });
}

#[core::prelude::v1::test]
fn pick_mode_double_click_and_hidden_panel() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_pick_edges");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);

    // A double-click picks once and opens no viewer.
    click(vcx, "album-panel-key-photo-change");
    double_click(vcx, card(ids[2]));
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[2]),
        "the double-click picks"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "the second click of a picking double-click opens no viewer"
        );
        assert_eq!(app.undo.undo_stack.len(), 1, "one pick, one undo step");
    });
    // Later double-clicks open the viewer again.
    double_click(vcx, card(ids[0]));
    view.update(vcx, |app, cx| {
        assert!(
            app.is_fullscreen_viewer,
            "outside pick mode a double-click opens"
        );
        app.close_viewer(cx);
    });
    render_ui(vcx);

    // Info takes the panel's column: the pick ends with the panel.
    click(vcx, "album-panel-key-photo-change");
    assert!(picking(&view, vcx));
    keystrokes(vcx, "i");
    render_ui(vcx);
    assert!(
        !picking(&view, vcx),
        "hiding the panel (Info on) ends pick mode"
    );
    click(vcx, card(ids[1]));
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[2]),
        "after the pick ended a click only selects"
    );
}

#[core::prelude::v1::test]
fn undo_cover_of_a_deleted_album_shows_the_error() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_gone");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    click(vcx, card(ids[0]));
    click(vcx, "album-bar-set-cover");
    engine.delete_album(id).unwrap();
    keystrokes(vcx, "cmd-z");
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.contains("no longer exists")),
            "errors other than a departed cover are in the banner: {:?}",
            app.error_message
        );
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "the failed step stays on the undo stack"
        );
    });
}

/// Only a key photo that left the album is cleared: a failed undo while it
/// is still a member changes nothing and shows the error.
#[core::prelude::v1::test]
fn failed_cover_undo_keeps_the_cover_and_the_step() {
    let (_dir, engine, id, ids) = library_with_album("album_key_photo_undo_fail");
    engine.set_album_cover(id, Some(ids[0])).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    click(vcx, card(ids[1]));
    click(vcx, "album-bar-set-cover");

    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "cmd-z");
    crate::app::albums::ALBUM_WRITE_FAILS.with(|f| f.set(false));
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[1]),
        "a failed undo neither restores nor clears the cover"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.error_message.is_some(),
            "the failed undo is in the banner"
        );
        assert_eq!(app.undo.undo_stack.len(), 1, "the step stays to retry");
    });
    keystrokes(vcx, "escape");
    keystrokes(vcx, "cmd-z");
    assert_eq!(
        stored_cover(&engine, id),
        Some(ids[0]),
        "the retried undo restores the member key photo"
    );
}
