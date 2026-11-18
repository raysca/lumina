//! The search field lives in the sidebar.

use crate::tests::helpers::*;
use crate::tests::helpers::{
    app_with_library, focus_app, keystrokes, render_ui, seed_library, type_text,
};
use crate::*;

#[core::prelude::v1::test]
fn search_field_lives_in_the_sidebar() {
    let (_dir, engine) = seed_library("search_field_sidebar", 2);
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
}

#[core::prelude::v1::test]
fn cmd_f_focuses_the_sidebar_field_and_typing_searches() {
    let (_dir, engine) = seed_library("search_field_cmd_f", 2);
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
}

#[core::prelude::v1::test]
fn cmd_f_with_the_sidebar_collapsed_expands_it_to_the_field() {
    let (_dir, engine) = seed_library("search_field_rail", 2);
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
}

#[core::prelude::v1::test]
fn cmd_f_leaves_search_alone_when_showing_the_sidebar_fails() {
    let (_dir, engine) = seed_library("search_field_rail_save_fails", 2);
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
}

#[core::prelude::v1::test]
fn ui_automation_search_click_type_clear() {
    let (_temp_dir, engine) = seed_library("search", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "search-input");
    view.update(vcx, |app, _| assert!(app.search_focused));

    // A query that matches the seeded photos, so the grid stays painted
    // once the results land.
    type_text(vcx, "photo");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "photo");
        assert!(app.search_focused, "typing keeps search focused");
    });

    // Clicking elsewhere must blur search so shortcuts work again. With the
    // layout switcher gone, an empty grid point is the neutral rendered target.
    let surface = vcx
        .debug_bounds("media-grid-surface")
        .expect("grid surface must be painted");
    let empty_point = gpui::point(
        surface.origin.x + gpui::px(24.0),
        surface.origin.y + surface.size.height - gpui::px(24.0),
    );
    vcx.simulate_click(empty_point, gpui::Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| assert!(!app.search_focused));
    assert!(vcx.debug_bounds("clear-search").is_some());

    click(vcx, "search-input");
    click(vcx, "clear-search");
    view.update(vcx, |app, _| {
        assert!(app.search_query.is_empty());
        assert!(!app.search_focused);
    });
}

#[core::prelude::v1::test]
fn ui_automation_empty_search_state_offers_a_clear_action() {
    let (_temp_dir, engine) = seed_library("empty_search", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.set_search_query("no-such-photo", cx);
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-state").is_some());
    assert!(
        vcx.debug_bounds("empty-primary").is_none(),
        "no-results offers term chips and Clear all filters, never a primary button"
    );

    click(vcx, "empty-search-term-0");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "removing the only term's chip clears the search"
        );
        assert!(app.selected_item.is_none());
    });
}

#[core::prelude::v1::test]
fn ui_automation_search_suggestions_filter_and_keyboard_apply() {
    let (_temp_dir, engine) = seed_library("search_suggestions", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    click(vcx, "search-input");
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "clicking the field focuses search");
        assert!(
            !app.search_suggestions.dismissed,
            "focusing the field opens suggestions"
        );
        assert!(
            !app.search_suggestion_items().is_empty(),
            "an empty query offers recent searches and operators"
        );
    });
    assert!(
        vcx.debug_bounds("search-suggestions").is_some(),
        "the dropdown must paint under the focused field"
    );
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_some(),
        "the first suggestion row must be painted"
    );

    type_text(vcx, "is:f");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "is:f");
        let items = app.search_suggestion_items();
        assert!(
            items.iter().any(|item| item == "is:favorite"),
            "the matching operator must be suggested: {items:?}"
        );
        assert!(
            items
                .iter()
                .all(|item| item.to_lowercase().contains("is:f")),
            "suggestions must be filtered by the typed token: {items:?}"
        );
    });

    keystrokes(vcx, "down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_suggestions.selected,
            Some(0),
            "ArrowDown highlights the first suggestion"
        );
        assert_eq!(
            app.search_query, "is:f",
            "moving the highlight must not edit the query"
        );
    });

    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "is:favorite",
            "Enter applies the highlighted suggestion"
        );
        assert!(
            app.search_focused,
            "applying a suggestion keeps the field focused for refinement"
        );
        assert!(
            app.search_suggestions.dismissed,
            "applying a suggestion closes the dropdown"
        );
        assert_eq!(
            app.config.recent_searches.first().map(String::as_str),
            Some("is:favorite"),
            "an applied suggestion is a committed search"
        );
    });

    // Escape hides the open dropdown first and only the second Escape blurs.
    click(vcx, "search-input");
    view.update(vcx, |app, _| {
        assert!(!app.search_suggestions.dismissed);
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.search_focused,
            "the first Escape only closes the dropdown"
        );
        assert!(app.search_suggestions.dismissed);
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            !app.search_focused,
            "the second Escape unfocuses the search field"
        );
    });

    // Committing the same query again must not duplicate recent history.
    click(vcx, "search-input");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(!app.search_focused, "Enter without a highlight unfocuses");
        assert_eq!(
            app.config
                .recent_searches
                .iter()
                .filter(|entry| entry.eq_ignore_ascii_case("is:favorite"))
                .count(),
            1,
            "recent searches dedupe case-insensitively"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_search_suggestion_click_applies_without_blurring() {
    let (_temp_dir, engine) = seed_library("search_suggestion_click", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.config.recent_searches = vec!["camera:sony".to_string(), "is:video".to_string()];
    });

    click(vcx, "search-input");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_suggestion_items().first().map(String::as_str),
            Some("camera:sony"),
            "the newest recent search is the first empty-query suggestion"
        );
    });
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_some(),
        "the recent-search row must be painted"
    );

    click(vcx, "search-suggestion-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "camera:sony",
            "clicking a suggestion applies it"
        );
        assert!(
            app.search_focused,
            "clicking inside the dropdown must not blur the search field"
        );
        assert!(app.search_suggestions.dismissed);
    });
}
