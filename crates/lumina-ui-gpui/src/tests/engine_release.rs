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
