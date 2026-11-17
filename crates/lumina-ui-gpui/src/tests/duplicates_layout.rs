//! Review Duplicates layout at the default 1280×840 window and the 960 px
//! minimum width: group rows fill the list, the footer buttons stay in the
//! window, and the copy cards fit above the footer.

use super::duplicates_review::seed_two_groups;
use super::ui_automation::{app_with_library, click, leak, render_ui};

/// Opens Review Duplicates in a `width`×`height` window (sidebar shown,
/// inspector hidden) and returns the footer's bounds.
fn open(vcx: &mut gpui::VisualTestContext, width: f32, height: f32) -> gpui::Bounds<gpui::Pixels> {
    vcx.simulate_resize(gpui::size(gpui::px(width), gpui::px(height)));
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    vcx.debug_bounds("duplicates-footer")
        .expect("the compare footer renders")
}

#[core::prelude::v1::test]
fn group_rows_span_the_list_column() {
    let (temp_dir, engine) = seed_two_groups("dup_layout_rows");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(vcx, 1280.0, 840.0);

    let list = vcx
        .debug_bounds("duplicates-group-list")
        .expect("the group list renders");
    for index in 0..2 {
        let row = vcx
            .debug_bounds(leak(format!("duplicates-group-row-{index}")))
            .unwrap_or_else(|| panic!("group row {index} renders"));
        // The list's 1 px right border is the only gap allowed.
        assert!(
            (f32::from(list.right()) - f32::from(row.right())).abs() <= 1.0,
            "group row {index} must span the list column: row ends at {:?}, column at {:?}",
            row.right(),
            list.right()
        );
        assert!(
            (f32::from(row.left()) - f32::from(list.left())).abs() <= 1.0,
            "group row {index} starts at the column's left edge"
        );
    }

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn footer_buttons_stay_inside_the_window() {
    for width in [1280.0, 960.0] {
        let (temp_dir, engine) = seed_two_groups("dup_layout_footer");
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = app_with_library(&mut cx, engine);
        let footer = open(vcx, width, 840.0);
        let window = vcx.update(|window, _| window.viewport_size());

        for selector in ["duplicates-skip-group", "duplicates-merge-group"] {
            let button = vcx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} renders"));
            assert!(
                button.right() <= footer.right() && button.right() <= window.width,
                "{selector} must be fully visible at {width} px: ends at {:?}, footer at {:?}",
                button.right(),
                footer.right()
            );
            assert!(
                f32::from(button.size.width) > 60.0,
                "{selector} keeps its width at {width} px, got {:?}",
                button.size.width
            );
        }
        // The text never runs under the buttons.
        let text = vcx
            .debug_bounds("duplicates-footer-text")
            .expect("the footer text renders");
        let skip = vcx
            .debug_bounds("duplicates-skip-group")
            .expect("Skip group renders");
        assert!(
            text.right() <= skip.left(),
            "the footer text stops before Skip group at {width} px: {:?} > {:?}",
            text.right(),
            skip.left()
        );
        // At 960 the buttons take the whole footer; where there is room,
        // only the long summary gives way and "frees …" stays whole.
        if width >= 1280.0 {
            let summary = vcx
                .debug_bounds("duplicates-footer-summary")
                .expect("the footer summary renders");
            let frees = vcx
                .debug_bounds("duplicates-footer-frees")
                .expect("the footer frees text renders");
            assert!(
                f32::from(summary.size.width) >= 60.0,
                "the footer summary keeps a usable width at {width} px, got {:?}",
                summary.size.width
            );
            assert!(
                frees.right() <= text.right(),
                "\"frees …\" is shown whole at {width} px: ends {:?}, text box {:?}",
                frees.right(),
                text.right()
            );
        }

        drop(view);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[core::prelude::v1::test]
fn copy_cards_and_differences_fit_above_the_footer() {
    let (temp_dir, engine) = seed_two_groups("dup_layout_cards");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let footer = open(vcx, 1280.0, 840.0);

    let ids: Vec<u64> = view.update(vcx, |app, _| {
        app.duplicate_groups[0]
            .items
            .iter()
            .map(|i| i.id.0)
            .collect()
    });
    for id in &ids {
        let card = vcx
            .debug_bounds(leak(format!("duplicate-candidate-{id}")))
            .unwrap_or_else(|| panic!("card {id} renders"));
        assert!(
            card.bottom() <= footer.top(),
            "card {id} (its last detail row) must end above the footer: {:?} > {:?}",
            card.bottom(),
            footer.top()
        );
    }
    let differences = vcx
        .debug_bounds("duplicates-differences")
        .expect("the differences line renders");
    assert!(
        differences.bottom() <= footer.top(),
        "the differences line must sit above the footer: {:?} > {:?}",
        differences.bottom(),
        footer.top()
    );

    // A short window keeps every row whole and scrolls the pane instead.
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(560.0)));
    render_ui(vcx);
    let differences = vcx
        .debug_bounds("duplicates-differences")
        .expect("the differences line renders");
    for id in &ids {
        let card = vcx
            .debug_bounds(leak(format!("duplicate-candidate-{id}")))
            .unwrap_or_else(|| panic!("card {id} renders"));
        let facts = vcx
            .debug_bounds(leak(format!("duplicate-facts-{id}")))
            .unwrap_or_else(|| panic!("card {id} shows its facts"));
        let photo = vcx
            .debug_bounds(leak(format!("duplicate-photo-{id}")))
            .unwrap_or_else(|| panic!("card {id} shows its photo"));
        assert!(
            f32::from(photo.size.height) >= 139.5,
            "in a short window card {id}'s photo keeps its 140 px minimum, got {:?}",
            photo.size.height
        );
        assert!(
            f32::from(card.size.width) - f32::from(photo.size.width) <= 3.0,
            "card {id}'s photo fills the card's width: {:?} of {:?}",
            photo.size.width,
            card.size.width
        );
        assert!(
            facts.bottom() <= card.bottom() && card.bottom() <= differences.top(),
            "in a short window card {id} must not clip its facts or overlap the \
             differences line: facts end {:?}, card {:?}, differences start {:?}",
            facts.bottom(),
            card.bottom(),
            differences.top()
        );
    }

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn tall_window_caps_the_photo_and_leaves_spare_space_below() {
    let (temp_dir, engine) = seed_two_groups("dup_layout_tall");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(vcx, 1280.0, 1440.0);

    let ids: Vec<u64> = view.update(vcx, |app, _| {
        app.duplicate_groups[0]
            .items
            .iter()
            .map(|i| i.id.0)
            .collect()
    });
    let differences = vcx
        .debug_bounds("duplicates-differences")
        .expect("the differences line renders");
    for id in &ids {
        let card = vcx
            .debug_bounds(leak(format!("duplicate-candidate-{id}")))
            .unwrap_or_else(|| panic!("card {id} renders"));
        let photo = vcx
            .debug_bounds(leak(format!("duplicate-photo-{id}")))
            .unwrap_or_else(|| panic!("card {id} shows its photo"));
        let facts = vcx
            .debug_bounds(leak(format!("duplicate-facts-{id}")))
            .unwrap_or_else(|| panic!("card {id} shows its facts"));
        let height = f32::from(photo.size.height);
        assert!(
            (330.0..=360.0).contains(&height),
            "a tall window caps card {id}'s photo near the design's 345 px, got {height}"
        );
        assert!(
            f32::from(card.bottom()) - f32::from(facts.bottom()) <= 2.0,
            "card {id} has no empty space below its facts: facts end {:?}, card {:?}",
            facts.bottom(),
            card.bottom()
        );
        assert!(
            f32::from(differences.top()) - f32::from(card.bottom()) <= 21.0,
            "the differences line follows the cards; spare height goes below it: \
             card ends {:?}, differences start {:?}",
            card.bottom(),
            differences.top()
        );
    }

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
