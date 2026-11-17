//! Empty-state and toast checks:
//! the digits headline, rule chips beside both side panels, the scanning
//! Hero size, a long toast beside its action and the buttons' keyboard path.

use super::downloads::TempLibrary;
use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::ui_automation::{app_with_library, keystrokes, leak, render_ui, seed_library};
use crate::views::sidebar::AlbumSelection;
use crate::views::toast::{ToastAction, ToastKind};

/// Ten terms read as a word; more than ten switch to digits.
#[core::prelude::v1::test]
fn empty_search_headline_uses_digits_past_ten_terms() {
    let (dir, engine) = seed_library("states_gaps_digits", 2);
    let _dir = TempLibrary(dir);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    for (n, headline) in [
        (10, "Nothing matches all ten filters"),
        (11, "Nothing matches all 11 filters"),
        (14, "Nothing matches all 14 filters"),
    ] {
        let query: Vec<String> = (0..n).map(|i| format!("zzqx{i}")).collect();
        let query = query.join(" ");
        view.update(vcx, |app, cx| app.set_search_query(&query, cx));
        render_ui(vcx);
        assert!(
            vcx.debug_bounds(leak(format!("empty-state-headline-text:{headline}")))
                .is_some(),
            "{n} terms must read {headline:?}: words up to ten, digits above"
        );
        assert!(
            vcx.debug_bounds(leak(format!("empty-search-term-{}", n - 1)))
                .is_some()
                && vcx
                    .debug_bounds(leak(format!("empty-search-term-{n}")))
                    .is_none(),
            "{n} terms must draw exactly {n} Drop chips"
        );
    }
}

/// Review Focus: with the sidebar and the inspector both open,
/// many rule chips wrap inside the middle column and overflow nothing.
#[core::prelude::v1::test]
fn empty_smart_album_chips_wrap_between_sidebar_and_inspector() {
    let (dir, engine) = seed_library("states_gaps_chips", 1);
    let _dir = TempLibrary(dir);
    let rules: Vec<_> = (0..9)
        .map(|i| SmartAlbumRule {
            field: RuleField::Camera,
            operator: RuleOperator::Contains,
            value: RuleValue::Text(format!("Camera model {i}")),
        })
        .collect();
    let album = engine
        .create_smart_album("Many rules", "folder", MatchMode::Any, &rules)
        .unwrap();
    // 960×600 is the window's minimum (main.rs); 900 as the existing test.
    for width in [900.0, 960.0] {
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = app_with_library(&mut cx, engine.clone());
        vcx.simulate_resize(gpui::size(gpui::px(width), gpui::px(600.0)));
        view.update(vcx, |app, cx| {
            app.sidebar_collapsed = false;
            app.inspector_open = true;
            app.select_album(AlbumSelection::User(album.id), cx)
        });
        render_ui(vcx);

        let sidebar = vcx.debug_bounds("sidebar").expect("the sidebar is open");
        let inspector = vcx
            .debug_bounds("inspector-panel")
            .expect("the inspector is open");
        let column = vcx
            .debug_bounds("empty-state")
            .expect("the empty state renders");
        assert!(
            column.left() >= sidebar.right() && column.right() <= inspector.left(),
            "at {width} wide the empty state must shrink to the column between the panels, \
             sidebar {sidebar:?}, inspector {inspector:?}, got {column:?}"
        );
        let chips: Vec<_> = (0..rules.len())
            .map(|i| {
                vcx.debug_bounds(leak(format!("empty-rule-chip-{i}")))
                    .unwrap_or_else(|| panic!("chip {i} is drawn"))
            })
            .collect();
        let first_top = chips[0].top();
        assert!(
            chips.iter().any(|chip| chip.top() > first_top),
            "at {width} wide nine chips wrap onto more than one row"
        );
        let window = vcx.update(|window, _| window.viewport_size());
        for (i, chip) in chips.iter().enumerate() {
            assert!(
                chip.left() >= column.left()
                    && chip.right() <= column.right()
                    && chip.top() >= column.top()
                    && chip.bottom() <= column.bottom()
                    && chip.bottom() <= window.height,
                "at {width} wide chip {i} must stay inside the column {column:?} and the window, \
                 got {chip:?}"
            );
        }
        for button in ["empty-primary", "empty-secondary"] {
            let b = vcx
                .debug_bounds(button)
                .unwrap_or_else(|| panic!("{button} renders"));
            assert!(
                b.left() >= column.left() && b.right() <= column.right(),
                "at {width} wide {button} must stay inside the column {column:?}, got {b:?}"
            );
        }
    }
}

