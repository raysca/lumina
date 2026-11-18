//! Similar shots members: a burst of RAW+JPEG pairs
//! keeps its images, a stack wider than the pane says how many members are
//! not drawn, and Show Similar Shots from the viewer leaves no selection.

use crate::app::stacks::{DuplicatesTab, StacksList};
use crate::tests::duplicates::run_palette_command;
use crate::tests::helpers::{app_with_library, click, focus_app, keystrokes, leak};
use crate::tests::stacks::{flags, open_similar_shots, stacked_library, undo_len, Fixture};
use crate::*;
use lumina_core::{Flag, MediaId, MediaType, StackKind};

/// Adds `name.ext` rows cloned from the fixture's camera row, `offset_ms`
/// after it, and returns their ids in order.
fn add(f: &Fixture, rows: &[(&str, MediaType, i64)]) -> Vec<MediaId> {
    let tpl = f.engine.get_item_by_id(f.jpg).unwrap();
    let items: Vec<_> = rows
        .iter()
        .map(|(name, kind, offset)| {
            let mut it = tpl.clone();
            it.file_name = name.to_string();
            it.path = f.dir.join(name);
            it.media_type = *kind;
            it.timestamp_epoch_ms += offset;
            it
        })
        .collect();
    let stored = f.engine.catalog().insert_or_update_batch(&items).unwrap();
    items
        .iter()
        .map(|it| stored.iter().find(|s| s.path == it.path).unwrap().id)
        .collect()
}

fn stacks(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<(StackKind, Vec<MediaId>)> {
    view.update(vcx, |app, _| match &app.stacks.list {
        StacksList::Loaded(s) => s
            .iter()
            .map(|s| (s.kind, s.items.iter().map(|i| i.id).collect()))
            .collect(),
        _ => panic!("the Similar shots list must be loaded"),
    })
}

#[core::prelude::v1::test]
fn burst_of_raw_jpeg_pairs_lists_pairs_and_burst() {
    let f = stacked_library("stacks_pair_burst");
    const H: i64 = 7_200_000;
    let ids = add(
        &f,
        &[
            ("PB_1.jpg", MediaType::Image, H),
            ("PB_1.arw", MediaType::Raw, H),
            ("PB_2.jpg", MediaType::Image, H),
            ("PB_2.arw", MediaType::Raw, H),
            ("PB_3.jpg", MediaType::Image, H + 1_000),
            ("PB_3.arw", MediaType::Raw, H + 1_000),
        ],
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let listed = stacks(&view, vcx);
    let pairs: Vec<_> = [0, 2, 4].map(|i| vec![ids[i], ids[i + 1]]).to_vec();
    for pair in &pairs {
        assert!(
            listed.contains(&(StackKind::RawJpeg, pair.clone())),
            "each RAW+JPEG pair of the burst is listed"
        );
    }
    assert!(
        listed.contains(&(StackKind::Burst, vec![ids[0], ids[2], ids[4]])),
        "the burst of the pairs' images is listed too, not dropped"
    );
    drop(view);
}

#[core::prelude::v1::test]
fn wide_stack_shows_how_many_members_are_not_drawn() {
    let f = stacked_library("stacks_wide");
    const H: i64 = 7_200_000;
    let names: Vec<String> = (0..12).map(|i| format!("W_{i:02}.jpg")).collect();
    let rows: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), MediaType::Image, H + (i as i64 / 2) * 1_000))
        .collect();
    let ids = add(&f, &rows);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    // A normal window with the inspector open: eight members do not fit.
    vcx.simulate_resize(gpui::size(gpui::px(1100.0), gpui::px(800.0)));
    view.update(vcx, |app, _| app.inspector_open = true);
    open_similar_shots(vcx);
    let index = stacks(&view, vcx)
        .iter()
        .position(|(_, m)| *m == ids)
        .expect("the 12 shots are one burst");

    let window = vcx.update(|window, _| window.viewport_size());
    let more = vcx
        .debug_bounds(leak(format!("stack-{index}-more")))
        .expect("a stack wider than the pane says how many members are not drawn");
    assert!(
        more.right() <= window.width && more.bottom() <= window.height,
        "the count is inside the window, not clipped"
    );
    for id in &ids {
        if let Some(b) = vcx.debug_bounds(leak(format!("stack-{index}-item-{}", id.0))) {
            assert!(
                b.right() <= more.left() && b.size.width >= gpui::px(140.0),
                "a drawn member is full size and fits before the count, not squeezed or cut off"
            );
        }
    }

    // Both actions still act on every member, drawn or not.
    let last = *ids.last().unwrap();
    f.engine.set_flags(&[(last, Flag::Pick)]).unwrap();
    view.update(vcx, |app, _| app.item_flags.insert(last, Flag::Pick));
    let before = undo_len(&view, vcx);
    click(vcx, leak(format!("stack-keep-picks-{index}")));
    let rejected = |f: &Fixture| {
        ids.iter()
            .filter(|id| flags(&f.engine).get(id) == Some(&Flag::Reject))
            .count()
    };
    assert_eq!(rejected(&f), 11, "Keep picks rejects all 11 others");
    assert_eq!(undo_len(&view, vcx), before + 1);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    click(vcx, leak(format!("stack-reject-others-{index}")));
    let after = flags(&f.engine);
    assert_eq!(rejected(&f), 11, "Reject all but the first rejects 11");
    assert_eq!(after.get(&ids[0]), None, "the first is kept");
    drop(view);
}

