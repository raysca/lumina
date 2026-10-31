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
    keystrokes(vcx, "secondary-k");
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
    keystrokes(vcx, "secondary-a");
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

#[core::prelude::v1::test]
fn palette_matches_each_word_caps_commands_and_clamps() {
    use crate::app::command_palette::MAX_COMMANDS;
    let (dir, engine) = seed_library("palette_v3_words", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");

    view.update(vcx, |app, _| {
        let rows = app.palette_rows();
        let commands = rows.iter().filter(|r| !r.is_go_to()).count();
        assert_eq!(commands, MAX_COMMANDS, "commands are capped at eight");
        assert!(
            rows.iter().any(|r| r.is_go_to()),
            "GO TO follows the commands"
        );
    });
    for selector in ["palette-go-all", "command-footer", "command-result-count"] {
        assert!(vcx.debug_bounds(selector).is_some(), "{selector} renders");
    }

    // ↑ at the top and ↓ past the end clamp instead of wrapping.
    keystrokes(vcx, "up");
    view.update(vcx, |app, _| {
        assert_eq!(app.command_selected, 0, "↑ clamps at the top")
    });
    let last = view.update(vcx, |app, _| app.palette_rows().len() - 1);
    for _ in 0..last + 3 {
        keystrokes(vcx, "down");
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.command_selected, last, "↓ clamps at the end")
    });

    // "name sort" matches word by word, in any order.
    keystrokes(vcx, "escape secondary-k n a m e space s o r t");
    view.update(vcx, |app, _| {
        let titles: Vec<String> = app.palette_rows().into_iter().map(|r| r.title).collect();
        assert_eq!(
            titles,
            vec!["Sort by Name".to_string()],
            "each word must match"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_go_to_opens_views_tags_and_years() {
    use crate::views::sidebar::AlbumSelection;
    let (dir, engine) = seed_library("palette_v3_goto", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .catalog()
        .add_tags_batch(&[item.id], &["trip".to_string()])
        .unwrap();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    focus_app(vcx, &view);

    keystrokes(vcx, "secondary-k t r i p enter");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("trip".into()),
            "#tag opens the tag"
        );
    });

    keystrokes(vcx, "secondary-k d e l e t e d");
    render_ui(vcx);
    click(vcx, "palette-go-trash");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "Recently Deleted opens Trash"
        )
    });

    let year = view.update(vcx, |app, _| {
        app.timeline_year_buckets.first().map(|(y, _)| *y)
    });
    let year = year.expect("the seeded library has a year");
    keystrokes(vcx, "secondary-k");
    keystrokes(
        vcx,
        &year
            .to_string()
            .chars()
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" "),
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::AllLibrary);
        assert_eq!(app.active_year, Some(year), "a year row jumps to that year");
    });
    view.update(vcx, |app, cx| app.navigate_back(cx));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Trash,
            "one Back undoes the whole year jump"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}
