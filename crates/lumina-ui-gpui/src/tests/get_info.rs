//! v3 Get Info window (FIXES §5): title bar, header, General, Camera,
//! Location and Tags tabs, driven in a real floating-inspector window.

use crate::tests::ui_automation::{click, keystrokes, render_ui, seed_library};
use crate::views::floating_inspector::FloatingInspectorView;
use crate::LuminaApp;
use gpui::{Entity, TestAppContext, VisualTestContext};

/// Main app over a seeded library with the first item selected, plus a Get
/// Info window bound to it.
fn get_info<'a>(
    cx: &'a mut TestAppContext,
    prefix: &str,
) -> (
    std::path::PathBuf,
    Entity<LuminaApp>,
    &'a mut VisualTestContext,
) {
    let (temp_dir, engine) = seed_library(prefix, 3);
    let (view, _) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let tx = cx.update(|cx| {
        view.update(cx, |app, cx| {
            app.grid_state = Some(crate::views::grid::GridState::new(engine));
            app.refresh_album_counts();
            app.select_index(0, cx);
            app.ui_tx.clone()
        })
    });
    let (inspector, icx) =
        cx.add_window_view(|_window, cx| FloatingInspectorView::new(view.downgrade(), tx, cx));
    icx.update(|window, cx| {
        let handle = inspector.read(cx).focus_handle.clone();
        window.focus(&handle);
    });
    render_ui(icx);
    (temp_dir, view, icx)
}

fn bounds(icx: &mut VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    icx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} must render"))
}

