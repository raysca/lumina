//! Typed text reaches fields through their input (app/text_input.rs), so
//! Shift, dead keys, IME composition, paste and mid-text edits all land in
//! the saved value.

use gpui::{ClipboardItem, EntityInputHandler};

use crate::app::text_input::{FieldEditor, TextTarget};

use crate::app::LuminaApp;
use crate::tests::ui_automation::{app_with_library, keystrokes, render_ui, seed_library};
use crate::views::floating_inspector::FloatingInspectorView;

/// Opens the floating inspector the way app/windows.rs does: its Info tab
/// draws the title, caption and date fields.
fn info_window<'a>(
    cx: &'a mut gpui::TestAppContext,
    view: &gpui::Entity<LuminaApp>,
) -> &'a mut gpui::VisualTestContext {
    let tx = view.read_with(cx, |app, _| app.ui_tx.clone());
    let (_inspector, icx) = cx
        .add_window_view(|window, cx| FloatingInspectorView::new(view.downgrade(), tx, window, cx));
    let handle = icx.update(|window, _| window.window_handle());
    view.update(icx, |app, _| app.floating_inspector_window = Some(handle));
    render_ui(icx);
    icx
}

#[core::prelude::v1::test]
fn a_title_keeps_capitals_and_punctuation() {
    let (dir, engine) = seed_library("text_caps", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_title_input(cx));
    let icx = info_window(&mut cx, &view);
    icx.simulate_input("Lisbon, Portugal!");
    keystrokes(icx, "enter");

    let id = view.update(icx, |app, _| app.selected_item.clone().unwrap().id);
    assert_eq!(
        engine.get_item_by_id(id).unwrap().title.as_deref(),
        Some("Lisbon, Portugal!"),
        "Shift must reach the saved title (capitals and punctuation)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_composed_accent_lands_once_in_the_title() {
    let (dir, engine) = seed_library("text_ime", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_title_input(cx));
    let icx = info_window(&mut cx, &view);
    icx.simulate_input("caf");
    // ⌥E then E on a US layout: the IME marks "´", then replaces it with "é".
    let input = field_input(&view, icx, TextTarget::Title);
    icx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "´", None, window, cx);
            assert_eq!(input.value(), "caf´", "marked text shows while composing");
            input.replace_text_in_range(None, "é", window, cx);
        })
    });
    view.update(icx, |app, _| {
        assert_eq!(
            app.caption.title_input_buffer, "café",
            "the composed character replaces the marked one"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// The one-line input drawn for `target`, which must be taking typing.
fn field_input(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    target: TextTarget,
) -> gpui::Entity<gpui::base::input::InputState> {
    match view.update(vcx, |app, _| app.field_input_for(target)) {
        Some(FieldEditor::Line(input)) => input,
        _ => panic!("{target:?} takes typing through the one-line field input"),
    }
}

/// Where the input of `target` is drawn.
fn field_bounds(
    view: &gpui::Entity<LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    target: TextTarget,
) -> gpui::Bounds<gpui::Pixels> {
    match view.update(vcx, |app, _| app.field_input_for(target)) {
        Some(FieldEditor::Line(input)) => input.read_with(vcx, |input, _| input.input_bounds()),
        Some(FieldEditor::Lines(input)) => input.read_with(vcx, |input, _| input.input_bounds()),
        None => panic!("{target:?} takes typing through a field input"),
    }
}

#[core::prelude::v1::test]
fn cmd_v_pastes_into_the_search_field_as_one_line() {
    let (dir, engine) = seed_library("text_paste", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.search_focused = true;
        cx.notify();
    });
    render_ui(vcx);
    vcx.write_to_clipboard(ClipboardItem::new_string("Rua\nAugusta".to_string()));
    keystrokes(vcx, "secondary-v");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "Rua Augusta",
            "⌘V pastes the clipboard; newlines become spaces in a one-line field"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// The toolbar search, focused and drawn so its input takes keys.
fn focused_search<'a>(
    name: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    gpui::Entity<LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(name, 2);
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.search_focused = true;
        cx.notify();
    });
    render_ui(vcx);
    (dir, view, vcx)
}

