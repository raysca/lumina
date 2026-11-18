//! Regressions: shortcuts the menus
//! advertise, chords that were swallowed, and state that vanished.

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use crate::app::viewer_actions::REVEALED_PATHS;
use crate::tests::helpers::{
    app_with_library, click, click_point, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use lumina_core::DeletionTarget;

#[core::prelude::v1::test]
fn shift_cmd_r_reveals_the_selection_instead_of_rescanning() {
    let (_dir, engine) = seed_library("reg_reveal", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    REVEALED_PATHS.with(|p| p.borrow_mut().clear());
    view.update(vcx, |app, _| {
        app.grid_state.as_mut().unwrap().is_scanning = false
    });

    keystrokes(vcx, "home shift-secondary-r");
    let selected = view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().unwrap();
        assert!(!state.is_scanning, "⇧⌘R must not start a rescan");
        app.selected_item.clone().unwrap().path
    });
    REVEALED_PATHS.with(|p| {
        assert_eq!(*p.borrow(), [selected], "⇧⌘R reveals the selected file");
    });

    keystrokes(vcx, "secondary-r");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().unwrap().is_scanning,
            "plain ⌘R still rescans"
        );
    });
}

#[core::prelude::v1::test]
fn alt_cmd_c_copies_the_selected_file_path() {
    let (_dir, engine) = seed_library("reg_copy", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home alt-secondary-c");
    let path = view.update(vcx, |app, _| app.selected_item.clone().unwrap().path);
    let copied = vcx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .expect("⌥⌘C writes the clipboard");
    assert_eq!(copied, path.to_string_lossy());
}

#[core::prelude::v1::test]
fn t_opens_the_tag_field_for_the_selection() {
    let (_dir, engine) = seed_library("reg_tag_key", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home t");
    view.update(vcx, |app, _| {
        assert!(app.tag_input_active, "T starts tagging the selection");
        assert!(app.inspector_open, "the inspector docks to show the field");
    });
    keystrokes(vcx, "s u n enter");
    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    assert_eq!(engine.get_tags_for_item(id), ["sun"]);
}

#[core::prelude::v1::test]
fn context_menu_tags_tags_the_selection_instead_of_opening_the_manager() {
    let (_dir, engine) = seed_library("reg_tag_menu", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    crate::tests::helpers::right_click(vcx, leak(format!("media-card-{}", id.0)));
    click(vcx, "tags-menu-btn");
    view.update(vcx, |app, _| {
        assert!(app.tag_input_active, "Tags… starts the tag field");
        assert!(
            !app.tags_page_shown(),
            "Tags… no longer opens the Tag Manager"
        );
    });
}

#[core::prelude::v1::test]
fn cmd_backspace_deletes_like_backspace() {
    let (_dir, engine) = seed_library("reg_cmd_bs", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home");
    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    keystrokes(vcx, "secondary-backspace");
    view.update(vcx, |app, cx| {
        if app.delete_confirm_state.is_some() {
            app.execute_confirmed_deletion(cx);
        }
    });
    assert!(
        engine.get_item_by_id(id).unwrap().is_trashed,
        "⌘⌫ (the menu's shortcut) moves the item to the Trash"
    );
}

#[core::prelude::v1::test]
fn viewer_f_favorites_and_ctrl_cmd_f_goes_full_screen() {
    let (_dir, engine) = seed_library("reg_viewer_f", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home enter");
    let id = view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer);
        app.selected_item.clone().unwrap().id
    });
    keystrokes(vcx, "f");
    assert!(
        engine.get_item_by_id(id).unwrap().is_favorite,
        "F favorites in the viewer, as in the grid"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.is_immersive_fullscreen,
            "F no longer toggles full screen"
        );
    });

    keystrokes(vcx, "ctrl-cmd-f");
    view.update(vcx, |app, _| {
        assert!(app.is_immersive_fullscreen, "⌃⌘F toggles full screen");
        assert!(
            !app.search_focused,
            "⌃⌘F must not be swallowed by ⌘F search"
        );
    });
}

