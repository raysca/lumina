//! Back/Forward browsing history: ⌘[ / ⌘], the Go menu actions and ⌘K rows.

use super::helpers::{
    app_with_library, click, double_click, focus_app, keystrokes, leak, render_ui, seed_library,
    type_text,
};
use crate::app::command_palette::{CommandId, PaletteTarget};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::SystemAlbum;

const FAVORITES: AlbumSelection = AlbumSelection::System(SystemAlbum::Favorites);

/// Types `text` into the search field, then leaves it with Esc (the first
/// Esc may only close the suggestion dropdown).
fn type_search(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    text: &str,
) {
    click(vcx, "search-input");
    type_text(vcx, text);
    for _ in 0..2 {
        if view.update(vcx, |app, _| app.search_focused) {
            keystrokes(vcx, "escape");
        }
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, text);
        assert!(!app.search_focused, "Esc leaves the field");
    });
    focus_app(vcx, view);
}

/// Which of Back / Forward the palette offers for `query`.
fn palette_offers(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    id: CommandId,
    query: &str,
) -> bool {
    view.update(vcx, |app, _| {
        app.command_query = query.into();
        let shown = app
            .palette_rows()
            .iter()
            .any(|row| row.target == PaletteTarget::Command(id));
        app.command_query.clear();
        shown
    })
}

#[core::prelude::v1::test]
fn navigation_forward_redoes_back() {
    let (_dir, engine) = seed_library("hist_fwd", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.select_album(FAVORITES, cx));
    render_ui(vcx);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary)
    });
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, FAVORITES, "⌘] must redo the Back");
    });
    // Nothing ahead now: Forward is a no-op.
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| assert_eq!(app.selection, FAVORITES));
}

#[core::prelude::v1::test]
fn go_menu_actions_go_back_and_forward() {
    let (_dir, engine) = seed_library("hist_menu", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.select_album(FAVORITES, cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    vcx.dispatch_action(crate::actions::GoBack);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary, "Go > Back");
    });
    vcx.dispatch_action(crate::actions::GoForward);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, FAVORITES, "Go > Forward")
    });
}

#[core::prelude::v1::test]
fn forward_after_viewer_back_reopens_viewer() {
    let (_dir, engine) = seed_library("hist_viewer", 4);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    double_click(vcx, card);
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer, "Back closes it");
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.id),
            Some(item.id),
            "Back must keep the grid selection on the item that was open"
        );
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "Forward must reopen the viewer");
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.id),
            Some(item.id),
            "Forward reopens the same item"
        );
    });
}

#[core::prelude::v1::test]
fn forward_to_deleted_item_opens_no_viewer() {
    let (_dir, engine) = seed_library("hist_gone", 4);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    click(vcx, card);
    double_click(vcx, card);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| assert!(!app.is_fullscreen_viewer));

    engine.set_trashed(item.id, true).unwrap();
    view.update(vcx, |app, cx| app.refresh_after_history(cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "Forward must never open a viewer for an item that is gone"
        );
        assert!(
            app.selected_item.is_none(),
            "a gone item cannot stay selected"
        );
    });
}

#[core::prelude::v1::test]
fn forward_to_deleted_album_falls_back_to_all() {
    let (_dir, engine) = seed_library("hist_tag", 4);
    let id = engine.get_item_slice(0..1)[0].id;
    engine.add_tags_batch(&[id], &["trip".to_string()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // All -> Favorites -> tag, so Back from the fallback has somewhere real to go.
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.select_album(FAVORITES, cx);
        app.select_album(AlbumSelection::Tag("trip".into()), cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, cx| {
        assert_eq!(app.selection, FAVORITES);
        app.delete_tag_globally("trip".into(), cx);
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Forward to a deleted tag falls back to All Photos"
        );
        assert_eq!(
            app.navigation_history
                .current()
                .map(|entry| &entry.selection),
            Some(&AlbumSelection::AllLibrary),
            "the fallback must replace the deleted tag's entry, not leave it in the trail"
        );
        assert!(!app.can_navigate_forward(), "nothing is left ahead");
    });
    // The trail stays consistent: Back reaches the entry before the fallback.
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| assert_eq!(app.selection, FAVORITES));
}

