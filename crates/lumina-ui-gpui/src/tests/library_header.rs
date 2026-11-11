//! Design 01 Task 1: the Library page header in the main column: title over a
//! meta line on the left, the filter, sort and Import actions on the right.

use super::downloads::{install, queue_app, settle, submit};
use super::ui_automation::{app_with_library, click, focus_app, keystrokes, render_ui};
use gpui::{Bounds, Pixels};

fn bounds(vcx: &mut gpui::VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    vcx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"))
}

fn inside(outer: Bounds<Pixels>, inner: Bounds<Pixels>) -> bool {
    inner.left() >= outer.left()
        && inner.right() <= outer.right()
        && inner.top() >= outer.top()
        && inner.bottom() <= outer.bottom()
}

#[core::prelude::v1::test]
fn header_sits_in_the_main_column_with_title_meta_and_actions() {
    let (_dir, engine, fake, mut cx) = queue_app("lh_header");
    let (view, vcx) = app_with_library(&mut cx, engine);
    // A download row makes the Downloads button show.
    install(&view, vcx, &fake);
    submit(&view, vcx, "https://example.com/a.mp4");
    settle(vcx, &view, "the row is stored", |app| {
        app.downloads.entries.len() == 1 && app.downloads.entries[0].row.id > 0
    });
    render_ui(vcx);

    let header = bounds(vcx, "page-header");
    let title = bounds(vcx, "page-title");
    let sidebar = bounds(vcx, "sidebar");
    assert!(
        f32::from(title.left()) >= f32::from(sidebar.right()) + 28.0,
        "the title sits in the main column past the 28px padding: {title:?} vs {sidebar:?}"
    );
    assert!(
        (f32::from(header.size.height) - 67.5).abs() <= 0.5,
        "the header is 67.5 high: {header:?}"
    );
    assert!(inside(header, title), "the title is inside the header");

    let meta = bounds(vcx, "page-meta");
    assert!(
        meta.top() >= title.bottom() && inside(header, meta),
        "the meta line sits under the title, inside the header: {meta:?} vs {title:?}"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("1 item"),
            "the meta line counts the view"
        );
    });

    let actions = bounds(vcx, "page-actions");
    assert!(inside(header, actions), "the actions are in the header");
    for selector in [
        "header-import",
        "header-filter",
        "sort-menu-button",
        "downloads-toolbar-btn",
        "tags-toolbar-btn",
        "inspector-toggle",
    ] {
        let button = bounds(vcx, selector);
        assert!(
            inside(actions, button) && button.left() >= title.right(),
            "{selector} sits in page-actions, right of the title: {button:?} vs {actions:?}"
        );
    }
    let search = bounds(vcx, "search-input");
    assert!(
        !header.intersects(&search),
        "the search field lives in the sidebar, not the header: {search:?} vs {header:?}"
    );
}

#[core::prelude::v1::test]
fn import_button_and_sort_button_run_their_actions() {
    use crate::app::import::IMPORT_PICKS;
    let (_dir, engine, _fake, mut cx) = queue_app("lh_actions");
    let (view, vcx) = app_with_library(&mut cx, engine);

    let before = IMPORT_PICKS.with(|n| n.get());
    click(vcx, "header-import");
    assert_eq!(
        IMPORT_PICKS.with(|n| n.get()),
        before + 1,
        "Import asks for folders"
    );

    click(vcx, "sort-menu-button");
    view.update(vcx, |app, _| {
        assert!(app.sort_menu_open, "Sort opens its menu")
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.sort_menu_open, "Escape closes the sort menu")
    });

    // Until the Add filter menu exists, the filter button opens the search
    // field with its filter suggestions.
    view.update(vcx, |app, cx| {
        app.toggle_sidebar(cx);
        assert!(app.sidebar_collapsed);
    });
    render_ui(vcx);
    click(vcx, "header-filter");
    view.update(vcx, |app, _| {
        assert!(
            !app.sidebar_collapsed && app.search_focused,
            "the filter button shows the sidebar and focuses its search field"
        );
    });
    assert!(
        vcx.debug_bounds("search-suggestions").is_some(),
        "the filter suggestions show"
    );
    // Unlike ⌘F, it keeps the query: a filter adds to it.
    view.update(vcx, |app, cx| {
        app.set_search_query("is:video", cx);
        app.blur_search(cx);
    });
    render_ui(vcx);
    click(vcx, "header-filter");
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "the field is focused again");
        assert_eq!(app.search_query, "is:video", "the query is kept");
    });
}