/// An empty library mid-scan also draws at the Hero size.
#[core::prelude::v1::test]
fn empty_library_scanning_uses_hero_size() {
    let (dir, engine) = seed_library("states_gaps_scan_hero", 0);
    let _dir = TempLibrary(dir);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = true;
        }
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state-headline-text:Scanning…")
            .is_some(),
        "precondition: the scanning copy is shown"
    );
    let glyph = vcx.debug_bounds("empty-state-glyph").expect("a glyph");
    assert_eq!(
        (f32::from(glyph.size.width), f32::from(glyph.size.height)),
        (150.0, 150.0),
        "an empty library mid-scan must use the 150 px Hero glyph"
    );
    let primary = vcx
        .debug_bounds("empty-primary")
        .expect("Choose Another Folder… stays");
    assert_eq!(
        f32::from(primary.size.height),
        40.0,
        "the Hero button is 40 high"
    );
}

/// A long title and subtitle truncate before Undo, which
/// keeps its full width inside the card.
#[core::prelude::v1::test]
fn long_toast_text_truncates_before_the_action() {
    let (dir, engine) = seed_library("states_gaps_toast", 1);
    let _dir = TempLibrary(dir);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));

    view.update(vcx, |app, cx| app.show_undo_toast("Rated 1 item", cx));
    render_ui(vcx);
    let natural = vcx
        .debug_bounds("toast-action")
        .expect("Undo renders")
        .size
        .width;

    let long = "Moved 1,234 photos and videos from the Summer Holiday in the Mountains \
                album to Recently Deleted, including every edited copy and sidecar";
    view.update(vcx, |app, cx| {
        app.show_toast_kind(
            ToastKind::Positive,
            long,
            Some(long),
            Some(ToastAction::Undo),
            cx,
        )
    });
    render_ui(vcx);
    let toast = vcx.debug_bounds("toast").expect("the toast renders");
    let action = vcx.debug_bounds("toast-action").expect("Undo renders");
    let dismiss = vcx.debug_bounds("toast-dismiss").expect("× renders");
    for line in ["toast-message", "toast-subtitle"] {
        let b = vcx
            .debug_bounds(line)
            .unwrap_or_else(|| panic!("{line} renders"));
        assert!(
            b.right() <= action.left(),
            "{line} must end before Undo, {b:?} vs {action:?}"
        );
        assert!(
            f32::from(b.size.height) <= 20.0,
            "{line} must stay on one line, got {b:?}"
        );
    }
    assert_eq!(
        action.size.width, natural,
        "Undo must keep its full width beside long text"
    );
    assert!(
        action.left() >= toast.left()
            && action.right() <= dismiss.left()
            && dismiss.right() <= toast.right(),
        "Undo and × must stay inside the card, toast {toast:?}, Undo {action:?}, × {dismiss:?}"
    );
}

/// Which empty-state button draws the focus ring, by its test marker.
fn empty_focus(vcx: &mut gpui::VisualTestContext) -> (bool, bool) {
    (
        vcx.debug_bounds("empty-primary-focused").is_some(),
        vcx.debug_bounds("empty-secondary-focused").is_some(),
    )
}

/// An empty tag view: Show all photos (primary) and Manage tags (secondary).
fn empty_tag_view<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    TempLibrary,
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 1);
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-has-this".into()), cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-secondary").is_some(),
        "precondition: the empty tag view shows two buttons"
    );
    (TempLibrary(dir), view, vcx)
}