#[core::prelude::v1::test]
fn typed_search_then_back_then_forward() {
    let (_dir, engine) = seed_library("hist_typed", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    type_search(&view, vcx, "cat");
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert!(app.search_query.is_empty(), "Back undoes the typed search");
    });
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "cat", "Forward brings it back");
    });
}

#[core::prelude::v1::test]
fn cmd_bracket_in_viewer_navigates_history_not_items() {
    let (_dir, engine) = seed_library("hist_bracket", 4);
    let items = engine.get_item_slice(0..3);
    let card = leak(format!("media-card-{}", items[1].id.0));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, card);
    double_click(vcx, card);
    focus_app(vcx, &view);
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer);
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.id),
            Some(items[1].id)
        );
    });
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "nothing ahead: ⌘] is a no-op");
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.id),
            Some(items[1].id),
            "⌘] in the viewer must not step to the next item"
        );
    });
    keystrokes(vcx, "]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.id),
            Some(items[2].id),
            "plain ] still steps to the next item"
        );
    });
}

#[core::prelude::v1::test]
fn palette_shows_back_forward_only_when_available() {
    let (_dir, engine) = seed_library("hist_palette", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    assert!(!palette_offers(&view, vcx, CommandId::Back, "back"));
    assert!(!palette_offers(&view, vcx, CommandId::Forward, "forward"));

    // A typed search at cursor 0 is unsynced but Back can still undo it.
    type_search(&view, vcx, "cat");
    assert!(
        palette_offers(&view, vcx, CommandId::Back, "back"),
        "a typed search can go Back even before it is recorded"
    );
    assert!(!palette_offers(&view, vcx, CommandId::Forward, "forward"));

    // Run Back from the palette: Forward appears, Back disappears.
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k b a c k enter");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        assert!(app.search_query.is_empty(), "the palette's Back ran");
    });
    assert!(!palette_offers(&view, vcx, CommandId::Back, "back"));
    assert!(palette_offers(&view, vcx, CommandId::Forward, "forward"));

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k f o r w a r d enter");
    view.update(vcx, |app, _| assert_eq!(app.search_query, "cat"));

    // Back, then a new typed search: the sync would drop Forward, so the
    // palette must not offer it.
    keystrokes(vcx, "secondary-[");
    type_search(&view, vcx, "dog");
    assert!(
        !palette_offers(&view, vcx, CommandId::Forward, "forward"),
        "an unsynced new destination truncates Forward"
    );
}

#[core::prelude::v1::test]
fn library_switch_clears_forward() {
    let (_dir, engine) = seed_library("hist_switch", 2);
    let (other, _) = seed_library("hist_switch_other", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.select_album(FAVORITES, cx));
    render_ui(vcx);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, cx| {
        assert!(app.can_navigate_forward());
        app.open_library(other.clone(), cx);
        assert!(
            !app.can_navigate_forward() && !app.can_navigate_back(),
            "no Back or Forward across libraries"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_undo_redo_rating_and_sidecar() {
    let (_temp_dir, engine) = seed_library("undo", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let sidecar = lumina_core::sidecar_path(&item.path);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.set_rating_selected(4, cx);
    });
    // Sidecars are written on the background executor.
    vcx.run_until_parked();
    assert!(sidecar.exists(), "rating should write an XMP sidecar");

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_ratings.get(&item.id).copied().unwrap_or(0),
            0,
            "undo should clear the rating"
        );
    });
    assert!(
        !sidecar.exists(),
        "undoing the only metadata should remove the sidecar"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_ratings.get(&item.id).copied().unwrap_or(0),
            4,
            "redo should restore the rating"
        );
    });
    assert!(sidecar.exists(), "redo should rewrite the sidecar");
}