#[core::prelude::v1::test]
fn enter_without_the_permanent_acknowledgement_keeps_the_dialog_open() {
    let (_dir, engine) = seed_library("reg_perm_ack", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home");
    let id = view.update(vcx, |app, cx| {
        let id = app.selected_item.clone().unwrap().id;
        app.trash_or_restore_selected(cx);
        app.select_album(AlbumSelection::Trash, cx);
        id
    });
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert_eq!(app.get_active_count(), 1, "the trashed item is listed");
        app.select_index(0, cx);
    });
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, cx| {
        assert!(app.delete_confirm_state.is_some(), "⌫ in Trash asks first");
        app.set_delete_target(DeletionTarget::PermanentDisk, cx);
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_some(),
            "Enter before the acknowledgement must leave the dialog open"
        );
    });
    assert!(engine.get_item_by_id(id).is_some(), "nothing was deleted");
}

#[core::prelude::v1::test]
fn viewer_next_button_moves_exactly_one_item() {
    let (_dir, engine) = seed_library("reg_viewer_nav", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home enter");
    click(vcx, "viewer-next");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(1), "one click, one step");
    });
}

#[core::prelude::v1::test]
fn deleting_a_smart_album_can_be_undone_with_the_same_id() {
    let (_dir, engine) = seed_library("reg_album_undo", 2);
    let rules = [SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    }];
    let album = engine
        .create_smart_album("Keepers", "folder", MatchMode::All, &rules)
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    click(vcx, leak(format!("smart-album-delete-{}", album.id)));
    click(vcx, "smart-album-delete-confirm");
    assert!(
        engine.list_smart_albums().is_empty(),
        "confirming deletes the album"
    );
    assert!(
        vcx.debug_bounds("toast-action").is_some(),
        "the toast offers Undo"
    );

    keystrokes(vcx, "secondary-z");
    let restored = engine.list_smart_albums();
    assert_eq!(restored.len(), 1, "⌘Z restores the album");
    assert_eq!(restored[0].id, album.id, "under its original id");
    assert_eq!(restored[0].title, "Keepers");
    view.update(vcx, |app, _| assert_eq!(app.user_smart_albums.len(), 1));

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx)
    });
    keystrokes(vcx, "shift-secondary-z");
    assert!(
        engine.list_smart_albums().is_empty(),
        "⇧⌘Z deletes it again"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "redo must not leave the view on the deleted album"
        )
    });
}

#[core::prelude::v1::test]
fn info_toggle_keeps_its_selector_and_the_inspector_stays_with_no_selection() {
    let (_dir, engine) = seed_library("reg_inspector", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(app.inspector_open));
    assert!(
        vcx.debug_bounds("inspector-toggle").is_some(),
        "the lit Info toggle keeps its automation selector"
    );
    view.update(vcx, |app, cx| app.clear_selection(cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("inspector-empty").is_some(),
        "with Info on and nothing selected the inspector shows its empty state"
    );
    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| {
        assert!(!app.inspector_open, "and still closes")
    });
}

#[core::prelude::v1::test]
fn palette_keyboard_selection_stays_in_view() {
    let (_dir, engine) = seed_library("reg_palette_scroll", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "secondary-k");
    let count = view.update(vcx, |app, _| app.palette_rows().len());
    assert!(count > 12, "the test needs a list longer than the viewport");
    for _ in 0..count - 1 {
        keystrokes(vcx, "down");
    }
    let key = view.update(vcx, |app, _| {
        app.palette_rows()[app.command_selected].selector.clone()
    });
    let list = vcx.debug_bounds("command-list").expect("palette list");
    let row = vcx.debug_bounds(leak(key)).expect("selected row");
    assert!(
        row.top() >= list.top() && row.bottom() <= list.bottom(),
        "the last row ({row:?}) must be scrolled into the list ({list:?})"
    );

    // Reopening resets the selection to the first row; it must be visible too.
    keystrokes(vcx, "escape secondary-k");
    let first = view.update(vcx, |app, _| app.palette_rows()[0].selector.clone());
    let list = vcx.debug_bounds("command-list").expect("palette list");
    let row = vcx.debug_bounds(leak(first)).expect("first row");
    assert!(
        row.top() >= list.top(),
        "after reopening, the selected first row ({row:?}) must be in view ({list:?})"
    );
}

