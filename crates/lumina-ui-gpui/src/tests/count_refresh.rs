//! Sidebar count refreshes driven by engine events: one per event, however
//! many items it names, and never a hashing scan on the UI thread.

use crate::app::duplicates_review::DUPLICATE_SCANS;
use crate::app::sidebar_counts::COUNT_FETCHES;
use crate::app::types::UiMessage;
use crate::tests::helpers::*;
use crate::views::sidebar::AlbumSelection;

#[core::prelude::v1::test]
fn a_removal_of_many_items_refreshes_the_counts_once() {
    let (_dir, engine) = seed_library("count_refresh_once", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids: Vec<_> = engine
        .get_item_slice(0..6)
        .into_iter()
        .map(|item| item.id)
        .collect();
    assert_eq!(ids.len(), 6, "the seeded library has six items");
    engine
        .permanently_delete_items(&ids, lumina_core::DeletionTarget::CatalogOnly)
        .expect("catalog delete");

    let before = COUNT_FETCHES.with(|n| n.get());
    view.update(vcx, |app, cx| {
        app.handle_ui_message(
            UiMessage::Engine(lumina_core::EngineEvent::ItemsRemoved(ids.clone())),
            cx,
        );
        assert_eq!(
            COUNT_FETCHES.with(|n| n.get()) - before,
            1,
            "one removal event must refresh the counts once, not once per item"
        );
        assert_eq!(app.total_library_count, 0, "the refresh sees the delete");
    });
}

#[core::prelude::v1::test]
fn a_count_refresh_in_duplicates_reads_cached_groups_and_hashes_in_the_background() {
    let (_dir, engine) = seed_library("count_refresh_duplicates", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    click(vcx, "sidebar-nav-duplicates");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, AlbumSelection::Duplicates);
        let before = DUPLICATE_SCANS.with(|n| n.get());
        app.refresh_album_counts();
        assert_eq!(
            DUPLICATE_SCANS.with(|n| n.get()) - before,
            1,
            "the count refresh hands hashing to the duplicate scan (its own thread \
             in the app), never scan_duplicate_groups on the calling thread"
        );
        assert_eq!(app.duplicate_groups().len(), 1, "the group is still shown");
    });
}

#[core::prelude::v1::test]
fn background_requests_made_during_a_fetch_fold_into_one_more_fetch() {
    let (_dir, engine) = seed_library("count_refresh_fold", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let before = COUNT_FETCHES.with(|n| n.get());
    view.update(vcx, |app, cx| {
        for _ in 0..5 {
            app.refresh_album_counts_in_background(cx);
        }
    });
    vcx.run_until_parked();
    assert_eq!(
        COUNT_FETCHES.with(|n| n.get()) - before,
        2,
        "five requests while one fetch runs must cost that fetch plus one more"
    );
}

#[core::prelude::v1::test]
fn a_background_fetch_never_undoes_a_write_made_while_it_ran() {
    let (_dir, engine) = seed_library("count_refresh_stale", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let id = engine.get_item_slice(0..1)[0].id;
    let before = COUNT_FETCHES.with(|n| n.get());
    view.update(vcx, |app, cx| {
        app.refresh_album_counts_in_background(cx);
        // A pick on the UI thread while the fetch is in flight.
        engine
            .set_flags(&[(id, lumina_core::Flag::Pick)])
            .expect("flag write");
        app.item_flags.insert(id, lumina_core::Flag::Pick);
    });
    vcx.run_until_parked();
    assert_eq!(
        COUNT_FETCHES.with(|n| n.get()) - before,
        2,
        "a fetch that started before this thread wrote is fetched again"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.item_flags.get(&id),
            Some(&lumina_core::Flag::Pick),
            "the pick survives the background refresh"
        );
    });
}

#[core::prelude::v1::test]
fn a_library_switch_during_a_fetch_still_refreshes_the_new_library() {
    let (_dir, engine) = seed_library("count_refresh_switch", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let before = COUNT_FETCHES.with(|n| n.get());
    view.update(vcx, |app, cx| {
        app.refresh_album_counts_in_background(cx);
        // As opening another library does; its first events ask again.
        app.library_generation += 1;
        app.refresh_album_counts_in_background(cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        COUNT_FETCHES.with(|n| n.get()) - before,
        2,
        "the old library's answer is dropped and the new library is fetched"
    );
}
