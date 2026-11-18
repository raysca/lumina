//! Modal sheets: backdrop dismissal, the shortcut sheet and Preferences.

use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn ui_automation_modals_dismiss_on_backdrop() {
    let (_temp_dir, engine) = seed_library("modals", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    // The Tags page is a page, not a sheet: no backdrop to dismiss.
    click(vcx, "tags-toolbar-btn");
    view.update(vcx, |app, _| assert!(app.tags_page_shown()));
    assert!(vcx.debug_bounds("tags-page").is_some());
    assert!(vcx.debug_bounds("tag-manager-backdrop").is_none());
    view.update(vcx, |app, cx| app.navigate_back(cx));
    view.update(vcx, |app, _| assert!(!app.tags_page_shown()));

    click(vcx, "new-smart-album");
    view.update(vcx, |app, _| assert!(app.album_draft.modal_open));
    assert!(vcx.debug_bounds("album-builder-card").is_some());
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.album_draft.modal_open));
}

#[core::prelude::v1::test]
fn ui_automation_shortcut_cheat_sheet() {
    let (_temp_dir, engine) = seed_library("shortcuts", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    keystrokes(vcx, "secondary-/");
    view.update(vcx, |app, _| assert!(app.shortcuts_overlay_open));
    assert!(vcx.debug_bounds("shortcuts-card").is_some());

    // Keystrokes are captured by the sheet while open.
    keystrokes(vcx, "i");
    view.update(vcx, |app, _| assert!(!app.inspector_open));

    click(vcx, "shortcuts-close");
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));

    keystrokes(vcx, "secondary-/");
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| assert!(!app.shortcuts_overlay_open));
}

/// The sheet is taller than its card, so its lower rows must be reachable
/// without a mouse (AGENTS.md section 11).
#[core::prelude::v1::test]
fn ui_automation_shortcut_sheet_scrolls_from_the_keyboard() {
    let (_temp_dir, engine) = seed_library("shortcuts_scroll", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    keystrokes(vcx, "secondary-/");
    render_ui(vcx);
    let offset = |vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            let max = app.shortcuts_scroll.max_offset().y;
            assert!(max > px(0.0), "the sheet overflows its card, so it scrolls");
            (app.shortcuts_scroll.offset().y, max)
        })
    };
    assert_eq!(offset(vcx).0, px(0.0), "the sheet opens at the top");

    keystrokes(vcx, "down");
    let (y, _) = offset(vcx);
    assert!(y < px(0.0), "Down scrolls the sheet");
    keystrokes(vcx, "end");
    let (y, max) = offset(vcx);
    assert_eq!(y, -max, "End reaches the last row");
    keystrokes(vcx, "pageup");
    assert!(offset(vcx).0 > -max, "PageUp scrolls back up");
    keystrokes(vcx, "home");
    assert_eq!(offset(vcx).0, px(0.0), "Home returns to the top");
    view.update(vcx, |app, _| {
        assert!(
            app.shortcuts_overlay_open,
            "scroll keys keep the sheet open"
        );
        assert_eq!(app.selected_indices.len(), 0, "and never move the grid");
    });

    keystrokes(vcx, "end");
    keystrokes(vcx, "escape");
    keystrokes(vcx, "secondary-/");
    assert_eq!(offset(vcx).0, px(0.0), "reopening starts at the top again");
}

#[core::prelude::v1::test]
fn ui_automation_preferences_modal() {
    let (_temp_dir, engine) = seed_library("preferences", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "Cmd+, should open preferences");
        assert!(app.preferences_cache.is_some());
    });

    click(vcx, "settings-category-storage");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::Storage
        );
    });
    click(vcx, "settings-category-about");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::About
        );
    });
    click(vcx, "preferences-close");
    view.update(vcx, |app, _| assert!(!app.preferences_open));

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| assert!(app.preferences_open));
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Escape should close preferences");
    });
}
