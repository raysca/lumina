//! Batch inspector edits driven from the keyboard.

use super::helpers::{app_with_library, keystrokes, render_ui, seed_library, type_text};
use crate::tests::helpers::*;
use crate::*;

#[core::prelude::v1::test]
fn batch_date_shift_moves_each_item_by_delta() {
    let (_dir, engine) = seed_library("batch_date_shift", 3);
    let ids: Vec<_> = engine
        .get_item_slice(0..3)
        .iter()
        .map(|item| item.id)
        .collect();
    // Three different times, minutes to days apart.
    let before = [1_700_000_000_000_i64, 1_700_000_600_000, 1_700_090_000_000];
    let seeded: Vec<_> = ids.iter().copied().zip(before).collect();
    engine.set_timestamps(&seeded).unwrap();
    let times = || -> Vec<i64> {
        ids.iter()
            .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
            .collect()
    };

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Keyboard only: select all, ⌘K Adjust Date Taken…, type a shift.
    keystrokes(vcx, "cmd-a");
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "adjust date");
    keystrokes(vcx, "enter");
    // Adjust Date Taken… opens the field in the Get Info window.
    vcx.run_until_parked();
    let handle = view
        .update(vcx, |app, _| {
            assert_eq!(app.selected_indices.len(), 3, "⌘A selects every item");
            assert!(
                app.caption.date_input_active,
                "Adjust Date Taken… opens the batch DATE field"
            );
            app.floating_inspector_window
        })
        .expect("Adjust Date Taken… opens the Get Info window");
    let mut info = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    render_ui(&mut info);
    type_text(&mut info, "+2h");
    keystrokes(&mut info, "enter");
    vcx.run_until_parked();

    let two_hours = 2 * 3_600_000;
    assert_eq!(
        times(),
        before.map(|ts| ts + two_hours).to_vec(),
        "a shift moves each item by the same delta, keeping their spacing"
    );
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
        assert_eq!(app.undo.undo_stack.len(), 1, "the shift is one undo step");
    });

    keystrokes(vcx, "cmd-z");
    vcx.run_until_parked();
    assert_eq!(times(), before.to_vec(), "one ⌘Z restores every item");
}

#[core::prelude::v1::test]
fn single_item_date_shift_replaces_the_prefilled_date() {
    let (_dir, engine) = seed_library("single_date_shift", 2);
    let id = engine.get_item_slice(0..1)[0].id;
    let before = 1_700_000_000_000_i64;
    engine.set_timestamps(&[(id, before)]).unwrap();
    let time = || engine.get_item_by_id(id).unwrap().timestamp_epoch_ms;

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Keyboard only: select the first item, ⌘K Adjust Date Taken…, type a
    // shift over the prefilled date (the field opens with it selected).
    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "adjust date");
    keystrokes(vcx, "enter");
    vcx.run_until_parked();
    let handle = view
        .update(vcx, |app, _| {
            assert_eq!(app.selected_indices.len(), 1, "one item is selected");
            assert_eq!(app.selected_item.as_ref().map(|i| i.id), Some(id));
            assert!(
                !app.caption.date_input_buffer.is_empty(),
                "the single-item field opens prefilled with the date"
            );
            app.floating_inspector_window
        })
        .expect("Adjust Date Taken… opens the Get Info window");
    let mut info = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    render_ui(&mut info);
    type_text(&mut info, "+2h");
    keystrokes(&mut info, "enter");
    vcx.run_until_parked();

    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "{:?}", app.error_message);
    });
    assert_eq!(
        time(),
        before + 2 * 3_600_000,
        "typing a shift replaces the prefilled date and moves the item"
    );
    keystrokes(vcx, "cmd-z");
    vcx.run_until_parked();
    assert_eq!(time(), before, "⌘Z restores the date");
}

fn test_epoch_ms(value: &str) -> i64 {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .expect("test fixture timestamps use the editor syntax")
        .and_utc()
        .timestamp_millis()
}

