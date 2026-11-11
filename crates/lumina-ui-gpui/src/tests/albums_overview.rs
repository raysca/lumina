//! Design 05: the Albums overview page in the main column, reached from the
//! sidebar ALBUMS label, the palette and the Go menu.

use super::duplicates::run_palette_command;
use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;

fn assert_on_overview(vcx: &mut gpui::VisualTestContext, how: &str) {
    assert!(
        vcx.debug_bounds("albums-overview").is_some(),
        "{how} shows the albums overview"
    );
    for gone in [
        "media-grid-surface",
        "status-bar",
        "multi-select-action-dock",
    ] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{how}: the overview hides {gone}"
        );
    }
}

#[core::prelude::v1::test]
fn albums_overview_opens_from_sidebar_and_palette() {
    let (dir, engine) = seed_library("albums_overview_nav", 2);
    let album = engine.create_album("Lisbon").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    assert!(vcx.debug_bounds("media-grid-surface").is_some());

    click(vcx, "sidebar-albums-header");
    assert_on_overview(vcx, "clicking the ALBUMS label");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AlbumsOverview);
        for row in [
            AlbumSelection::AllLibrary,
            AlbumSelection::Album(album.id),
            AlbumSelection::Trash,
        ] {
            assert!(
                !app.sidebar_highlights(&row),
                "no sidebar row is active on the overview: {row:?}"
            );
        }
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "Back returns to the view before the overview"
        );
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("media-grid-surface").is_some());
    assert!(vcx.debug_bounds("albums-overview").is_none());

    run_palette_command(vcx, &view, "Show Albums");
    render_ui(vcx);
    assert_on_overview(vcx, "the palette's Show Albums");

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    vcx.dispatch_action(crate::actions::ShowAlbums);
    render_ui(vcx);
    assert_on_overview(vcx, "Go > Albums");
    let _ = std::fs::remove_dir_all(&dir);
}
