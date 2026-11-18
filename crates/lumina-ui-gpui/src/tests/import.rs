//! UI automation for import by copy: the dialog, its keys, ⇧⌘I, and the
//! library switch. (`ExternalPaths` cannot be built in tests, so the drop
//! handler's one call, `begin_import`, is driven directly.)

use super::helpers::{
    app_with_library, click, click_point, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::import::ImportDialog;
use std::path::{Path, PathBuf};

/// A folder outside any library with `count` (at most 13) distinct jpegs.
fn source_folder(name: &str, count: u8) -> crate::tests::helpers::TempLibrary {
    let dir = std::env::temp_dir().join(format!(
        "lumina_uitest_import_src_{name}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for seed in 0..count {
        image::RgbImage::from_fn(8, 8, |x, y| {
            image::Rgb([x as u8 * 30, y as u8 * 30, seed * 19])
        })
        .save(dir.join(format!("new_{seed}.jpg")))
        .unwrap();
    }
    crate::tests::helpers::TempLibrary(dir)
}

fn imported_files(library: &Path) -> usize {
    let imports = library.join("Imports");
    if !imports.exists() {
        return 0;
    }
    lumina_core::scanner::walk_files(&imports).count()
}

#[core::prelude::v1::test]
fn import_dialog_flow() {
    let (library, engine) = seed_library("import_flow", 1);
    let source = source_folder("flow", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    view.update(vcx, |app, _| {
        assert!(
            matches!(
                app.import.dialog,
                Some(ImportDialog::Planning { count: 1, .. })
            ),
            "the dialog opens at once, checking the dropped folder"
        )
    });
    render_ui(vcx);
    view.update(vcx, |app, _| match &app.import.dialog {
        Some(ImportDialog::Ready(plan)) => assert_eq!(plan.files.len(), 2),
        _ => panic!("planning finishes with a plan of two files"),
    });
    assert!(vcx.debug_bounds("import-dialog").is_some());

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.import.dialog.is_none(), "Esc closes")
    });
    assert_eq!(imported_files(&library), 0, "Esc copies nothing");

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    keystrokes(vcx, "enter");
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(app.import.dialog.is_none());
        assert_eq!(
            app.import.progress, None,
            "the status bar is back to normal"
        );
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            Some("Imported 2 items")
        );
        assert_eq!(app.total_library_count, 3, "both copies are catalogued");
    });
    assert_eq!(imported_files(&library), 2, "↵ copies into Imports");
    assert_eq!(
        lumina_core::scanner::walk_files(&source).count(),
        2,
        "the originals stay"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn import_dialog_backdrop_and_cancel_close() {
    let (library, engine) = seed_library("import_close", 1);
    let source = source_folder("close", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    click(vcx, "import-cancel");
    view.update(vcx, |app, _| {
        assert!(app.import.dialog.is_none(), "Cancel closes")
    });

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    click(vcx, "import-dialog");
    view.update(vcx, |app, _| {
        assert!(app.import.dialog.is_some(), "a click on the card keeps it")
    });
    click_point(vcx, 4.0, 4.0);
    view.update(vcx, |app, _| {
        assert!(app.import.dialog.is_none(), "the backdrop closes")
    });
    assert_eq!(imported_files(&library), 0);

    drop(view);
}

#[core::prelude::v1::test]
fn import_dialog_absorbs_keys_and_text() {
    let (_library, engine) = seed_library("import_keys", 2);
    let source = source_folder("keys", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    view.update(vcx, |app, _| assert!(app.selected_item.is_some()));

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    keystrokes(vcx, "backspace");
    keystrokes(vcx, "i");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ does not start a delete"
        );
        assert!(!app.inspector_open, "I does not reach the browser");
        assert!(app.text_target().is_none(), "no field takes typing");
        assert!(app.import.dialog.is_some());
    });

    drop(view);
}

#[core::prelude::v1::test]
fn shift_cmd_i_opens_import_not_get_info() {
    use crate::app::import::IMPORT_PICKS;
    let (_library, engine) = seed_library("import_shortcut", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    let before = IMPORT_PICKS.with(|n| n.get());

    keystrokes(vcx, "secondary-shift-i");

    assert_eq!(
        IMPORT_PICKS.with(|n| n.get()),
        before + 1,
        "⇧⌘I asks for folders"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.floating_inspector_window.is_none(),
            "⇧⌘I is not ⌘I: Get Info stays closed"
        )
    });

    drop(view);
}

