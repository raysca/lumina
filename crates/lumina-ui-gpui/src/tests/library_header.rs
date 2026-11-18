//! The Library page header in the main column: title over a
//! meta line on the left, the filter, sort and Import actions on the right.

use super::downloads::{install, queue_app, settle, submit};
use super::helpers::{app_with_library, click, focus_app, keystrokes, render_ui};
use crate::tests::helpers::*;
use crate::*;
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
    assert!(inside(header, title), "the title is inside the header");

    let meta = bounds(vcx, "page-meta");
    assert!(
        meta.top() >= title.bottom() && inside(header, meta),
        "the meta line sits under the title, inside the header: {meta:?} vs {title:?}"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta()
                .as_deref()
                .and_then(|meta| meta.split(" · ").next()),
            Some("1 item"),
            "the meta line counts the view (the seed's date is its file time)"
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

    // The filter button opens the Add filter menu under it
    // (`tests/filter_bar.rs`).
    click(vcx, "header-filter");
    render_ui(vcx);
    assert_under(vcx, "filter-add-menu", "header-filter");
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
    // The Tags page is a page with the Downloads button in its header, not
    // a sheet: the card may stay open under its button.
    view.update(vcx, |app, _| {
        assert!(app.tags_page_shown() && app.downloads.open)
    });
    assert!(vcx.debug_bounds("downloads-toolbar-btn").is_some());
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

#[core::prelude::v1::test]
fn header_meta_shows_count_and_date_range() {
    use super::helpers::seed_library;
    use crate::views::sidebar::AlbumSelection;
    let (_dir, engine) = seed_library("lh_range", 3);
    let ids: Vec<_> = engine
        .get_item_slice(0..3)
        .iter()
        .map(|item| item.id)
        .collect();
    // Mid-month noon UTC: Jun 2018, Mar 2022, Nov 2026.
    let times = [1_529_064_000_000_i64, 1_647_345_600_000, 1_794_744_000_000];
    engine
        .set_timestamps(&ids.iter().copied().zip(times).collect::<Vec<_>>())
        .unwrap();
    let album = engine.create_album_with("June", &ids[..1]).unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("3 items · Jun 2018 — Nov 2026"),
            "All Media reads its count and the months of its oldest and newest items"
        );
    });
    assert!(
        vcx.debug_bounds("page-meta").is_some(),
        "the meta line is painted"
    );

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("1 item · Jun 2018"),
            "a view within one month names that month once"
        );
    });

    view.update(vcx, |app, cx| app.set_search_query("year:2026", cx));
    view.update(vcx, |app, _| {
        assert!(
            app.page_meta().is_none_or(|meta| !meta.contains('·')),
            "a view whose span has not landed shows no range, not the last view's"
        );
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("1 item · Nov 2026"),
            "a search spans only its matches"
        );
    });
}

#[core::prelude::v1::test]
fn date_span_rereads_after_a_write_before_a_scroll_fetch() {
    use super::helpers::seed_library;
    let (_dir, engine) = seed_library("lh_span_race", 80);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let span = |vcx: &mut gpui::VisualTestContext| view.update(vcx, |app, _| app.displayed_span());
    let before = span(vcx).expect("the seed is dated");
    assert_ne!(before.first, (2010, 1), "the seed is not from 2010");

    // Rows past the cached window: this fetch would reuse the cached span.
    view.update(vcx, |app, cx| app.want_view_rows(76..80, cx));
    // A write lands after the fetch is scheduled, before it runs.
    let id = engine.get_item_slice(0..1)[0].id;
    engine.set_timestamps(&[(id, 1_263_556_800_000)]).unwrap(); // 15 Jan 2010
    vcx.run_until_parked();
    render_ui(vcx);
    assert_eq!(
        span(vcx).map(|s| s.first),
        Some((2010, 1)),
        "a span reused across a write would show the old months until the next edit"
    );
}

#[core::prelude::v1::test]
fn ui_automation_sort_menu_orders_grid() {
    let (_temp_dir, engine) = seed_library("sorting", 5);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    click(vcx, "sort-menu-button");
    view.update(vcx, |app, _| assert!(app.sort_menu_open));

    click(vcx, "sort-field-name");
    view.update(vcx, |app, _| {
        assert_eq!(app.sort.field, lumina_core::SortField::FileName);
        assert!(!app.sort_menu_open);
    });

    click(vcx, "sort-menu-button");
    click(vcx, "sort-direction-asc");
    view.update(vcx, |app, _| {
        assert_eq!(app.sort.direction, lumina_core::SortDirection::Ascending);
        assert_eq!(app.sort_label(), "Name ↑");
        let names: Vec<String> = app
            .get_active_slice(0..5)
            .into_iter()
            .map(|item| item.file_name)
            .collect();
        let mut expected = names.clone();
        expected.sort_by_key(|name| name.to_lowercase());
        assert_eq!(names, expected, "grid must follow the selected sort");
        assert_eq!(names.first().map(String::as_str), Some("photo_00.jpg"));
    });
}