/// Selects items by identity so a rank-changing edit (for example a date
/// rewrite that reorders the grid) cannot silently move the selection to other
/// media before the next step of a test.
fn select_media_ids(app: &mut LuminaApp, ids: &[lumina_core::MediaId]) {
    let order = app.get_active_slice(0..app.get_active_count());
    let mut indices = std::collections::BTreeSet::new();
    for id in ids {
        if let Some(index) = order.iter().position(|item| item.id == *id) {
            indices.insert(index);
        }
    }
    let Some(&first_index) = indices.iter().next() else {
        return;
    };
    app.selected_indices = indices;
    app.selected_index = Some(first_index);
    app.selected_item = order.get(first_index).cloned();
    app.update_batch_summary();
}

#[core::prelude::v1::test]
fn ui_automation_batch_title_and_caption_mixed_apply_and_grouped_undo() {
    let (_temp_dir, engine) = seed_library("batch_meta", 4);
    let items = engine.get_item_slice(0..4);
    let first = items[0].clone();
    let ids: Vec<_> = items[0..3].iter().map(|item| item.id).collect();
    engine.set_title(first.id, Some("First")).unwrap();
    engine.set_caption(first.id, "First caption").unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    view.update(vcx, |app, cx| {
        // Test harnesses wire grid_state directly, so load the metadata caches
        // the real open path fills.
        app.refresh_after_history(cx);
        app.selected_indices = [0usize, 1, 2].into_iter().collect();
        app.selected_index = Some(0);
        app.selected_item = Some(first.clone());
        app.update_batch_summary();
        cx.notify();
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        assert!(
            app.batch_metadata.title.mixed,
            "differing titles must be flagged mixed"
        );
        assert_eq!(app.batch_metadata.title.common, None);
        assert!(
            app.batch_metadata.caption.mixed,
            "differing captions must be flagged mixed"
        );
        assert_eq!(
            app.caption.title_input_buffer, "Mixed",
            "the batch inspector shows Mixed for differing titles"
        );
        assert_eq!(app.caption.input_buffer, "Mixed");

        app.start_title_input(cx);
        assert_eq!(
            app.caption.title_input_buffer, "",
            "a mixed draft starts empty so an unchanged Enter cannot overwrite N items"
        );
        app.caption.title_input_buffer = "Batch Title".to_string();
        app.commit_title_input(cx);
    });

    let titles: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().title)
        .collect();
    assert_eq!(
        titles,
        vec![Some("Batch Title".to_string()); 3],
        "the batch title must apply to every selected item"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "applying to N items must create exactly one undo entry"
        );
        assert!(
            matches!(
                &app.undo.undo_stack[0],
                crate::app::undo::UndoAction::BatchMetadata {
                    field: crate::app::undo::BatchField::Title,
                    ..
                }
            ),
            "the entry must be the grouped batch variant"
        );
        assert!(!app.batch_metadata.title.mixed);
        assert_eq!(
            app.batch_metadata.title.common.as_deref(),
            Some("Batch Title"),
            "the summary refreshes after the apply"
        );
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    let restored: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().title)
        .collect();
    assert_eq!(
        restored,
        vec![Some("First".to_string()), None, None],
        "one Cmd+Z must restore every item touched by the batch"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty());
        assert!(
            app.batch_metadata.title.mixed,
            "undo refreshes the mixed-value summary"
        );
    });

    // Same contract for captions, with the optimistic map reverted on undo.
    view.update(vcx, |app, cx| {
        app.start_caption_input(cx);
        assert_eq!(app.caption.input_buffer, "", "mixed captions start empty");
        app.caption.input_buffer = "Shared caption".to_string();
        app.commit_caption_input(cx);
    });
    view.update(vcx, |app, _| {
        for id in &ids {
            assert_eq!(
                app.item_captions.get(id).map(String::as_str),
                Some("Shared caption")
            );
        }
        assert_eq!(app.undo.undo_stack.len(), 1);
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_captions.get(&first.id).map(String::as_str),
            Some("First caption")
        );
        assert!(!app.item_captions.contains_key(&ids[1]));
        assert!(!app.item_captions.contains_key(&ids[2]));
    });
}

