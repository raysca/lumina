//! The search header with its token query bar, and the
//! rule that decides between the Search screen and the Library.

use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use gpui::{point, px, Bounds, Entity, Modifiers, MouseButton, Pixels, VisualTestContext};

const QUERY: &str = "city:Lisbon month:jun year:2025 tram";

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

#[core::prelude::v1::test]
fn query_tokens_render_as_chips_with_connectors() {
    let (dir, engine) = seed_library("search_header_chips", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, QUERY);
    render_ui(vcx);

    let header = bounds(vcx, "search-header");
    assert!(
        (f32::from(header.size.height) - 131.5).abs() <= 0.5,
        "the search header is 131.5 high: {header:?}"
    );
    let bar = bounds(vcx, "search-query-bar");
    assert_eq!(f32::from(bar.size.height), 48.0, "the query bar is 48 high");
    for selector in [
        "search-token-0",
        "search-token-1",
        "search-token-connector-0",
        "search-token-connector-1",
        "search-typed-text",
    ] {
        let el = bounds(vcx, selector);
        assert!(bar.contains(&el.center()), "{selector} draws in the bar");
    }
    assert!(
        vcx.debug_bounds("search-token-2").is_none(),
        "month and year merge into one chip: two chips for three filter tokens"
    );
    let (first, second, text) = (
        bounds(vcx, "search-token-0"),
        bounds(vcx, "search-token-1"),
        bounds(vcx, "search-typed-text"),
    );
    assert!(
        first.right() <= second.left() && second.right() <= text.left(),
        "chips in query order, the typed text after them"
    );
    view.update(vcx, |app, _| {
        let (chips, typed) = app.search_bar_parts();
        let shown: Vec<(String, Option<String>)> = chips
            .iter()
            .map(|chip| (chip.bar_label(), chip.family.connector()))
            .collect();
        assert_eq!(
            shown,
            vec![
                ("Lisbon".to_string(), Some("in".to_string())),
                ("Jun 2025".to_string(), Some("during".to_string())),
            ],
            "place chips read \"in\", date chips \"during\" (Design 01's labels)"
        );
        assert_eq!(typed, "tram", "free text stays plain text");
    });

    let count = bounds(vcx, "search-result-count");
    assert!(
        bar.contains(&count.center()) && count.left() >= text.right(),
        "the count sits at the bar's right: {count:?} in {bar:?}"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_result_label(),
            Some(crate::i18n::results(app.displayed_count())),
            "the count reads the displayed results"
        );
    });
    assert_eq!(crate::i18n::results(0), "0 results");
    assert_eq!(crate::i18n::results(1), "1 result", "singular");
    assert_eq!(crate::i18n::results(1284), "1,284 results");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn removing_a_token_chip_edits_the_query() {
    let (dir, engine) = seed_library("search_header_remove", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, QUERY);

    click(vcx, "search-token-remove-0");
    assert_eq!(
        query(&view, vcx),
        "month:jun year:2025 tram",
        "the chip's remove button drops its token"
    );

    set_query(&view, vcx, QUERY);
    focus_app(vcx, &view);
    click(vcx, "search-typed-text");
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "clicking the bar focuses search")
    });
    keystrokes(vcx, "backspace");
    assert_eq!(
        query(&view, vcx),
        "city:Lisbon month:jun year:2025 tra",
        "⌫ after typed text deletes a character, not a chip"
    );
    keystrokes(vcx, "left left left");
    keystrokes(vcx, "backspace");
    assert_eq!(
        query(&view, vcx),
        "city:Lisbon tra",
        "⌫ at the start of the typed text removes the last chip (month+year)"
    );
    // ← at the start selects the last chip; ⌫ then removes that one.
    keystrokes(vcx, "left");
    assert!(
        vcx.debug_bounds("search-token-0-selected").is_some(),
        "← at the caret's start selects the last chip"
    );
    keystrokes(vcx, "right");
    assert!(
        vcx.debug_bounds("search-token-0-selected").is_none(),
        "→ past the last chip returns to the text"
    );
    keystrokes(vcx, "left");
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("search-token-0-selected").is_none(),
        "Esc drops the chip selection first"
    );
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "that Esc keeps the field focused");
        assert!(
            !app.search_suggestions.dismissed,
            "that Esc leaves the suggestion layer alone"
        );
    });
    keystrokes(vcx, "left");
    keystrokes(vcx, "backspace");
    assert_eq!(query(&view, vcx), "tra", "⌫ removes the selected chip");
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn search_screen_rule_free_text_or_focus() {
    let (dir, engine) = seed_library("search_header_rule", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    set_query(&view, vcx, "tram");
    assert!(
        vcx.debug_bounds("search-header").is_some(),
        "free text shows the Search screen"
    );
    assert!(
        vcx.debug_bounds("page-header").is_none(),
        "the page header hides on the Search screen"
    );

    set_query(&view, vcx, "city:Lisbon");
    assert!(
        vcx.debug_bounds("page-header").is_some() && vcx.debug_bounds("filter-bar").is_some(),
        "filter tokens alone show the Library with its chip row"
    );
    assert!(
        vcx.debug_bounds("search-header").is_none(),
        "no search header for filter tokens alone"
    );

    set_query(&view, vcx, "");
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-f");
    assert!(
        vcx.debug_bounds("search-header").is_some() && vcx.debug_bounds("page-header").is_none(),
        "focusing the empty field shows the Search screen"
    );
    let bar = bounds(vcx, "search-query-bar");
    let panel = bounds(vcx, "search-suggestions");
    assert!(
        panel.top() >= bar.bottom()
            && panel.left() == bar.left()
            && panel.size.width == bar.size.width,
        "the dropdown anchors under the query bar, full width: {panel:?} under {bar:?}"
    );

    // Esc hides the suggestions, a second Esc blurs.
    keystrokes(vcx, "escape");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| assert!(!app.search_focused, "Esc blurs"));
    assert!(
        vcx.debug_bounds("page-header").is_some() && vcx.debug_bounds("search-header").is_none(),
        "blurring with no query restores the page header"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn entering_the_field_keeps_the_last_token_a_chip() {
    let (dir, engine) = seed_library("search_header_enter", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // A filter after the text: clicking in leaves it a chip, not text.
    set_query(&view, vcx, "tram city:Lisbon");
    focus_app(vcx, &view);
    click(vcx, "search-typed-text");
    view.update(vcx, |app, _| assert!(app.search_focused));
    assert!(
        vcx.debug_bounds("search-token-0").is_some(),
        "the committed last filter stays a chip when the field is entered"
    );
    keystrokes(vcx, "left left left left");
    keystrokes(vcx, "backspace");
    assert_eq!(query(&view, vcx), "tram", "⌫ at the start removes the chip");

    // A lone filter, entered from the sidebar field: ⌫ removes the chip
    // rather than editing its text.
    view.update(vcx, |app, cx| app.blur_search(cx));
    set_query(&view, vcx, "city:Lisbon");
    click(vcx, "search-input");
    assert!(
        vcx.debug_bounds("search-token-0").is_some(),
        "the lone filter shows as a chip in the bar"
    );
    keystrokes(vcx, "backspace");
    assert_eq!(query(&view, vcx), "", "⌫ removes the whole chip");

    // The cap holds for the whole query, chips included, not the typed half.
    let max = lumina_core::text::MAX_TEXT_CHARS;
    set_query(&view, vcx, "city:Lisbon tram");
    vcx.write_to_clipboard(gpui::ClipboardItem::new_string("x".repeat(max)));
    keystrokes(vcx, "secondary-v");
    let q = query(&view, vcx);
    assert!(
        q.starts_with("city:Lisbon tram") && q.chars().count() == max,
        "a paste fills the query up to the cap and no further: {} chars",
        q.chars().count()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_click_that_blurs_search_keeps_the_header_until_mouse_up() {
    let (dir, engine) = seed_library("search_header_hold", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-f");
    // Esc hides the suggestions (empty query: no explanation either), so
    // the press lands on the grid column, not the dropdown.
    keystrokes(vcx, "escape");
    assert!(vcx.debug_bounds("search-suggestions").is_none());
    let header = bounds(vcx, "search-header");
    // Below the header, in the grid column.
    let at = point(header.center().x, header.bottom() + px(120.0));

    vcx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.search_focused, "mouse-down blurs")
    });
    assert!(
        vcx.debug_bounds("search-header").is_some(),
        "the header keeps its height while the button is down, so the click lands where it was aimed"
    );
    vcx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("page-header").is_some() && vcx.debug_bounds("search-header").is_none(),
        "the Library header returns on mouse-up"
    );

    // The sidebar field's own click does not blur on the way down.
    let field = bounds(vcx, "search-input").center();
    vcx.simulate_mouse_down(field, MouseButton::Left, Modifiers::default());
    render_ui(vcx);
    vcx.simulate_mouse_up(field, MouseButton::Left, Modifiers::default());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.search_focused, "clicking the field focuses search");
        assert!(!app.query_bar.held_for_click, "no hold left behind");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn emptying_the_typed_text_keeps_the_chips() {
    let (dir, engine) = seed_library("search_header_empty_typed", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // ⌫ through the free text: the last filter stays a chip.
    set_query(&view, vcx, "tram city:Lisbon");
    focus_app(vcx, &view);
    click(vcx, "search-typed-text");
    keystrokes(vcx, "backspace backspace backspace backspace");
    assert!(
        vcx.debug_bounds("search-token-0").is_some(),
        "emptying the typed text leaves Lisbon a chip, not text"
    );
    keystrokes(vcx, "backspace");
    assert_eq!(query(&view, vcx), "", "the next ⌫ removes the chip whole");

    // One character typed after a lone chip, then ⌫: the chip stays.
    view.update(vcx, |app, cx| app.blur_search(cx));
    set_query(&view, vcx, "city:Lisbon");
    click(vcx, "search-input");
    vcx.simulate_input("x");
    render_ui(vcx);
    keystrokes(vcx, "backspace");
    assert!(
        vcx.debug_bounds("search-token-0").is_some(),
        "typing and erasing one character leaves the chip"
    );
    keystrokes(vcx, "backspace");
    assert_eq!(query(&view, vcx), "", "the next ⌫ removes the chip whole");
    let _ = std::fs::remove_dir_all(&dir);
}

// Save as Smart Album and the Narrow by chips.

#[core::prelude::v1::test]
fn save_button_opens_the_prefilled_builder() {
    use lumina_core::{RuleField, RuleOperator, RuleValue, SmartAlbumRule};
    let (dir, engine) = seed_library("search_header_save", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // No item matches: an empty smart album is still a valid one to save.
    let query = "tram is:favorite";
    set_query(&view, vcx, query);
    let save = bounds(vcx, "search-save-smart-album");
    let bar = bounds(vcx, "search-query-bar");
    let count = bounds(vcx, "search-result-count");
    assert!(
        bar.contains(&save.center()) && save.left() >= count.right(),
        "Save sits right of the count, inside the bar: {save:?}"
    );
    assert_eq!(f32::from(save.size.height), 28.0, "Save is 28 high");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_result_label().as_deref(), Some("0 results"));
        assert!(app.search_save_enabled(), "Save stays enabled at 0 results");
    });

    click(vcx, "search-save-smart-album");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "Save opens the builder");
        assert_eq!(app.album_draft.editing_album_id, None, "a new album");
        assert_eq!(app.album_draft.album_title, query, "the query is the title");
        assert_eq!(
            app.draft_rules(),
            Some(vec![SmartAlbumRule {
                field: RuleField::Query,
                operator: RuleOperator::Equals,
                value: RuleValue::Text(query.into()),
            }]),
            "one Query rule holding the raw search text"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

fn save_search_listed(app: &crate::LuminaApp) -> bool {
    use crate::app::command_palette::{CommandId, PaletteTarget};
    app.palette_rows()
        .iter()
        .any(|row| row.target == PaletteTarget::Command(CommandId::SaveSearch))
}

#[core::prelude::v1::test]
fn save_is_disabled_while_compare_holds_the_view() {
    let (dir, engine) = seed_library("search_header_save_compare", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "photo");
    view.update(vcx, |app, cx| {
        assert!(app.search_save_enabled());
        app.command_query = "save search".into();
        assert!(save_search_listed(app), "the palette offers Save Search");
        app.select_all(cx);
        app.open_compare(cx);
        assert!(app.compare.is_some(), "compare opened on the results");
        assert!(
            !app.search_save_enabled(),
            "Save is off while compare holds the view"
        );
        assert!(
            !save_search_listed(app),
            "the palette's Save Search is unavailable under compare"
        );
        app.command_query.clear();
    });
    render_ui(vcx);
    click(vcx, "search-save-smart-album");
    view.update(vcx, |app, _| {
        assert!(
            !app.album_draft.modal_open,
            "a click on the disabled Save opens nothing"
        );
        app.compare = None;
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn save_is_disabled_in_edit_mode() {
    use crate::tests::quick_edit::{edit_library, editing, open_viewer_on};
    let (dir, engine, photos, _) = edit_library("search_header_save_edit");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    set_query(&view, vcx, "tram");
    open_viewer_on(&view, vcx, &engine, photos[0]);
    keystrokes(vcx, "e");
    assert!(editing(&view, vcx), "E opens edit mode");
    view.update(vcx, |app, _| {
        assert!(!app.search_save_enabled(), "Save is off in edit mode");
        app.command_query = "save search".into();
        assert!(
            !save_search_listed(app),
            "the palette's Save Search is unavailable in edit mode"
        );
        app.command_query.clear();
    });
    // The viewer covers the header; a click there must not save either.
    click(vcx, "search-save-smart-album");
    view.update(vcx, |app, _| {
        assert!(
            !app.album_draft.modal_open,
            "a click on the disabled Save opens nothing"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn save_leaves_the_search_field() {
    let (dir, engine) = seed_library("search_header_save_blur", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "tram");
    focus_app(vcx, &view);
    click(vcx, "search-typed-text");
    assert!(vcx.debug_bounds("search-suggestions").is_some());
    click(vcx, "search-save-smart-album");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "Save opens the builder");
        assert!(
            !app.search_focused,
            "the builder takes the keys, not search"
        );
    });
    assert!(
        vcx.debug_bounds("search-suggestions").is_none(),
        "no dropdown paints over the builder"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn narrow_chips_toggle_query_tokens() {
    let (dir, engine) = seed_library("search_header_narrow", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "tram");

    let header = bounds(vcx, "search-header");
    let bar = bounds(vcx, "search-query-bar");
    let row = bounds(vcx, "search-narrow");
    assert!(
        row.top() >= bar.bottom() && row.bottom() <= header.bottom(),
        "the Narrow by row sits under the bar, inside the header: {row:?}"
    );
    let (rating, edited, raw) = (
        bounds(vcx, "search-narrow-rating"),
        bounds(vcx, "search-narrow-edited"),
        bounds(vcx, "search-narrow-raw"),
    );
    assert!(
        rating.right() <= edited.left() && edited.right() <= raw.left(),
        "Edited sits after Rated 4+ and before RAW, as in the frame"
    );
    assert!(
        (f32::from(row.top() - bar.bottom()) - 13.0).abs() <= 0.5,
        "13 between the bar and the row"
    );

    for (selector, token) in [
        ("search-narrow-video", "is:video"),
        ("search-narrow-fnumber", "f:<2"),
        ("search-narrow-rating", "rating:>=4"),
        ("search-narrow-edited", "is:edited"),
        ("search-narrow-raw", "is:raw"),
    ] {
        assert_eq!(f32::from(bounds(vcx, selector).size.height), 28.0);
        click(vcx, selector);
        assert_eq!(
            query(&view, vcx),
            format!("tram {token}"),
            "{selector} appends {token}"
        );
        assert!(
            vcx.debug_bounds(crate::tests::ui_automation::leak(format!("{selector}-on")))
                .is_some(),
            "{selector} shows on"
        );
        click(vcx, selector);
        assert_eq!(query(&view, vcx), "tram", "{selector} again removes it");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn narrow_and_save_have_palette_paths_and_fit_a_narrow_window() {
    use crate::app::command_palette::CommandId;
    let (dir, engine) = seed_library("search_header_narrow_keys", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "tram");
    for token in ["f:<2", "is:video", "rating:>=4", "is:edited", "is:raw"] {
        view.update(vcx, |app, cx| {
            app.execute_command(CommandId::QuickFilter(token), cx)
        });
        assert_eq!(
            query(&view, vcx),
            format!("tram {token}"),
            "a palette quick filter is the keyboard path for {token}"
        );
        view.update(vcx, |app, cx| {
            app.execute_command(CommandId::QuickFilter(token), cx)
        });
    }
    let listed = |token: &'static str| {
        crate::app::command_palette::commands()
            .iter()
            .any(|command| command.id == CommandId::QuickFilter(token))
    };
    for token in ["f:<2", "is:video", "rating:>=4", "is:edited", "is:raw"] {
        assert!(listed(token), "the palette lists the {token} quick filter");
    }
    view.update(vcx, |app, cx| {
        app.execute_command(CommandId::SaveSearch, cx)
    });
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "⌘K Save Search is Save's path")
    });
    view.update(vcx, |app, _| app.album_draft.modal_open = false);

    // A collapsed sidebar keeps its toggle on the Search screen.
    view.update(vcx, |app, cx| app.toggle_sidebar(cx));
    render_ui(vcx);
    let header = bounds(vcx, "search-header");
    assert!(
        header.contains(&bounds(vcx, "sidebar-toggle").center()),
        "the sidebar toggle shows in the search header while collapsed"
    );
    click(vcx, "sidebar-toggle");
    view.update(vcx, |app, _| assert!(!app.sidebar_collapsed));

    // A narrow window: the chips run past the row, which scrolls them
    // rather than squeezing or wrapping them.
    vcx.simulate_resize(gpui::size(px(600.0), px(700.0)));
    render_ui(vcx);
    let header = bounds(vcx, "search-header");
    let row = bounds(vcx, "search-narrow");
    let last = bounds(vcx, "search-narrow-raw");
    assert!(
        row.right() <= header.right() && last.right() > row.right(),
        "the row overflows inside the header and scrolls: {last:?} past {row:?}"
    );
    assert_eq!(
        f32::from(last.size.height),
        28.0,
        "the chips keep their size, in one line"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// Top matches.

/// Lets the matches' debounced fetch run and land.
fn settle(vcx: &mut VisualTestContext) {
    render_ui(vcx);
    vcx.executor()
        .advance_clock(std::time::Duration::from_millis(300));
    render_ui(vcx);
}

/// Two items: one in Lisbon taken in June 2025, one in Porto; a smart album
/// titled "Lisbon trips"; a `lisbon2025` tag.
fn top_match_app<'a>(
    name: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    Entity<crate::LuminaApp>,
    &'a mut VisualTestContext,
    i64,
) {
    let (dir, engine) = seed_library(name, 2);
    let ids: Vec<u64> = engine
        .get_item_slice(0..2)
        .iter()
        .map(|item| item.id.0)
        .collect();
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).expect("catalog opens");
    conn.execute(
        "UPDATE media_items SET city = 'Lisbon', country_code = 'PT' WHERE id = ?1",
        [ids[0] as i64],
    )
    .unwrap();
    conn.execute(
        "UPDATE media_items SET city = 'Porto', country_code = 'PT' WHERE id = ?1",
        [ids[1] as i64],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO timeline_buckets (bucket_type, bucket_key, start_timestamp, end_timestamp, item_count, cover_media_id)
         VALUES ('month', '2025-06', 0, 0, 1, NULL)",
        [],
    )
    .unwrap();
    drop(conn);
    engine
        .add_tags_batch(&[lumina_core::MediaId(ids[1])], &["lisbon2025".to_string()])
        .unwrap();
    let album = engine
        .create_smart_album("Lisbon trips", "folder", lumina_core::MatchMode::All, &[])
        .expect("album saves");
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, _| {
        app.refresh_album_counts();
        app.refresh_tag_counts();
    });
    render_ui(vcx);
    (dir, view, vcx, album.id)
}

#[core::prelude::v1::test]
fn top_matches_show_place_album_month_and_navigate() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx, album) = top_match_app("search_top_matches", &mut cx);

    set_query(&view, vcx, "lisbon june");
    settle(vcx);
    let header = bounds(vcx, "search-header");
    let row = bounds(vcx, "top-matches");
    assert!(
        row.top() >= header.bottom(),
        "the matches sit under the 131.5 header, above the grid: {row:?}"
    );
    for selector in [
        "top-match-place-0",
        "top-match-album-0",
        "top-match-month-0",
        "top-match-tag-0",
    ] {
        let card = bounds(vcx, selector);
        assert_eq!(f32::from(card.size.height), 64.0, "{selector} is 64 high");
    }
    assert!(
        vcx.debug_bounds("top-match-place-1").is_none(),
        "Porto does not match"
    );
    let (place, album_card, month, tag) = (
        bounds(vcx, "top-match-place-0"),
        bounds(vcx, "top-match-album-0"),
        bounds(vcx, "top-match-month-0"),
        bounds(vcx, "top-match-tag-0"),
    );
    assert!(
        place.left() < album_card.left() && album_card.left() < month.left(),
        "Place, Smart Album, Moment, Tag, in that order"
    );
    assert!(
        f32::from(row.size.width) >= 1100.0
            && month.right() < tag.left()
            && tag.top() == place.top(),
        "four in one row in a wide window: {row:?}"
    );

    click(vcx, "top-match-place-0");
    assert_eq!(
        query(&view, vcx),
        "city:Lisbon june",
        "the place card turns its word into a city: token"
    );

    set_query(&view, vcx, "lisbon june");
    settle(vcx);
    click(vcx, "top-match-month-0");
    assert_eq!(
        query(&view, vcx),
        "lisbon month:jun year:2025",
        "the month card turns its word into month: and year: tokens"
    );

    set_query(&view, vcx, "lisbon june");
    settle(vcx);
    click(vcx, "top-match-album-0");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::User(album),
            "the album card opens the smart album"
        )
    });

    set_query(&view, vcx, "zzzz");
    settle(vcx);
    assert!(
        vcx.debug_bounds("top-matches").is_none(),
        "no match, no row"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn top_matches_take_two_columns_when_narrow_and_arrows_from_the_bar() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx, _) = top_match_app("search_top_matches_keys", &mut cx);
    vcx.simulate_resize(gpui::size(px(1100.0), px(800.0)));
    set_query(&view, vcx, "lisbon june");
    settle(vcx);
    let (place, album, month) = (
        bounds(vcx, "top-match-place-0"),
        bounds(vcx, "top-match-album-0"),
        bounds(vcx, "top-match-month-0"),
    );
    assert!(
        place.top() == album.top() && month.top() > place.bottom(),
        "two cards per row below 1100 px of content"
    );

    // Keyboard: with no suggestions showing (Esc hides them), ↓ from the
    // bar walks the cards and ↵ opens one.
    focus_app(vcx, &view);
    click(vcx, "search-typed-text");
    if view.update(vcx, |app, _| !app.search_suggestion_items().is_empty()) {
        keystrokes(vcx, "escape");
    }
    view.update(vcx, |app, _| assert!(app.search_focused));
    keystrokes(vcx, "down");
    assert!(
        vcx.debug_bounds("top-match-place-0-highlight").is_some(),
        "↓ from the bar highlights the first card"
    );
    keystrokes(vcx, "down down");
    assert!(vcx.debug_bounds("top-match-month-0-highlight").is_some());
    keystrokes(vcx, "enter");
    assert_eq!(
        query(&view, vcx),
        "lisbon month:jun year:2025",
        "↵ opens the highlighted card"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn top_matches_stay_up_while_typing_and_stale_cards_do_nothing() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx, _) = top_match_app("search_top_matches_typing", &mut cx);
    set_query(&view, vcx, "lisbon june");
    settle(vcx);
    assert!(vcx.debug_bounds("top-match-place-0").is_some());

    // One more keystroke: the new fetch waits for the debounce, and the
    // last cards stay meanwhile instead of the row vanishing.
    set_query(&view, vcx, "lisbon jun");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("top-matches").is_some(),
        "the row stays up while the newer fetch runs, so the grid does not jump"
    );
    settle(vcx);
    assert!(vcx.debug_bounds("top-match-month-0").is_some());

    // A card whose words left the query does nothing on click.
    set_query(&view, vcx, "porto");
    render_ui(vcx);
    click(vcx, "top-match-month-0");
    assert_eq!(
        query(&view, vcx),
        "porto",
        "a stale card does not put its tokens into a query that no longer names it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn arrow_to_a_card_deselects_the_chip() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx, _) = top_match_app("search_top_matches_chip", &mut cx);
    set_query(&view, vcx, "is:raw lisbon");
    settle(vcx);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| {
        app.search_focused = true;
        app.search_suggestions.dismissed = true;
        app.query_bar.chip = Some(0);
        cx.notify();
    });
    render_ui(vcx);
    keystrokes(vcx, "down");
    assert!(vcx.debug_bounds("top-match-place-0-highlight").is_some());
    view.update(vcx, |app, _| {
        assert_eq!(
            app.query_bar.chip, None,
            "one thing selected at a time: highlighting a card drops the chip"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

// The Photos section head.

#[core::prelude::v1::test]
fn results_head_shows_count_and_opens_sort() {
    let (dir, engine) = seed_library("search_results_head", 3);
    let video = engine.get_item_slice(0..1)[0].id.0 as i64;
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).expect("catalog opens");
    conn.execute(
        "UPDATE media_items SET media_type = 'video' WHERE id = ?1",
        [video],
    )
    .unwrap();
    drop(conn);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    set_query(&view, vcx, "photo");
    render_ui(vcx);

    for selector in [
        "search-results-title",
        "search-results-count",
        "search-results-sort",
    ] {
        bounds(vcx, selector);
    }
    assert!(
        vcx.debug_bounds("grid-month-header").is_none(),
        "the head replaces the month band while the search header shows"
    );
    let header = bounds(vcx, "search-header");
    let head = bounds(vcx, "search-results-head");
    assert!(
        head.top() >= header.bottom(),
        "the head sits under the header"
    );
    view.update(vcx, |app, _| {
        let (title, count) = app.results_head();
        assert_eq!(title, "Photos & Videos", "a mix of photos and a video");
        assert_eq!(app.displayed_count(), 3);
        assert_eq!(
            count,
            crate::i18n::items(app.displayed_count()),
            "the count is the grid's displayed count"
        );
        assert_eq!(
            app.results_sort_label(),
            "Date taken ▾",
            "the real sort, not Best match"
        );
    });

    // The sort label opens the existing sort menu under itself; Esc closes it.
    focus_app(vcx, &view);
    click(vcx, "search-results-sort");
    let sort = bounds(vcx, "search-results-sort");
    let menu = bounds(vcx, "sort-menu");
    assert!(
        vcx.debug_bounds("sort-field-date").is_some(),
        "the head's sort label opens the sort menu"
    );
    assert!(
        menu.top() >= sort.bottom() && (f32::from(menu.right() - sort.right())).abs() <= 2.0,
        "the menu hangs from the sort label: {menu:?} under {sort:?}"
    );
    assert!(
        vcx.debug_bounds("search-header").is_some(),
        "the Search screen stays while its own sort menu is open"
    );
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("sort-menu").is_none(),
        "Esc closes the menu"
    );

    set_query(&view, vcx, "photo is:photo");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.results_head().0, "Photos", "photos only");
        assert_eq!(app.displayed_count(), 2);
    });
    set_query(&view, vcx, "photo is:video");
    render_ui(vcx);
    view.update(vcx, |app, _| assert_eq!(app.results_head().0, "Videos"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A library of `count` items, the first `videos` of them videos.
fn kinds_app<'a>(
    name: &str,
    count: usize,
    videos: usize,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    Entity<crate::LuminaApp>,
    &'a mut VisualTestContext,
) {
    let (dir, engine) = seed_library(name, count);
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).expect("catalog opens");
    for item in engine.get_item_slice(0..videos) {
        conn.execute(
            "UPDATE media_items SET media_type = 'video' WHERE id = ?1",
            [item.id.0 as i64],
        )
        .unwrap();
    }
    drop(conn);
    let (view, vcx) = app_with_library(cx, engine);
    (dir, view, vcx)
}