#[core::prelude::v1::test]
fn ui_automation_undo_tag_and_caption() {
    let (_temp_dir, engine) = seed_library("undo_meta", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.selected_indices.insert(0);
        app.batch_add_tag("travel".to_string(), cx);
    });
    view.update(vcx, |app, _| {
        assert!(app.selected_item_tags.contains(&"travel".to_string()));
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_item_tags.contains(&"travel".to_string()),
            "undo should remove the tag"
        );
    });
    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert!(app.selected_item_tags.contains(&"travel".to_string()));
    });

    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        app.caption.input_buffer = "A short caption".to_string();
        app.commit_caption_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_captions.get(&item.id).map(String::as_str),
            Some("A short caption")
        );
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert!(!app.item_captions.contains_key(&item.id));
    });
}

#[core::prelude::v1::test]
fn ui_automation_title_edit_undo_and_xmp_import() {
    let (_temp_dir, engine) = seed_library("title_import", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let sidecar = lumina_core::sidecar_path(&item.path);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.start_title_input(cx);
        app.caption.title_input_buffer = "Kyoto Trip".to_string();
        app.commit_title_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.title.as_deref()),
            Some("Kyoto Trip")
        );
    });
    vcx.run_until_parked();
    let written = lumina_core::read_sidecar(&item.path).unwrap().unwrap();
    assert_eq!(written.title.as_deref(), Some("Kyoto Trip"));

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.title.as_deref()),
            None,
            "undo should clear the title"
        );
    });
    assert!(!sidecar.exists(), "empty title should remove the sidecar");

    lumina_core::write_sidecar(
        &item.path,
        &lumina_core::SidecarMetadata {
            title: Some("Imported title".to_string()),
            caption: Some("Imported caption".to_string()),
            rating: 5,
            color_label: 0,
            tags: vec!["imported".to_string()],
            ..Default::default()
        },
    )
    .unwrap();
    view.update(vcx, |app, cx| {
        app.import_sidecars_selected(cx);
    });
    // The sidecars are read in the background.
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.title.as_deref()),
            Some("Imported title")
        );
        assert_eq!(app.item_ratings.get(&item.id).copied(), Some(5));
        assert_eq!(
            app.item_captions.get(&item.id).map(String::as_str),
            Some("Imported caption")
        );
        assert!(app.selected_item_tags.contains(&"imported".to_string()));
    });
}

#[core::prelude::v1::test]
fn ui_automation_date_and_location_edit_undo() {
    let (_temp_dir, engine) = seed_library("edit_time_place", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let original_timestamp = item.timestamp_epoch_ms;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selected_item = Some(item.clone());
        app.start_date_input(cx);
        app.caption.date_input_buffer = "2020-05-06".to_string();
        app.commit_date_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.year),
            Some(2020),
            "date edit should update the selected item"
        );
        assert!(app
            .timeline_year_buckets
            .iter()
            .any(|(year, _)| *year == 2020));
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|it| it.timestamp_epoch_ms),
            Some(original_timestamp),
            "undo should restore the original date"
        );
    });
    keystrokes(vcx, "secondary-shift-z");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_item.as_ref().map(|it| it.year), Some(2020));
    });

    view.update(vcx, |app, cx| {
        app.start_location_input(cx);
        app.caption.location_input_buffer = "48.8566, 2.3522".to_string();
        app.commit_location_input(cx);
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.latitude)
                .map(|lat| (lat * 10_000.0).round() / 10_000.0),
            Some(48.8566)
        );
        assert_eq!(
            app.selected_item
                .as_ref()
                .and_then(|it| it.longitude)
                .map(|lon| (lon * 10_000.0).round() / 10_000.0),
            Some(2.3522)
        );
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().and_then(|it| it.latitude),
            None,
            "undo should clear the location"
        );
    });
}
