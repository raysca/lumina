//! Design 03 Task 2: the search header with its token query bar, and the
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
