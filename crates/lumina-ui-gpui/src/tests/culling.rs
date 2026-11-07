//! Keyboard-first culling (plan 5 B2): P/X/U flags and 0-5 ratings in the
//! grid and the viewer, the viewer's auto-advance, the flag badge, the
//! palette flag commands and quick filters, and the guards that keep
//! modals, text fields and ⌥P out of it.

use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click, seed_library,
};
use crate::*;
use lumina_core::{Flag, LuminaEngine, MediaId};

pub(super) fn card(id: MediaId) -> &'static str {
    leak(format!("media-card-{}", id.0))
}

pub(super) fn ids(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<MediaId> {
    view.update(vcx, |app, _| {
        app.get_active_slice(0..app.get_active_count())
            .into_iter()
            .map(|item| item.id)
            .collect()
    })
}

pub(super) fn flags(engine: &LuminaEngine) -> std::collections::HashMap<MediaId, Flag> {
    engine.get_flags_map()
}

pub(super) fn rating(engine: &LuminaEngine, id: MediaId) -> u8 {
    engine.catalog().get_rating(id).unwrap()
}

fn type_keys(vcx: &mut VisualTestContext, text: &str) {
    for ch in text.chars() {
        keystrokes(
            vcx,
            if ch == ' ' {
                "space"
            } else {
                leak(ch.to_string())
            },
        );
    }
}

#[core::prelude::v1::test]
fn p_x_u_flag_selection_and_undo_groups_batch() {
    let (dir, engine) = seed_library("cull_pxu", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    let (a, b) = (ids[0], ids[1]);

    click(vcx, card(a));
    focus_app(vcx, &view);
    keystrokes(vcx, "shift-right");
    let focus = view.update(vcx, |app, _| app.selected_index);
    let both = |f: Flag| {
        [(a, f), (b, f)]
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>()
    };

    keystrokes(vcx, "p");
    assert_eq!(
        flags(&engine),
        both(Flag::Pick),
        "P picks the whole selection"
    );
    assert!(
        vcx.debug_bounds(leak(format!("grid-flag-badge-{}", a.0)))
            .is_some(),
        "a picked card shows its flag badge"
    );
    keystrokes(vcx, "p");
    assert!(
        flags(&engine).is_empty(),
        "P on a selection that is all picked clears the flag"
    );
    keystrokes(vcx, "x");
    assert_eq!(
        flags(&engine),
        both(Flag::Reject),
        "X rejects the selection"
    );
    keystrokes(vcx, "u");
    assert!(flags(&engine).is_empty(), "U unflags the selection");
    view.update(vcx, |app, _| {
        assert!(
            app.item_flags.is_empty(),
            "the flag map follows the catalog"
        );
        assert_eq!(
            app.selected_indices.len(),
            2,
            "a flag key never moves the grid selection"
        );
        assert_eq!(app.selected_index, focus, "the grid selection stays put");
    });

    keystrokes(vcx, "secondary-z");
    assert_eq!(
        flags(&engine),
        both(Flag::Reject),
        "one undo restores the whole batch to its earlier flag"
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.item_flags.get(&a), Some(&Flag::Reject));
    });
    keystrokes(vcx, "shift-secondary-z");
    assert!(flags(&engine).is_empty(), "redo unflags both again");

    // Mixed earlier values: undo puts each item back to its own flag.
    click(vcx, card(a));
    keystrokes(vcx, "p");
    keystrokes(vcx, "shift-right");
    keystrokes(vcx, "x");
    assert_eq!(
        flags(&engine),
        both(Flag::Reject),
        "X on a mixed selection rejects both"
    );
    keystrokes(vcx, "secondary-z");
    assert_eq!(
        flags(&engine),
        [(a, Flag::Pick)].into_iter().collect(),
        "undo restores a's pick and leaves b unflagged"
    );

    // The context menu's Pick/Reject row acts on the selection and never
    // moves it.
    click(vcx, card(ids[2]));
    right_click(vcx, card(ids[2]));
    click(vcx, "context-flag-reject");
    assert_eq!(flags(&engine).get(&ids[2]), Some(&Flag::Reject));
    click(vcx, "context-flag-pick");
    assert_eq!(flags(&engine).get(&ids[2]), Some(&Flag::Pick));
    view.update(vcx, |app, _| assert_eq!(app.selected_index, Some(2)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn digit_keys_rate_selection_in_grid_and_viewer() {
    let (dir, engine) = seed_library("cull_digits", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);

    click(vcx, card(ids[0]));
    focus_app(vcx, &view);
    keystrokes(vcx, "3");
    assert_eq!(rating(&engine, ids[0]), 3, "3 rates the grid selection");
    keystrokes(vcx, "5");
    assert_eq!(rating(&engine, ids[0]), 5);
    keystrokes(vcx, "0");
    assert_eq!(rating(&engine, ids[0]), 0, "0 clears the rating");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(0),
            "rating in the grid never moves"
        )
    });

    keystrokes(vcx, "space");
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));
    keystrokes(vcx, "4");
    assert_eq!(rating(&engine, ids[0]), 4, "4 rates the viewer item");
    keystrokes(vcx, "[");
    keystrokes(vcx, "0");
    assert_eq!(rating(&engine, ids[0]), 0, "0 clears in the viewer too");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "digits never close the viewer");
        assert_eq!(app.item_ratings.get(&ids[0]), None);
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_flag_auto_advances() {
    let (dir, engine) = seed_library("cull_advance", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);

    click(vcx, card(ids[0]));
    focus_app(vcx, &view);
    keystrokes(vcx, "space");
    keystrokes(vcx, "p");
    let shown = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            (app.selected_index, app.selected_item.as_ref().map(|i| i.id))
        })
    };
    assert_eq!(flags(&engine).get(&ids[0]), Some(&Flag::Pick));
    assert_eq!(
        shown(&view, vcx),
        (Some(1), Some(ids[1])),
        "a flag key in the viewer moves to the next item"
    );
    keystrokes(vcx, "2");
    assert_eq!(rating(&engine, ids[1]), 2);
    assert_eq!(
        shown(&view, vcx),
        (Some(2), Some(ids[2])),
        "a rating key in the viewer moves to the next item"
    );
    keystrokes(vcx, "x");
    assert_eq!(flags(&engine).get(&ids[2]), Some(&Flag::Reject));
    assert_eq!(
        shown(&view, vcx),
        (Some(2), Some(ids[2])),
        "on the last item the viewer stays put"
    );
    view.update(vcx, |app, _| assert!(app.is_fullscreen_viewer));

    // The palette never advances.
    keystrokes(vcx, "[");
    keystrokes(vcx, "secondary-k");
    type_keys(vcx, "pick");
    keystrokes(vcx, "enter");
    assert_eq!(flags(&engine).get(&ids[1]), Some(&Flag::Pick));
    assert_eq!(
        shown(&view, vcx),
        (Some(1), Some(ids[1])),
        "a palette flag never advances"
    );

    // Nor does the viewer's context menu.
    right_click(vcx, "viewer-viewport");
    click(vcx, "context-flag-reject");
    assert_eq!(flags(&engine).get(&ids[1]), Some(&Flag::Reject));
    assert_eq!(
        shown(&view, vcx),
        (Some(1), Some(ids[1])),
        "a context-menu flag never advances"
    );
    keystrokes(vcx, "escape");

    let _ = std::fs::remove_dir_all(&dir);
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn alt_p_still_toggles_pip() {
    let (dir, _) = seed_library("cull_pip", 0);
    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
    std::fs::copy(&clip, dir.join("video.mp4")).unwrap();
    let (engine, rx) =
        LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), Some(dir.join("cache")))
            .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let video = ids(&view, vcx)[0];

    click(vcx, card(video));
    focus_app(vcx, &view);
    keystrokes(vcx, "alt-p");
    view.update(vcx, |app, _| {
        assert!(app.video.in_pip, "⌥P opens PiP in the grid")
    });
    assert!(flags(&engine).is_empty(), "⌥P is not P: nothing is picked");
    keystrokes(vcx, "alt-p");
    view.update(vcx, |app, _| assert!(!app.video.in_pip, "⌥P closes PiP"));

    keystrokes(vcx, "alt-secondary-u");
    view.update(vcx, |app, cx| {
        assert!(
            app.network_import.stage.is_some(),
            "⌥⌘U still opens Import Video from URL"
        );
        app.close_network_import_dialog(cx);
    });
    assert!(flags(&engine).is_empty(), "⌥⌘U is not U");
    render_ui(vcx);

    keystrokes(vcx, "space");
    keystrokes(vcx, "alt-p");
    view.update(vcx, |app, _| {
        assert!(app.video.in_pip, "⌥P in the viewer moves the video to PiP")
    });
    assert!(flags(&engine).is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn show_picks_palette_filter_adds_is_pick_token() {
    let (dir, engine) = seed_library("cull_filter", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    engine
        .set_flags(&[(ids[0], Flag::Pick), (ids[1], Flag::Reject)])
        .unwrap();
    view.update(vcx, |app, _| app.refresh_album_counts());

    keystrokes(vcx, "secondary-k");
    type_keys(vcx, "show picks");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:pick", "Show Picks adds the token");
        assert_eq!(app.get_active_count(), 1, "only the pick is shown");
    });

    keystrokes(vcx, "secondary-k");
    type_keys(vcx, "show rejected");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "is:reject",
            "Show Rejected replaces is:pick (one flag family)"
        );
        assert_eq!(app.get_active_count(), 1, "only the reject is shown");
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn flag_keys_absorbed_by_modals_and_text() {
    let (dir, engine) = seed_library("cull_absorb", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    click(vcx, card(ids[0]));
    focus_app(vcx, &view);

    // Every key, in an order that leaves a pick and a 5 if any got through.
    let untouched = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, case: &str| {
        assert!(flags(&engine).is_empty(), "{case}: no flag may be written");
        assert_eq!(
            rating(&engine, ids[0]),
            0,
            "{case}: no rating may be written"
        );
        view.update(vcx, |app, _| {
            assert!(
                app.item_flags.is_empty() && app.item_ratings.is_empty(),
                "{case}"
            );
        });
    };

    keystrokes(vcx, "secondary-k");
    view.update(vcx, |app, _| assert!(app.command_palette_open));
    type_keys(vcx, "uxp305");
    untouched(&view, vcx, "palette open");
    keystrokes(vcx, "escape");

    keystrokes(vcx, "t");
    view.update(vcx, |app, _| assert!(app.tag_input_active));
    type_keys(vcx, "uxp305");
    untouched(&view, vcx, "tag field focused");
    keystrokes(vcx, "escape");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.tag_input_active));

    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| assert!(app.delete_confirm_state.is_some()));
    type_keys(vcx, "uxp305");
    untouched(&view, vcx, "delete dialog open");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(app.delete_confirm_state.is_none()));

    // A sheet with no text field: only the modal shield stops the keys.
    keystrokes(vcx, "secondary-/");
    view.update(vcx, |app, _| assert!(app.shortcuts_overlay_open));
    type_keys(vcx, "uxp305");
    untouched(&view, vcx, "shortcut sheet open");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));

    keystrokes(vcx, "shift-secondary-a");
    view.update(vcx, |app, _| assert!(app.albums.picker.is_some()));
    type_keys(vcx, "uxp305");
    untouched(&view, vcx, "album picker open");
    keystrokes(vcx, "escape");

    // The guards above are what stopped them: the same keys now act.
    focus_app(vcx, &view);
    keystrokes(vcx, "p");
    assert_eq!(flags(&engine).get(&ids[0]), Some(&Flag::Pick));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn flag_write_failure_shows_banner_and_keeps_state() {
    let (dir, engine) = seed_library("cull_fail", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    click(vcx, card(ids[0]));
    focus_app(vcx, &view);

    crate::app::culling::FLAG_WRITE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "p");
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "test premise: viewer open")
    });
    keystrokes(vcx, "x");
    crate::app::culling::FLAG_WRITE_FAILS.with(|f| f.set(false));

    assert!(flags(&engine).is_empty(), "a failed write changes no row");
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.contains("database is locked")),
            "the banner says why the flag was not saved, got {:?}",
            app.error_message
        );
        assert!(
            app.item_flags.is_empty(),
            "the flag map never leads the catalog"
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a failed flag is not undoable"
        );
        assert_eq!(
            app.selected_index,
            Some(0),
            "a failed flag in the viewer does not advance"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}
