//! v3 command palette rating commands and empty-state actions.

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};

fn run_palette_command(
    vcx: &mut gpui::VisualTestContext,
    view: &gpui::Entity<crate::LuminaApp>,
    query: &str,
) {
    focus_app(vcx, view);
    keystrokes(vcx, "cmd-k");
    for ch in query.chars() {
        let key = if ch == ' ' {
            "space".to_string()
        } else {
            ch.to_string()
        };
        keystrokes(vcx, &key);
    }
    keystrokes(vcx, "enter");
}

#[core::prelude::v1::test]
fn palette_rates_clears_and_favorites_the_selection() {
    let (temp_dir, engine) = seed_library("palette_rating", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, card);

    run_palette_command(vcx, &view, "rate 3");
    assert_eq!(
        engine.catalog().get_rating(item.id).unwrap(),
        3,
        "\"Rate 3 Stars\" from the palette must persist the rating"
    );

    run_palette_command(vcx, &view, "rate 3");
    assert_eq!(
        engine.catalog().get_rating(item.id).unwrap(),
        0,
        "re-applying the rating every selected item has clears it"
    );

    run_palette_command(vcx, &view, "rate 2");
    run_palette_command(vcx, &view, "clear rating");
    assert_eq!(
        engine.catalog().get_rating(item.id).unwrap(),
        0,
        "\"Clear Rating\" must remove the stored rating"
    );

    run_palette_command(vcx, &view, "toggle fav");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        assert!(
            app.selected_item.as_ref().is_some_and(|i| i.is_favorite),
            "\"Toggle Favorite\" must favorite the selected item"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn empty_search_offers_chips_that_remove_one_term() {
    let (temp_dir, engine) = seed_library("empty_terms", 3);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.set_search_query("nosuch alsonot", cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-search-term-1").is_some(),
        "a multi-term empty search must offer one chip per term"
    );

    click(vcx, "empty-search-term-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "alsonot",
            "clicking a term chip must drop only that term"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn empty_library_offers_rescan() {
    let (temp_dir, engine) = seed_library("empty_library", 0);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-primary").is_some(),
        "an empty library must offer to choose another folder"
    );

    click(vcx, "empty-secondary");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "the empty library's Rescan action must start a scan"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn palette_rating_a_mixed_selection_sets_every_item() {
    let (temp_dir, engine) = seed_library("palette_mixed", 2);
    let items = engine.get_item_slice(0..2);
    let (a, b) = (items[0].clone(), items[1].clone());

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", a.id.0)));
    run_palette_command(vcx, &view, "rate 3");
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-a");
    view.update(vcx, |app, _| assert_eq!(app.selected_indices.len(), 2));

    run_palette_command(vcx, &view, "rate 3");
    for id in [a.id, b.id] {
        assert_eq!(
            engine.catalog().get_rating(id).unwrap(),
            3,
            "rating a mixed selection must set it on every item, never toggle one off"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn empty_search_chips_keep_quoted_terms_whole() {
    let (temp_dir, engine) = seed_library("empty_quoted", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.set_search_query("tag:\"new york\" beach", cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-search-term-2").is_none(),
        "a quoted phrase must be one chip, not one per word"
    );

    click(vcx, "empty-search-term-1");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "tag:\"new york\"",
            "removing a term must leave the quoted phrase intact"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn empty_library_while_scanning_says_so_without_rescan() {
    let (temp_dir, engine) = seed_library("empty_scanning", 0);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = true;
        }
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-primary").is_some());
    assert!(
        vcx.debug_bounds("empty-secondary").is_none(),
        "an empty library mid-scan must not offer Rescan or claim it is empty"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