#[core::prelude::v1::test]
fn ui_automation_batch_date_mixed_apply_validation_and_grouped_undo() {
    let (_temp_dir, engine) = seed_library("batch_date", 4);
    let dates = [
        test_epoch_ms("2020-01-02 03:04:05"),
        test_epoch_ms("2021-02-03 04:05:06"),
        test_epoch_ms("2022-03-04 05:06:07"),
        test_epoch_ms("2023-05-06 07:08:09"),
    ];
    for (item, date) in engine.get_item_slice(0..4).iter().zip(dates) {
        engine.set_timestamp(item.id, date).unwrap();
    }

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Derive the selection from the app's own sorted view: changing timestamps
    // reorders the grid, so raw catalog order is not the rendered order.
    let (ids, originals): (Vec<_>, Vec<_>) = view.update(vcx, |app, cx| {
        let batch = app.get_active_slice(0..3);
        app.refresh_after_history(cx);
        app.selected_indices = (0..batch.len()).collect();
        app.selected_index = Some(0);
        app.selected_item = Some(batch[0].clone());
        app.update_batch_summary();
        cx.notify();
        (
            batch.iter().map(|item| item.id).collect(),
            batch.iter().map(|item| item.timestamp_epoch_ms).collect(),
        )
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        assert!(
            app.batch_metadata.date.mixed,
            "differing capture dates must be flagged mixed"
        );
        assert_eq!(app.batch_metadata.date.common, None);
        assert_eq!(
            app.caption.date_input_buffer, "Mixed",
            "the batch inspector shows Mixed for differing dates"
        );

        // Invalid syntax must fail validation without touching any item.
        app.start_date_input(cx);
        app.caption.date_input_buffer = "not-a-date".to_string();
        app.commit_date_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .unwrap_or("")
                .contains("Enter a date"),
            "invalid date must surface a validation error, got {:?}",
            app.error_message
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a rejected draft must not push an undo entry"
        );
    });
    let unchanged: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(
        unchanged, originals,
        "an invalid draft must not change any timestamp"
    );

    // A Mixed draft opens empty, so committing it untouched is a no-op.
    view.update(vcx, |app, cx| {
        app.dismiss_error(cx);
        app.start_date_input(cx);
        assert_eq!(
            app.caption.date_input_buffer, "",
            "a mixed draft starts empty so an unchanged Enter cannot rewrite N items"
        );
        app.commit_date_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "an untouched mixed draft must be a no-op"
        );
    });
    let still_unchanged: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(still_unchanged, originals);

    view.update(vcx, |app, cx| {
        app.start_date_input(cx);
        app.caption.date_input_buffer = "2023-06-07 08:09:10".to_string();
        app.commit_date_input(cx);
    });
    let applied: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(
        applied,
        vec![test_epoch_ms("2023-06-07 08:09:10"); ids.len()],
        "the batch date must apply to every selected item"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "applying to N items must create exactly one undo entry"
        );
        assert!(
            matches!(
                &app.undo.undo_stack[0],
                crate::app::undo::UndoAction::BatchDate { .. }
            ),
            "the entry must be the grouped batch date variant"
        );
        assert!(!app.batch_metadata.date.mixed);
        assert_eq!(
            app.batch_metadata.date.common.as_deref(),
            Some("2023-06-07 08:09:10")
        );
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    let restored: Vec<_> = ids
        .iter()
        .map(|id| engine.get_item_by_id(*id).unwrap().timestamp_epoch_ms)
        .collect();
    assert_eq!(
        restored, originals,
        "one Cmd+Z must restore every date touched by the batch"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty());
        assert!(
            app.batch_metadata.date.mixed,
            "undo refreshes the mixed-value summary"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_batch_location_mixed_apply_validation_and_grouped_undo() {
    let (_temp_dir, engine) = seed_library("batch_location", 4);
    let locations = [
        (Some(48.8566), Some(2.3522)),
        (Some(35.6762), Some(139.6503)),
        (None, None),
        (Some(51.5074), Some(-0.1278)),
    ];
    for (item, (lat, lon)) in engine.get_item_slice(0..4).iter().zip(locations) {
        engine.set_location(item.id, lat, lon).unwrap();
    }

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Read the selection from the rendered (sorted) view so the assertions do
    // not depend on raw catalog order.
    let (ids, originals): (Vec<_>, Vec<_>) = view.update(vcx, |app, cx| {
        let batch = app.get_active_slice(0..3);
        app.refresh_after_history(cx);
        app.selected_indices = (0..batch.len()).collect();
        app.selected_index = Some(0);
        app.selected_item = Some(batch[0].clone());
        app.update_batch_summary();
        cx.notify();
        (
            batch.iter().map(|item| item.id).collect(),
            batch
                .iter()
                .map(|item| (item.latitude, item.longitude))
                .collect(),
        )
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| {
        assert!(
            app.batch_metadata.location.mixed,
            "differing coordinates (including none) must be flagged mixed"
        );
        assert_eq!(
            app.caption.location_input_buffer, "Mixed",
            "the batch inspector shows Mixed for differing locations"
        );

        app.start_location_input(cx);
        assert_eq!(
            app.caption.location_input_buffer, "",
            "a mixed location draft starts empty"
        );
        app.commit_location_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.undo.undo_stack.is_empty(),
            "an untouched mixed location draft must be a no-op"
        );
    });

    view.update(vcx, |app, cx| {
        app.start_location_input(cx);
        app.caption.location_input_buffer = "not coordinates".to_string();
        app.commit_location_input(cx);
    });
    view.update(vcx, |app, _| {
        assert!(
            app.error_message
                .as_deref()
                .unwrap_or("")
                .contains("Coordinates"),
            "invalid coordinates must surface a validation error, got {:?}",
            app.error_message
        );
        assert!(
            app.undo.undo_stack.is_empty(),
            "a rejected location draft must not push an undo entry"
        );
    });
    let unchanged: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = engine.get_item_by_id(*id).unwrap();
            (item.latitude, item.longitude)
        })
        .collect();
    assert_eq!(
        unchanged, originals,
        "an invalid draft must not change any location"
    );

    let target = (40.7128, -74.0060);
    view.update(vcx, |app, cx| {
        app.dismiss_error(cx);
        app.start_location_input(cx);
        app.caption.location_input_buffer = "40.7128, -74.0060".to_string();
        app.commit_location_input(cx);
    });
    let applied: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = engine.get_item_by_id(*id).unwrap();
            (item.latitude, item.longitude)
        })
        .collect();
    assert_eq!(
        applied,
        vec![(Some(target.0), Some(target.1)); ids.len()],
        "the batch location must apply to every selected item"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.undo.undo_stack.len(),
            1,
            "applying to N items must create exactly one undo entry"
        );
        assert!(
            matches!(
                &app.undo.undo_stack[0],
                crate::app::undo::UndoAction::BatchLocation { .. }
            ),
            "the entry must be the grouped batch location variant"
        );
        assert!(!app.batch_metadata.location.mixed);
    });

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    let restored: Vec<_> = ids
        .iter()
        .map(|id| {
            let item = engine.get_item_by_id(*id).unwrap();
            (item.latitude, item.longitude)
        })
        .collect();
    assert_eq!(
        restored, originals,
        "one Cmd+Z must restore every location touched by the batch"
    );
    view.update(vcx, |app, _| {
        assert!(app.undo.undo_stack.is_empty());
        assert!(
            app.batch_metadata.location.mixed,
            "undo refreshes the mixed-value summary"
        );
    });
}