#[core::prelude::v1::test]
fn viewer_specs_show_only_known_facts() {
    let (_dir, engine) = seed_library("reg_specs", 1);
    let mut item = engine.get_item_slice(0..1).remove(0);
    item.width = 0;
    item.height = 0;
    item.file_size = 2048;
    let line = crate::views::viewer::top_bar::specs_line(&item);
    assert_eq!(line, "JPG · 2.0 KB", "no invented dimensions, codec or fps");
    item.width = 4032;
    item.height = 3024;
    assert!(crate::views::viewer::top_bar::specs_line(&item).starts_with("4032 × 3024 · JPG"));
}

#[core::prelude::v1::test]
fn a_frame_step_accepts_the_frame_it_lands_on() {
    use crate::app::types::{get_sample_items, UiMessage};
    let video = get_sample_items()
        .into_iter()
        .find(|item| item.media_type == lumina_core::MediaType::Video)
        .expect("sample video");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = None;
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video, cx);
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.video.duration = 60.0;
        app.video.is_playing = false;
        // A step from 5s landed somewhere the app could not predict (the stub
        // player reports 0s): a fixed seek target would reject that frame.
        app.video.playback_time = 5.0;
        app.video.seek_refresh_until =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
        app.video.step_from = Some(5.0);
        app.handle_ui_message(UiMessage::VideoFrameTick, cx);
        let landed = crate::player::backend::VideoPlayerBackend::position(
            app.video.player.as_ref().unwrap(),
        );
        assert_eq!(
            app.video.playback_time, landed,
            "the clock follows the frame the step landed on"
        );
        app.stop_video_ticker();
    });
}

/// Regression: Reveal in Finder on a file that has gone away did nothing at
/// all; it now says so in the error banner and launches nothing.
#[core::prelude::v1::test]
fn reveal_in_finder_reports_a_missing_file() {
    let (_dir, engine) = seed_library("reg_reveal_missing", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    REVEALED_PATHS.with(|p| p.borrow_mut().clear());
    keystrokes(vcx, "home");
    let path = view.update(vcx, |app, _| app.selected_item.clone().unwrap().path);
    std::fs::remove_file(&path).unwrap();

    keystrokes(vcx, "shift-secondary-r");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(crate::app::viewer_actions::EXTERNAL_EDITOR_FILE_MISSING_MESSAGE),
            "a vanished file must be reported, not silently ignored"
        );
    });
    REVEALED_PATHS.with(|p| assert!(p.borrow().is_empty(), "nothing is launched"));
}

#[core::prelude::v1::test]
fn deleting_a_smart_album_asks_first() {
    let (_dir, engine) = seed_library("reg_album_confirm", 3);
    let rules = [SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    }];
    let album = engine
        .create_smart_album("Keepers", "folder", MatchMode::All, &rules)
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let delete = leak(format!("smart-album-delete-{}", album.id));
    let items = engine.total_count();

    click(vcx, delete);
    assert_eq!(engine.list_smart_albums().len(), 1, "× only asks");
    assert!(
        vcx.debug_bounds("smart-album-delete-dialog").is_some(),
        "× opens the confirmation"
    );
    // Background shortcuts must not reach the grid while it is open.
    view.update(vcx, |app, cx| app.select_index(0, cx));
    focus_app(vcx, &view);
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "the dialog absorbs ⌫, so no media delete starts behind it"
        )
    });
    keystrokes(vcx, "escape");
    assert_eq!(engine.list_smart_albums().len(), 1, "Escape keeps it");
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.confirm_delete.is_none(),
            "the dialog closed"
        )
    });

    click(vcx, delete);
    click(vcx, "smart-album-delete-cancel");
    assert_eq!(engine.list_smart_albums().len(), 1, "Cancel keeps it");

    click(vcx, delete);
    click_point(vcx, 4.0, 4.0);
    assert_eq!(
        engine.list_smart_albums().len(),
        1,
        "a click outside keeps it"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.confirm_delete.is_none(),
            "the dialog closed"
        )
    });

    // A text field left active behind the dialog gets no typing.
    view.update(vcx, |app, cx| app.start_tag_input(cx));
    click(vcx, delete);
    focus_app(vcx, &view);
    keystrokes(vcx, "x");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_input_buffer.is_empty(),
            "typing goes nowhere while the dialog is open"
        )
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.tag_input_active = false;
        cx.notify();
    });

    // The palette reaches the same confirmation for the open album.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx);
        app.execute_command(crate::app::command_palette::CommandId::DeleteSmartAlbum, cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.album_draft.confirm_delete, Some(album.id))
    });
    keystrokes(vcx, "escape");

    click(vcx, delete);
    keystrokes(vcx, "enter");
    assert!(engine.list_smart_albums().is_empty(), "↵ deletes it");
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.confirm_delete.is_none(),
            "the dialog closed"
        )
    });
    assert_eq!(
        engine.total_count(),
        items,
        "no photo or video goes with it"
    );
}

