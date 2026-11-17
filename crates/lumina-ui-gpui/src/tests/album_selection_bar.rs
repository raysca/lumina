//! Design 07 Task 2: the album view's inline selection bar, which replaces
//! the floating dock while an album page has a selection.

use super::album_page::{library_with_album, open_album, painted};
use super::albums_overview::height;
use super::ui_automation::{
    app_with_library, bounds_center, click, focus_app, keystrokes, render_ui,
};
use crate::app::library::EXPORT_PICKS;
use crate::app::LuminaApp;
use gpui::{Entity, VisualTestContext};
use lumina_core::{ExportPreset, MediaId};

/// Selects the first two members through the real input path: a click,
/// then a ⌘-click.
fn select_two(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, ids: &[MediaId]) {
    click(
        vcx,
        super::ui_automation::leak(format!("media-card-{}", ids[0].0)),
    );
    let second = bounds_center(
        vcx,
        super::ui_automation::leak(format!("media-card-{}", ids[1].0)),
    );
    vcx.simulate_click(second, crate::tests::ui_automation::primary_modifier());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            2,
            "⌘-click extends the selection"
        );
    });
}

fn width(vcx: &mut VisualTestContext, selector: &'static str) -> f32 {
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"));
    f32::from(bounds.size.width)
}

/// Design 07 Task 2: the inline selection bar replaces the dock in an
/// album and runs its actions on the selection.
#[core::prelude::v1::test]
fn album_selection_bar_actions() {
    let (_dir, engine, id, ids) = library_with_album("album_selection_bar");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    select_two(&view, vcx, &ids);

    let got = height(vcx, "album-selection-bar");
    assert!(
        (got - 55.5).abs() <= 0.5,
        "the selection bar is 55.5 high: {got}"
    );
    assert!(
        vcx.debug_bounds("multi-select-action-dock").is_none(),
        "the album view shows the bar, not the floating dock"
    );
    assert!(painted(vcx, "album-bar-count-text:2 selected".into()));
    assert!(
        painted(vcx, "album-select-all-text:Select all 3".into()),
        "Select all names what the grid shows"
    );
    let header = vcx.debug_bounds("album-header").expect("album-header");
    let bar = vcx.debug_bounds("album-selection-bar").unwrap();
    assert_eq!(
        bar.origin.y,
        header.origin.y + header.size.height,
        "the bar sits directly under the header"
    );

    click(vcx, "album-select-all");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), 3, "Select all selects the grid");
    });
    assert!(painted(vcx, "album-bar-count-text:3 selected".into()));

    click(vcx, "album-bar-clear");
    view.update(vcx, |app, _| {
        assert!(!app.has_selection(), "Clear clears the selection");
    });
    assert!(
        vcx.debug_bounds("album-selection-bar").is_none(),
        "the bar leaves with the selection"
    );

    // Esc clears too.
    select_two(&view, vcx, &ids);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.selected_indices.is_empty(), "Esc clears the selection");
    });

    select_two(&view, vcx, &ids);
    click(vcx, "album-bar-add");
    assert!(
        vcx.debug_bounds("album-picker").is_some(),
        "Add to Album opens the album picker"
    );
    view.update(vcx, |app, cx| app.close_album_picker(cx));
    render_ui(vcx);

    click(vcx, "album-bar-tag");
    view.update(vcx, |app, cx| {
        assert!(app.tag_input_active, "Tag starts tagging the selection");
        app.tag_input_active = false;
        cx.notify();
    });
    render_ui(vcx);

    EXPORT_PICKS.with(|picks| picks.borrow_mut().clear());
    click(vcx, "album-bar-export");
    assert_eq!(
        EXPORT_PICKS.with(|picks| picks.borrow().clone()),
        vec![ExportPreset::Originals],
        "Export… asks for a folder to export the originals"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.get_selected_ids().len(),
            2,
            "Export… exports the selection it was clicked for"
        );
    });

    click(vcx, "album-bar-delete");
    view.update(vcx, |app, cx| {
        assert!(
            app.delete_confirm_state.is_some(),
            "Delete asks before trashing the selection"
        );
        app.delete_confirm_state = None;
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "album-bar-remove");
    assert_eq!(
        engine.query_album_count(id),
        1,
        "Remove takes the selection out of the album"
    );
    assert_eq!(
        engine.get_item_slice(0..3).len(),
        3,
        "media stay in the library"
    );
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    assert_eq!(
        engine.query_album_count(id),
        3,
        "⌘Z puts the removed members back"
    );
}

/// In a narrow window the bar's buttons drop their labels, keeping
/// tooltips.
#[core::prelude::v1::test]
fn album_selection_bar_collapses_to_icons_when_narrow() {
    let (_dir, engine, id, ids) = library_with_album("album_selection_bar_narrow");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);
    select_two(&view, vcx, &ids);
    assert!(
        width(vcx, "album-bar-add") > 40.0,
        "a wide window labels the buttons"
    );

    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(800.0)));
    render_ui(vcx);
    for selector in [
        "album-bar-add",
        "album-bar-tag",
        "album-bar-set-cover",
        "album-bar-export",
        "album-bar-remove",
        "album-bar-delete",
    ] {
        let got = width(vcx, selector);
        assert!(
            (got - 32.0).abs() <= 0.5,
            "{selector} collapses to a 32-wide icon button: {got}"
        );
    }
    let center = bounds_center(vcx, "album-bar-add");
    vcx.simulate_mouse_move(center, None, gpui::Modifiers::default());
    vcx.background_executor
        .advance_clock(std::time::Duration::from_millis(600));
    vcx.run_until_parked();
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tooltip-Add to Album… (⇧⌘A)").is_some(),
        "an icon-only button names its action in a tooltip"
    );
}

/// Outside an album the floating dock stays.
#[core::prelude::v1::test]
fn album_selection_bar_only_in_albums() {
    let (_dir, engine, _id, ids) = library_with_album("album_selection_bar_library");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    select_two(&view, vcx, &ids);
    assert!(
        vcx.debug_bounds("multi-select-action-dock").is_some(),
        "the library keeps the floating dock"
    );
    assert!(
        vcx.debug_bounds("album-selection-bar").is_none(),
        "the bar is for albums only"
    );
}

/// A filter-bar filter is a query token, and an album under a query shows
/// the search results, not the album page: no bar, and the dock acts on
/// the filtered selection instead.
#[core::prelude::v1::test]
fn album_selection_bar_hides_under_a_filter() {
    let (_dir, engine, id, ids) = library_with_album("album_selection_bar_filter");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);
    view.update(vcx, |app, cx| app.set_search_query("is:photo", cx));
    render_ui(vcx);
    render_ui(vcx);
    select_two(&view, vcx, &ids);
    assert!(
        vcx.debug_bounds("album-selection-bar").is_none(),
        "the bar belongs to the album page, which a filter replaces"
    );
    assert!(
        vcx.debug_bounds("multi-select-action-dock").is_some(),
        "the filtered selection keeps the dock"
    );
    keystrokes(vcx, "cmd-a");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            2,
            "select all takes only the photos the filter shows"
        );
    });
}
