//! UI automation for the Review Duplicates compare pane and kind chips.

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::stacks::DuplicatesTab;

/// A scanned library of `files` (paths relative to a fresh temp dir).
pub(crate) fn seed_files(
    prefix: &str,
    files: &[(&str, &[u8])],
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_uitest_{prefix}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    for (path, bytes) in files {
        let path = temp_dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        // One fixed mtime: with no EXIF date the scan dates a file by it, so
        // copies written a few ms apart would differ in Captured.
        std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| {
                f.set_modified(
                    std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
                )
            })
            .unwrap();
    }
    let (engine, rx) = lumina_core::LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    (temp_dir, engine)
}

/// A scanned library with two exact-copy groups: three copies of one photo
/// and two of another, so the three-copy group sorts first.
pub(crate) fn seed_two_groups(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let a = super::ui_automation::shaded_jpeg(10);
    let b = super::ui_automation::shaded_jpeg(200);
    seed_files(
        prefix,
        &[
            ("a1.jpg", &a),
            ("a2.jpg", &a),
            ("a3.jpg", &a),
            ("b1.jpg", &b),
            ("b2.jpg", &b),
        ],
    )
}

#[core::prelude::v1::test]
fn differences_list_only_fields_that_differ() {
    let photo = super::ui_automation::shaded_jpeg(40);
    let (temp_dir, engine) = seed_files(
        "dup_diff",
        &[("a/photo.jpg", &photo), ("b/photo.jpg", &photo)],
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("duplicates-differences-text:1 difference")
            .is_some(),
        "two copies that differ only by folder have one difference"
    );
    assert!(
        vcx.debug_bounds("duplicates-difference-folder").is_some(),
        "the folder chip is shown"
    );
    for field in ["dimensions", "size", "format", "edits", "source"] {
        assert!(
            vcx.debug_bounds(super::ui_automation::leak(format!(
                "duplicates-difference-{field}"
            )))
            .is_none(),
            "{field} is the same on both copies, so it has no chip"
        );
    }
    assert!(vcx.debug_bounds("duplicates-kind-all").is_some());
    assert!(vcx.debug_bounds("duplicates-kind-exact").is_some());
    assert!(
        vcx.debug_bounds("duplicates-sorted").is_some(),
        "the filter row says how the groups are sorted"
    );

    click(vcx, "duplicates-kind-similar");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.stacks.tab,
            DuplicatesTab::Similar,
            "the Similar chip opens the Similar shots review"
        );
    });

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn choosing_a_keeper_merges_the_others() {
    let (temp_dir, engine) = seed_library("dup_keeper", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let (first, second) = view.update(vcx, |app, _| {
        let items = &app.duplicate_groups[0].items;
        (items[0].id, items[1].id)
    });
    let keeper = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| {
            app.stacks.review.keeper(&app.duplicate_groups[0])
        })
    };
    assert_eq!(
        keeper(&view, vcx),
        first,
        "the heuristic pick keeps by default"
    );
    assert!(
        vcx.debug_bounds(super::ui_automation::leak(format!(
            "duplicate-best-{}",
            first.0
        )))
        .is_some(),
        "BEST marks the heuristic pick"
    );
    assert!(
        vcx.debug_bounds(
            "duplicates-summary-text:Keeping 1 of 2 · 1 copy moves to Recently Deleted"
        )
        .is_some(),
        "the footer says what the merge will do"
    );

    click(
        vcx,
        super::ui_automation::leak(format!("duplicate-keep-{}", second.0)),
    );
    assert_eq!(keeper(&view, vcx), second, "the radio chooses the keeper");
    view.update(vcx, |app, _| {
        assert!(
            app.selected_item.as_ref().is_none_or(|i| i.id != second),
            "the radio is its own control: it does not select the card"
        );
    });

    click(vcx, "duplicates-merge-group");
    let active: Vec<_> = engine.get_item_slice(0..10).iter().map(|i| i.id).collect();
    assert_eq!(
        active,
        vec![second],
        "the chosen copy stays, the other is trashed"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    assert_eq!(
        engine.get_item_slice(0..10).len(),
        2,
        "⌘Z restores the trashed copy"
    );

    render_ui(vcx);
    assert_eq!(keeper(&view, vcx), second, "the choice outlives an undo");
    click(vcx, "duplicates-auto-best");
    assert_eq!(
        keeper(&view, vcx),
        first,
        "Auto-select best goes back to the heuristic"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn merge_all_keeps_the_chosen_copy() {
    let (temp_dir, engine) = seed_library("dup_keeper_all", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let second = view.update(vcx, |app, _| app.duplicate_groups[0].items[1].id);

    click(
        vcx,
        super::ui_automation::leak(format!("duplicate-keep-{}", second.0)),
    );
    click(vcx, "merge-all-duplicates");
    let active: Vec<_> = engine.get_item_slice(0..10).iter().map(|i| i.id).collect();
    assert_eq!(
        active,
        vec![second],
        "Merge all keeps the copy the user chose"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn footer_counts_copies_and_freed_bytes() {
    let (temp_dir, engine) = seed_two_groups("dup_footer");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let (count, freed) = view.update(vcx, |app, _| {
        let items = &app.duplicate_groups[0].items;
        let freed: u64 = items[1..].iter().map(|i| i.file_size).sum();
        (items.len(), crate::app::types::format_bytes(freed))
    });
    assert_eq!(count, 3, "the three-copy group is shown first");
    assert!(
        vcx.debug_bounds(
            "duplicates-summary-text:Keeping 1 of 3 · 2 copies move to Recently Deleted"
        )
        .is_some(),
        "two copies read in the plural"
    );
    assert!(
        vcx.debug_bounds(super::ui_automation::leak(format!(
            "duplicates-frees-text:· frees {freed}"
        )))
        .is_some(),
        "the footer frees the non-keepers' bytes ({freed})"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn skip_group_moves_on_and_skip_all_empties_the_review() {
    let (temp_dir, engine) = seed_two_groups("dup_skip");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let before = engine.get_item_slice(0..100).len();
    let (second_ids, second_meta) = view.update(vcx, |app, _| {
        let g = &app.duplicate_groups[1];
        let meta = format!(
            "page-meta-text:1 group · 2 items · {} recoverable",
            crate::app::types::format_bytes(g.potential_reclaimable_bytes)
        );
        (g.items.iter().map(|i| i.id.0).collect::<Vec<_>>(), meta)
    });
    assert!(vcx
        .debug_bounds("duplicates-position-text:1 of 2")
        .is_some());

    click(vcx, "duplicates-skip-group");
    for id in &second_ids {
        assert!(
            vcx.debug_bounds(super::ui_automation::leak(format!(
                "duplicate-candidate-{id}"
            )))
            .is_some(),
            "skipping moves on to the next group"
        );
    }
    assert!(
        vcx.debug_bounds("duplicates-position-text:1 of 1")
            .is_some(),
        "the skipped group leaves the count"
    );
    assert!(
        vcx.debug_bounds(super::ui_automation::leak(second_meta.clone()))
            .is_some(),
        "the header counts only the groups left: {second_meta}"
    );
    assert!(
        vcx.debug_bounds("duplicates-group-row-1").is_none(),
        "the skipped group leaves the list"
    );

    click(vcx, "duplicates-skip-all");
    assert!(
        vcx.debug_bounds("empty-state-headline-text:All groups skipped")
            .is_some(),
        "with every group skipped the review shows its empty state"
    );
    assert!(vcx.debug_bounds("duplicates-group-list").is_none());
    view.update(vcx, |app, _| {
        assert!(
            !app.command_applies(crate::app::command_palette::CommandId::MergeAllDuplicates),
            "with every group skipped, Merge All Duplicates has nothing to do"
        );
    });
    assert_eq!(
        engine.get_item_slice(0..100).len(),
        before,
        "skipping never changes the catalog"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn merge_all_leaves_skipped_groups() {
    let (temp_dir, engine) = seed_two_groups("dup_skip_merge");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let skipped: Vec<_> = view.update(vcx, |app, _| {
        app.duplicate_groups[0].items.iter().map(|i| i.id).collect()
    });

    click(vcx, "duplicates-skip-group");
    click(vcx, "merge-all-duplicates");
    let active: Vec<_> = engine.get_item_slice(0..100).iter().map(|i| i.id).collect();
    assert_eq!(active.len(), 4, "only the shown group lost a copy");
    assert!(
        skipped.iter().all(|id| active.contains(id)),
        "Merge all leaves a skipped group alone"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn merging_a_group_moves_on_to_the_next() {
    let (temp_dir, engine) = seed_two_groups("dup_merge_next");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    let second_ids: Vec<u64> = view.update(vcx, |app, _| {
        app.duplicate_groups[1]
            .items
            .iter()
            .map(|i| i.id.0)
            .collect()
    });

    click(vcx, "duplicates-merge-group");
    for id in &second_ids {
        assert!(
            vcx.debug_bounds(super::ui_automation::leak(format!(
                "duplicate-candidate-{id}"
            )))
            .is_some(),
            "after a merge the next group is compared"
        );
    }
    assert!(
        vcx.debug_bounds("duplicates-position-text:1 of 1")
            .is_some(),
        "the merged group leaves the count"
    );

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn differences_name_renamed_copies_and_edits() {
    let photo = super::ui_automation::shaded_jpeg(60);
    let (temp_dir, engine) = seed_files(
        "dup_diff_name",
        &[("IMG_1.jpg", &photo), ("IMG_1 copy.jpg", &photo)],
    );
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    render_ui(vcx);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);

    let only = |vcx: &mut gpui::VisualTestContext, expected: &[&str]| {
        for field in [
            "dimensions",
            "size",
            "format",
            "edits",
            "source",
            "folder",
            "name",
            "captured",
            "title",
        ] {
            let shown = vcx
                .debug_bounds(super::ui_automation::leak(format!(
                    "duplicates-difference-{field}"
                )))
                .is_some();
            assert_eq!(
                shown,
                expected.contains(&field),
                "only {expected:?} differ, but the {field} chip shown = {shown}"
            );
        }
    };
    assert!(
        vcx.debug_bounds("duplicates-differences-text:1 difference")
            .is_some(),
        "renamed copies in one folder differ by name"
    );
    only(vcx, &["name"]);
    let first = view.update(vcx, |app, _| app.duplicate_groups[0].items[0].id);
    assert!(
        vcx.debug_bounds(super::ui_automation::leak(format!(
            "duplicate-name-{}",
            first.0
        )))
        .is_some(),
        "each card shows its file name"
    );

    view.update(vcx, |app, cx| {
        app.edits.edited_ids.insert(first);
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("duplicates-differences-text:2 differences")
            .is_some(),
        "one edited copy adds an Edits difference"
    );
    only(vcx, &["name", "edits"]);

    drop(view);
    let _ = std::fs::remove_dir_all(&temp_dir);
}