/// A popover's right edge lines up with its button's, and it opens below it.
fn assert_under(vcx: &mut gpui::VisualTestContext, popover: &'static str, button: &'static str) {
    let card = bounds(vcx, popover);
    let anchor = bounds(vcx, button);
    assert!(
        (f32::from(card.right()) - f32::from(anchor.right())).abs() <= 2.0
            && card.top() >= anchor.bottom(),
        "{popover} opens under {button}, right edges aligned: {card:?} vs {anchor:?}"
    );
}

#[core::prelude::v1::test]
fn sort_and_downloads_popovers_open_under_their_buttons() {
    let (_dir, engine, fake, mut cx) = queue_app("lh_anchor");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, "https://example.com/a.mp4");
    settle(vcx, &view, "the row is stored", |app| {
        app.downloads.entries.len() == 1 && app.downloads.entries[0].row.id > 0
    });
    render_ui(vcx);

    click(vcx, "sort-menu-button");
    render_ui(vcx);
    assert_under(vcx, "sort-menu", "sort-menu-button");
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.sort_menu_open, "Escape closed the sort menu")
    });
    assert!(vcx.debug_bounds("sort-menu").is_none(), "the card is gone");

    click(vcx, "downloads-toolbar-btn");
    render_ui(vcx);
    assert_under(vcx, "downloads-panel", "downloads-toolbar-btn");
}

#[core::prelude::v1::test]
fn delete_keys_under_the_sort_menu_leave_the_selection_alone() {
    let (_dir, engine, _fake, mut cx) = queue_app("lh_sort_keys");
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    let selected = view.update(vcx, |app, _| app.selected_indices.clone());
    assert!(!selected.is_empty(), "an item is selected");

    click(vcx, "sort-menu-button");
    focus_app(vcx, &view);
    // ⌫ and ⌘⌫ (Move to Trash) both open the Delete dialog from the grid.
    keystrokes(vcx, "backspace");
    keystrokes(vcx, "secondary-backspace");
    view.update(vcx, |app, _| {
        assert!(app.sort_menu_open, "the sort menu absorbed the keys");
        assert!(
            app.delete_confirm_state.is_none(),
            "no Delete dialog opens behind the sort menu"
        );
        assert_eq!(app.selected_indices, selected, "the selection is unchanged");
    });
    assert_eq!(
        engine.get_item_slice(0..10).len(),
        1,
        "the media is still in the library"
    );
}

#[core::prelude::v1::test]
fn meta_line_hides_until_the_count_is_known() {
    let (_dir, engine, _fake, mut cx) = queue_app("lh_meta_loading");
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.set_search_query("zzz-no-match", cx);
        assert_eq!(
            app.page_meta(),
            None,
            "a search with no count yet shows no meta line, not \"0 items\""
        );
    });
    // Draw without letting the background count land.
    vcx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    view.update(vcx, |app, _| {
        assert!(app.view_loading(), "the count has not landed yet")
    });
    assert!(
        vcx.debug_bounds("page-meta").is_none(),
        "no meta line is painted while the count loads"
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("0 items"),
            "once the search lands its count shows"
        );
    });
}

#[core::prelude::v1::test]
fn modals_opened_without_keys_close_the_header_popovers() {
    let (_dir, engine, fake, mut cx) = queue_app("lh_popover_modal");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    submit(&view, vcx, "https://example.com/a.mp4");
    settle(vcx, &view, "the row is stored", |app| {
        app.downloads.entries.len() == 1 && app.downloads.entries[0].row.id > 0
    });
    render_ui(vcx);

    click(vcx, "sort-menu-button");
    vcx.dispatch_action(crate::actions::OpenPreferences);
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        assert!(app.preferences_open, "Preferences opened");
        assert!(
            !app.sort_menu_open,
            "the sort card must not paint over Preferences"
        );
        app.close_preferences(cx);
    });
    assert!(vcx.debug_bounds("sort-menu").is_none());

    render_ui(vcx);
    click(vcx, "downloads-toolbar-btn");
    view.update(vcx, |app, cx| {
        assert!(app.downloads.open);
        app.open_tag_manager(cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            !app.downloads.open,
            "the downloads card must not paint over the tag manager"
        )
    });
    assert!(vcx.debug_bounds("downloads-panel").is_none());
}

#[core::prelude::v1::test]
fn headerless_screens_drop_the_header_popovers() {
    let (_dir, engine, fake, mut cx) = queue_app("lh_popover_welcome");
    let (view, vcx) = app_with_library(&mut cx, engine);
    install(&view, vcx, &fake);
    render_ui(vcx);
    click(vcx, "sort-menu-button");
    view.update(vcx, |app, cx| {
        assert!(app.sort_menu_open);
        // As switching libraries does on its way to Welcome.
        app.grid_state = None;
        cx.notify();
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            !app.sort_menu_open,
            "the sort menu must not outlive its header and shadow Welcome's keys"
        )
    });
}
