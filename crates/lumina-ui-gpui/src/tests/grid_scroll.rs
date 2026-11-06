//! The virtualized grid settles after a deep scroll.

use super::ui_automation::{app_with_library, render_ui, seed_library};

/// Regression: `uniform_list` also renders row 0 on every layout just to
/// measure it. That call used to move the fetch window and the month band
/// back to row 0, so a scroll past the first fetch window (16 rows at 4
/// columns) swapped them every frame and the window never stopped redrawing.
/// Without the fix this test never parks (it hangs rather than failing an
/// assertion).
#[core::prelude::v1::test]
fn deep_scroll_settles_and_keeps_the_visible_rows() {
    let (dir, engine) = seed_library("grid_deep_scroll", 200);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.grid_columns = 4;
        cx.notify();
    });
    render_ui(vcx);
    let target = view.update(vcx, |app, _| {
        let item = app.get_active_slice(160..161).into_iter().next();
        app.grid_state
            .as_ref()
            .unwrap()
            .scroll_handle
            .scroll_to_item_strict(40, gpui::ScrollStrategy::Top);
        item.expect("the scan must produce 200 items")
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.grid_top_row(), 40, "the scroll held at row 40");
        let top = app.displayed_rows(160..161);
        assert_eq!(
            top.first().map(|item| item.id),
            Some(target.id),
            "the fetch window must stay on the visible rows, not jump back to row 0"
        );
        assert_eq!(
            app.month_header.top,
            Some((target.year, target.month)),
            "the month band must follow the top visible row"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}
