//! Grid decodes follow the screen: the tiles drawn and the rows just below
//! are the wanted set, and a queued decode for a tile scrolled past is
//! skipped, its claim released, never recorded as a failure.

use std::time::{Duration, Instant};

use crate::app::state::THUMB_REQUESTS;
use crate::tests::helpers::{app_with_library, render_ui, seed_library};

#[core::prelude::v1::test]
fn a_grid_pass_wants_what_it_draws_and_prefetches() {
    let (_dir, engine) = seed_library("decode_wanted", 400);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("library");
        let wanted = state.wanted.lock().clone();
        let first = app.displayed_rows(0..1);
        assert!(
            first.iter().all(|item| wanted.contains(&item.id)),
            "the first drawn tile is wanted"
        );
        assert!(
            !wanted.is_empty() && wanted.len() < 400,
            "only the screen and the prefetch rows are wanted, not the \
             whole library: {}",
            wanted.len()
        );
    });
}

#[core::prelude::v1::test]
fn a_tile_scrolled_past_is_skipped_and_can_be_asked_again() {
    let (dir, engine) = seed_library("decode_skipped", 3);
    let cached = || files_under(&dir.join("cache"));
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let item = view.update(vcx, |app, _| app.get_active_slice(0..1).remove(0));
    let before = cached();
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("library");
        state.texture_cache.lock().pop(&item.id);
        state.in_flight.lock().remove(&item.id);
        // Scrolled away: the grid no longer wants this tile.
        state.wanted.lock().clear();
        THUMB_REQUESTS.with(|n| n.set(0));
        app.queue_grid_thumbnail(&item);
        // The skip may already have released the claim on the real pool.
        assert_eq!(THUMB_REQUESTS.with(|n| n.get()), 1, "the decode is queued");
    });
    // The decode pool is real; the skip lands on it shortly.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let released = view.update(vcx, |app, _| {
            !app.grid_state
                .as_ref()
                .expect("library")
                .in_flight
                .lock()
                .contains(&item.id)
        });
        if released {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the skipped decode releases its claim"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    view.update(vcx, |app, _| {
        let state = app.grid_state.as_ref().expect("library");
        assert!(
            !state.is_failed(item.id),
            "a skipped tile is not a failed one: it shows a picture when it \
             comes back"
        );
    });
    assert_eq!(
        cached(),
        before,
        "nothing was decoded: a decode writes the thumbnail to the disk cache"
    );
}

/// Files under `dir`, recursively.
fn files_under(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| {
                    if entry.path().is_dir() {
                        files_under(&entry.path())
                    } else {
                        1
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}

#[core::prelude::v1::test]
fn a_one_row_pass_adds_to_the_wanted_set_and_a_full_pass_replaces_it() {
    let (_dir, engine) = seed_library("decode_measure", 40);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, _| {
        let rows = app.get_active_slice(0..3);
        app.set_grid_wanted([rows[1].id, rows[2].id], true);
        // uniform_list's measuring pass at the top looks like a 0..1 pass.
        app.set_grid_wanted([rows[0].id], false);
        let wanted = app
            .grid_state
            .as_ref()
            .expect("library")
            .wanted
            .lock()
            .clone();
        assert!(
            rows.iter().all(|row| wanted.contains(&row.id)),
            "a 0..1 pass must not drop the screen the real pass wants"
        );
        app.set_grid_wanted([rows[0].id], true);
        let wanted = app
            .grid_state
            .as_ref()
            .expect("library")
            .wanted
            .lock()
            .clone();
        assert_eq!(wanted.len(), 1, "a real pass replaces the set");
    });
}
