//! Command palette rating commands and empty-state actions.

use super::helpers::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::*;

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
    let (_temp_dir, engine) = seed_library("palette_rating", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn empty_search_offers_chips_that_remove_one_term() {
    let (_temp_dir, engine) = seed_library("empty_terms", 3);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn empty_library_offers_rescan() {
    let (_temp_dir, engine) = seed_library("empty_library", 0);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn palette_rating_a_mixed_selection_sets_every_item() {
    let (_temp_dir, engine) = seed_library("palette_mixed", 2);
    let items = engine.get_item_slice(0..2);
    let (a, b) = (items[0].clone(), items[1].clone());

    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn empty_search_chips_keep_quoted_terms_whole() {
    let (_temp_dir, engine) = seed_library("empty_quoted", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.set_search_query("tag:\"new york\" beach", cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-search-term-2").is_none(),
        "a quoted phrase must be one chip, not one per word"
    );
    assert!(
        vcx.debug_bounds("empty-search-term-0-text:Drop “tag:new york”")
            .is_some(),
        "a quoted term's own quotes must not nest inside the chip's “…”"
    );

    click(vcx, "empty-search-term-1");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "tag:\"new york\"",
            "removing a term must leave the quoted phrase intact"
        );
    });
}

#[core::prelude::v1::test]
fn empty_library_while_scanning_says_so_without_rescan() {
    let (_temp_dir, engine) = seed_library("empty_scanning", 0);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn palette_matches_each_word_caps_commands_and_clamps() {
    use crate::app::command_palette::MAX_COMMANDS;
    let (_dir, engine) = seed_library("palette_v3_words", 2);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn palette_go_to_opens_views_tags_and_years() {
    use crate::views::sidebar::AlbumSelection;
    let (_dir, engine) = seed_library("palette_v3_goto", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine
        .catalog()
        .add_tags_batch(&[item.id], &["trip".to_string()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn palette_hides_trash_and_retry_commands_that_cannot_act() {
    let (_dir, engine) = seed_library("palette_applies", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    // Narrow the list so the three commands are not cut by MAX_COMMANDS.
    let shown =
        |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext, q: &str| {
            view.update(vcx, |app, _| {
                app.command_query = q.into();
                let n = app.palette_rows().iter().filter(|r| !r.is_go_to()).count();
                app.command_query.clear();
                n
            })
        };
    assert_eq!(
        shown(&view, vcx, "empty trash"),
        0,
        "no trash, no Empty Trash…"
    );
    assert_eq!(
        shown(&view, vcx, "restore all"),
        0,
        "no trash, no Restore All"
    );
    assert_eq!(shown(&view, vcx, "retry"), 0, "no failure, no Retry");

    engine.set_trashed(item.id, true).unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.set_error_with_retry("Couldn't clear cache: test", |_, _| {}, cx);
    });
    assert_eq!(shown(&view, vcx, "empty trash"), 1, "trash has items");
    assert_eq!(shown(&view, vcx, "restore all"), 1, "trash has items");
    assert_eq!(
        shown(&view, vcx, "retry"),
        1,
        "a retryable failure is showing"
    );
}

#[core::prelude::v1::test]
fn palette_go_to_opens_screenshots_places_and_missing() {
    use crate::views::sidebar::AlbumSelection;
    use lumina_core::SystemAlbum;
    let (_dir, engine) = seed_library("palette_goto_sidebar", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);

    for (query, slug, album) in [
        ("screenshots", "screenshots", SystemAlbum::Screenshots),
        ("places", "places", SystemAlbum::Places),
        // Listed even while nothing is missing, like Downloaded. "Go to"
        // skips the Locate Missing File command, which ranks first.
        ("go to missing", "missing", SystemAlbum::Missing),
    ] {
        view.update(vcx, |app, cx| {
            app.select_album(AlbumSelection::AllLibrary, cx)
        });
        focus_app(vcx, &view);
        keystrokes(vcx, "secondary-k");
        for ch in query.chars() {
            keystrokes(
                vcx,
                if ch == ' ' {
                    "space"
                } else {
                    leak(ch.to_string())
                },
            );
        }
        render_ui(vcx);
        assert!(
            vcx.debug_bounds(leak(format!("palette-go-{slug}")))
                .is_some(),
            "⌘K {query} lists the {slug} collection"
        );
        keystrokes(vcx, "enter");
        view.update(vcx, |app, _| {
            assert!(!app.command_palette_open);
            assert_eq!(
                app.selection,
                AlbumSelection::System(album),
                "every sidebar collection has a keyboard path: ⌘K {query} ↵"
            );
        });
    }
}

#[core::prelude::v1::test]
fn empty_search_drop_chips_and_clear_all() {
    let (_temp_dir, engine) = seed_library("empty_search_clear_all", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let palette_titles = |vcx: &mut gpui::VisualTestContext| {
        focus_app(vcx, &view);
        keystrokes(vcx, "secondary-k c l e a r space s e a r c h");
        let titles: Vec<String> = view.update(vcx, |app, _| {
            app.palette_rows().into_iter().map(|r| r.title).collect()
        });
        keystrokes(vcx, "escape");
        titles
    };

    for (query, headline) in [
        ("zzqx", "Nothing matches this filter"),
        ("zzqx yyqx xxqx", "Nothing matches all three filters"),
        ("zzqx yyqx", "Nothing matches both filters"),
    ] {
        view.update(vcx, |app, cx| app.set_search_query(query, cx));
        render_ui(vcx);
        assert!(
            vcx.debug_bounds(leak(format!("empty-state-headline-text:{headline}")))
                .is_some(),
            "{query:?} must read {headline:?}: the headline counts terms in words"
        );
    }
    assert!(
        vcx.debug_bounds("empty-state-body-text:Try loosening one of them.")
            .is_some(),
        "two or more terms suggest loosening one"
    );
    assert!(
        vcx.debug_bounds("empty-search-term-0-text:Drop “zzqx”")
            .is_some(),
        "each term chip reads Drop “term”"
    );

    click(vcx, "empty-secondary");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "Clear all filters must empty the query"
        )
    });

    assert!(
        !palette_titles(vcx).iter().any(|t| t == "Clear Search"),
        "⌘K Clear Search is hidden while the query is empty"
    );
    view.update(vcx, |app, cx| app.set_search_query("zzqx yyqx", cx));
    render_ui(vcx);
    assert!(
        palette_titles(vcx).iter().any(|t| t == "Clear Search"),
        "⌘K offers Clear Search while there is a query"
    );
    run_palette_command(vcx, &view, "clear search");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "⌘K Clear Search must empty the query"
        )
    });

    // Esc from the search field: hide suggestions, blur, then clear.
    view.update(vcx, |app, cx| {
        app.set_search_query("zzqx", cx);
        app.inspector_open = false;
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-f escape escape escape");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "Esc from the search field must still clear the query"
        )
    });
}

/// An empty library after its scan uses the First Run
/// (Hero) glyph and buttons; other empty views keep the default size.
#[core::prelude::v1::test]
fn empty_library_state_uses_first_run_glyph_and_buttons() {
    use crate::views::sidebar::AlbumSelection;

    let (_temp_dir, engine) = seed_library("empty_hero", 0);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    render_ui(vcx);
    let glyph = vcx.debug_bounds("empty-state-glyph").expect("a glyph");
    let side = (f32::from(glyph.size.width), f32::from(glyph.size.height));
    assert_eq!(
        side,
        (150.0, 150.0),
        "the empty library uses the hero glyph"
    );
    assert!(
        vcx.debug_bounds("empty-state-headline-text:No photos or videos found")
            .is_some(),
        "the copy stays the same"
    );
    for button in ["empty-primary", "empty-secondary"] {
        let bounds = vcx.debug_bounds(button).expect("both actions stay");
        assert_eq!(f32::from(bounds.size.height), 40.0, "{button} is 40 high");
    }

    let (_temp_dir, engine) = seed_library("empty_tag_standard", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-here".into()), cx)
    });
    render_ui(vcx);
    let glyph = vcx
        .debug_bounds("empty-state-glyph")
        .expect("the empty tag view draws a glyph");
    assert_eq!(
        f32::from(glyph.size.width),
        54.0,
        "an empty tag view keeps the default glyph"
    );
}

#[core::prelude::v1::test]
fn ui_automation_command_palette() {
    let (_temp_dir, engine) = seed_library("command_palette", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    view.update(vcx, |app, _| assert!(app.command_palette_open));

    for key in ["p", "r", "e", "f"] {
        keystrokes(vcx, key);
    }
    view.update(vcx, |app, _| {
        assert_eq!(app.command_query, "pref");
        let titles: Vec<String> = app
            .palette_rows()
            .into_iter()
            .map(|row| row.title)
            .collect();
        assert_eq!(titles.first().map(String::as_str), Some("Settings…"));
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        assert!(app.preferences_open);
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.preferences_open));

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    for key in ["a", "b", "o", "u", "t"] {
        keystrokes(vcx, key);
    }
    click(vcx, "command-item-command.about");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open);
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::About
        );
    });
    keystrokes(vcx, "escape");

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.command_palette_open));
}