#[core::prelude::v1::test]
fn search_inserts_typed_text_at_the_caret() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = focused_search("text_mid_insert", &mut cx);
    vcx.simulate_input("Lisbn");
    keystrokes(vcx, "left");
    vcx.simulate_input("o");
    keystrokes(vcx, "end");
    vcx.simulate_input("!");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "Lisbon!",
            "← moves the caret and typing lands there, not at the end"
        );
    });
    let field = vcx
        .debug_bounds("search-input")
        .expect("search field drawn");
    let text = field_bounds(&view, vcx, TextTarget::Search);
    assert!(
        text.size.height > gpui::px(0.0)
            && field.contains(&text.origin)
            && field.contains(&text.bottom_right()),
        "the input draws inside the search field: {text:?} in {field:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn search_typing_replaces_the_selection_and_cut_copies_it() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = focused_search("text_select", &mut cx);
    vcx.simulate_input("Lisbon");
    keystrokes(vcx, "shift-left shift-left shift-left");
    vcx.simulate_input("ka");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "Liska",
            "typing replaces the selected text"
        );
    });
    keystrokes(vcx, "secondary-a secondary-x");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "", "⌘A ⌘X cuts the whole query");
    });
    assert_eq!(
        vcx.read_from_clipboard()
            .and_then(|item| item.text())
            .as_deref(),
        Some("Liska"),
        "the cut text is on the clipboard"
    );
    keystrokes(vcx, "secondary-v");
    view.update(vcx, |app, _| {
        assert_eq!(app.search_query, "Liska", "⌘V puts it back");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn search_paste_replaces_the_selection_and_stops_at_the_cap() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = focused_search("text_paste_cap", &mut cx);
    vcx.simulate_input("ab");
    let cap = lumina_core::text::MAX_TEXT_CHARS;
    vcx.write_to_clipboard(ClipboardItem::new_string("x\ty".repeat(cap)));
    keystrokes(vcx, "shift-left secondary-v");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.starts_with("ax y"),
            "the paste replaces the selected b and tabs become spaces: {:?}",
            &app.search_query[..8]
        );
        assert_eq!(
            app.search_query.chars().count(),
            cap,
            "a paste is cut to the room the field has left"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_dialog_over_search_takes_its_keys() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = focused_search("text_modal_focus", &mut cx);
    vcx.simulate_input("Rio");
    view.update(vcx, |app, cx| {
        app.preferences_open = true;
        cx.notify();
    });
    render_ui(vcx);
    vcx.simulate_input("x");
    keystrokes(vcx, "backspace backspace");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "Rio",
            "keys under an open dialog must not edit the search behind it"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn the_main_window_takes_typed_text_without_a_test_focus() {
    let (dir, engine) = seed_library("text_focus", 2);
    let mut cx = crate::tests::TestApp::single();
    // Built the way main.rs builds it, with no focus_app() helper.
    let (view, vcx) = cx.add_window_view(LuminaApp::new_in_window);
    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine));
        app.search_focused = true;
        cx.notify();
    });
    render_ui(vcx);
    vcx.simulate_input("Rio");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "Rio",
            "the window focuses its root so the input handler registers"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_composition_left_in_one_field_never_edits_the_next() {
    let (dir, engine) = seed_library("text_stale_mark", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_date_input(cx));
    let icx = info_window(&mut cx, &view);
    icx.simulate_input("caf");
    let date = field_input(&view, icx, TextTarget::Date);
    icx.update(|window, cx| {
        date.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "´", None, window, cx);
        })
    });
    // Leave the date mid-composition and edit the title instead.
    keystrokes(icx, "escape");
    view.update(icx, |app, cx| {
        app.start_title_input(cx);
        app.caption.title_input_buffer = "Lisbon".to_string();
    });
    render_ui(icx);
    let title = field_input(&view, icx, TextTarget::Title);
    icx.update(|window, cx| {
        title.update(cx, |input, cx| {
            assert_eq!(
                input.marked_text_range(window, cx),
                None,
                "the date's composition is not reported for the title"
            );
            input.replace_text_in_range(None, "!", window, cx);
        })
    });
    view.update(icx, |app, _| {
        assert_eq!(
            app.caption.title_input_buffer, "Lisbon!",
            "a stale composition range must not replace title text"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn the_floating_inspector_takes_typed_text_without_a_click() {
    let (dir, engine) = seed_library("text_floating", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    let tx = view.update(vcx, |app, cx| {
        app.start_title_input(cx);
        app.ui_tx.clone()
    });
    // Built the way app/windows.rs builds it; no click, no test focus.
    let (_inspector, icx) = cx
        .add_window_view(|window, cx| FloatingInspectorView::new(view.downgrade(), tx, window, cx));
    let handle = icx.update(|window, _| window.window_handle());
    view.update(icx, |app, _| app.floating_inspector_window = Some(handle));
    render_ui(icx);
    icx.simulate_input("Tram");
    view.update(icx, |app, _| {
        assert_eq!(
            app.caption.title_input_buffer, "Tram",
            "keys reaching the floating window must land in the active field"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_composition_left_in_a_field_never_edits_it_after_reopening() {
    let (dir, engine) = seed_library("text_stale_same", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_title_input(cx));
    let icx = info_window(&mut cx, &view);
    icx.simulate_input("caf");
    let input = field_input(&view, icx, TextTarget::Title);
    icx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "´", None, window, cx);
        })
    });
    // Close the title, then reopen it holding different text.
    keystrokes(icx, "escape");
    view.update(icx, |app, cx| {
        app.start_title_input(cx);
        app.caption.title_input_buffer = "Hello".to_string();
        cx.notify();
    });
    render_ui(icx);
    icx.update(|window, cx| {
        input.update(cx, |input, cx| {
            assert_eq!(
                input.marked_text_range(window, cx),
                None,
                "the old composition is gone once its text is"
            );
            input.replace_text_in_range(None, "x", window, cx);
        })
    });
    view.update(icx, |app, _| {
        assert_eq!(
            app.caption.title_input_buffer, "Hellox",
            "a stale composition range must not replace title text"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_tag_field_drops_disguising_characters_and_stops_at_the_name_cap() {
    let (dir, engine) = seed_library("text_clean", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| {
        // The docked inspector draws the tag field.
        app.inspector_open = true;
        app.start_tag_input(cx);
    });
    render_ui(vcx);
    vcx.write_to_clipboard(ClipboardItem::new_string("gpj.\u{202E}exe".to_string()));
    keystrokes(vcx, "secondary-v");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_input_buffer, "gpj. exe",
            "a pasted bidi override becomes a space"
        );
        app.tag_input_buffer.clear();
    });
    vcx.simulate_input(&"x".repeat(lumina_core::text::MAX_NAME_CHARS + 20));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_input_buffer.chars().count(),
            lumina_core::text::MAX_NAME_CHARS,
            "typing stops at the longest tag the catalog keeps"
        );
    });
    keystrokes(vcx, "enter");

    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    assert_eq!(
        engine.get_tags_for_item(id),
        vec!["x".repeat(lumina_core::text::MAX_NAME_CHARS)],
        "the saved tag matches what the field showed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn confirming_an_untouched_legacy_tag_rename_keeps_it() {
    let (dir, engine) = seed_library("text_legacy_tag", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    // A tag saved before input was cleaned: longer than the name cap.
    let legacy = "y".repeat(lumina_core::text::MAX_NAME_CHARS + 50);
    // Written the way the catalog writes tags, keeping the FTS row in step.
    let fts_row = "rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags";
    let fts_values =
        "m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, \
        m.model, m.lens, CAST(m.year AS TEXT), \
        COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), '') \
        FROM media_items m WHERE m.id = 1";
    let conn = rusqlite::Connection::open(dir.join("catalog.db")).unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO media_search_fts(media_search_fts, {fts_row}) SELECT 'delete', {fts_values};"
    ))
    .unwrap();
    conn.execute(
        "INSERT INTO media_tags (media_id, tag) VALUES (1, ?1)",
        [&legacy],
    )
    .unwrap();
    conn.execute_batch(&format!(
        "INSERT INTO media_search_fts({fts_row}) SELECT {fts_values};"
    ))
    .unwrap();
    drop(conn);

    view.update(vcx, |app, cx| {
        app.start_tag_rename(legacy.clone(), cx);
        app.commit_tag_rename(cx);
        app.start_modal_tag_rename(legacy.clone(), cx);
        app.commit_modal_tag_rename(cx);
    });

    assert_eq!(
        engine.query_tag_counts(),
        vec![(legacy, 1)],
        "confirming a rename without editing must not shorten an existing tag"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Types "ac", steps back one character and types "b" into `target`, which
/// `setup` opens; the field must end in "abc" and draw its input with room.
fn edits_mid_text(
    name: &str,
    target: TextTarget,
    in_info_window: bool,
    setup: impl FnOnce(&mut LuminaApp, &mut gpui::Context<LuminaApp>),
) {
    fn type_mid(
        view: &gpui::Entity<LuminaApp>,
        wcx: &mut gpui::VisualTestContext,
        target: TextTarget,
    ) {
        wcx.simulate_input("ac");
        keystrokes(wcx, "left");
        wcx.simulate_input("b");
        render_ui(wcx);
        let text = view.update(wcx, |app, _| app.text_buffer(target).cloned());
        assert!(
            text.as_deref().is_some_and(|text| text.ends_with("abc")),
            "{target:?}: typing lands at the caret, got {text:?}"
        );
        let bounds = field_bounds(view, wcx, target);
        assert!(
            bounds.size.width > gpui::px(40.0) && bounds.size.height > gpui::px(0.0),
            "{target:?}: the input is drawn with room to type, got {bounds:?}"
        );
    }
    let (dir, engine) = seed_library(name, 2);
    let first = engine.get_item_slice(0..1)[0].id;
    engine
        .add_tags_batch(&[first], &["beach".to_string()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.refresh_tag_counts();
        setup(app, cx);
        cx.notify();
    });
    if in_info_window {
        let icx = info_window(&mut cx, &view);
        type_mid(&view, icx, target);
    } else {
        render_ui(vcx);
        type_mid(&view, vcx, target);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn every_field_edits_at_its_caret() {
    use TextTarget::*;
    edits_mid_text("mid_palette", Palette, false, |app, cx| {
        app.open_command_palette(cx)
    });
    edits_mid_text("mid_search", Search, false, |app, _| {
        app.search_focused = true
    });
    edits_mid_text("mid_tag", Tag, false, |app, cx| app.start_tag_input(cx));
    edits_mid_text("mid_title", Title, true, |app, cx| {
        app.start_title_input(cx)
    });
    edits_mid_text("mid_caption", Caption, true, |app, cx| {
        app.start_caption_input(cx)
    });
    edits_mid_text("mid_date", Date, true, |app, cx| app.start_date_input(cx));
    edits_mid_text("mid_location", Location, true, |app, cx| {
        app.inspector_tab = crate::views::inspector::InspectorTab::Location;
        app.start_location_input(cx);
    });
    edits_mid_text("mid_tag_rename", TagRename, false, |app, cx| {
        app.start_tag_rename("beach".to_string(), cx)
    });
    edits_mid_text("mid_tm_search", TagManagerSearch, false, |app, cx| {
        app.open_tag_manager(cx)
    });
    edits_mid_text("mid_tm_rename", TagManagerRename, false, |app, cx| {
        app.open_tag_manager(cx);
        // Renaming starts from the tag the detail column shows.
        app.tag_manager.focused = Some("beach".to_string());
        app.start_modal_tag_rename("beach".to_string(), cx);
    });
    edits_mid_text("mid_album", AlbumDraft, false, |app, cx| {
        app.open_album_builder(cx)
    });
    edits_mid_text("mid_url", NetworkImportUrl, false, |app, cx| {
        app.open_network_import_dialog(cx)
    });
}

#[core::prelude::v1::test]
fn a_caption_keeps_its_line_breaks_and_shift_enter_adds_one() {
    let (dir, engine) = seed_library("text_caption_lines", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        // An imported caption with paragraphs (`clean_caption` keeps them).
        app.caption.input_buffer = "Day one\nDay two".to_string();
    });
    let icx = info_window(&mut cx, &view);
    icx.simulate_input("!");
    keystrokes(icx, "shift-enter");
    icx.simulate_input("x");
    view.update(icx, |app, _| {
        assert_eq!(
            app.caption.input_buffer, "Day one\nDay two!\nx",
            "editing a caption keeps its line breaks and ⇧Enter adds one"
        );
    });
    keystrokes(icx, "enter");
    let id = view.update(icx, |app, _| app.selected_item.clone().unwrap().id);
    assert_eq!(
        engine.get_captions_map().get(&id).map(String::as_str),
        Some("Day one\nDay two!\nx"),
        "Enter saves the caption with its lines"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_stored_value_past_the_cap_still_loads_into_its_field() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = focused_search("text_legacy_load", &mut cx);
    vcx.simulate_input("beach");
    // Text the app sets can be longer than typing allows (a legacy value).
    let long = "y".repeat(lumina_core::text::MAX_TEXT_CHARS + 5);
    view.update(vcx, |app, cx| {
        app.search_query = long.clone();
        cx.notify();
    });
    render_ui(vcx);
    let input = field_input(&view, vcx, TextTarget::Search);
    let shown = input.read_with(vcx, |input, _| input.value().to_string());
    assert!(
        shown == long,
        "the field shows the stored text, not the last field's: {:?}…",
        &shown[..shown.len().min(8)]
    );
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query == long[1..],
            "editing the long value changes that value"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn the_link_dialog_takes_keys_while_it_checks() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = focused_search("text_checking_focus", &mut cx);
    vcx.simulate_input("Rio");
    view.update(vcx, |app, cx| {
        app.network_import.stage = Some(crate::app::network_import::NetworkImportStage::Checking {
            url: "https://example.com/v".to_string(),
        });
        cx.notify();
    });
    render_ui(vcx);
    keystrokes(vcx, "backspace secondary-x secondary-z");
    vcx.simulate_input("x");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "Rio",
            "keys under the link dialog must not edit the search behind it"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tagging_from_the_floating_inspector_shows_the_tag_field() {
    let (dir, engine) = seed_library("text_float_tag", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    let icx = info_window(&mut cx, &view);
    view.update(icx, |app, cx| app.begin_tagging_selection(cx));
    render_ui(icx);
    icx.simulate_input("sun");
    view.update(icx, |app, _| {
        assert_eq!(
            app.tag_input_buffer, "sun",
            "T with the inspector floating must open a tag field that takes typing"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn shift_arrows_still_move_the_palette_selection() {
    let (dir, engine) = seed_library("text_shift_arrows", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "secondary-k");
    render_ui(vcx);
    keystrokes(vcx, "shift-down shift-down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.command_selected, 2,
            "Shift-↓ in the one-line palette field must move the row"
        )
    });
    keystrokes(vcx, "shift-up");
    view.update(vcx, |app, _| {
        assert_eq!(app.command_selected, 1, "Shift-↑ must move it back")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_full_caption_refuses_typing_without_losing_its_end_or_caret() {
    let (dir, engine) = seed_library("text_caption_full", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    let full = format!("{}Z", "a".repeat(lumina_core::text::MAX_TEXT_CHARS - 1));
    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        app.caption.input_buffer = full.clone();
    });
    let icx = info_window(&mut cx, &view);
    keystrokes(icx, "secondary-up");
    icx.simulate_input("x");
    view.update(icx, |app, _| {
        assert!(
            app.caption.input_buffer == full,
            "a keystroke past the cap is refused, not paid for with the last character"
        );
    });
    // The caret stayed at the start: make room there and type again.
    keystrokes(icx, "delete");
    icx.simulate_input("x");
    view.update(icx, |app, _| {
        assert!(
            app.caption.input_buffer.starts_with("xa") && app.caption.input_buffer.ends_with('Z'),
            "the refused keystroke must leave the caret where it was"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_caption_stored_past_the_cap_keeps_its_end_when_edited() {
    let (dir, engine) = seed_library("text_caption_long", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    let long = format!("{}Z!", "a".repeat(lumina_core::text::MAX_TEXT_CHARS + 3));
    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        app.caption.input_buffer = long.clone();
    });
    let icx = info_window(&mut cx, &view);
    keystrokes(icx, "backspace");
    view.update(icx, |app, _| {
        assert!(
            app.caption.input_buffer == long[..long.len() - 1],
            "one ⌫ removes one character, not the text past the cap"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tagging_from_the_viewer_shows_the_tag_field() {
    let (dir, engine) = seed_library("text_viewer_tag", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| {
        let item = app.selected_item.clone().unwrap();
        app.inspector_open = true;
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, cx| app.begin_tagging_selection(cx));
    render_ui(vcx);
    vcx.simulate_input("sun");
    keystrokes(vcx, "enter");
    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    assert_eq!(
        engine.get_tags_for_item(id),
        vec!["sun".to_string()],
        "Tags… in the viewer types into its inspector drawer and saves"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn leaving_a_fields_tab_ends_its_edit_unsaved() {
    let (dir, engine) = seed_library("text_tab_away", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_caption_input(cx));
    let icx = info_window(&mut cx, &view);
    icx.simulate_input("Hi");
    view.update(icx, |app, cx| {
        app.set_inspector_tab(crate::views::inspector::InspectorTab::Camera, cx)
    });
    render_ui(icx);
    icx.simulate_input("lost?");
    let id = view.update(icx, |app, _| {
        assert!(
            !app.caption.input_active,
            "a field that is no longer drawn must stop taking typing"
        );
        app.selected_item.clone().unwrap().id
    });
    assert_eq!(
        engine.get_captions_map().get(&id),
        None,
        "leaving the tab is Esc: nothing typed is saved unconfirmed"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
