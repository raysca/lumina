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
