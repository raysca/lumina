//! Design 05 Task 5 follow-ups: smart card arrows and ⌘⌫, a focus whose
//! album is deleted, and mosaics that follow a rule edit.

use lumina_core::MatchMode;

use super::album_cards::{open_overview_at_1600, two_album_overview};
use super::smart_cards::{iso_over_1600, photos};
use super::ui_automation::{
    app_with_library, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::app::albums_overview::OverviewFocus;

fn smart_library(
    prefix: &str,
    n: usize,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    Vec<i64>,
) {
    let (dir, engine) = seed_library(prefix, 1);
    let ids = (0..n)
        .map(|i| {
            engine
                .create_smart_album(&format!("S{i}"), "sparkles", MatchMode::All, &[photos()])
                .unwrap()
                .id
        })
        .collect();
    (dir, engine, ids)
}

fn focus(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> Option<OverviewFocus> {
    view.update(vcx, |app, _| app.albums.overview.focus)
}

#[core::prelude::v1::test]
fn arrows_move_between_smart_cards() {
    let (dir, engine, s) = smart_library("smart_keys_arrows", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    focus_app(vcx, &view);
    for _ in 0..3 {
        keystrokes(vcx, "tab");
    }
    let at = |i: usize| Some(OverviewFocus::SmartCard(s[i]));
    assert_eq!(focus(&view, vcx), at(0));
    keystrokes(vcx, "right");
    assert_eq!(focus(&view, vcx), at(1), "→ moves to the next smart card");
    for _ in 0..3 {
        keystrokes(vcx, "right");
    }
    assert_eq!(focus(&view, vcx), at(3), "→ stops at the last");
    keystrokes(vcx, "left");
    keystrokes(vcx, "down");
    assert_eq!(focus(&view, vcx), at(2), "↓ in a single row stays put");

    // Two per row in a narrow window: ↑/↓ move a row.
    vcx.simulate_resize(gpui::size(gpui::px(860.0), gpui::px(1000.0)));
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "left");
    keystrokes(vcx, "left");
    assert_eq!(focus(&view, vcx), at(0));
    keystrokes(vcx, "down");
    assert_eq!(focus(&view, vcx), at(2), "↓ moves down a row of two");
    keystrokes(vcx, "down");
    assert_eq!(focus(&view, vcx), at(2), "↓ on the last row stays put");
    keystrokes(vcx, "right");
    keystrokes(vcx, "up");
    assert_eq!(focus(&view, vcx), at(1), "↑ moves up a row of two");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn deleting_the_focused_smart_card_focuses_a_neighbour() {
    let (dir, engine, s) = smart_library("smart_keys_delete", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    focus_app(vcx, &view);
    for _ in 0..3 {
        keystrokes(vcx, "tab");
    }
    for (deleted, then) in [(s[0], Some(s[1])), (s[1], None)] {
        keystrokes(vcx, "cmd-backspace");
        view.update(vcx, |app, cx| {
            assert_eq!(
                app.album_draft.confirm_delete,
                Some(deleted),
                "⌘⌫ asks to delete the focused smart card"
            );
            app.confirm_delete_smart_album(cx);
        });
        render_ui(vcx);
        assert_eq!(
            focus(&view, vcx),
            then.map(OverviewFocus::SmartCard),
            "after deleting {deleted} the focus moves to a neighbour, else clears"
        );
        focus_app(vcx, &view);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn an_album_deleted_elsewhere_moves_its_card_focus() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, [alpha, beta], view, vcx) =
        two_album_overview("smart_keys_album_gone", &mut cx);
    view.update(vcx, |app, _| {
        app.albums.overview.focus = Some(OverviewFocus::Card(beta));
    });
    render_ui(vcx);
    // Not through the card: an engine write, as the sidebar or another
    // window would leave it.
    engine.delete_album(beta).unwrap();
    view.update(vcx, |app, _| app.refresh_album_counts());
    render_ui(vcx);
    assert_eq!(
        focus(&view, vcx),
        Some(OverviewFocus::Card(alpha)),
        "a focused card whose album is gone hands the focus to its neighbour"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_rule_edit_refreshes_the_mosaic_and_count() {
    let (dir, engine, s) = smart_library("smart_keys_rule_edit", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_overview_at_1600(vcx);
    render_ui(vcx);
    let tiles = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            app.albums.overview.smart_tiles.get(&s[0]).map(Vec::len)
        })
    };
    assert_eq!(tiles(&view, vcx), Some(1));
    assert!(vcx
        .debug_bounds("smart-card-meta-text:1 item · live")
        .is_some());
    engine
        .update_smart_album(s[0], "S0", "sparkles", MatchMode::All, &[iso_over_1600()])
        .unwrap();
    view.update(vcx, |app, _| app.refresh_album_counts());
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(
        tiles(&view, vcx),
        Some(0),
        "the mosaic follows the new rule"
    );
    assert!(
        vcx.debug_bounds(leak("smart-card-meta-text:0 items · live".into()))
            .is_some(),
        "and so does the count"
    );
    assert!(
        vcx.debug_bounds("smart-chip-text:ISO > 1600").is_some(),
        "and the chips"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
