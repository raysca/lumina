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

/// Seeded library, window 1440×900, first card selected and Info on.
fn docked<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    lumina_core::MediaItem,
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    let (view, vcx) = app_with_library(cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    click(vcx, leak(format!("media-card-{}", item.id.0)));
    click(vcx, "inspector-toggle");
    render_ui(vcx);
    (dir, engine, item, view, vcx)
}

#[core::prelude::v1::test]
fn keywords_field_adds_and_removes_plain_chips() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, engine, item, view, vcx) = docked("inspector_keywords", &mut cx);

    click(vcx, "keyword-add");
    super::ui_automation::keystrokes(vcx, "b e a c h enter");
    assert_eq!(engine.get_tags_for_item(item.id), vec!["beach".to_string()]);
    render_ui(vcx);
    let chip = vcx
        .debug_bounds("keyword-chip-beach")
        .expect("the chip renders");
    let field = vcx.debug_bounds("inspector-keywords").unwrap();
    assert!(
        field.contains(&chip.origin),
        "chips sit inside the one field"
    );
    assert_eq!(f32::from(chip.size.height), 20.0);

    click(vcx, "keyword-remove-beach");
    assert!(
        engine.get_tags_for_item(item.id).is_empty(),
        "× removes the keyword"
    );
    view.update(vcx, |app, _| assert!(app.error_message.is_none()));

    // Keyboard: ⌫ in the empty field removes the last keyword.
    for word in ["s u n enter", "s e a enter", "backspace"] {
        click(vcx, "keyword-add");
        super::ui_automation::keystrokes(vcx, word);
    }
    let left = engine.get_tags_for_item(item.id);
    assert_eq!(left.len(), 1, "⌫ removes exactly one keyword: {left:?}");
    view.update(vcx, |app, _| {
        assert!(app.tag_input_active, "the field stays open after ⌫");
        assert!(app.delete_confirm_state.is_none(), "⌫ never reaches delete");
    });

    render_ui(vcx);
    click(vcx, leak(format!("keyword-filter-{}", left[0])));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query,
            format!("tag:{}", left[0]),
            "a chip's text filters by its keyword"
        );
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn footer_is_pinned_and_rating_targets_are_22px() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, engine, item, view, vcx) = docked("inspector_footer", &mut cx);

    let summary = vcx.debug_bounds("inspector-summary").unwrap();
    let footer = vcx.debug_bounds("inspector-footer").unwrap();
    assert!(
        (f32::from(summary.bottom()) - f32::from(footer.bottom())).abs() < 1.0,
        "the footer sits at the bottom: {footer:?} in {summary:?}"
    );
    assert_eq!(
        f32::from(summary.size.width) + 1.0,
        300.0,
        "the inspector is 300px with its 1px left border"
    );
    for selector in ["inspector-star-1", "inspector-label-0", "inspector-label-6"] {
        let b = vcx.debug_bounds(selector).unwrap();
        assert_eq!(f32::from(b.size.width), 22.0, "{selector} is a 22px target");
    }

    click(vcx, "inspector-label-3");
    let labels = engine.catalog().get_color_labels_map().unwrap();
    assert_eq!(labels.get(&item.id).copied(), Some(3));
    click(vcx, "inspector-label-0");
    let labels = engine.catalog().get_color_labels_map().unwrap();
    assert_eq!(
        labels.get(&item.id).copied().unwrap_or(0),
        0,
        "No label clears the colour"
    );
    view.update(vcx, |app, _| assert!(app.error_message.is_none()));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn info_with_nothing_selected_keeps_the_panel() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, _engine, _item, view, vcx) = docked("inspector_empty", &mut cx);
    view.update(vcx, |app, cx| app.clear_selection(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| assert!(app.inspector_open, "Info stays on"));
    assert!(
        vcx.debug_bounds("inspector-empty").is_some(),
        "an empty state shows"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn summary_formats_capture_time_place_and_video_strip() {
    let mut item = crate::app::types::get_sample_items()[0].clone();
    item.city = Some("Lisbon".into());
    item.admin1_region = Some("Lisbon".into());
    item.country_code = Some("PT".into());
    assert_eq!(
        crate::views::inspector::summary::place_line(&item).as_deref(),
        Some("Lisbon, PT"),
        "a region equal to the city is not repeated"
    );
    assert_eq!(
        crate::views::inspector::summary::resolution_label(3840, 2160),
        "4K"
    );
    assert_eq!(
        crate::views::inspector::summary::resolution_label(1920, 1080),
        "1080p"
    );
    assert_eq!(
        crate::views::inspector::summary::resolution_label(1000, 700),
        "1000 × 700"
    );
}
