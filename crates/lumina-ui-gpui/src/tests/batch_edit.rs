//! Batch inspector edits driven from the keyboard.

use super::helpers::{app_with_library, keystrokes, render_ui, seed_library, type_text};

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
