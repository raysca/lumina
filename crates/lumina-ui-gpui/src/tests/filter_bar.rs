//! The filter chip row under the page header and the Add
//! filter menu.

use crate::app::command_palette::CommandId;
use crate::app::search::filter_chips;
use crate::icons::Icon;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use gpui::{Bounds, Entity, Pixels, VisualTestContext};

const QUERY: &str = r#"month:jun year:2025 city:Lisbon camera:"X-T5" rating:>=3"#;

fn bounds(vcx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    vcx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"))
}

fn query(view: &Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> String {
    view.update(vcx, |app, _| app.search_query.clone())
}

fn set_query(view: &Entity<crate::LuminaApp>, vcx: &mut VisualTestContext, q: &str) {
    view.update(vcx, |app, cx| app.set_search_query(q, cx));
    render_ui(vcx);
}

/// The menu card hangs under `opener`: right edges aligned under a header
/// button, left edges under the row's Add filter chip.
fn assert_menu_under(vcx: &mut VisualTestContext, opener: &'static str, left: bool) {
    let card = bounds(vcx, "filter-add-menu");
    let anchor = bounds(vcx, opener);
    let (a, b) = if left {
        (card.left(), anchor.left())
    } else {
        (card.right(), anchor.right())
    };
    assert!(
        (f32::from(a) - f32::from(b)).abs() <= 2.0 && card.top() >= anchor.bottom(),
        "the menu opens under {opener}: {card:?} vs {anchor:?}"
    );
}

#[core::prelude::v1::test]
fn filter_tokens_render_as_chips_with_count_and_clear_all() {
    let (dir, engine) = seed_library("filter_bar_chips", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, QUERY);

    let row = bounds(vcx, "filter-bar");
    assert!(
        (f32::from(row.size.height) - 51.5).abs() <= 0.5,
        "the chip row is 51.5 high: {row:?}"
    );
    let header = bounds(vcx, "page-header");
    assert!(
        row.top() >= header.bottom(),
        "the row sits under the header"
    );
    for i in 0..4 {
        let chip = bounds(vcx, leak(format!("filter-chip-{i}")));
        assert!(
            chip.top() >= row.top() && chip.bottom() <= row.bottom(),
            "filter-chip-{i} sits in the row"
        );
    }
    assert!(
        vcx.debug_bounds("filter-chip-4").is_none(),
        "month and year merge into one date chip: five tokens, four chips"
    );
    let chips = filter_chips(QUERY);
    assert_eq!(
        chips.iter().map(|chip| chip.icon).collect::<Vec<_>>(),
        vec![Icon::Calendar, Icon::MapPin, Icon::Camera, Icon::Star],
        "each family has its icon"
    );
    assert_eq!(
        chips
            .iter()
            .map(|chip| chip.label.as_str())
            .collect::<Vec<_>>(),
        vec!["Jun 2025", "Lisbon", "X-T5", "Rated 3+"],
        "each family has its label"
    );

    assert!(vcx.debug_bounds("filter-match-count").is_some());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.filter_match_label().as_deref(),
            Some("0 matches"),
            "nothing in the seed was shot in Lisbon"
        );
    });
    assert_eq!(crate::i18n::plural("filter.matches", 1284), "1,284 matches");

    click(vcx, "filter-clear-all");
    assert_eq!(query(&view, vcx), "", "Clear all drops every filter token");
    assert!(
        vcx.debug_bounds("filter-bar").is_none(),
        "no filters, no row"
    );

    set_query(&view, vcx, "is:photo");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.filter_match_label().as_deref(),
            Some("1 match"),
            "the seed's one photo is one match, singular"
        );
    });

    set_query(&view, vcx, "tram");
    assert!(
        vcx.debug_bounds("filter-bar").is_none(),
        "a free-text-only query renders no chip row"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn chip_remove_and_add_filter_are_keyboard_reachable() {
    let (dir, engine) = seed_library("filter_bar_keys", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "month:jun year:2025 city:Lisbon is:favorite");

    click(vcx, "filter-chip-remove-1");
    assert_eq!(
        query(&view, vcx),
        "month:jun year:2025 is:favorite",
        "removing a chip drops only its token"
    );

    click(vcx, "filter-add");
    assert_menu_under(vcx, "filter-add", true);
    // Row 0 is the checked date chip (favourites has its own toggle row),
    // row 1 is Videos.
    focus_app(vcx, &view);
    keystrokes(vcx, "down down enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "month:jun year:2025 is:favorite is:video",
            "↓ ↓ ↵ adds Videos"
        );
        assert!(
            app.filter_menu.is_none(),
            "choosing a filter closes the menu"
        );
    });
    assert!(vcx.debug_bounds("filter-chip-2").is_some());
    assert_eq!(
        filter_chips("month:jun year:2025 is:favorite is:video")
            .last()
            .map(|chip| chip.label.clone()),
        Some("Videos".to_string()),
        "the added filter shows as a chip"
    );

    // The header's Filter button opens the same menu; Esc closes it and keys
    // stay in it, so ⌫ never reaches the grid selection behind.
    click(vcx, "header-filter");
    assert_menu_under(vcx, "header-filter", false);
    focus_app(vcx, &view);
    keystrokes(vcx, "down backspace escape");
    view.update(vcx, |app, _| {
        assert!(app.filter_menu.is_none(), "Esc closes the menu");
        assert_eq!(
            app.search_query, "month:jun year:2025 is:favorite is:video",
            "Esc leaves the query unchanged"
        );
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ under the menu opens no Delete dialog"
        );
    });
    assert!(vcx.debug_bounds("filter-add-menu").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn menu_lists_typed_chips_checked_so_the_keyboard_removes_them() {
    let (dir, engine) = seed_library("filter_bar_remove_keys", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, r#"city:Lisbon camera:"X-T5" is:video"#);

    click(vcx, "header-filter");
    focus_app(vcx, &view);
    // ↑ stops at the first row.
    keystrokes(vcx, "down up up up");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.filter_menu.as_ref().and_then(|menu| menu.highlight),
            Some(0),
            "↑ stops at the first row"
        );
    });
    // Rows 0 and 1 are Lisbon and X-T5; is:video has its own toggle row.
    keystrokes(vcx, "down enter");
    assert_eq!(
        query(&view, vcx),
        "city:Lisbon is:video",
        "↵ on a checked chip row removes that chip"
    );
    click(vcx, "header-filter");
    focus_app(vcx, &view);
    keystrokes(vcx, "down enter");
    assert_eq!(query(&view, vcx), "is:video", "the place chip goes too");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn menu_rows_take_clicks_above_the_backdrop() {
    let (dir, engine) = seed_library("filter_bar_click_row", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "city:Lisbon");

    // Rows: Lisbon (checked), Videos, Photos, …
    click(vcx, "header-filter");
    click(vcx, "filter-menu-row-2");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "city:Lisbon is:photo",
            "a click on a row adds its token; the card occludes the backdrop"
        );
        assert!(app.filter_menu.is_none());
    });

    // Camera… starts a camera: token, outside an unclosed quote, and
    // focuses the search field for its value.
    // Free text shows the Search screen, whose header has no Filter button:
    // the palette's Add Filter opens the menu, and the
    // page header comes back to carry it.
    set_query(&view, vcx, r#""golden hour"#);
    view.update(vcx, |app, cx| app.execute_command(CommandId::AddFilter, cx));
    render_ui(vcx);
    let camera_row = 8; // eight toggle rows, then Camera…
    click(vcx, leak(format!("filter-menu-row-{camera_row}")));
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, r#""golden hour" camera:"#);
        assert!(app.search_focused, "the search field takes the value");
        assert!(app.filter_menu.is_none());
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_and_vanishing_row_put_the_menu_under_the_header_button() {
    let (dir, engine) = seed_library("filter_bar_palette", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.open_command_palette(cx));
    render_ui(vcx);
    view.update(vcx, |app, cx| app.execute_command(CommandId::AddFilter, cx));
    render_ui(vcx);
    assert_menu_under(vcx, "header-filter", false);
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");

    // Opened from the chip, the card follows to the header button when the
    // row goes, so an open menu always shows.
    set_query(&view, vcx, "city:Lisbon");
    click(vcx, "filter-add");
    assert_menu_under(vcx, "filter-add", true);
    set_query(&view, vcx, "");
    assert!(vcx.debug_bounds("filter-bar").is_none());
    assert_menu_under(vcx, "header-filter", false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn chip_row_scrolls_in_a_narrow_window_with_the_inspector_open() {
    let (dir, engine) = seed_library("filter_bar_narrow", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(720.0)));
    view.update(vcx, |app, _| app.inspector_open = true);
    set_query(
        &view,
        vcx,
        r#"month:jun year:2025 city:Lisbon camera:"X-T5" lens:"XF 23mm" rating:>=3 is:video is:favorite is:pick has:location tag:travel"#,
    );

    let row = bounds(vcx, "filter-bar");
    assert!(
        (f32::from(row.size.height) - 51.5).abs() <= 0.5,
        "the row stays one line high: {row:?}"
    );
    let inspector_width = view.update(vcx, |app, _| app.inspector_width);
    assert!(
        f32::from(row.right()) <= 1024.0 - inspector_width,
        "the row stays left of the {inspector_width}px inspector: {row:?}"
    );
    let clear = bounds(vcx, "filter-clear-all");
    assert!(
        clear.left() >= row.left() && clear.right() <= row.right(),
        "Clear all stays on screen: {clear:?} in {row:?}"
    );
    let scroller = bounds(vcx, "filter-chips");
    let add = bounds(vcx, "filter-add");
    assert!(
        add.right() > scroller.right() && scroller.right() <= clear.left(),
        "the chips overflow their scroller instead of pushing Clear all: \
         {add:?} vs {scroller:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
