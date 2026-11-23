//! Sidebar count refreshes driven by engine events: one per event, however
//! many items it names, and never a hashing scan on the UI thread.

use crate::app::duplicates_review::DUPLICATE_REFRESHES;
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
        let before = DUPLICATE_REFRESHES.with(|n| n.get());
        app.refresh_album_counts();
        assert_eq!(
            DUPLICATE_REFRESHES.with(|n| n.get()) - before,
            1,
            "the count refresh must go through refresh_duplicates (cached groups now, \
             hashing off the UI thread), never hash on the calling thread"
        );
        assert_eq!(app.duplicate_groups().len(), 1, "the group is still shown");
    });
}
