//! Cross-cutting layout rules: pages hang from under the header, hovered
//! menu rows are tracked in state, and nav rows keep their count inside.

use crate::tests::ui_automation::{
    app_with_library, keystrokes, leak, render_ui, right_click, seed_library,
};

use crate::views::page_header::PAGE_HEADER_HEIGHT as HEADER_HEIGHT;

fn assert_hangs_from_toolbar(vcx: &mut gpui::VisualTestContext, selector: &'static str) {
    render_ui(vcx);
    let card = vcx.debug_bounds(selector).expect(selector);
    assert_eq!(
        f32::from(card.top()),
        HEADER_HEIGHT,
        "{selector} must hang from the bottom of the header, not be centred"
    );
}

#[core::prelude::v1::test]
fn tags_page_sits_under_the_page_header() {
    let (dir, engine) = seed_library("ds_tag_page", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_tag_manager(cx));
    assert_hangs_from_toolbar(vcx, "tags-page");
    let sidebar = vcx.debug_bounds("sidebar").expect("sidebar");
    let page = vcx.debug_bounds("tags-page").expect("tags-page");
    assert!(
        page.left() >= sidebar.right(),
        "the Tags page sits beside the sidebar, not over it: {page:?}"
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
fn nav_item_keeps_its_icon_and_count_inside_the_row() {
    let (dir, engine) = seed_library("ds_rows", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let row = vcx.debug_bounds("sidebar-nav-all").expect("library row");
    let icon = vcx.debug_bounds("sidebar-nav-all-icon").expect("row icon");
    let count = vcx
        .debug_bounds("sidebar-nav-all-count")
        .expect("row count");
    assert!(
        icon.left() > row.left() && icon.right() <= count.left() && count.right() < row.right(),
        "the icon leads and the count trails, both inside the row's padding: \
         row {row:?} icon {icon:?} count {count:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
