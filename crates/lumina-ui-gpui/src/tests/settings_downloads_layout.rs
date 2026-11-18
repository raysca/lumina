//! Settings › Downloads at the minimum window (960×600): the path row stays
//! inside the window and keeps its controls visible, like the Library,
//! General and Storage rows (`tests::preferences::long_library_path_truncates_inside_its_row`).

use super::helpers::{app_with_library, render_ui, seed_library};
use crate::app::preferences::PreferencesTab;

#[core::prelude::v1::test]
fn downloads_path_row_fits_the_minimum_window() {
    let (_dir, engine) = seed_library("settings_downloads_960", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    // The window's minimum size (main.rs).
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    view.update(vcx, |app, cx| {
        app.open_preferences_at(PreferencesTab::Downloads, cx)
    });
    render_ui(vcx);

    let window = vcx.update(|window, _| window.viewport_size());
    let row = vcx
        .debug_bounds("settings-row-downloads-path")
        .expect("the Downloads path row renders with a library open");
    assert!(
        row.left() >= gpui::px(0.0) && row.right() <= window.width && row.bottom() <= window.height,
        "at 960×600 the Downloads path row stays inside the {window:?} window, got {row:?}"
    );
    let pill = vcx
        .debug_bounds("settings-downloads-path")
        .expect("the path pill renders");
    let button = vcx
        .debug_bounds("settings-downloads-reveal")
        .expect("the reveal button renders");
    for (name, control) in [("path pill", pill), ("reveal button", button)] {
        assert!(
            control.left() >= row.left() + gpui::px(240.0)
                && control.right() <= row.right()
                && control.top() >= row.top()
                && control.bottom() <= row.bottom(),
            "at 960×600 the {name} {control:?} stays inside its row {row:?}, right of the label"
        );
        assert_eq!(
            f32::from(control.size.height),
            34.0,
            "the {name} never wraps"
        );
    }
    assert!(
        pill.right() <= button.left() && f32::from(pill.size.width) > 0.0,
        "the pill (truncating if need be) ends before the button: {pill:?}, {button:?}"
    );
}
