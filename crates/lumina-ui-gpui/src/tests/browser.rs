//! Browser screen (FIXES.md §2): top bar, layout, filter bar, sidebar and
//! context menu against the v3 grid design.

use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, leak, render_ui, right_click, seed_library,
};
use crate::views::search_bar::GridFilter;

#[core::prelude::v1::test]
fn top_bar_actions_are_28px_icon_buttons_and_back_is_keyboard_only() {
    let (dir, engine) = seed_library("br_topbar", 2);
    let mut cx = gpui::TestAppContext::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    for selector in [
        "sort-menu-button",
        "tags-toolbar-btn",
        "inspector-toggle",
        "switch-library",
    ] {
        let b = vcx.debug_bounds(selector).expect(selector);
        assert_eq!(
            (f32::from(b.size.width), f32::from(b.size.height)),
            (28.0, 28.0),
            "{selector} is a 28×28 icon button"
        );
    }
    assert!(
        vcx.debug_bounds("nav-back").is_none(),
        "the design has no Back button"
    );
    let search = vcx.debug_bounds("search-input").expect("search");
    assert_eq!(f32::from(search.size.width), 300.0, "search is 300px");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_runs_full_height_beside_the_filter_and_status_bars() {
    let (dir, engine) = seed_library("br_layout", 2);
    let mut cx = gpui::TestAppContext::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let sidebar = vcx.debug_bounds("sidebar").expect("sidebar");
    let filter = vcx.debug_bounds("filter-bar").expect("filter bar");
    let status = vcx.debug_bounds("status-bar").expect("status bar");
    assert_eq!(
        f32::from(sidebar.top()),
        52.0,
        "the sidebar starts under the toolbar"
    );
    assert!(
        sidebar.bottom() >= status.bottom(),
        "the sidebar runs to the window bottom: sidebar={sidebar:?} status={status:?}"
    );
    for (name, bar) in [("filter", filter), ("status", status)] {
        assert!(
            bar.left() >= sidebar.right(),
            "the {name} bar sits in the main column: {name}={bar:?} sidebar={sidebar:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn filter_chips_select_a_filter_that_dims_rather_than_hides() {
    let (dir, engine) = seed_library("br_filter", 4);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home f");
    let (fav, count) = view.update(vcx, |app, _| {
        (
            app.selected_item.clone().unwrap().id,
            app.get_active_count(),
        )
    });
    let other = engine
        .get_item_slice(0..count)
        .into_iter()
        .find(|item| item.id != fav)
        .unwrap()
        .id;
    click(vcx, "filter-chip-favorites");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_filter, GridFilter::Favorites);
        assert_eq!(app.get_active_count(), count, "filters dim, never hide");
    });
    assert!(
        vcx.debug_bounds(leak(format!("media-card-dimmed-{}", other.0)))
            .is_some(),
        "a non-favorite is dimmed"
    );
    assert!(
        vcx.debug_bounds(leak(format!("media-card-dimmed-{}", fav.0)))
            .is_none(),
        "the favorite stays at full opacity"
    );

    // Keyboard path: the palette has every filter.
    keystrokes(vcx, "secondary-k v i d e o s enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_filter, GridFilter::Videos)
    });
    click(vcx, "filter-chip-all");
    view.update(vcx, |app, _| assert_eq!(app.grid_filter, GridFilter::All));

    assert!(GridFilter::All.matches(false, false, 0, false));
    assert!(GridFilter::Favorites.matches(true, false, 0, false));
    assert!(!GridFilter::Favorites.matches(false, true, 5, true));
    assert!(GridFilter::Videos.matches(false, true, 0, false));
    assert!(GridFilter::ThreeStars.matches(false, false, 3, false));
    assert!(!GridFilter::ThreeStars.matches(false, false, 2, false));
    assert!(GridFilter::Labeled.matches(false, false, 0, true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_has_no_events_and_untagged_sits_under_smart_albums() {
    let (dir, engine) = seed_library("br_sidebar", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let years = view.update(vcx, |app, _| app.timeline_year_buckets.clone());
    for (year, _) in years {
        assert!(
            vcx.debug_bounds(leak(format!("sidebar-nav-{year}")))
                .is_none(),
            "the EVENTS section is gone"
        );
    }
    let untagged = vcx.debug_bounds("sidebar-nav-Untagged").expect("Untagged");
    let new_album = vcx.debug_bounds("new-smart-album").expect("+");
    let manage = vcx.debug_bounds("open-tag-manager").expect("TAGS header");
    assert!(
        untagged.top() > new_album.top() && untagged.top() < manage.top(),
        "Untagged sits in SMART ALBUMS, above TAGS"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn context_menu_sets_label_and_rating_with_20px_targets() {
    let (dir, engine) = seed_library("br_menu", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home");
    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);

    right_click(vcx, leak(format!("media-card-{}", id.0)));
    render_ui(vcx);
    let menu = vcx.debug_bounds("context-menu").expect("menu");
    assert_eq!(f32::from(menu.size.width), 256.0, "the menu is 256px");
    assert!(
        vcx.debug_bounds("add-album-menu-btn").is_none(),
        "no manual albums"
    );
    let star = vcx.debug_bounds("context-rating-star-4").expect("4th star");
    assert_eq!(f32::from(star.size.width), 20.0, "stars are 20px targets");
    assert!(
        vcx.debug_bounds("context-label-0").is_some(),
        "No label comes first"
    );

    click(vcx, "context-rating-star-4");
    click(vcx, "context-label-3");
    assert_eq!(engine.get_ratings_map().get(&id).copied(), Some(4));
    assert_eq!(engine.get_color_labels_map().get(&id).copied(), Some(3));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn size_slider_follows_a_layout_change_and_keys_step_its_levels() {
    let (dir, engine) = seed_library("br_slider", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.inspector_open = false);
    render_ui(vcx);
    // Opening the inspector moves the slider left. Toggling the inspector moves the slider; the next click must use the
    // new position, not the one captured before the move.
    // Exactly one frame after the move, so a listener built from the
    // previous frame's x would still be live.
    view.update(vcx, |app, cx| app.toggle_inspector(cx));
    vcx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear();
    });
    let slider = vcx.debug_bounds("thumb-size-slider").expect("slider");
    crate::tests::ui_automation::click_point(
        vcx,
        f32::from(slider.right()) - 1.0,
        f32::from(slider.center().y),
    );
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_columns, 2, "right end = largest")
    });
    keystrokes(vcx, "secondary--");
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_columns, 3, "⌘- steps one slider level")
    });
    keystrokes(vcx, "secondary-= secondary-=");
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_columns, 2, "and clamps at the end")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn date_sorted_grid_shows_the_top_rows_month_above_the_cards() {
    let (dir, engine) = seed_library("br_month_band", 4);
    let items = engine.get_item_slice(0..4);
    let first = items[0].clone();
    let in_month = items
        .iter()
        .filter(|i| (i.year, i.month) == (first.year, first.month))
        .count();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.refresh_album_counts());
    render_ui(vcx);
    render_ui(vcx);

    let band = vcx
        .debug_bounds("grid-month-header")
        .expect("a date-sorted grid shows the month band");
    let card = vcx
        .debug_bounds(leak(format!("media-card-{}", first.id.0)))
        .expect("the first card renders");
    assert!(
        card.top() >= band.bottom(),
        "the band sits above the cards, never over them: band={band:?} card={card:?}"
    );
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.month_header.top,
            Some((first.year, first.month)),
            "the band names the top row's month"
        );
        assert_eq!(
            app.month_header_count(),
            Some(in_month),
            "All Library counts the top month's items"
        );
        app.search_query = "beach".into();
        assert_eq!(
            app.month_header_count(),
            None,
            "a library-wide count would lie about search results"
        );
        app.search_query.clear();
        app.selection = crate::views::sidebar::AlbumSelection::Untagged;
        assert_eq!(
            app.month_header_count(),
            None,
            "a library-wide count would lie about an album"
        );
        app.selection = crate::views::sidebar::AlbumSelection::AllLibrary;
        app.set_sort_field(lumina_core::SortField::FileName, cx);
        // debug_bounds keeps stale entries, so ask the renderer directly.
        assert!(
            app.render_month_header().is_none(),
            "months mean nothing in a name-sorted grid"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}
