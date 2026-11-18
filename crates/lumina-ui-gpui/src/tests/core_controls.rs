//! Core controls: every toolbar control paints and answers the mouse.

use crate::tests::helpers::*;

#[core::prelude::v1::test]
fn ui_automation_renders_all_core_controls() {
    let (_temp_dir, engine) = seed_library("render", 8);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);

    assert!(vcx.debug_bounds("app-root").is_some());
    let selectors = [
        "sidebar-toggle",
        "search-input",
        "switch-library",
        "inspector-toggle",
        "page-title",
        "tags-toolbar-btn",
        "new-smart-album",
    ];
    let missing: Vec<_> = selectors
        .iter()
        .filter(|selector| vcx.debug_bounds(selector).is_none())
        .collect();
    assert!(missing.is_empty(), "controls not painted: {missing:?}");
}

#[core::prelude::v1::test]
fn ui_automation_toolbar_buttons_by_mouse() {
    let (_temp_dir, engine) = seed_library("toolbar", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(app.sidebar_collapsed));
    // One toggle: the rail has no expand button of its own.
    assert!(vcx.debug_bounds("expand-sidebar").is_none());
    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(!app.sidebar_collapsed));

    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(app.inspector_open));
    click(vcx, "inspector-toggle");
    view.update(vcx, |app, _| assert!(!app.inspector_open));
}
