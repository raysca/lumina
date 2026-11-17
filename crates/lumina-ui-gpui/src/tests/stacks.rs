//! Similar shots (plan 5 D2): the Duplicates destination's second tab lists
//! RAW+JPEG pairs and bursts from `find_stacks`, and its two actions only
//! flag (one undo entry each); nothing is deleted here.

use crate::app::stacks::{DuplicatesTab, StacksList};
use crate::tests::duplicates::run_palette_command;
use crate::tests::quick_edit::{scanned, temp_library};
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui,
};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::{Flag, LuminaEngine, MediaId, StackKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

const T0: i64 = 1_789_430_400_000;

/// The fixture's ids: a RAW+JPEG pair (stack 0) and a three-shot burst
/// (stack 1), in each stack's member order.
pub(crate) struct Fixture {
    pub(crate) dir: PathBuf,
    pub(crate) engine: Arc<LuminaEngine>,
    pub(crate) jpg: MediaId,
    pub(crate) raw: MediaId,
    pub(crate) shots: [MediaId; 3],
}

/// Real files through the real scan, then the camera and capture times the
/// fake files cannot carry, written to the catalog (D1 pairs only with a
/// known, equal make and model).
pub(crate) fn stacked_library(prefix: &str) -> Fixture {
    let dir = temp_library(prefix);
    let names = [
        "IMG_0001.jpg",
        "IMG_0001.arw",
        "shot_1.jpg",
        "shot_2.jpg",
        "shot_3.jpg",
    ];
    // Real, distinct JPEGs decode in process; the RAW stays a placeholder.
    for (i, name) in names.iter().enumerate() {
        let bytes = if name.ends_with(".jpg") {
            crate::tests::ui_automation::shaded_jpeg(i as u8 * 40)
        } else {
            format!("fixture-{i}").into_bytes()
        };
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    // An exact-copy pair with no camera (never a stack), so Exact copies
    // has items a hidden selection could take.
    for name in ["copy_a.jpg", "copy_b.jpg"] {
        std::fs::write(dir.join(name), crate::tests::ui_automation::tiny_jpeg()).unwrap();
    }
    let engine = scanned(&dir);
    let all = engine.get_item_slice(0..100);
    let id = |name: &str| all.iter().find(|i| i.file_name == name).unwrap().id;
    // shot_1 and shot_2 must not share a time: ties break by id, which
    // follows the scan's directory order (sorted on APFS, unordered on
    // ext4), so equal times made the burst order filesystem-dependent.
    // 100 ms apart stays inside the one-second burst gap.
    let times = [T0, T0, T0 + 60_000, T0 + 60_100, T0 + 61_000];
    let edited: Vec<_> = names
        .iter()
        .zip(times)
        .map(|(name, ts)| {
            let mut item = all.iter().find(|i| i.file_name == *name).unwrap().clone();
            item.make = Some("SONY".into());
            item.model = Some("ILCE-7M4".into());
            item.timestamp_epoch_ms = ts;
            item
        })
        .collect();
    engine.catalog().insert_or_update_batch(&edited).unwrap();
    Fixture {
        jpg: id("IMG_0001.jpg"),
        raw: id("IMG_0001.arw"),
        shots: [id("shot_1.jpg"), id("shot_2.jpg"), id("shot_3.jpg")],
        dir,
        engine,
    }
}

pub(crate) fn open_similar_shots(vcx: &mut VisualTestContext) {
    click(vcx, "sidebar-nav-duplicates");
    click(vcx, "duplicates-kind-similar");
}

pub(crate) fn loaded_ids(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
) -> Vec<Vec<MediaId>> {
    view.update(vcx, |app, _| match &app.stacks.list {
        StacksList::Loaded(stacks) => stacks
            .iter()
            .map(|s| s.items.iter().map(|i| i.id).collect())
            .collect(),
        _ => panic!("the Similar shots list must be loaded"),
    })
}

pub(crate) fn flags(engine: &LuminaEngine) -> HashMap<MediaId, Flag> {
    engine.get_flags_map()
}

pub(crate) fn undo_len(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> usize {
    view.update(vcx, |app, _| app.undo.undo_stack.len())
}

pub(crate) fn toast(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<String> {
    view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()))
}

#[core::prelude::v1::test]
fn stacks_tab_lists_raw_jpeg_pair() {
    let f = stacked_library("stacks_list");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    click(vcx, "sidebar-nav-duplicates");
    assert!(
        vcx.debug_bounds("duplicates-kind-exact").is_some()
            && vcx.debug_bounds("duplicates-kind-similar").is_some(),
        "the Duplicates destination shows both tabs"
    );

    // Open the tab without letting the background read finish: the
    // loading state is drawn meanwhile.
    view.update(vcx, |app, cx| {
        app.set_duplicates_tab(DuplicatesTab::Similar, cx)
    });
    vcx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    assert!(
        vcx.debug_bounds("stacks-loading").is_some(),
        "a running stack read shows the loading state"
    );
    render_ui(vcx);
    assert!(vcx.debug_bounds("stacks-loading").is_none());

    assert_eq!(
        loaded_ids(&view, vcx),
        vec![vec![f.jpg, f.raw], f.shots.to_vec()],
        "the pair (image first) and the burst, in find_stacks order"
    );
    let kinds = view.update(vcx, |app, _| match &app.stacks.list {
        StacksList::Loaded(s) => s.iter().map(|s| s.kind).collect::<Vec<_>>(),
        _ => Vec::new(),
    });
    assert_eq!(kinds, vec![StackKind::RawJpeg, StackKind::Burst]);
    for selector in [
        "stack-0",
        "stack-1",
        "stack-keep-picks-0",
        "stack-reject-others-0",
        "stack-0-kind",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }
    for id in [f.jpg, f.raw] {
        assert!(
            vcx.debug_bounds(leak(format!("stack-0-item-{}", id.0)))
                .is_some(),
            "every member of the pair gets a thumbnail"
        );
    }
    assert!(vcx.debug_bounds("stacks-empty").is_none());

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[core::prelude::v1::test]
fn stacks_tab_empty_state() {
    let (dir, engine) = crate::tests::ui_automation::seed_library("stacks_empty", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_similar_shots(vcx);

    assert!(
        vcx.debug_bounds("stacks-empty").is_some(),
        "a library with no pairs or bursts shows the empty state"
    );
    assert!(vcx.debug_bounds("stack-0").is_none());
    assert!(vcx.debug_bounds("stacks-error").is_none());

    drop(view);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn keep_picks_rejects_others_with_one_undo() {
    let f = stacked_library("stacks_keep");
    f.engine.set_flags(&[(f.shots[1], Flag::Pick)]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let before = undo_len(&view, vcx);

    click(vcx, "stack-keep-picks-1");
    let expected: HashMap<_, _> = [
        (f.shots[0], Flag::Reject),
        (f.shots[1], Flag::Pick),
        (f.shots[2], Flag::Reject),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        flags(&f.engine),
        expected,
        "Keep picks rejects every non-pick member and keeps the pick"
    );
    assert_eq!(undo_len(&view, vcx), before + 1, "one undo entry");
    assert!(
        vcx.debug_bounds(leak(format!("grid-flag-badge-{}", f.shots[0].0)))
            .is_some(),
        "the stack shows the new reject mark"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    assert_eq!(
        flags(&f.engine),
        [(f.shots[1], Flag::Pick)].into_iter().collect(),
        "a single ⌘Z restores every member's flag"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[core::prelude::v1::test]
fn keep_picks_without_a_pick_changes_nothing() {
    let f = stacked_library("stacks_nopick");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let before = undo_len(&view, vcx);

    click(vcx, "stack-keep-picks-1");
    assert!(
        flags(&f.engine).is_empty(),
        "with no pick, Keep picks must not reject the whole stack"
    );
    assert_eq!(undo_len(&view, vcx), before, "no undo entry");
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some(crate::i18n::t("stacks.pick_first").as_str()),
        "the toast says to pick one first"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[core::prelude::v1::test]
fn reject_all_but_first_keeps_the_first() {
    let f = stacked_library("stacks_first");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let before = undo_len(&view, vcx);

    click(vcx, "stack-reject-others-1");
    assert_eq!(
        flags(&f.engine),
        [(f.shots[1], Flag::Reject), (f.shots[2], Flag::Reject)]
            .into_iter()
            .collect(),
        "every member but the earliest shot is rejected"
    );
    assert_eq!(undo_len(&view, vcx), before + 1, "one undo entry");

    // ⇧↵ on the focused stack (the click focused the burst; ↑ the pair):
    // the RAW goes, the image stays.
    focus_app(vcx, &view);
    keystrokes(vcx, "up");
    keystrokes(vcx, "shift-enter");
    assert_eq!(
        flags(&f.engine).get(&f.raw),
        Some(&Flag::Reject),
        "⇧↵ rejects all but the first of the focused stack"
    );
    assert_eq!(flags(&f.engine).get(&f.jpg), None, "the image is kept");

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    keystrokes(vcx, "secondary-z");
    assert!(flags(&f.engine).is_empty(), "two ⌘Z undo the two actions");

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[core::prelude::v1::test]
fn stacks_keyboard_keep_picks() {
    let f = stacked_library("stacks_keys");
    f.engine
        .set_flags(&[(f.raw, Flag::Pick), (f.shots[2], Flag::Pick)])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    focus_app(vcx, &view);

    let focused = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.stacks.focused)
    };
    assert_eq!(focused(&view, vcx), 0, "the first stack starts focused");
    let card = vcx
        .debug_bounds("stack-0")
        .expect("the first stack renders");
    let bar = vcx.debug_bounds("stack-focus-0").expect("focus is drawn");
    assert!(
        f32::from(bar.size.width) >= f32::from(card.size.width) - 2.0
            && f32::from(bar.size.height) >= f32::from(card.size.height) - 2.0,
        "the focus bar is the left border of a card-sized quad, so it follows \
         the card's rounded corners, got {bar:?} in {card:?}"
    );
    keystrokes(vcx, "down");
    assert_eq!(focused(&view, vcx), 1, "↓ moves to the next stack");
    assert!(vcx.debug_bounds("stack-focus-1").is_some());
    keystrokes(vcx, "down");
    assert_eq!(focused(&view, vcx), 1, "↓ stops at the last stack");
    keystrokes(vcx, "up");
    keystrokes(vcx, "down");

    keystrokes(vcx, "enter");
    let expected: HashMap<_, _> = [
        (f.raw, Flag::Pick),
        (f.shots[0], Flag::Reject),
        (f.shots[1], Flag::Reject),
        (f.shots[2], Flag::Pick),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        flags(&f.engine),
        expected,
        "↵ runs Keep picks on the focused stack only"
    );

    // Grid shortcuts must not act on a selection hidden behind this view.
    let first = f.engine.get_item_by_id(f.jpg).unwrap();
    view.update(vcx, |app, _| app.selected_item = Some(first));
    for key in ["x", "f", "backspace", "1"] {
        keystrokes(vcx, key);
    }
    assert_eq!(flags(&f.engine), expected, "X does not flag the selection");
    assert!(
        !f.engine.get_item_by_id(f.jpg).unwrap().is_favorite,
        "F does not favorite the selection"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ opens no Delete dialog"
        );
    });
    assert_eq!(f.engine.catalog().get_rating(f.jpg).unwrap(), 0);

    // Esc does what it does on Exact copies: the first dismisses the
    // toast, the next is the grid's Esc (inspector closed: clear selection).
    view.update(vcx, |app, _| app.inspector_open = false);
    keystrokes(vcx, "escape");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.selected_item.is_none(), "Esc clears the selection");
        assert_eq!(app.selection, AlbumSelection::Duplicates);
    });

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[core::prelude::v1::test]
fn stack_flag_write_failure_shows_banner_and_keeps_flags() {
    let f = stacked_library("stacks_fail");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    open_similar_shots(vcx);
    let before = undo_len(&view, vcx);

    crate::app::culling::FLAG_WRITE_FAILS.with(|c| c.set(true));
    click(vcx, "stack-reject-others-1");
    crate::app::culling::FLAG_WRITE_FAILS.with(|c| c.set(false));

    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .is_some_and(|m| m.contains("Could not save flag")),
            "the failed write reaches the error banner"
        );
        assert!(app.item_flags.is_empty(), "no optimistic flag is left");
    });
    assert!(flags(&f.engine).is_empty());
    assert_eq!(undo_len(&view, vcx), before, "a failed write adds no undo");
    assert!(
        vcx.debug_bounds(leak(format!("grid-flag-badge-{}", f.shots[1].0)))
            .is_none(),
        "the stack shows no reject mark"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}

#[core::prelude::v1::test]
fn stale_stacks_result_dropped_after_library_switch() {
    let f = stacked_library("stacks_stale");
    let other = temp_library("stacks_stale_other");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());
    click(vcx, "sidebar-nav-duplicates");

    // The read for this library is still running when another one opens.
    view.update(vcx, |app, cx| {
        app.set_duplicates_tab(DuplicatesTab::Similar, cx);
        app.open_library_inner(other.clone(), cx);
    });
    render_ui(vcx);

    view.update(vcx, |app, _| {
        assert!(
            !matches!(app.stacks.list, StacksList::Loaded(_)),
            "the old library's stacks must never land after a switch"
        );
        assert_eq!(app.stacks.tab, DuplicatesTab::Exact);
    });

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
    let _ = std::fs::remove_dir_all(&other);
}

#[core::prelude::v1::test]
fn palette_switches_duplicates_tabs() {
    let f = stacked_library("stacks_palette");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, f.engine.clone());

    run_palette_command(vcx, &view, "Show Similar Shots");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Duplicates);
        assert_eq!(app.stacks.tab, DuplicatesTab::Similar, "⌘K opens the tab");
    });
    assert!(
        vcx.debug_bounds("stack-0").is_some(),
        "and lists the stacks"
    );

    run_palette_command(vcx, &view, "Show Exact Copies");
    view.update(vcx, |app, _| {
        assert_eq!(app.stacks.tab, DuplicatesTab::Exact)
    });
    assert!(vcx.debug_bounds("stack-0").is_none());

    drop(view);
    let _ = std::fs::remove_dir_all(&f.dir);
}