#[core::prelude::v1::test]
fn show_similar_shots_from_the_viewer_leaves_no_selection() {
    let f = stacked_library("stacks_from_viewer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    click(vcx, "sidebar-nav-duplicates");
    view.update(vcx, |app, cx| {
        let item = app.duplicate_groups[0].items[0].clone();
        app.open_viewer(item, cx);
    });
    assert!(view.update(vcx, |app, _| app.is_fullscreen_viewer));

    run_palette_command(vcx, &view, "Show Similar Shots");
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    keystrokes(vcx, "secondary-a");
    view.update(vcx, |app, _| {
        assert_eq!(app.stacks.tab, DuplicatesTab::Similar);
        assert!(
            !app.is_fullscreen_viewer,
            "Show Similar Shots closes the viewer"
        );
        assert!(
            app.selected_item.is_none()
                && app.selected_indices.is_empty()
                && app.get_selected_ids().is_empty(),
            "→ and ⌘A select nothing on Similar shots"
        );
    });
    drop(view);
}

#[core::prelude::v1::test]
fn slideshow_does_not_start_on_similar_shots() {
    let f = stacked_library("stacks_slideshow");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    focus_app(vcx, &view);

    let check = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, how: &str| {
        view.update(vcx, |app, _| {
            assert!(app.slideshow.is_none(), "{how} starts no slideshow");
            assert!(!app.is_fullscreen_viewer, "{how} leaves the viewer closed");
            assert!(
                app.selected_item.is_none() && app.selected_indices.is_empty(),
                "{how} selects nothing"
            );
            assert!(app.similar_shots_shown(), "{how} stays on Similar shots");
        });
        assert_eq!(
            crate::tests::stacks::toast(view, vcx),
            Some(crate::i18n::t("stacks.no_slideshow")),
            "{how} says why"
        );
    };
    keystrokes(vcx, "alt-secondary-s");
    check(&view, vcx, "⌥⌘S");
    view.update(vcx, |app, cx| {
        app.dismiss_toast(cx);
        app.execute_command(crate::app::command_palette::CommandId::Slideshow, cx);
    });
    vcx.run_until_parked();
    check(&view, vcx, "⌘K Slideshow");
    drop(view);
}

#[core::prelude::v1::test]
fn narrowest_window_shows_only_the_count_when_one_member_does_not_fit() {
    let f = stacked_library("stacks_narrow");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    // The smallest window with both panes dragged to their maxima.
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(800.0)));
    view.update(vcx, |app, _| {
        app.sidebar_collapsed = false;
        app.sidebar_width = 320.0;
        app.inspector_open = true;
        app.inspector_width = 380.0;
    });
    open_similar_shots(vcx);

    let window = vcx.update(|window, _| window.viewport_size());
    for index in 0..2 {
        let card = vcx
            .debug_bounds(leak(format!("stack-{index}")))
            .expect("the stack renders");
        let more = vcx
            .debug_bounds(leak(format!("stack-{index}-more")))
            .expect("a stack that does not fit says how many members are not drawn");
        assert!(
            more.right() <= card.right() && more.right() <= window.width,
            "the count is inside its card, not clipped"
        );
        let ids = stacks(&view, vcx)[index].1.clone();
        for id in ids {
            if let Some(b) = vcx.debug_bounds(leak(format!("stack-{index}-item-{}", id.0))) {
                assert!(
                    b.size.width >= gpui::px(140.0) && b.right() <= more.left(),
                    "a drawn member is full size and before the count"
                );
            }
        }
    }
    drop(view);
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn expanding_pip_on_similar_shots_shows_the_video_on_exact_copies() {
    let dir = crate::tests::quick_edit::temp_library("stacks_pip");
    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
    // Two copies of one clip: an exact-copy group holding the video.
    std::fs::copy(&clip, dir.join("video_a.mp4")).unwrap();
    std::fs::copy(&clip, dir.join("video_b.mp4")).unwrap();
    let engine = crate::tests::quick_edit::scanned(&dir);
    let video = engine
        .get_item_slice(0..10)
        .into_iter()
        .find(|i| i.file_name == "video_a.mp4")
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    click(vcx, leak(format!("media-card-{}", video.id.0)));
    focus_app(vcx, &view);
    keystrokes(vcx, "alt-p");
    assert!(
        view.update(vcx, |app, _| app.video.in_pip),
        "the video is in PiP"
    );

    open_similar_shots(vcx);
    view.update(vcx, |app, cx| app.expand_pip_to_viewer(cx));
    crate::tests::helpers::render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "expand opens the viewer");
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(video.id),
            "the viewer shows the PiP video"
        );
        assert_eq!(app.stacks.tab, DuplicatesTab::Exact, "on Exact copies");
        assert!(
            !app.similar_shots_shown(),
            "Similar shots holds no selection"
        );
    });
    drop(view);
}
