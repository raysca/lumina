//! The search field lives in the sidebar.

use crate::tests::ui_automation::{
    app_with_library, focus_app, keystrokes, render_ui, seed_library, type_text,
};

#[core::prelude::v1::test]
fn search_field_lives_in_the_sidebar() {
    let (dir, engine) = seed_library("search_field_sidebar", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    let sidebar_width = view.update(vcx, |app, _| app.sidebar_width);
    let sidebar = vcx.debug_bounds("sidebar").expect("sidebar painted");
    let field = vcx
        .debug_bounds("search-input")
        .expect("search field painted");
    let (x, w, h) = (
        f32::from(field.origin.x),
        f32::from(field.size.width),
        f32::from(field.size.height),
    );
    assert!(
        x < 252.0 && field.origin.x >= sidebar.origin.x && field.right() <= sidebar.right(),
        "the search field lies inside the sidebar: {field:?} in {sidebar:?}"
    );
    assert_eq!(h, 34.0, "the field is 34 high");
    // The plan's "sidebar width − 24" less the sidebar's 1px right border,
    // which GPUI lays out inside the 252: the field is as wide as the rows.
    assert_eq!(
        w,
        sidebar_width - 24.0 - 1.0,
        "the field spans the sidebar less its 12px padding each side and border"
    );
    let row = vcx.debug_bounds("sidebar-nav-all").expect("Library row");
    assert_eq!(field.size.width, row.size.width, "the field is row width");
    // The header starts where the sidebar ends; nothing of the field is there.
    let header = vcx
        .debug_bounds("toolbar-drag-area")
        .expect("header painted");
    assert!(
        field.right() <= header.origin.x,
        "the search field is gone from the header: {field:?} vs {header:?}"
    );

    let placeholder = vcx
        .debug_bounds("search-placeholder")
        .expect("the idle field shows its placeholder");
    let shortcut = vcx
        .debug_bounds("search-shortcut")
        .expect("the idle field shows its shortcut hint");
    assert!(
        field.contains(&placeholder.center()) && field.contains(&shortcut.center()),
        "placeholder and hint draw inside the field"
    );
    assert!(
        shortcut.origin.x > placeholder.origin.x,
        "the shortcut hint sits to the right"
    );
    assert_eq!(crate::i18n::t("search.placeholder"), "Search library");
    assert_eq!(crate::i18n::t("search.shortcut"), "⌘F");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_f_focuses_the_sidebar_field_and_typing_searches() {
    let (dir, engine) = seed_library("search_field_cmd_f", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    assert!(
        vcx.debug_bounds("sidebar-nav-all-active").is_some(),
        "with no query the Library row shows as active"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-f");
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "⌘F focuses search")
    });
    // Focus shows the Search screen: the dropdown hangs
    // under its query bar, which takes the typing.
    let bar = vcx.debug_bounds("search-query-bar").expect("query bar");
    let panel = vcx
        .debug_bounds("search-suggestions")
        .expect("focusing the field opens the suggestions");
    assert!(
        panel.origin.y >= bar.bottom()
            && panel.origin.x == bar.origin.x
            && panel.size.width == bar.size.width,
        "the dropdown anchors under the query bar, full width: {panel:?} under {bar:?}"
    );

    type_text(vcx, "city:Lisbon");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "city:Lisbon");
        assert!(
            app.config
                .recent_searches
                .iter()
                .any(|recent| recent == "city:Lisbon"),
            "↵ records a recent search: {:?}",
            app.config.recent_searches
        );
    });
    assert!(
        vcx.debug_bounds("sidebar-nav-all-active").is_none(),
        "no sidebar row is highlighted while a query shows"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_f_with_the_sidebar_collapsed_expands_it_to_the_field() {
    let (dir, engine) = seed_library("search_field_rail", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.toggle_sidebar(cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("search-input").is_none(),
        "the rail has no field"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-f");
    view.update(vcx, |app, _| {
        assert!(
            !app.sidebar_collapsed,
            "⌘F shows the sidebar with the field"
        );
        assert!(app.search_focused, "⌘F focuses search");
    });
    assert!(
        vcx.debug_bounds("search-input").is_some(),
        "the focused field is on screen"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_f_leaves_search_alone_when_showing_the_sidebar_fails() {
    let (dir, engine) = seed_library("search_field_rail_save_fails", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.toggle_sidebar(cx));
    render_ui(vcx);

    focus_app(vcx, &view);
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    keystrokes(vcx, "secondary-f");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert!(app.sidebar_collapsed, "a failed save keeps the rail");
        assert!(
            !app.search_focused,
            "⌘F must not focus a search field that is off screen"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