#[core::prelude::v1::test]
fn results_head_counts_kinds_through_an_open_quote_and_all_videos() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = kinds_app("search_results_quote", 3, 1, &mut cx);
    set_query(&view, vcx, "\"photo");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.displayed_count(), 3, "the open quote still matches all");
        assert_eq!(
            app.results_head().0,
            "Photos & Videos",
            "is:video is counted outside the unclosed quote"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);

    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = kinds_app("search_results_videos", 2, 2, &mut cx);
    set_query(&view, vcx, "photo");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.results_head().0,
            "Videos",
            "every counted result a video, with no is: in the query"
        )
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn results_head_gives_way_to_the_page_header_and_the_month_band() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = kinds_app("search_results_fallback", 2, 0, &mut cx);

    // No results: no head to hang from, so the sort menu brings back the
    // page header and hangs from its Sort button.
    set_query(&view, vcx, "zzzz");
    view.update(vcx, |app, cx| app.toggle_sort_menu(cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("search-header").is_none(),
        "with 0 results the sort menu falls back to the page header"
    );
    let (menu, button) = (bounds(vcx, "sort-menu"), bounds(vcx, "sort-menu-button"));
    assert!(
        menu.top() >= button.bottom(),
        "the menu hangs from the page header's Sort button"
    );
    view.update(vcx, |app, cx| app.close_sort_menu(cx));

    // A filter-only view shows no head, so it skips the video count.
    set_query(&view, vcx, "has:location");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.view_loading(), "the fetch landed");
        assert_eq!(
            app.displayed_videos(),
            None,
            "no free text, no extra video count"
        )
    });

    // Clearing the search brings the month band back in place of the head.
    set_query(&view, vcx, "photo");
    render_ui(vcx);
    bounds(vcx, "search-results-head");
    set_query(&view, vcx, "");
    render_ui(vcx);
    render_ui(vcx);
    assert!(vcx.debug_bounds("search-results-head").is_none());
    assert!(
        vcx.debug_bounds("grid-month-header").is_some(),
        "the month band returns once the search clears"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
