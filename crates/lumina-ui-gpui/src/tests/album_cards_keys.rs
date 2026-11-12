//! Design 05 Task 4: the album cards' keyboard focus and menus on the
//! Albums overview (tab changes, Esc, ⌘K Rename/Delete on a focused card).

use super::album_cards::{open_overview_at_1600, two_album_overview};
use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click, seed_library,
};
use crate::app::albums_overview::OverviewFocus;
use crate::views::sidebar::AlbumSelection;

/// ↵ and ⌘⌫ do nothing: no album opens, no delete is asked.
fn assert_keys_inert(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    how: &str,
) {
    keystrokes(vcx, "enter");
    keystrokes(vcx, "cmd-backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AlbumsOverview,
            "{how}: ↵ opens nothing"
        );
        assert_eq!(app.albums.confirm_delete, None, "{how}: ⌘⌫ asks nothing");
    });
}

#[core::prelude::v1::test]
fn switching_tabs_drops_the_card_focus_and_menus() {
    use crate::config::AlbumSort;
    let mut cx = crate::tests::TestApp::single();
    let (dir, _engine, [alpha, _], view, vcx) = two_album_overview("album_cards_tabs", &mut cx);
    let back_to_albums = |view: &gpui::Entity<crate::LuminaApp>,
                          vcx: &mut gpui::VisualTestContext| {
        click(vcx, "page-tab-albums");
        focus_app(vcx, view);
    };

    // A focused card, then the palette's Show Smart Albums Tab.
    focus_app(vcx, &view);
    for _ in 0..4 {
        keystrokes(vcx, "tab");
    }
    assert_eq!(
        view.update(vcx, |app, _| app.albums.overview.focus),
        Some(OverviewFocus::Card(alpha))
    );
    super::duplicates::run_palette_command(vcx, &view, "Show Smart Albums Tab");
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.overview.focus, None, "the card focus is dropped");
    });
    assert_keys_inert(&view, vcx, "card focus then the Smart tab");

    // The open sort menu, then a click on the Smart tab.
    back_to_albums(&view, vcx);
    click(vcx, "albums-sort");
    click(vcx, "page-tab-smart-albums");
    focus_app(vcx, &view);
    keystrokes(vcx, "up");
    assert_keys_inert(&view, vcx, "sort menu then the Smart tab");
    view.update(vcx, |app, _| {
        assert!(
            app.albums.overview.sort_menu.is_none(),
            "the sort menu closed"
        );
        assert_eq!(
            app.config.album_sort,
            AlbumSort::Name,
            "the sort is unchanged"
        );
    });

    // The open card menu, then a click on the Smart tab.
    back_to_albums(&view, vcx);
    right_click(vcx, leak(format!("album-card-{alpha}")));
    click(vcx, "page-tab-smart-albums");
    focus_app(vcx, &view);
    view.update(vcx, |app, _| {
        assert!(
            app.albums.overview.card_menu.is_none(),
            "the card menu closed"
        );
    });
    assert_keys_inert(&view, vcx, "card menu then the Smart tab");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn open_menus_absorb_keys_and_close_on_escape() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, _engine, [alpha, _], view, vcx) = two_album_overview("album_cards_menus", &mut cx);
    focus_app(vcx, &view);
    for _ in 0..4 {
        keystrokes(vcx, "tab");
    }
    right_click(vcx, leak(format!("album-card-{alpha}")));
    focus_app(vcx, &view);
    assert_keys_inert(&view, vcx, "the open card menu");
    assert!(
        vcx.debug_bounds("album-card-menu").is_some(),
        "the card menu stays open under ↵ and ⌘⌫"
    );
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("album-card-menu").is_none(),
        "Esc closes the card menu"
    );

    keystrokes(vcx, "shift-tab"); // the card -> Sort
    keystrokes(vcx, "enter");
    assert!(vcx.debug_bounds("albums-sort-menu").is_some());
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("albums-sort-menu").is_none(),
        "Esc closes the sort menu"
    );

    // The palette closes an open menu (it would paint above it).
    click(vcx, "albums-sort");
    super::duplicates::run_palette_command(vcx, &view, "Show Albums Tab");
    view.update(vcx, |app, _| {
        assert!(
            app.albums.overview.sort_menu.is_none(),
            "⌘K closes the sort menu"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn deleting_the_focused_card_focuses_a_neighbour() {
    let (dir, engine) = seed_library("album_cards_refocus", 1);
    let ids: Vec<i64> = ["A", "B", "C"]
        .iter()
        .map(|t| engine.create_album(t).unwrap().id)
        .collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    focus_app(vcx, &view);
    for _ in 0..4 {
        keystrokes(vcx, "tab");
    }
    keystrokes(vcx, "right");
    for (deleted, then) in [
        (ids[1], Some(ids[2])),
        (ids[2], Some(ids[0])),
        (ids[0], None),
    ] {
        keystrokes(vcx, "cmd-backspace");
        view.update(vcx, |app, cx| {
            assert_eq!(app.albums.confirm_delete, Some(deleted));
            app.confirm_delete_album(cx);
            assert_eq!(
                app.albums.overview.focus,
                then.map(OverviewFocus::Card),
                "after deleting {deleted} the focus moves to the next card, else the previous"
            );
        });
        render_ui(vcx);
        focus_app(vcx, &view);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_rename_and_delete_act_on_the_focused_card() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, _engine, [alpha, _], view, vcx) = two_album_overview("album_cards_palette", &mut cx);
    focus_app(vcx, &view);
    for _ in 0..4 {
        keystrokes(vcx, "tab");
    }
    super::duplicates::run_palette_command(vcx, &view, "Rename Album");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.albums.renaming,
            Some(alpha),
            "⌘K Rename Album… renames the focused card"
        );
        app.cancel_album_rename(cx);
    });
    super::duplicates::run_palette_command(vcx, &view, "Delete Album");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.confirm_delete,
            Some(alpha),
            "⌘K Delete Album… asks about the focused card"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn inspector_key_does_nothing_on_the_overview() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, _engine, _, view, vcx) = two_album_overview("album_cards_inspector", &mut cx);
    let before = view.update(vcx, |app, _| app.inspector_open);
    focus_app(vcx, &view);
    keystrokes(vcx, "i");
    assert_eq!(
        view.update(vcx, |app, _| app.inspector_open),
        before,
        "I does not flip the hidden inspector setting"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
