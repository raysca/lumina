//! v3 Preferences sheet: grouped rows per tab, and the sheet fits the window.

use super::ui_automation::{app_with_library, click, render_ui, seed_library};

#[core::prelude::v1::test]
fn preferences_tabs_group_actions_and_fit_a_small_window() {
    let (temp_dir, engine) = seed_library("prefs_v3", 2);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(560.0)));
    view.update(vcx, |app, cx| app.open_preferences(cx));
    render_ui(vcx);

    let card = vcx.debug_bounds("preferences-card").expect("sheet renders");
    assert!(
        card.bottom() <= gpui::px(560.0),
        "the Preferences sheet must fit inside a 560px-tall window"
    );
    for selector in [
        "preferences-change-library",
        "preferences-rescan",
        "preferences-choose-editor",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "General must offer {selector}"
        );
    }

    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    click(vcx, "preferences-rescan");
    view.update(vcx, |app, _| {
        assert!(
            app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
            "Rescan Library in Preferences must start a scan"
        );
        assert!(
            app.preferences_open,
            "rescanning must keep Preferences open"
        );
    });

    click(vcx, "preferences-tab-storage");
    for selector in [
        "preferences-clear-cache",
        "preferences-reveal-config",
        "preferences-reveal-logs",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "Storage must offer {selector}"
        );
    }

    click(vcx, "preferences-tab-about");
    assert!(
        vcx.debug_bounds("preferences-version").is_some(),
        "About must show the version"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
