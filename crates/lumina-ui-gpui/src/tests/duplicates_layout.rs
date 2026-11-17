//! Review Duplicates layout at the default 1280×840 window and the 960 px
//! minimum width: group rows fill the list, the footer buttons stay in the
//! window, and the copy cards fit above the footer.

use super::duplicates_review::{seed_files, seed_two_groups};
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
            text.right() <= skip.left() || text.bottom() <= skip.top(),
            "the footer text stays beside or above Skip group at {width} px: {text:?} vs {skip:?}"
        );
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

#[core::prelude::v1::test]
fn narrow_footer_shows_the_summary_whole_never_a_fragment() {
    let (temp_dir, engine) = seed_two_groups("dup_layout_summary");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // The summary's one-line width, where there is room for it.
    open(vcx, 1600.0, 840.0);
    let wide = vcx
        .debug_bounds("duplicates-footer-summary")
        .expect("the footer summary renders");
    let line = f32::from(wide.size.height);

    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(840.0)));
    render_ui(vcx);
    let footer = vcx.debug_bounds("duplicates-footer").expect("footer");
    let text = vcx
        .debug_bounds("duplicates-footer-text")
        .expect("the footer text renders");
    let frees = vcx
        .debug_bounds("duplicates-footer-frees")
        .expect("the footer frees text renders");
    let inside = |b: gpui::Bounds<gpui::Pixels>| {
        b.left() >= footer.left()
            && b.right() <= footer.right()
            && b.top() >= footer.top()
            && b.bottom() <= footer.bottom()
    };
    assert!(
        inside(frees) && frees.right() <= text.right() && f32::from(frees.size.width) >= 90.0,
        "\"frees …\" is shown whole at 960 px: {frees:?} in text {text:?}, footer {footer:?}"
    );
    for selector in ["duplicates-skip-group", "duplicates-merge-group"] {
        let button = vcx.debug_bounds(selector).expect("button renders");
        assert!(
            inside(button) && f32::from(button.size.width) > 60.0,
            "{selector} stays whole at 960 px: {button:?} in {footer:?}"
        );
        assert!(
            text.bottom() <= button.top() || text.right() <= button.left(),
            "the footer text never runs under {selector}: {text:?} vs {button:?}"
        );
    }
    // Shown in full (wrapped onto more lines) or not at all: a narrower box
    // must have the lines to hold the one-line width, all inside the footer.
    if let Some(summary) = vcx
        .debug_bounds("duplicates-footer-summary")
        .filter(|b| f32::from(b.size.width) > 0.0)
    {
        let lines = (f32::from(summary.size.height) / line).round();
        assert!(
            f32::from(summary.size.width) * lines >= f32::from(wide.size.width) - 1.0,
            "the summary is cut to a fragment at 960 px: {lines} line(s) of {:?}, it needs {:?}",
            summary.size.width,
            wide.size.width
        );
        assert!(
            inside(summary) && summary.right() <= text.right(),
            "every line of the summary is inside the footer: {summary:?} in {footer:?}"
        );
    } else {
        assert!(
            vcx.debug_bounds("duplicates-footer-summary").is_none(),
            "a summary with no room is hidden, not drawn zero-wide"
        );
    }

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn six_copies_scroll_sideways_with_the_footer_fixed() {
    let photo = super::ui_automation::shaded_jpeg(60);
    let names: Vec<String> = (1..=6).map(|i| format!("copy{i}.jpg")).collect();
    let files: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &photo[..])).collect();
    let (temp_dir, engine) = seed_files("dup_layout_six", &files);
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
    assert_eq!(ids.len(), 6, "one group of six copies");
    let last = leak(format!("duplicate-candidate-{}", ids[5]));
    let row = vcx
        .debug_bounds("duplicates-candidates-row")
        .expect("the cards row renders");
    let before = vcx.debug_bounds(last).expect("the last card renders");
    assert!(
        before.right() > row.right(),
        "six cards overflow the row: last card ends {:?}, row {:?}",
        before.right(),
        row.right()
    );
    for id in &ids {
        let card = vcx
            .debug_bounds(leak(format!("duplicate-candidate-{id}")))
            .unwrap_or_else(|| panic!("card {id} renders"));
        assert!(
            f32::from(card.size.width) >= 179.5,
            "card {id} keeps its 180 px minimum instead of squeezing: {:?}",
            card.size.width
        );
    }

    let position = row.center();
    vcx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    vcx.simulate_event(gpui::ScrollEvent {
        position,
        delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(-5000.0), gpui::px(0.0))),
        ..Default::default()
    });
    render_ui(vcx);
    let after = vcx.debug_bounds(last).expect("the last card renders");
    assert!(
        after.right() <= row.right() && after.left() >= row.left(),
        "scrolling sideways brings the last card into view: {after:?} in row {row:?}"
    );
    assert_eq!(
        vcx.debug_bounds("duplicates-footer"),
        Some(footer),
        "the footer stays put while the cards scroll"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn selection_dock_hides_on_duplicates_and_the_toast_clears_the_footer() {
    let (temp_dir, engine) = seed_two_groups("dup_layout_dock");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let footer = open(vcx, 1280.0, 840.0);
    let id = view.update(vcx, |app, _| app.duplicate_groups[0].items[1].id.0);
    // The photo: a squeezed card's centre can land on its keep radio.
    click(vcx, leak(format!("duplicate-photo-{id}")));
    view.update(vcx, |app, _| {
        assert!(app.has_selection(), "clicking a card selects that copy")
    });
    assert!(
        vcx.debug_bounds("multi-select-action-dock").is_none(),
        "the selection dock must not cover the Review Duplicates footer"
    );

    view.update(vcx, |app, cx| app.show_toast("Saved", cx));
    render_ui(vcx);
    let toast = vcx.debug_bounds("toast").expect("the toast renders");
    assert!(
        toast.bottom() <= footer.top(),
        "the toast sits above the footer: toast ends {:?}, footer starts {:?}",
        toast.bottom(),
        footer.top()
    );

    // At the 960 minimum the footer wraps taller. One draw lays it out
    // while the toast still uses the old height; the footer must ask for
    // the next frame (prepaint ignores `refresh`), which moves the toast.
    let draw_once = |vcx: &mut gpui::VisualTestContext| {
        vcx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
    };
    // Frames other things (running animations) ask for, with no change.
    vcx.update(|window, cx| window.simulate_next_frame(cx));
    draw_once(vcx);
    let baseline = vcx.update(|window, cx| window.simulate_next_frame(cx));
    // The resize draws the window once.
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(840.0)));
    let footer = vcx.debug_bounds("duplicates-footer").expect("footer");
    assert!(
        f32::from(footer.size.height) > 68.0,
        "the narrow footer wraps onto two rows: {footer:?}"
    );
    let stale = vcx.debug_bounds("toast").expect("the toast renders");
    assert!(
        stale.bottom() > footer.top(),
        "the first frame after the wrap still places the toast for one row"
    );
    let pending = vcx.update(|window, cx| window.simulate_next_frame(cx));
    assert!(
        pending > baseline,
        "the footer's new height asks for the next frame"
    );
    let recorded = view.update(vcx, |app, _| app.stacks.review.footer_height.get());
    assert!(
        (recorded - f32::from(footer.size.height)).abs() <= 0.5,
        "the toast reads the footer's measured height: {recorded} vs {footer:?}"
    );
    // No refresh: only the notify the next frame delivered redraws the view.
    vcx.update(|window, cx| window.draw(cx).clear(cx));
    let toast = vcx.debug_bounds("toast").expect("the toast renders");
    assert!(
        toast.bottom() <= footer.top(),
        "at 960 px the toast sits above the wrapped footer: toast ends {:?}, footer starts {:?}",
        toast.bottom(),
        footer.top()
    );

    // With every group skipped there is no footer: the toast drops back to
    // the normal offset above the column's bottom.
    view.update(vcx, |app, cx| {
        let hashes: Vec<String> = app
            .duplicate_groups
            .iter()
            .map(|g| g.hash.clone())
            .collect();
        app.stacks.review.skipped.extend(hashes);
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("duplicates-footer").is_none(),
        "no footer once every group is skipped"
    );
    let toast = vcx.debug_bounds("toast").expect("the toast renders");
    let lift = f32::from(footer.bottom()) - f32::from(toast.bottom());
    assert!(
        (lift - crate::app::render::overlays::DOCK_BOTTOM).abs() <= 1.0,
        "with no footer the toast keeps the normal {} px offset, got {lift}",
        crate::app::render::overlays::DOCK_BOTTOM
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
