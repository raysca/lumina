//! Empty-state and toast gaps from the design index (plans 09, 10, 11):
//! the digits headline, rule chips beside both side panels, the scanning
//! Hero size and a long toast beside its action.

use super::downloads::TempLibrary;
use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::ui_automation::{app_with_library, leak, render_ui, seed_library};
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

/// Review Focus (plan 09): with the sidebar and the inspector both open,
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

/// Design 11 Task 4: an empty library mid-scan also draws at the Hero size.
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

/// Plan 10 Task 2: a long title and subtitle truncate before Undo, which
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