/// AGENTS §11: Tab / ⇧Tab move a visible ring between the shown buttons,
/// Esc clears it and ↵ presses the focused one, and only that.
#[core::prelude::v1::test]
fn empty_state_buttons_take_tab_and_enter() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = empty_tag_view("states_gaps_tab", &mut cx);
    assert_eq!(empty_focus(vcx), (false, false), "no button starts focused");
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (true, false),
        "Tab focuses the first button"
    );
    keystrokes(vcx, "tab");
    assert_eq!(empty_focus(vcx), (false, true), "Tab moves to the second");
    keystrokes(vcx, "tab");
    assert_eq!(empty_focus(vcx), (true, false), "Tab wraps to the first");
    keystrokes(vcx, "shift-tab");
    assert_eq!(empty_focus(vcx), (false, true), "⇧Tab goes back, wrapping");
    keystrokes(vcx, "escape");
    assert_eq!(empty_focus(vcx), (false, false), "Esc clears the focus");

    keystrokes(vcx, "tab enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "↵ on the focused Show all photos shows All Media"
        );
        // The grid's ↵ with nothing selected selects the first item and
        // opens it (All Media holds one), so either would show here.
        assert!(
            !app.is_fullscreen_viewer && app.selected_item.is_none(),
            "↵ presses the button only; the grid's ↵ must not also select and open an item"
        );
    });

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-has-this".into()), cx)
    });
    render_ui(vcx);
    keystrokes(vcx, "shift-tab enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Tags,
            "⇧Tab ↵ presses the last button, Manage tags"
        )
    });
}

/// With one button Tab focuses it, and ↵ presses it.
#[core::prelude::v1::test]
fn empty_search_single_button_takes_tab_and_enter() {
    let (dir, engine) = seed_library("states_gaps_tab_one", 1);
    let _dir = TempLibrary(dir);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.set_search_query("zzqxnothing", cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-primary").is_none()
            && vcx.debug_bounds("empty-secondary").is_some(),
        "precondition: a search with no results shows only Clear all filters"
    );
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "Tab focuses the only button"
    );
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "Tab stays on the only button"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "↵ on Clear all filters clears the search"
        )
    });
}

/// A focus set in one empty state must not press a button in the next.
#[core::prelude::v1::test]
fn empty_state_focus_resets_when_the_view_changes() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = empty_tag_view("states_gaps_tab_reset", &mut cx);
    keystrokes(vcx, "tab tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Manage tags focused"
    );
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-has-this-either".into()), cx)
    });
    render_ui(vcx);
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "another empty view starts with no focused button"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("nothing-has-this-either".into()),
            "↵ with no focused button must not press the stale Manage tags"
        )
    });
}

/// Leaving an empty view and coming back (a click or Back) starts with no
/// focused button, as docs/accessibility.md promises.
#[core::prelude::v1::test]
fn empty_state_focus_does_not_return_with_the_view() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = empty_tag_view("states_gaps_tab_return", &mut cx);
    let tag = AlbumSelection::Tag("nothing-has-this".into());
    keystrokes(vcx, "tab tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Manage tags focused"
    );
    view.update(vcx, |app, cx| {
        app.select_album(
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            cx,
        );
        app.select_album(tag.clone(), cx);
    });
    render_ui(vcx);
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "returning to the view by a click starts with no focused button"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, tag, "↵ must not press the stale Manage tags")
    });

    // Back restores a search (a tag no item has falls back to All Media).
    view.update(vcx, |app, cx| app.set_search_query("zzqxnothing", cx));
    render_ui(vcx);
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Clear all filters focused"
    );
    view.update(vcx, |app, cx| {
        app.select_album(
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            cx,
        );
    });
    render_ui(vcx);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "zzqxnothing",
            "precondition: Back returns to the search"
        )
    });
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "returning to the view by Back starts with no focused button"
    );

    // Back is a way out too: Forward then returns with no focus.
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Clear all filters focused"
    );
    keystrokes(vcx, "secondary-[");
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "zzqxnothing",
            "precondition: Forward returns to the search"
        )
    });
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "returning to the view by Forward starts with no focused button"
    );
}