#[core::prelude::v1::test]
fn library_switch_clears_import_dialog() {
    let (_library, engine) = seed_library("import_switch", 1);
    let (other, _other_engine) = seed_library("import_switch_other", 1);
    let source = source_folder("switch", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    view.update(vcx, |app, cx| app.open_library_inner(other.clone(), cx));
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.import.dialog.is_none(),
            "a plan for one library cannot be confirmed into another"
        )
    });

    drop(view);
}

#[core::prelude::v1::test]
fn nothing_new_shows_ok_only() {
    let (library, engine) = seed_library("import_nothing", 1);
    let source = source_folder("nothing", 0);
    std::fs::write(source.join("notes.txt"), b"not media").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    // Debug bounds outlive their element (Planning's Cancel lingers), so
    // look for a Cancel on the OK button's row.
    let ok = vcx.debug_bounds("import-confirm").expect("an OK button");
    assert!(
        vcx.debug_bounds("import-cancel")
            .is_none_or(|cancel| cancel.center().y != ok.center().y),
        "with nothing to copy the dialog offers only OK"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.import.dialog.is_none(), "OK closes")
    });
    assert_eq!(imported_files(&library), 0);

    drop(view);
}

#[core::prelude::v1::test]
fn import_needs_a_library() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_, cx| crate::LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.begin_import(vec![std::env::temp_dir()], cx);
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            Some("Open a library before importing")
        );
        assert!(app.import.dialog.is_none());
    });
}

#[core::prelude::v1::test]
fn library_switch_stops_a_running_copy() {
    let (_library, engine) = seed_library("import_switch_copy", 1);
    let (other, _other_engine) = seed_library("import_switch_copy_other", 1);
    let source = source_folder("switch_copy", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    // Confirm and switch before the copy task gets to run.
    view.update(vcx, |app, cx| {
        app.confirm_import(cx);
        assert!(app.import.progress.is_some(), "the copy has started");
        app.open_library_inner(other.clone(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            !app.import.is_busy() && app.import.progress.is_none(),
            "the new library shows no import of the old one"
        );
        assert!(
            app.toast
                .current()
                .is_none_or(|toast| !toast.message.starts_with("Import")),
            "the stopped copy reports nothing in the new library"
        );
        assert!(app.import.progress.is_none());
    });

    drop(view);
}

#[core::prelude::v1::test]
fn import_waits_while_a_delete_confirmation_is_open() {
    let (_library, engine) = seed_library("import_vs_delete", 2);
    let source = source_folder("vs_delete", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    keystrokes(vcx, "right");
    keystrokes(vcx, "backspace");
    view.update(vcx, |app, _| assert!(app.delete_confirm_state.is_some()));

    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    view.update(vcx, |app, _| {
        assert!(
            app.import.dialog.is_none(),
            "no import dialog over a delete confirmation, where ↵ would delete"
        );
        assert_eq!(
            app.toast.current().map(|toast| toast.message.as_str()),
            Some("Close the open dialog before importing")
        );
    });

    drop(view);
}

#[core::prelude::v1::test]
fn checking_shows_a_live_count() {
    let (_library, engine) = seed_library("import_checking", 1);
    let source = source_folder("checking", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.begin_import(vec![source.clone()], cx);
        app.handle_ui_message(
            crate::app::types::UiMessage::Engine(lumina_core::EngineEvent::ImportChecking {
                checked: 340,
                total: 1200,
            }),
            cx,
        );
        assert!(
            matches!(
                app.import.dialog,
                Some(ImportDialog::Planning {
                    checked: Some((340, 1200)),
                    ..
                })
            ),
            "the dialog follows the engine's count"
        );
    });
    vcx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    assert!(
        vcx.debug_bounds("import-check-progress").is_some(),
        "a bar shows how far checking has got"
    );

    drop(view);
}

#[core::prelude::v1::test]
fn status_bar_shows_progress_and_cancels() {
    let (_library, engine) = seed_library("import_status", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    view.update(vcx, |app, cx| {
        app.import.pretend_running(3, 10);
        cx.notify();
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.status_right_text(), "Importing… 3 of 10")
    });
    assert!(
        vcx.debug_bounds("import-progress").is_some(),
        "a progress bar"
    );

    click(vcx, "import-cancel-running");
    view.update(vcx, |app, _| {
        assert!(app.import.is_stopping(), "Cancel stops the copy");
        assert_eq!(
            app.status_right_text(),
            "Stopping import…",
            "the bar says it is finishing the current file"
        );
    });

    drop(view);
}

/// Plans `total` files, removes `failing` of them, then imports: the copy
/// of each removed file fails.
fn import_with_failures(name: &str, total: u8, failing: u8) -> Option<String> {
    let (_library, engine) = seed_library(name, 1);
    let source = source_folder(name, total);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| app.begin_import(vec![source.clone()], cx));
    render_ui(vcx);
    for seed in 0..failing {
        std::fs::remove_file(source.join(format!("new_{seed}.jpg"))).unwrap();
    }
    keystrokes(vcx, "enter");
    vcx.run_until_parked();
    let message = view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "the banner carries the result; no toast repeats it: {:?}",
            app.toast.current().map(|t| &t.message)
        );
        app.error_message.clone()
    });
    drop(view);
    message
}

