//! The Review Duplicates keyboard path and its palette commands.

use super::duplicates::run_palette_command;
use super::duplicates_review::seed_two_groups;
use super::helpers::{app_with_library, click, focus_app, keystrokes, leak, render_ui};
use gpui::{Entity, VisualTestContext};
use lumina_core::MediaId;

use crate::LuminaApp;

/// The ids of each duplicate group, in list order.
fn groups(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<Vec<MediaId>> {
    view.update(vcx, |app, _| {
        app.duplicate_groups()
            .iter()
            .map(|g| g.items.iter().map(|i| i.id).collect())
            .collect()
    })
}

/// The shown group's index among the visible groups, and its keeper.
fn shown(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> (Option<usize>, MediaId) {
    view.update(vcx, |app, _| {
        let visible = app.stacks.review.visible(app.duplicate_groups());
        let index = app.stacks.review.selected(&visible);
        let keeper = index.map_or(MediaId(0), |i| app.stacks.review.keeper(visible[i]));
        (index, keeper)
    })
}

fn selected(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<MediaId> {
    view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id))
}

fn active(engine: &lumina_core::LuminaEngine) -> Vec<MediaId> {
    engine.get_item_slice(0..100).iter().map(|i| i.id).collect()
}

#[core::prelude::v1::test]
fn duplicates_keyboard_review() {
    let (_temp_dir, engine) = seed_two_groups("dup_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let g = groups(&view, vcx);
    click(vcx, leak(format!("duplicate-candidate-{}", g[0][1].0)));
    focus_app(vcx, &view);

    keystrokes(vcx, "down");
    assert_eq!(shown(&view, vcx), (Some(1), g[1][0]), "↓ selects group 1");
    assert_eq!(
        selected(&view, vcx),
        None,
        "↓ clears the selection: ⌫ never acts on a copy no longer shown"
    );
    keystrokes(vcx, "2");
    assert_eq!(shown(&view, vcx).1, g[1][1], "2 keeps the second copy");
    keystrokes(vcx, "left");
    assert_eq!(shown(&view, vcx).1, g[1][0], "← keeps the copy before");
    keystrokes(vcx, "right");
    assert_eq!(shown(&view, vcx).1, g[1][1], "→ keeps the copy after");

    keystrokes(vcx, "s");
    assert_eq!(shown(&view, vcx), (Some(0), g[0][0]), "S skips to group 0");

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "↵ merges, it does not open the viewer"
        );
    });
    assert_eq!(active(&engine).len(), 3, "↵ merged group 0 into its keeper");

    keystrokes(vcx, "secondary-z");
    assert_eq!(active(&engine).len(), 5, "⌘Z undoes the merge");

    // With the delete dialog open, ↵ belongs to the dialog.
    render_ui(vcx);
    click(vcx, leak(format!("duplicate-candidate-{}", g[0][1].0)));
    view.update(vcx, |app, cx| app.prompt_delete_selected(cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "enter");
    assert!(
        active(&engine).contains(&g[0][2]),
        "↵ in the delete dialog never merges the group"
    );
    assert!(
        !active(&engine).contains(&g[0][1]),
        "the dialog took ↵: the selected copy moved to Recently Deleted"
    );
    view.update(vcx, |app, _| {
        assert!(app.delete_confirm_state.is_none(), "the dialog closed");
    });

    drop(view);
}

#[core::prelude::v1::test]
fn digit_picks_keeper_without_rating() {
    let (_temp_dir, engine) = seed_two_groups("dup_digit");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let g = groups(&view, vcx);
    // A selected copy is what the culling keys would rate.
    click(vcx, leak(format!("duplicate-candidate-{}", g[0][0].0)));
    focus_app(vcx, &view);

    keystrokes(vcx, "2");
    assert_eq!(shown(&view, vcx).1, g[0][1], "2 keeps the second copy");
    keystrokes(vcx, "9");
    assert_eq!(
        shown(&view, vcx).1,
        g[0][1],
        "9 is past the copies: nothing"
    );
    let ratings = engine.get_ratings_map();
    assert!(
        g.iter()
            .flatten()
            .all(|id| ratings.get(id).copied().unwrap_or(0) == 0),
        "a digit picks the keeper and never rates: {ratings:?}"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn cmd_down_opens_the_keeper_in_the_viewer() {
    let (_temp_dir, engine) = seed_two_groups("dup_open");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let g = groups(&view, vcx);
    focus_app(vcx, &view);

    keystrokes(vcx, "3");
    keystrokes(vcx, "secondary-down");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "⌘↓ opens the viewer");
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(g[0][2]),
            "with no copy selected, ⌘↓ opens the keeper"
        );
    });

    // The review's keys are not the viewer's: S skips nothing, 2 picks nothing.
    keystrokes(vcx, "s");
    keystrokes(vcx, "2");
    view.update(vcx, |app, _| {
        assert!(
            app.stacks.review.skipped.is_empty(),
            "S in the viewer skips nothing"
        );
    });
    assert_eq!(
        shown(&view, vcx).1,
        g[0][2],
        "2 in the viewer picks no keeper"
    );

    // A selected copy of the shown group is what ⌘↓ opens.
    keystrokes(vcx, "escape");
    render_ui(vcx);
    click(vcx, leak(format!("duplicate-candidate-{}", g[0][1].0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-down");
    assert_eq!(
        selected(&view, vcx),
        Some(g[0][1]),
        "⌘↓ opens the selected copy"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn palette_skips_all_groups() {
    let (_temp_dir, engine) = seed_two_groups("dup_pal_skip");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);

    run_palette_command(vcx, &view, "Skip All Groups");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state-headline-text:All groups skipped")
            .is_some(),
        "⌘K Skip All Groups is the keyboard path for Skip all"
    );
    assert_eq!(
        active(&engine).len(),
        5,
        "skipping changes nothing in the catalog"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn review_clicks_and_grid_keys_hold_no_hidden_selection() {
    let (_temp_dir, engine) = seed_two_groups("dup_hidden_sel");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let g = groups(&view, vcx);

    click(vcx, leak(format!("duplicate-candidate-{}", g[0][0].0)));
    click(vcx, "duplicates-group-row-1");
    assert_eq!(
        selected(&view, vcx),
        None,
        "a row click clears the selection"
    );
    click(vcx, leak(format!("duplicate-candidate-{}", g[1][0].0)));
    click(vcx, "duplicates-prev");
    assert_eq!(selected(&view, vcx), None, "prev clears the selection");

    focus_app(vcx, &view);
    for keys in ["secondary-a", "shift-down", "shift-right", "home", "end"] {
        keystrokes(vcx, keys);
        view.update(vcx, |app, _| {
            assert!(
                app.selected_indices.is_empty() && app.selected_item.is_none(),
                "{keys} selects nothing in the hidden grid"
            );
        });
    }
    keystrokes(vcx, "delete");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ has nothing to delete"
        );
    });

    for keys in ["shift-enter", "alt-enter"] {
        keystrokes(vcx, keys);
        view.update(vcx, |app, _| {
            assert!(!app.is_fullscreen_viewer, "{keys} never opens the viewer");
        });
    }
    assert_eq!(active(&engine).len(), 5, "and nothing was merged");

    drop(view);
}
