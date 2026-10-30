//! v3 photo viewer: toolbar rating, stage prev/next, floating zoom pill.

use super::ui_automation::{app_with_library, click, render_ui, right_click, seed_library};

#[core::prelude::v1::test]
fn viewer_stage_rates_navigates_and_steps_zoom() {
    let (temp_dir, engine) = seed_library("viewer_stage", 3);

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    let first = view.update(vcx, |app, cx| {
        let first = app.get_active_slice(0..1)[0].clone();
        app.select_item_at(first.clone(), 0, cx);
        app.open_viewer(first.clone(), cx);
        first
    });
    render_ui(vcx);

    right_click(vcx, "viewer-viewport");
    click(vcx, "context-rating-star-3");
    assert_eq!(
        engine.catalog().get_rating(first.id).unwrap(),
        3,
        "the viewer menu's third star must persist a 3-star rating"
    );

    click(vcx, "viewer-next");
    let second = view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(1), "Next must advance to item 2");
        assert!(app.is_fullscreen_viewer, "Next must stay in the viewer");
        app.selected_item.clone().expect("Next must select an item")
    });
    assert_ne!(second.id, first.id, "Next must show a different item");
    click(vcx, "viewer-prev");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(0), "Previous must step back");
    });

    click(vcx, "viewer-zoom-in");
    let zoomed = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(zoomed > 1.0, "+ must zoom in from Fit, got {zoomed}");
    click(vcx, "viewer-zoom-out");
    view.update(vcx, |app, _| {
        assert!(app.viewer_state.zoom_scale < zoomed, "− must zoom back out");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
