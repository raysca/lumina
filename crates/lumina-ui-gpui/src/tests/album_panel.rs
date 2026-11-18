//! The album settings panel, the right column of an open
//! album while Info is off, with Delete Album.

use super::album_page::{library_with_album, open_album, painted};
use super::helpers::{app_with_library, click, focus_app, keystrokes, render_ui, type_text};
use gpui::VisualTestContext;

fn width(vcx: &mut VisualTestContext, selector: &'static str) -> f32 {
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"));
    f32::from(bounds.size.width)
}

#[core::prelude::v1::test]
fn album_panel_delete_opens_confirm() {
    let (_dir, engine, id, _ids) = library_with_album("album_panel_delete");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);

    let got = width(vcx, "album-panel");
    assert!(
        (got - 319.5).abs() <= 0.5,
        "the album panel is 319.5 wide with Info off: {got}"
    );
    assert!(
        painted(
            vcx,
            "album-panel-hint-text:The 3 items stay in your library.".into()
        ),
        "the hint counts the album's members"
    );

    // Info takes the right column; off again, the panel returns.
    keystrokes(vcx, "i");
    assert!(
        vcx.debug_bounds("album-panel").is_none() && vcx.debug_bounds("inspector-panel").is_some(),
        "with Info on the inspector replaces the album panel"
    );
    keystrokes(vcx, "i");
    assert!(
        vcx.debug_bounds("album-panel").is_some(),
        "with Info off the album panel returns"
    );

    click(vcx, "album-panel-delete");
    assert!(
        vcx.debug_bounds("album-delete-dialog").is_some(),
        "Delete Album asks first"
    );
    assert_eq!(
        engine.list_albums_with_counts().len(),
        1,
        "nothing is deleted before the confirmation"
    );
    click(vcx, "album-delete-confirm");
    assert!(
        engine.list_albums_with_counts().is_empty(),
        "confirming deletes the album from the catalog"
    );
    assert_eq!(engine.total_count(), 3, "its items stay in the library");
    view.update(vcx, |app, _| {
        assert!(
            app.albums_overview_shown(),
            "deleting the open album returns to the Albums overview"
        );
    });
    assert!(
        vcx.debug_bounds("album-panel").is_none(),
        "the panel leaves with the album"
    );
}

#[core::prelude::v1::test]
fn album_panel_hint_counts_one_item() {
    let (_dir, engine) = super::helpers::seed_library("album_panel_one", 1);
    let one = engine.get_item_slice(0..1)[0].id;
    let album = engine.create_album_with("Solo", &[one]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, album.id);
    assert!(
        painted(
            vcx,
            "album-panel-hint-text:The 1 item stays in your library.".into()
        ),
        "one member reads in the singular"
    );
}

#[core::prelude::v1::test]
fn album_panel_collapses_when_narrow_and_palette_toggles() {
    let (_dir, engine, id, _ids) = library_with_album("album_panel_toggle");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    let row_with_panel = view.update(vcx, |app, _| app.row_height());

    // Under 1100 px the panel gives its width back to the grid.
    vcx.simulate_resize(gpui::size(gpui::px(1099.0), gpui::px(900.0)));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-panel").is_none(),
        "the panel collapses in a window under 1100 px"
    );
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-panel").is_some(),
        "the panel returns in a wide window"
    );

    // ⌘K Hide / Show Album Settings, by keyboard.
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "hide album settings");
    keystrokes(vcx, "enter");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-panel").is_none(),
        "Hide Album Settings hides the panel"
    );
    view.update(vcx, |app, _| {
        assert!(app.config.album_panel_hidden, "the choice is in the config");
        assert!(
            app.row_height() > row_with_panel,
            "the grid takes the panel's width back"
        );
    });
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "show album settings");
    keystrokes(vcx, "enter");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("album-panel").is_some(),
        "Show Album Settings brings it back"
    );

    // A failed save keeps the panel as the saved setting has it.
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    view.update(vcx, |app, cx| app.toggle_album_panel(cx));
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert!(
            !app.config.album_panel_hidden && app.error_message.is_some(),
            "a failed save reverts the toggle and shows the error"
        );
    });
}

/// The grid and the Mosaic size to the right column: the panel takes its
/// 319.5, Info its width plus the resizer, and hiding both gives it back.
#[core::prelude::v1::test]
fn album_panel_and_inspector_narrow_grid_and_mosaic() {
    let (_dir, engine, id, _ids) = library_with_album("album_panel_widths");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);
    let measure = |view: &gpui::Entity<crate::app::LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| (app.row_height(), app.mosaic_width()))
    };
    let (panel_row, panel_mosaic) = measure(&view, vcx);
    view.update(vcx, |app, cx| app.toggle_album_panel(cx));
    render_ui(vcx);
    let (open_row, open_mosaic) = measure(&view, vcx);
    assert!(
        (open_mosaic - panel_mosaic - 319.5).abs() < 0.01,
        "the panel takes 319.5 off the Mosaic: {panel_mosaic} vs {open_mosaic}"
    );
    assert!(open_row > panel_row, "the panel narrows the grid rows");

    view.update(vcx, |app, cx| app.toggle_inspector(cx));
    render_ui(vcx);
    let (info_row, info_mosaic) = measure(&view, vcx);
    let inspector = view.update(vcx, |app, _| app.inspector_width);
    assert!(
        (open_mosaic - info_mosaic - inspector - crate::views::grid::RESIZER_WIDTH).abs() < 0.01,
        "Info takes its width and resizer off the Mosaic: {info_mosaic} vs {open_mosaic}"
    );
    assert!(info_row < open_row, "Info narrows the grid rows");
    assert!(
        vcx.debug_bounds("album-panel").is_none(),
        "the panel stays out while Info is on"
    );
}

/// Redo of an album delete made from its page lands where the confirm
/// did: the Albums overview.
#[core::prelude::v1::test]
fn redo_album_delete_returns_to_the_overview() {
    let (_dir, engine, id, _ids) = library_with_album("album_panel_redo");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    click(vcx, "album-panel-delete");
    click(vcx, "album-delete-confirm");
    keystrokes(vcx, "cmd-z");
    assert_eq!(
        engine.list_albums_with_counts().len(),
        1,
        "undo restores it"
    );
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-shift-z");
    assert!(
        engine.list_albums_with_counts().is_empty(),
        "redo deletes it again"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::AlbumsOverview,
            "redo leaves the deleted album's page for the Albums overview"
        );
    });
}
