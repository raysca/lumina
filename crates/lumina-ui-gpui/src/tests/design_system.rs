//! Cross-cutting v3 design rules (FIXES.md §1): sheets hang from under the
//! toolbar, modals dim the library lightly, and shared rows use the tokens.

use crate::tests::ui_automation::{
    app_with_library, bounds_center, click, keystrokes, leak, render_ui, right_click, seed_library,
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
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_tag_manager(cx));
    assert_hangs_from_toolbar(vcx, "tag-manager-dialog");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn smart_album_builder_is_a_760px_sheet_under_the_toolbar() {
    let (dir, engine) = seed_library("ds_album_sheet", 2);
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
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
fn nav_item_is_32px_with_right_count() {
    let (dir, engine) = seed_library("ds_rows", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let row = vcx.debug_bounds("sidebar-nav-all").expect("library row");
    let icon = vcx.debug_bounds("sidebar-nav-all-icon").expect("row icon");
    let count = vcx
        .debug_bounds("sidebar-nav-all-count")
        .expect("row count");
    assert_eq!(
        f32::from(row.size.height),
        32.0,
        "NavItem rows are 32px tall"
    );
    assert_eq!(
        f32::from(icon.left() - row.left()),
        10.0,
        "NavItem has 10px side padding"
    );
    assert_eq!(
        f32::from(row.right() - count.right()),
        10.0,
        "the count sits at the row's right edge, inside the padding"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn chip_is_28px_pill() {
    let (dir, engine) = seed_library("ds_chips", 3);
    let items = engine.get_item_slice(0..2);
    let (a, b) = (items[0].clone(), items[1].clone());
    let catalog = engine.catalog();
    catalog
        .add_tags_batch(&[a.id, b.id], &["trip".to_string()])
        .unwrap();
    catalog
        .add_tags_batch(&[a.id], &["street".to_string()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", a.id.0)));
    let second = bounds_center(vcx, leak(format!("media-card-{}", b.id.0)));
    vcx.simulate_click(second, gpui::Modifiers::command());
    click(vcx, "inspector-toggle");
    render_ui(vcx);
    for selector in ["batch-tag-trip", "batch-partial-tag-street"] {
        let chip = vcx.debug_bounds(selector).expect(selector);
        assert_eq!(
            f32::from(chip.size.height),
            28.0,
            "{selector}: chips are 28px tall"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
