//! Engines must be released when their app or library goes away: each one
//! holds a decoder pool, a watcher and a catalog connection.

use super::helpers::{app_with_library, seed_library};
use super::TestApp;
use crate::LuminaApp;

/// Regression: the UI test binary once kept every test's engine alive and ran
/// out of threads (`kern.num_taskthreads`) part way through the suite.
#[core::prelude::v1::test]
fn ending_a_test_app_releases_its_engine() {
    let (_temp_dir, engine) = seed_library("release_app", 2);
    let weak = std::sync::Arc::downgrade(&engine);
    {
        let mut cx = TestApp::single();
        let (_view, _vcx) = app_with_library(&mut cx, engine);
        assert!(weak.upgrade().is_some(), "the open app holds its engine");
    }
    assert_eq!(
        weak.strong_count(),
        0,
        "a finished test app must drop its window and with it the engine"
    );
}

/// The engines `open_engine` opened for `path`.
fn opened_for(path: &std::path::Path) -> Vec<std::sync::Weak<lumina_core::LuminaEngine>> {
    crate::app::library::OPENED_ENGINES
        .lock()
        .iter()
        .filter(|(p, _)| p == path)
        .map(|(_, weak)| weak.clone())
        .collect()
}

/// An open superseded before it lands (Back, or another library chosen)
/// must drop the engine it opened, not leave its threads running.
#[core::prelude::v1::test]
fn a_superseded_open_drops_its_engine() {
    let (_current, engine) = seed_library("release_current", 1);
    let (backed_out, _) = seed_library("release_backed_out", 2);
    let (replaced, _) = seed_library("release_replaced", 2);
    let (chosen, _) = seed_library("release_chosen", 3);
    let mut cx = TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    drop(engine);

    view.update(vcx, |app, cx| {
        app.open_library(backed_out.to_path_buf(), cx);
        // Back (Esc) while the engine opens.
        app.leave_missing_library(cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.grid_state.as_ref().map(|s| s.engine.total_count()),
            Some(1),
            "Back keeps the current library"
        );
        app.open_library(replaced.to_path_buf(), cx);
        app.open_library(chosen.to_path_buf(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_state.as_ref().map(|s| s.engine.total_count()),
            Some(3),
            "the latest open wins"
        );
    });
    for (path, why) in [(&backed_out, "Back"), (&replaced, "a later open")] {
        let opened = opened_for(path);
        assert_eq!(opened.len(), 1, "control: the open ran for {why}");
        assert_eq!(
            opened[0].strong_count(),
            0,
            "the engine of an open superseded by {why} is dropped"
        );
    }
}

/// Switching libraries must drop the old engine, or every switch would keep
/// its threads running.
#[core::prelude::v1::test]
fn switching_libraries_releases_the_old_engine() {
    let (first, _) = seed_library("release_first", 2);
    let (second, _) = seed_library("release_second", 2);
    let mut cx = TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| app.open_library_inner(first.clone(), cx));
    vcx.run_until_parked();
    vcx.run_until_parked();
    let weak = view.read_with(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("first library opened");
        std::sync::Arc::downgrade(&state.engine)
    });
    view.update(vcx, |app, cx| app.open_library_inner(second.clone(), cx));
    vcx.run_until_parked();
    vcx.run_until_parked();
    assert_eq!(
        weak.strong_count(),
        0,
        "the previous library's engine must be dropped on a switch"
    );
}

/// Regression: opening the same folder twice in quick succession opened
/// two engines on one catalog; the second open is now ignored.
#[core::prelude::v1::test]
fn opening_the_same_folder_twice_opens_one_engine() {
    let (_current, engine) = seed_library("release_twice_current", 1);
    let (twice, _) = seed_library("release_twice", 2);
    let mut cx = TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.open_library(twice.to_path_buf(), cx);
        app.open_library(twice.to_path_buf(), cx);
    });
    vcx.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(
            app.grid_state.as_ref().map(|s| s.engine.total_count()),
            Some(2),
            "control: the folder opened"
        );
    });
    assert_eq!(
        opened_for(&twice).len(),
        1,
        "a second open of the folder already opening must not open another engine"
    );
}
