//! Cross-cutting v3 design rules (FIXES.md §1): sheets hang from under the
//! toolbar, modals dim the library lightly, and shared rows use the tokens.

use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, leak, render_ui, right_click, seed_library,
};

const TOOLBAR_HEIGHT: f32 = 52.0;

fn assert_hangs_from_toolbar(vcx: &mut gpui::VisualTestContext, selector: &'static str) {
    render_ui(vcx);
    let card = vcx.debug_bounds(selector).expect(selector);
    assert_eq!(
        f32::from(card.top()),
        TOOLBAR_HEIGHT,
        "{selector} must hang from the bottom of the toolbar, not be centred"
    );
}

#[core::prelude::v1::test]
fn tag_manager_is_a_sheet_under_the_toolbar() {
    let (dir, engine) = seed_library("ds_tag_sheet", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_tag_manager(cx));
    assert_hangs_from_toolbar(vcx, "tag-manager-dialog");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn smart_album_builder_is_a_760px_sheet_under_the_toolbar() {
    let (dir, engine) = seed_library("ds_album_sheet", 2);
    let mut cx = gpui::TestAppContext::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    click(vcx, "new-smart-album");
    assert_hangs_from_toolbar(vcx, "album-builder-card");
    let card = vcx.debug_bounds("album-builder-card").unwrap();
    assert_eq!(
        f32::from(card.size.width),
        760.0,
        "the design's sheet width"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn hovered_menu_row_is_tracked_for_dark_on_amber_text() {
    let (dir, engine) = seed_library("ds_menu_hover", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    right_click(vcx, leak(format!("media-card-{}", id.0)));
    render_ui(vcx);
    let row = vcx.debug_bounds("tags-menu-btn").expect("Tags… row");
    vcx.simulate_mouse_move(row.center(), None, gpui::Modifiers::default());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.context_menu_state.as_ref().unwrap().hovered,
            Some("tags-menu-btn"),
            "the hovered row must be in state: GPUI cannot recolour text on hover"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_rows_are_26px() {
    let (dir, engine) = seed_library("ds_rows", 2);
    let mut cx = gpui::TestAppContext::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let row = vcx
        .debug_bounds("sidebar-nav-All Photos & Videos")
        .expect("library row");
    assert_eq!(f32::from(row.size.height), 26.0, "list rows are 26px tall");
    let _ = std::fs::remove_dir_all(&dir);
}
