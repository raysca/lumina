//! Typed text reaches fields through the platform input handler, so Shift,
//! dead keys, IME composition and paste all land in the saved value.

use gpui::{ClipboardItem, EntityInputHandler};

use crate::app::LuminaApp;
use crate::tests::ui_automation::{app_with_library, keystrokes, render_ui, seed_library};
use crate::views::floating_inspector::FloatingInspectorView;

#[core::prelude::v1::test]
fn a_title_keeps_capitals_and_punctuation() {
    let (dir, engine) = seed_library("text_caps", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_title_input(cx));
    render_ui(vcx);
    vcx.simulate_input("Lisbon, Portugal!");
    keystrokes(vcx, "enter");

    let id = view.update(vcx, |app, _| app.selected_item.clone().unwrap().id);
    assert_eq!(
        engine.get_item_by_id(id).unwrap().title.as_deref(),
        Some("Lisbon, Portugal!"),
        "Shift must reach the saved title (capitals and punctuation)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_composed_accent_lands_once_in_the_caption() {
    let (dir, engine) = seed_library("text_ime", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_caption_input(cx));
    render_ui(vcx);
    vcx.simulate_input("caf");
    // ⌥E then E on a US layout: the IME marks "´", then replaces it with "é".
    vcx.update(|window, cx| {
        view.update(cx, |app, cx| {
            app.replace_and_mark_text_in_range(None, "´", None, window, cx);
            assert_eq!(
                app.caption.input_buffer, "caf´",
                "marked text shows while composing"
            );
            app.replace_text_in_range(None, "é", window, cx);
        })
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.caption.input_buffer, "café",
            "the composed character replaces the marked one"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_v_pastes_into_the_search_field_as_one_line() {
    let (dir, engine) = seed_library("text_paste", 2);
    let mut cx = gpui::TestAppContext::single();
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

#[core::prelude::v1::test]
fn the_main_window_takes_typed_text_without_a_test_focus() {
    let (dir, engine) = seed_library("text_focus", 2);
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_caption_input(cx));
    render_ui(vcx);
    vcx.simulate_input("caf");
    vcx.update(|window, cx| {
        view.update(cx, |app, cx| {
            app.replace_and_mark_text_in_range(None, "´", None, window, cx);
        })
    });
    // Leave the caption mid-composition and edit the title instead.
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.start_title_input(cx);
        app.caption.title_input_buffer = "Lisbon".to_string();
    });
    render_ui(vcx);
    vcx.update(|window, cx| {
        view.update(cx, |app, cx| {
            assert_eq!(
                app.marked_text_range(window, cx),
                None,
                "the caption's composition is not reported for the title"
            );
            app.replace_text_in_range(None, "!", window, cx);
            assert_eq!(
                app.caption.title_input_buffer, "Lisbon!",
                "a stale composition range must not replace title text"
            );
        })
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn the_floating_inspector_takes_typed_text_without_a_click() {
    let (dir, engine) = seed_library("text_floating", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "home");
    let tx = view.update(vcx, |app, cx| {
        app.start_title_input(cx);
        app.ui_tx.clone()
    });
    // Built the way app/windows.rs builds it; no click, no test focus.
    let (_inspector, icx) = cx
        .add_window_view(|window, cx| FloatingInspectorView::new(view.downgrade(), tx, window, cx));
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
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_caption_input(cx));
    render_ui(vcx);
    vcx.simulate_input("caf");
    vcx.update(|window, cx| {
        view.update(cx, |app, cx| {
            app.replace_and_mark_text_in_range(None, "´", None, window, cx);
        })
    });
    // Close the caption, then reopen it holding different text.
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        app.caption.input_buffer = "Hello".to_string();
        cx.notify();
    });
    render_ui(vcx);
    vcx.update(|window, cx| {
        view.update(cx, |app, cx| {
            assert_eq!(
                app.marked_text_range(window, cx),
                None,
                "the old composition is gone once its text is"
            );
            app.replace_text_in_range(None, "x", window, cx);
            assert_eq!(
                app.caption.input_buffer, "Hellox",
                "a stale composition range must not replace caption text"
            );
        })
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_tag_field_drops_disguising_characters_and_stops_at_the_name_cap() {
    let (dir, engine) = seed_library("text_clean", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    keystrokes(vcx, "home");
    view.update(vcx, |app, cx| app.start_tag_input(cx));
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
    let mut cx = gpui::TestAppContext::single();
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

/// Lets one half-blink pass and draws the frame it produces.
fn blink_tick(vcx: &mut gpui::VisualTestContext) -> &'static str {
    vcx.executor().advance_clock(crate::app::caret::CARET_BLINK);
    vcx.run_until_parked();
    render_ui(vcx);
    crate::app::caret::caret()
}

#[core::prelude::v1::test]
fn the_caret_blinks_while_idle_and_stays_solid_while_typing() {
    use crate::app::caret::caret;
    let (dir, engine) = seed_library("text_caret", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    crate::tests::ui_automation::focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    render_ui(vcx);
    assert_eq!(caret(), "|", "a newly focused field shows its caret");
    assert_eq!(
        blink_tick(vcx),
        "|",
        "the caret stays up for its first beat"
    );
    assert_eq!(blink_tick(vcx), " ", "an idle field blinks its caret off");
    assert_eq!(blink_tick(vcx), "|", "and back on");
    assert_eq!(blink_tick(vcx), " ", "test premise: the caret is off");

    vcx.simulate_input("ra");
    assert_eq!(
        blink_tick(vcx),
        "|",
        "a field whose text just changed keeps its caret solid"
    );
    assert_eq!(blink_tick(vcx), " ", "blinking resumes once typing stops");

    keystrokes(vcx, "escape");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open, "Escape closes the palette")
    });
    assert_eq!(
        caret(),
        "|",
        "closing the field resets the caret so the next one starts visible"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
