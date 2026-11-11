//! Browser screen (FIXES.md §2): top bar, layout, filter bar, sidebar and
//! context menu against the v3 grid design.

use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click, seed_library,
    seed_library_undecodable, type_text,
};
use crate::views::sidebar::AlbumSelection;

#[core::prelude::v1::test]
fn top_bar_actions_are_28px_icon_buttons_and_back_is_keyboard_only() {
    let (dir, engine) = seed_library("br_topbar", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    for selector in ["sort-menu-button", "tags-toolbar-btn", "inspector-toggle"] {
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
    let title = vcx.debug_bounds("toolbar-title").expect("toolbar-title");
    assert!(
        f32::from(title.size.height) >= 24.0,
        "the page title is 15px (v3), not 12px breadcrumb text: {title:?}"
    );
    let search = vcx.debug_bounds("search-input").expect("search");
    assert_eq!(f32::from(search.size.width), 300.0, "search is 300px");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_long_title_truncates_and_keeps_the_toolbar_buttons_on_screen() {
    let (dir, engine) = seed_library("br_long_title", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.selection = crate::views::sidebar::AlbumSelection::Tag("a".repeat(300));
        cx.notify();
    });
    render_ui(vcx);
    let root = vcx.debug_bounds("app-root").expect("app-root");
    let info = vcx
        .debug_bounds("inspector-toggle")
        .expect("inspector-toggle");
    assert!(
        info.right() <= root.right(),
        "a long album or tag name must truncate, not push Info off the bar: {info:?} vs {root:?}"
    );
    assert_eq!(f32::from(info.size.width), 28.0, "buttons never shrink");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn open_library_lives_in_the_sidebar_and_answers_cmd_o() {
    use crate::app::library::FOLDER_PICKS;
    let picks = || FOLDER_PICKS.with(|n| n.get());
    let (dir, engine) = seed_library("br_open_library", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    let button = vcx.debug_bounds("switch-library").expect("switch-library");
    let sidebar = vcx.debug_bounds("sidebar").expect("sidebar");
    assert!(
        sidebar.contains(&button.center()),
        "Open Library sits in the sidebar, not the toolbar (amber is for selection): {button:?}"
    );
    let before = picks();
    click(vcx, "switch-library");
    assert_eq!(picks(), before + 1, "the sidebar button opens the picker");
    keystrokes(vcx, "secondary-o");
    assert_eq!(
        picks(),
        before + 2,
        "⌘O opens the picker, as the palette advertises"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_runs_full_height_beside_the_status_bar() {
    let (dir, engine) = seed_library("br_layout", 2);
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let sidebar = vcx.debug_bounds("sidebar").expect("sidebar");
    let status = vcx.debug_bounds("status-bar").expect("status bar");
    assert_eq!(
        f32::from(sidebar.top()),
        0.0,
        "the sidebar starts at the window top, beside the header"
    );
    assert!(
        sidebar.bottom() >= status.bottom(),
        "the sidebar runs to the window bottom: sidebar={sidebar:?} status={status:?}"
    );
    assert!(
        status.left() >= sidebar.right(),
        "the status bar sits in the main column: status={status:?} sidebar={sidebar:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const FAVORITES: AlbumSelection = AlbumSelection::System(lumina_core::SystemAlbum::Favorites);

#[core::prelude::v1::test]
fn palette_filter_toggles_query_token() {
    let (dir, engine) = seed_library("br_quick_filter", 4);
    let rated = engine.get_item_slice(0..1)[0].id;
    engine.set_rating(rated, 4).expect("seed a rating");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "secondary-k s h o w space r a t e d space 4 enter");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.contains("rating:>=4"),
            "the palette quick filter writes a query token, got {:?}",
            app.search_query
        );
        let expected = engine.search_query_count("rating:>=4");
        assert_eq!(expected, 1, "one item is rated 4");
        assert_eq!(
            app.get_active_count(),
            expected,
            "the grid shows exactly what the query matches (no dimming)"
        );
    });

    keystrokes(vcx, "secondary-k s h o w space r a t e d space 4 enter");
    view.update(vcx, |app, _| {
        assert!(
            !app.search_query.contains("rating:>=4"),
            "running the same quick filter again removes its token, got {:?}",
            app.search_query
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn no_dimmed_cards_after_filter() {
    let (dir, engine) = seed_library("br_no_dim", 4);
    let ids: Vec<_> = engine.get_item_slice(0..4).iter().map(|i| i.id).collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let filters: Vec<_> = crate::app::command_palette::commands()
        .iter()
        .filter(|c| c.key.starts_with("command.filter_"))
        .map(|c| c.id)
        .collect();
    assert!(!filters.is_empty(), "the palette offers quick filters");
    for id in filters {
        view.update(vcx, |app, cx| {
            app.set_search_query("", cx);
            app.execute_command(id, cx);
        });
        render_ui(vcx);
        for item in &ids {
            assert!(
                vcx.debug_bounds(leak(format!("media-card-dimmed-{}", item.0)))
                    .is_none(),
                "{id:?} must filter through the query, never dim card {item:?}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn typing_a_query_moves_sidebar_to_library() {
    let (dir, engine) = seed_library("br_type_scope", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_album(FAVORITES, cx));
    render_ui(vcx);

    click(vcx, "search-input");
    type_text(vcx, "kyoto");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "kyoto");
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "a typed query searches the whole library, so the sidebar says Library"
        );
    });
    for _ in 0..2 {
        if view.update(vcx, |app, _| app.search_focused) {
            keystrokes(vcx, "escape");
        }
    }
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, FAVORITES, "Back restores the album");
        assert_eq!(app.search_query, "", "Back restores the empty query");
    });
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "the second Back leaves the album: the trail holds it once"
        );
        assert!(
            !app.can_navigate_back(),
            "the trail is Library → Favorites → query"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn suggestion_and_command_queries_move_sidebar_to_library() {
    let (dir, engine) = seed_library("br_cmd_scope", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // The inspector's tag and location links call set_search_query.
    view.update(vcx, |app, cx| {
        app.select_album(FAVORITES, cx);
        app.set_search_query("tag:kyoto", cx);
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "set_search_query searches the whole library"
        );
    });
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, cx| {
        assert_eq!(app.selection, FAVORITES, "Back restores the album");
        assert_eq!(app.search_query, "");

        app.apply_search_suggestion_at(0, cx);
        assert!(!app.search_query.is_empty(), "a suggestion was applied");
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "a suggestion searches the whole library"
        );
    });
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, FAVORITES, "Back restores the album");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn search_from_an_album_year_goes_back_to_that_year() {
    let (dir, engine) = seed_library("br_year_scope", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let year = view.update(vcx, |app, _| app.timeline_year_buckets[0].0);
    for suggestion in [false, true] {
        view.update(vcx, |app, cx| {
            app.select_album(AlbumSelection::AllLibrary, cx);
            app.reset_navigation_history();
            app.select_album(FAVORITES, cx);
            app.set_active_year(Some(year), cx);
            if suggestion {
                app.apply_search_suggestion_at(0, cx);
            } else {
                app.set_search_query("tag:kyoto", cx);
            }
            assert_eq!(app.selection, AlbumSelection::AllLibrary);
        });
        focus_app(vcx, &view);
        keystrokes(vcx, "secondary-[");
        view.update(vcx, |app, _| {
            assert_eq!(
                (
                    app.selection.clone(),
                    app.active_year,
                    app.search_query.as_str()
                ),
                (AlbumSelection::AllLibrary, Some(year), ""),
                "Back (suggestion: {suggestion}) lands on the year that was left \
                 (a year is the whole library's)"
            );
        });
        keystrokes(vcx, "secondary-[");
        view.update(vcx, |app, _| {
            assert_eq!(
                (app.selection.clone(), app.active_year),
                (FAVORITES, None),
                "one more Back is the album the year was chosen from"
            );
        });
        keystrokes(vcx, "secondary-[");
        view.update(vcx, |app, _| {
            assert_eq!(app.selection, AlbumSelection::AllLibrary);
            assert!(
                !app.can_navigate_back(),
                "no phantom album-without-year step (suggestion: {suggestion})"
            );
        });
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_clear_filters_keeps_free_text() {
    let (dir, engine) = seed_library("br_clear_filters", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let offered = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            app.command_query = "clear filters".into();
            let listed = app.palette_rows().iter().any(|row| {
                row.target
                    == crate::app::command_palette::PaletteTarget::Command(
                        crate::app::command_palette::CommandId::ClearFilters,
                    )
            });
            app.command_query.clear();
            listed
        })
    };
    assert!(
        !offered(&view, vcx),
        "Clear Filters is hidden with no filter"
    );

    view.update(vcx, |app, cx| {
        app.set_search_query("kyoto is:video -tag:work rating:>=2", cx)
    });
    assert!(offered(&view, vcx), "Clear Filters is listed with a filter");
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k c l e a r space f i l t e r s enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "kyoto",
            "Clear Filters drops the filter tokens and keeps the text"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn sidebar_has_no_events_and_untagged_sits_under_smart_albums() {
    let (dir, engine) = seed_library("br_sidebar", 2);
    let mut cx = crate::tests::TestApp::single();
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
    let untagged = vcx.debug_bounds("sidebar-nav-untagged").expect("Untagged");
    let new_album = vcx.debug_bounds("new-smart-album").expect("+");
    let tags_header = vcx
        .debug_bounds("sidebar-tags-header")
        .expect("TAGS header");
    assert!(
        untagged.top() > new_album.top() && untagged.top() < tags_header.top(),
        "Untagged sits in SMART ALBUMS, above TAGS"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn context_menu_sets_label_and_rating_with_20px_targets() {
    let (dir, engine) = seed_library("br_menu", 2);
    let mut cx = crate::tests::TestApp::single();
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
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| app.inspector_open = false);
    render_ui(vcx);
    // Opening the inspector moves the slider left. Toggling the inspector moves the slider; the next click must use the
    // new position, not the one captured before the move.
    // Exactly one frame after the move, so a listener built from the
    // previous frame's x would still be live.
    view.update(vcx, |app, cx| app.toggle_inspector(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        app.grid_columns = 2;
    });
    keystrokes(vcx, "secondary--");
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_columns, 3, "⌘- steps one zoom level")
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
    let mut cx = crate::tests::TestApp::single();
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

#[core::prelude::v1::test]
fn a_wide_thumbnail_never_widens_its_card() {
    // Undecodable: a real decode landing after the injected wide texture
    // would replace it, and the test would pass on a square thumbnail.
    let (dir, engine) = seed_library_undecodable("br_wide_thumb", 4);
    let items = engine.get_item_slice(0..4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    let column = vcx
        .debug_bounds(leak(format!("media-card-photo-{}", items[1].id.0)))
        .expect("the photo well renders")
        .size
        .width;

    // A panorama, far wider than its square well. Each mode
    // uses a fresh item: debug_bounds keeps an element's first-frame entry, so
    // only a thumbnail painted for the first time shows the new layout.
    let wide = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(2000, 200),
    )]));
    for (fit, item) in [(true, &items[0]), (false, &items[2])] {
        view.update(vcx, |app, cx| {
            app.card_fit_mode = fit;
            let state = app.grid_state.as_ref().expect("grid state");
            state.texture_cache.lock().put(item.id, wide.clone());
            cx.notify();
        });
        render_ui(vcx);
        let thumb = vcx
            .debug_bounds(leak(format!("media-card-thumb-{}", item.id.0)))
            .expect("the cached thumbnail paints");
        assert!(
            (thumb.size.width - column).abs() < gpui::px(1.0),
            "fit={fit}: a wide thumbnail must stay in its column ({column:?}), got {thumb:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Query explanation and recent searches (UX-2 Task 8) ----

#[core::prelude::v1::test]
fn explanation_lists_not_understood_tokens() {
    let (dir, engine) = seed_library("br_explain", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    click(vcx, "search-input");
    assert!(
        vcx.debug_bounds("search-explanation").is_none(),
        "an empty query has nothing to explain"
    );
    type_text(vcx, "is:fovorite sunset");
    assert!(
        vcx.debug_bounds("search-explanation").is_some(),
        "the focused field explains a non-empty query"
    );
    view.update(vcx, |app, _| {
        let text = app.search_explanation().unwrap_or_default();
        assert!(
            text.contains("Not understood: is:fovorite"),
            "a misspelled filter is reported, got {text:?}"
        );
        assert!(
            text.contains("Matches “sunset”"),
            "free text is explained too, got {text:?}"
        );
    });
    type_text(vcx, " is:f");
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_some(),
        "a partial is: token offers suggestion rows"
    );
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_none()
            && vcx.debug_bounds("search-explanation").is_some(),
        "the first Escape hides the suggestion rows but keeps the explanation"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn recent_rows_are_marked_and_clearable() {
    let (dir, engine) = seed_library("br_recent_clear", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        app.config.recent_searches = vec!["camera:sony".into(), "kyoto".into()];
    });
    click(vcx, "search-input");
    for i in 0..2 {
        assert!(
            vcx.debug_bounds(leak(format!("search-suggestion-recent-{i}")))
                .is_some(),
            "recent row {i} is marked as recent"
        );
    }
    assert!(
        vcx.debug_bounds("search-suggestion-2").is_some()
            && vcx.debug_bounds("search-suggestion-recent-2").is_none(),
        "operator rows are not marked as recent"
    );

    // Escape hides the rows, a second Escape leaves the field.
    keystrokes(vcx, "escape escape");
    view.update(vcx, |app, _| assert!(!app.search_focused));
    keystrokes(
        vcx,
        "secondary-k c l e a r space r e c e n t space s e a r c h e s enter",
    );
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open, "the command ran");
        assert!(
            app.config.recent_searches.is_empty(),
            "Clear Recent Searches empties the history in memory"
        );
    });
    click(vcx, "search-input");
    assert!(
        vcx.debug_bounds("search-suggestion-0").is_some(),
        "the dropdown is open again, so the next check is not vacuous"
    );
    assert!(
        vcx.debug_bounds("search-suggestion-recent-0").is_none(),
        "no recent rows are left in the dropdown"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn clear_recent_searches_is_hidden_when_empty_and_reverts_a_failed_save() {
    use crate::app::command_palette::{CommandId, PaletteTarget};
    let (dir, engine) = seed_library("br_recent_revert", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let listed = |app: &crate::LuminaApp| {
        app.palette_rows()
            .iter()
            .any(|row| row.target == PaletteTarget::Command(CommandId::ClearRecentSearches))
    };
    view.update(vcx, |app, cx| {
        app.command_query = "clear recent searches".into();
        assert!(!listed(app), "nothing to clear: the command is hidden");
        let recents = vec!["camera:sony".to_string(), "kyoto".to_string()];
        app.config.recent_searches = recents.clone();
        assert!(listed(app), "⌘K offers Clear Recent Searches");

        crate::app::state::SAVE_FAILS.with(|f| f.set(true));
        app.execute_command(CommandId::ClearRecentSearches, cx);
        crate::app::state::SAVE_FAILS.with(|f| f.set(false));
        assert_eq!(
            app.config.recent_searches, recents,
            "a failed save puts the cleared history back"
        );
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.starts_with("Could not save settings")),
            "and the banner says why, got {:?}",
            app.error_message
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