#[core::prelude::v1::test]
fn title_bar_and_header_follow_v3() {
    let mut cx = TestAppContext::single();
    let (temp_dir, view, icx) = get_info(&mut cx, "get_info_header");

    let bar = bounds(icx, "floating-inspector-titlebar");
    let title = bounds(icx, "floating-inspector-title");
    assert!(
        (f32::from(title.center().x) - f32::from(bar.center().x)).abs() < 2.0,
        "the title is centred on the bar: title={title:?} bar={bar:?}"
    );
    let dock = bounds(icx, "floating-inspector-redock-btn");
    assert_eq!(
        (f32::from(dock.size.width), f32::from(dock.size.height)),
        (24.0, 24.0),
        "Dock is icon-only"
    );
    assert!(
        icx.debug_bounds("floating-inspector-close-btn").is_none(),
        "no extra close button: the traffic lights close the window"
    );

    // Unknown dimensions fill the 132×99 box; a portrait keeps its aspect.
    let thumb = bounds(icx, "info-thumbnail");
    assert_eq!(
        (f32::from(thumb.size.width), f32::from(thumb.size.height)),
        (132.0, 99.0)
    );
    view.update(icx, |app, cx| {
        if let Some(item) = app.selected_item.as_mut() {
            item.width = 3000;
            item.height = 4000;
        }
        cx.notify();
    });
    render_ui(icx);
    let thumb = bounds(icx, "info-thumbnail");
    assert_eq!(f32::from(thumb.size.height), 99.0);
    assert!(
        (f32::from(thumb.size.width) - 74.25).abs() < 0.5,
        "got {thumb:?}"
    );

    let id = view.update(icx, |app, _| app.selected_item.as_ref().unwrap().id);
    click(icx, "info-quick-fav-btn");
    view.update(icx, |app, _| {
        let engine = &app.grid_state.as_ref().unwrap().engine;
        assert!(
            engine.get_item_by_id(id).unwrap().is_favorite,
            "Favorite must persist to the catalog"
        );
    });
    for selector in ["info-quick-copy-path-btn", "info-quick-open-with-btn"] {
        assert_eq!(f32::from(bounds(icx, selector).size.height), 26.0);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn general_tab_shows_dates_and_adjusts_date_taken() {
    let mut cx = TestAppContext::single();
    let (temp_dir, view, icx) = get_info(&mut cx, "get_info_general");

    view.update(icx, |app, _| {
        let details = app
            .info_details
            .as_ref()
            .expect("opening Get Info loads the details");
        assert!(
            details.added_epoch_ms.is_some(),
            "a freshly scanned item has an added date"
        );
    });
    for selector in [
        "info-row-kind",
        "info-row-dimensions",
        "info-row-created",
        "info-row-modified",
        "info-row-added-to-lumina",
    ] {
        let row = bounds(icx, selector);
        assert!(
            f32::from(row.size.height) >= 32.0,
            "{selector} is at least 32px tall"
        );
    }
    assert!(
        icx.debug_bounds("info-row-colour-profile").is_none(),
        "no colour profile row without one in the file (no guessing)"
    );

    click(icx, "inspector-date-adjust");
    view.update(icx, |app, _| {
        assert!(
            app.caption.date_input_active,
            "Adjust… opens the date editor"
        );
    });
    assert!(icx.debug_bounds("inspector-date-box").is_some());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn camera_tab_has_summary_strip_and_only_known_rows() {
    let mut cx = TestAppContext::single();
    let (temp_dir, view, icx) = get_info(&mut cx, "get_info_camera");
    view.update(icx, |app, cx| {
        if let Some(item) = app.selected_item.as_mut() {
            item.make = Some("FUJIFILM".into());
            item.model = Some("X100V".into());
            item.f_number = Some(2.0);
            item.exposure_time = Some("1/250".into());
            item.iso = Some(160);
            item.focal_length = Some(23.0);
        }
        if let Some(details) = app.info_details.as_mut() {
            details.file.exposure_bias_ev = Some(-0.7);
            details.file.focal_length_35mm = Some(35);
            details.file.white_balance = Some("Auto white balance".into());
        }
        cx.notify();
    });
    click(icx, "inspector-tab-Camera");

    assert!(icx.debug_bounds("camera-summary-strip").is_some());
    for selector in [
        "info-row-make",
        "info-row-model",
        "info-row-focal-length",
        "info-row-exposure",
        "info-row-exposure-bias",
        "info-row-white-balance",
    ] {
        assert!(
            icx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }
    for absent in ["info-row-metering", "info-row-flash", "info-row-software"] {
        assert!(
            icx.debug_bounds(absent).is_none(),
            "{absent} is hidden when the file does not record it"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn location_tab_formats_and_removes_the_location() {
    let mut cx = TestAppContext::single();
    let (temp_dir, view, icx) = get_info(&mut cx, "get_info_location");
    let id = view.update(icx, |app, cx| {
        let id = app.selected_item.as_ref().unwrap().id;
        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        engine
            .set_location(id, Some(38.7101), Some(-9.1366))
            .expect("seeding a location");
        let mut item = engine.get_item_by_id(id).unwrap();
        item.city = Some("Lisbon".into());
        item.altitude = Some(12.0);
        app.selected_item = Some(item);
        cx.notify();
        id
    });
    click(icx, "inspector-tab-Location");
    for selector in [
        "info-row-place",
        "info-row-coordinates",
        "info-row-altitude",
    ] {
        assert!(
            icx.debug_bounds(selector).is_some(),
            "{selector} must render"
        );
    }
    assert!(icx.debug_bounds("inspector-location-nearby").is_some());

    click(icx, "inspector-location-edit");
    view.update(icx, |app, _| {
        assert!(
            app.caption.location_input_active,
            "Edit location… opens the editor"
        );
    });
    keystrokes(icx, "escape");

    click(icx, "inspector-location-remove");
    view.update(icx, |app, _| {
        let engine = &app.grid_state.as_ref().unwrap().engine;
        let stored = engine.get_item_by_id(id).unwrap();
        assert_eq!(
            (stored.latitude, stored.longitude),
            (None, None),
            "Remove location must clear the stored coordinates"
        );
        assert_eq!(
            (stored.city, stored.country_code),
            (None, None),
            "…and the place derived from them"
        );
        assert!(app.error_message.is_none(), "removal must not error");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn tags_tab_adds_with_enter_and_suggests_library_tags() {
    let mut cx = TestAppContext::single();
    let (temp_dir, view, icx) = get_info(&mut cx, "get_info_tags");
    view.update(icx, |app, cx| {
        app.library_tag_counts = vec![("travel".into(), 3), ("trees".into(), 1)];
        app.selected_item_tags = vec!["travel".into()];
        cx.notify();
    });
    click(icx, "inspector-tab-Tags");

    assert!(
        icx.debug_bounds("tag-input").is_some(),
        "the ADD field is always visible"
    );
    assert!(icx.debug_bounds("suggest-tag-trees").is_some());
    assert!(
        icx.debug_bounds("suggest-tag-travel").is_none(),
        "tags already on the item are not suggested"
    );

    click(icx, "tag-input");
    keystrokes(icx, "s u n s e t enter");
    let id = view.update(icx, |app, _| app.selected_item.as_ref().unwrap().id);
    view.update(icx, |app, _| {
        let tags = app
            .grid_state
            .as_ref()
            .unwrap()
            .engine
            .get_tags_for_item(id);
        assert!(
            tags.iter().any(|t| t == "sunset"),
            "Enter in the ADD field must add the tag, got {tags:?}"
        );
    });

    click(icx, "open-tag-manager-link");
    view.update(icx, |app, _| {
        assert!(app.tag_manager.open, "the link opens the Tag Manager");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn get_info_actions_have_a_keyboard_path() {
    use crate::app::command_palette::CommandId;
    use crate::views::inspector::InspectorTab;

    let mut cx = TestAppContext::single();
    let (temp_dir, view, icx) = get_info(&mut cx, "get_info_palette");
    let id = view.update(icx, |app, cx| {
        app.execute_command(CommandId::AdjustDate, cx);
        assert!(
            app.caption.date_input_active,
            "⌘K Adjust Date Taken… opens the editor"
        );
        assert_eq!(app.inspector_tab, InspectorTab::Info);
        app.cancel_date_input(cx);

        app.execute_command(CommandId::EditLocation, cx);
        assert!(app.caption.location_input_active);
        assert_eq!(app.inspector_tab, InspectorTab::Location);
        app.cancel_location_input(cx);

        let id = app.selected_item.as_ref().unwrap().id;
        let engine = app.grid_state.as_ref().unwrap().engine.clone();
        engine
            .set_location(id, Some(48.8566), Some(2.3522))
            .unwrap();
        app.selected_item = engine.get_item_by_id(id);
        app.execute_command(CommandId::RemoveLocation, cx);

        app.execute_command(CommandId::OpenTagManager, cx);
        assert!(app.tag_manager.open);
        id
    });
    view.update(icx, |app, _| {
        let stored = app
            .grid_state
            .as_ref()
            .unwrap()
            .engine
            .get_item_by_id(id)
            .unwrap();
        assert_eq!(stored.latitude, None, "⌘K Remove Location clears it");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn cached_details_never_cross_libraries() {
    use crate::app::info_details::InfoDetails;
    let item = crate::app::types::get_sample_items()[0].clone();
    let details = InfoDetails {
        id: item.id,
        path: item.path.clone(),
        added_epoch_ms: Some(1),
        file: Default::default(),
    };
    assert!(details.is_for(&item));
    let mut same_id_other_library = item.clone();
    same_id_other_library.path = "/other/library/photo.jpg".into();
    assert!(
        !details.is_for(&same_id_other_library),
        "ids restart per catalog: the path must match too"
    );
}