#[core::prelude::v1::test]
fn partial_import_banner_reads_imported_x_of_y() {
    let message = import_with_failures("partial_one", 3, 1).expect("a failure sets the banner");
    let (title, detail) = message.split_once(": ").expect("title and detail");
    assert_eq!(title, "Imported 2 of 3. 1 file failed.");
    assert!(
        detail.starts_with("new_0.jpg: "),
        "the detail names the failed file and its reason: {detail}"
    );

    let message = import_with_failures("partial_many", 13, 12).expect("failures set the banner");
    let (title, detail) = message.split_once(": ").expect("title and detail");
    assert_eq!(
        title, "Imported 1 of 13. 12 files failed.",
        "the count is exact"
    );
    assert_eq!(
        detail.matches("; ").count(),
        2,
        "three names at most: {detail}"
    );
    assert!(
        detail.ends_with(" and 9 more"),
        "the rest are counted: {detail}"
    );
}

#[core::prelude::v1::test]
fn import_stopped_by_a_full_disk_counts_the_files_not_copied() {
    let (_library, engine) = seed_library("disk_full", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // 10 planned; 3 copied, the 4th hit a full disk and the copy stopped,
    // so files 5 to 10 were never tried.
    let report = lumina_core::import::CopyImportReport {
        copied: 3,
        failed: vec![(PathBuf::from("/src/d.jpg"), "No space left".into())],
        cancelled: false,
    };
    view.update(vcx, |app, cx| {
        app.import.progress = Some((4, 10));
        app.finish_import(report, cx);
    });
    let message = view.update(vcx, |app, _| app.error_message.clone());
    let message = message.expect("a stopped import sets the banner");
    let (title, detail) = message.split_once(": ").expect("title and detail");
    assert_eq!(
        title, "Imported 3 of 10. 7 files not copied.",
        "copied plus not copied must add up to the total"
    );
    assert!(detail.starts_with("d.jpg: No space left"), "{detail}");

    drop(view);
}

/// A later error takes the banner's title, but the import's failure line
/// (which files were not copied) stays under it until the banner is
/// dismissed.
#[core::prelude::v1::test]
fn a_later_error_keeps_the_import_failure_line() {
    let (_library, engine) = seed_library("import_then_error", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let report = lumina_core::import::CopyImportReport {
        copied: 1,
        failed: vec![(PathBuf::from("/src/b.jpg"), "Permission denied".into())],
        cancelled: false,
    };
    view.update(vcx, |app, cx| {
        app.import.progress = Some((2, 2));
        app.finish_import(report, cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("error-banner-import").is_none(),
        "the import failure alone is the banner's title and detail, not repeated"
    );

    view.update(vcx, |app, cx| {
        app.set_error("Could not save settings: disk full", cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("error-banner-title").is_some(),
        "the newer error shows"
    );
    let line = vcx
        .debug_bounds("error-banner-import")
        .expect("a later error must not drop the import failure line");
    let detail = vcx
        .debug_bounds("error-banner-detail")
        .expect("the newer error's detail line renders");
    assert!(
        line.top() >= detail.bottom(),
        "the import line sits under the newer error's detail"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.import.failure.as_deref(),
            Some("Imported 1 of 2. 1 file failed.: b.jpg: Permission denied"),
            "the kept line is the import's own message"
        );
    });

    click(vcx, "error-banner-dismiss");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("error-banner").is_none(),
        "Dismiss clears the banner"
    );
    view.update(vcx, |app, cx| app.set_error("Export failed: gone", cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("error-banner-import").is_none(),
        "a dismissed import failure does not come back with the next error"
    );

    drop(view);
}
