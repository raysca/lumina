//! v3 docked inspector: one-column summary for a single item.

use super::ui_automation::{app_with_library, click, leak, render_ui, seed_library};

#[core::prelude::v1::test]
fn docked_inspector_summary_rates_labels_and_opens_get_info() {
    let (temp_dir, engine) = seed_library("inspector_summary", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let card = leak(format!("media-card-{}", item.id.0));

    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);

    click(vcx, card);
    click(vcx, "inspector-toggle");
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("inspector-summary").is_some(),
        "a single selection must dock the one-column summary"
    );
    assert!(
        vcx.debug_bounds("inspector-filename").is_some(),
        "the summary must name the file"
    );

    click(vcx, "inspector-star-4");
    assert_eq!(
        engine.catalog().get_rating(item.id).unwrap(),
        4,
        "clicking the fourth star must persist a 4-star rating"
    );

    click(vcx, "inspector-label-2");
    let labels = engine.catalog().get_color_labels_map().unwrap();
    assert_eq!(
        labels.get(&item.id).copied(),
        Some(2),
        "clicking a label dot must persist that colour label"
    );

    click(vcx, "inspector-get-info");
    view.update(vcx, |app, _| {
        assert!(
            !app.inspector_open,
            "Get Info must hand off to the floating inspector and close the dock"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}