#[core::prelude::v1::test]
fn opening_another_library_drops_a_pending_smart_album_delete() {
    let (_dir, engine) = seed_library("reg_album_confirm_switch", 2);
    let (other, _other_engine) = seed_library("reg_album_confirm_other", 2);
    let rules = [SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    }];
    let album = engine
        .create_smart_album("Keepers", "folder", MatchMode::All, &rules)
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, leak(format!("smart-album-delete-{}", album.id)));
    view.update(vcx, |app, cx| app.open_library_inner(other.clone(), cx));
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.confirm_delete.is_none(),
            "ids restart per library, so the question cannot carry over"
        )
    });
}

/// Lumina › Preferences… while settings is open closes it, as ⌘, does,
/// instead of jumping back to General.
#[core::prelude::v1::test]
fn preferences_menu_item_toggles_settings_like_cmd_comma() {
    let (_dir, engine) = seed_library("reg_prefs_toggle", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| {
        app.open_preferences_at(crate::app::preferences::PreferencesTab::Storage, cx)
    });
    render_ui(vcx);
    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            !app.preferences_open,
            "Preferences… while settings is open closes it (tab was {:?})",
            app.preferences_tab
        );
    });
    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "and opens it again");
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::General,
            "reopened on General, as ⌘, does"
        );
    });
}

/// A second Clear Cache while one runs is ignored; both Clear buttons say
/// so by reading "Clearing…" until the clear's new reading lands.
#[core::prelude::v1::test]
fn clear_cache_buttons_read_clearing_while_a_clear_runs() {
    use crate::app::preferences::{thumbnail_cache_dir, PreferencesTab};
    let (_dir, engine) = seed_library("reg_clearing", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let cache = thumbnail_cache_dir();
    let _ = std::fs::remove_dir_all(&cache);
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("1.jpg"), b"thumb").unwrap();
    // Draws without running background work, so the clear stays pending.
    let draw = |vcx: &mut gpui::VisualTestContext| {
        vcx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
    };

    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Library, cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-storage-clear-cache-text:Clearing…")
            .is_none(),
        "idle, the card offers to clear"
    );
    // Pressed inside an update: a simulated click runs the clear to the end.
    view.update(vcx, |app, cx| {
        crate::app::preferences::SettingsButton::StorageCardClear.press(app, cx)
    });
    draw(vcx);
    assert!(
        vcx.debug_bounds("settings-storage-clear-cache-text:Clearing…")
            .is_some(),
        "while a clear runs the card says so, so a second press is not silently ignored"
    );

    view.update(vcx, |app, cx| {
        app.set_preferences_tab(PreferencesTab::Storage, cx)
    });
    draw(vcx);
    assert!(
        vcx.debug_bounds("preferences-clear-cache-text:Clearing…")
            .is_some(),
        "the Storage page's button says so too"
    );

    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.preferences_cache_job.clearing, "the clear finished")
    });
    assert!(
        vcx.debug_bounds("preferences-clear-cache-text:Clear Cache")
            .is_some(),
        "the button reads Clear Cache again once done"
    );
    let _ = std::fs::remove_dir_all(&cache);
}