#[core::prelude::v1::test]
fn ui_automation_batch_inspector_metadata_edit_ui_path() {
    let (_temp_dir, engine) = seed_library("batch_meta_ui", 3);
    let annotated = engine.get_item_slice(0..1)[0].clone();
    engine.set_title(annotated.id, Some("First")).unwrap();
    engine
        .set_timestamp(annotated.id, test_epoch_ms("2020-01-02 03:04:05"))
        .unwrap();
    engine
        .set_location(annotated.id, Some(48.8566), Some(2.3522))
        .unwrap();

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Only the annotated item differs from its neighbour, so the title, date
    // and location rows all show the Mixed marker.
    let (first, second) = view.update(vcx, |app, cx| {
        app.inspector_open = true;
        let order = app.get_active_slice(0..app.get_active_count());
        let first = order
            .iter()
            .find(|item| item.id == annotated.id)
            .cloned()
            .expect("the annotated item stays in the library");
        let second = order
            .iter()
            .find(|item| item.id != annotated.id)
            .cloned()
            .expect("the seeded library has another item");
        select_media_ids(app, &[first.id, second.id]);
        cx.notify();
        (first, second)
    });
    render_ui(vcx);

    assert!(
        vcx.debug_bounds("batch-title-input").is_some(),
        "the batch inspector must render the title editor"
    );
    assert!(
        vcx.debug_bounds("batch-caption-input").is_some(),
        "the batch inspector must render the caption editor"
    );
    assert!(
        vcx.debug_bounds("batch-title-input-mixed").is_some(),
        "differing titles must show the Mixed marker"
    );
    assert!(
        vcx.debug_bounds("batch-date-input").is_some(),
        "the batch inspector must render the date editor"
    );
    assert!(
        vcx.debug_bounds("batch-location-input").is_some(),
        "the batch inspector must render the location editor"
    );
    assert!(
        vcx.debug_bounds("batch-date-input-mixed").is_some(),
        "differing dates must show the Mixed marker"
    );
    assert!(
        vcx.debug_bounds("batch-location-input-mixed").is_some(),
        "differing locations must show the Mixed marker"
    );
    for row in [
        "batch-title-input-value",
        "batch-date-input-value",
        "batch-location-input-value",
    ] {
        assert!(
            vcx.debug_bounds(row).is_none(),
            "{row}: a mixed row says Mixed once, in its marker, not again as its value"
        );
    }
    assert!(
        vcx.debug_bounds("batch-caption-input-value").is_some(),
        "a shared value still shows in its row"
    );

    focus_app(vcx, &view);
    click(vcx, "batch-title-input");
    view.update(vcx, |app, _| {
        assert!(
            app.caption.title_input_active,
            "clicking the batch title row opens the inline editor"
        );
        assert_eq!(
            app.caption.title_input_buffer, "",
            "mixed draft starts empty"
        );
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("batch-title-input-value").is_some(),
        "editing a mixed row shows its draft"
    );
    type_text(vcx, "shared");
    keystrokes(vcx, "enter");

    view.update(vcx, |app, _| {
        assert!(!app.caption.title_input_active, "Enter closes the editor");
        assert_eq!(
            engine.get_item_by_id(first.id).unwrap().title.as_deref(),
            Some("shared")
        );
        assert_eq!(
            engine.get_item_by_id(second.id).unwrap().title.as_deref(),
            Some("shared")
        );
        assert_eq!(app.undo.undo_stack.len(), 1);
    });

    // Date and location use the same inline buffer path as title/caption, so
    // they must be driveable through the rendered rows and the keyboard.
    click(vcx, "batch-date-input");
    view.update(vcx, |app, _| {
        assert!(
            app.caption.date_input_active,
            "clicking the batch date row opens the inline editor"
        );
    });
    type_text(vcx, "2020-05-06");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        for id in [first.id, second.id] {
            let item = engine.get_item_by_id(id).unwrap();
            assert_eq!(
                (item.year, item.month, item.day),
                (2020, 5, 6),
                "the typed batch date must reach every selected item"
            );
        }
        assert_eq!(app.undo.undo_stack.len(), 2);
    });

    // The date rewrite can reorder the grid; re-anchor the selection by id.
    view.update(vcx, |app, cx| {
        select_media_ids(app, &[first.id, second.id]);
        cx.notify();
    });
    render_ui(vcx);

    click(vcx, "batch-location-input");
    view.update(vcx, |app, _| {
        assert!(
            app.caption.location_input_active,
            "clicking the batch location row opens the inline editor"
        );
    });
    type_text(vcx, "40.7128, -74.0060");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        for id in [first.id, second.id] {
            let item = engine.get_item_by_id(id).unwrap();
            assert_eq!(
                (item.latitude, item.longitude),
                (Some(40.7128), Some(-74.0060)),
                "the typed batch location must reach every selected item"
            );
        }
        assert_eq!(app.undo.undo_stack.len(), 3);
    });
}
