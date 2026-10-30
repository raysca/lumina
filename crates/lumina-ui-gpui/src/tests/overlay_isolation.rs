//! Overlay isolation regressions (BUGS.md 7, 10, 11, 13, 14, 15, 17).
//!
//! These drive rendered clicks against the fullscreen viewer and the tag
//! manager. Before the occlusion fix, one click could run both the overlay
//! control and the toolbar/grid control painted underneath.

use crate::app::types::get_sample_items;
use crate::tests::ui_automation::{
    app_with_library, click, click_point, leak, render_ui, seed_library,
};
use lumina_core::MediaType;

fn open_viewer_with_first_item(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) {
    let item = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    view.update(vcx, |app, cx| {
        // The viewer is always the fullscreen overlay over the grid.
        app.open_viewer(item, cx);
    });
    render_ui(vcx);
}

fn card_ids(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext) -> Vec<u64> {
    view.update(vcx, |app, _| {
        app.get_active_slice(0..app.get_active_count())
            .into_iter()
            .map(|item| item.id.0)
            .collect()
    })
}

fn selected_id(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> Option<u64> {
    view.update(vcx, |app, _| {
        app.selected_item.as_ref().map(|item| item.id.0)
    })
}

#[core::prelude::v1::test]
fn viewer_info_click_does_not_open_toolbar_sort() {
    let (temp_dir, engine) = seed_library("overlay_info", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_viewer_with_first_item(&view, vcx);

    // The toolbar is icon-only now and no longer sits under viewer Info, so
    // click the toolbar's Sort position itself: the viewer must swallow it.
    click(vcx, "sort-menu-button");
    view.update(vcx, |app, _| {
        assert!(
            !app.sort_menu_open,
            "a click over the hidden toolbar must not reach Sort"
        );
    });
    click(vcx, "viewer-info-btn");
    view.update(vcx, |app, _| {
        assert!(
            app.inspector_open,
            "viewer Info must still open the inspector"
        );
        assert!(
            !app.sort_menu_open,
            "the click on viewer Info must not reach the toolbar sort button"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_open_click_does_not_open_tag_manager() {
    let (temp_dir, engine) = seed_library("overlay_open", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_viewer_with_first_item(&view, vcx);

    click(vcx, "tags-toolbar-btn");
    view.update(vcx, |app, _| {
        assert!(
            !app.tag_manager.open,
            "a click over the hidden toolbar must not reach Tags"
        );
    });

    click(vcx, "viewer-quicktime-btn");
    view.update(vcx, |app, _| {
        assert!(
            !app.tag_manager.open,
            "the Open click must not also open the Tag Manager"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn viewer_play_badge_does_not_change_the_selected_media() {
    let (temp_dir, engine) = seed_library("overlay_play", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    let video = get_sample_items()
        .into_iter()
        .find(|item| item.media_type == MediaType::Video)
        .expect("the sample set must contain a video");
    let expected_id = video.id.0;
    view.update(vcx, |app, cx| {
        // Two columns of large cards so one sits under the stage centre
        // regardless of small viewer layout changes.
        app.grid_columns = 2;
        app.open_viewer(video, cx);
    });
    render_ui(vcx);

    let play_center = vcx
        .debug_bounds("viewer-center-play")
        .expect("a paused video must paint the centre play badge")
        .center();
    let covered = card_ids(&view, vcx).into_iter().any(|id| {
        vcx.debug_bounds(leak(format!("media-card-{id}")))
            .map(|bounds| bounds.contains(&play_center))
            .unwrap_or(false)
    });
    assert!(
        covered,
        "test premise: a grid card must sit under the play badge"
    );

    click(vcx, "viewer-center-play");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id.0),
            Some(expected_id),
            "starting playback must not change the selected media"
        );
        assert!(app.video.is_playing, "the play badge must start playback");
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn info_drawer_click_does_not_change_the_selected_media() {
    let (temp_dir, engine) = seed_library("overlay_drawer", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let item = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    view.update(vcx, |app, cx| {
        app.open_viewer(item, cx);
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);

    let drawer = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("open inspector must paint the info drawer");
    let target = card_ids(&view, vcx).into_iter().find_map(|id| {
        let card = vcx.debug_bounds(leak(format!("media-card-{id}")))?;
        let overlap = card.intersect(&drawer);
        (f32::from(overlap.size.width) > 40.0 && f32::from(overlap.size.height) > 40.0)
            .then(|| overlap.center())
    });
    let target = target.expect("test premise: a grid card must lie under the info drawer");
    let before = selected_id(&view, vcx);

    click_point(vcx, f32::from(target.x), f32::from(target.y));
    assert_eq!(
        selected_id(&view, vcx),
        before,
        "a click inside the info drawer must not select the grid card underneath"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn tag_manager_scrim_click_does_not_select_a_grid_card() {
    let (temp_dir, engine) = seed_library("overlay_tags", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_tag_manager(cx));
    render_ui(vcx);

    let dialog = vcx
        .debug_bounds("tag-manager-dialog")
        .expect("the tag manager must paint its card");
    let target = card_ids(&view, vcx).into_iter().find_map(|id| {
        let card = vcx.debug_bounds(leak(format!("media-card-{id}")))?;
        (!dialog.contains(&card.center())).then(|| card.center())
    });
    let target = target.expect("test premise: a grid card must sit outside the dialog");
    let before = selected_id(&view, vcx);

    click_point(vcx, f32::from(target.x), f32::from(target.y));
    view.update(vcx, |app, _| {
        assert!(
            !app.tag_manager.open,
            "the scrim click must dismiss the modal"
        );
    });
    assert_eq!(
        selected_id(&view, vcx),
        before,
        "the scrim click must not select the grid card underneath"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn select_first(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    n: usize,
) {
    view.update(vcx, |app, cx| {
        app.selected_indices = (0..n).collect();
        app.selection_anchor = Some(0);
        app.selected_index = Some(n - 1);
        app.selected_item = app.get_active_slice(n - 1..n).into_iter().next();
        app.update_batch_summary();
        cx.notify();
    });
    render_ui(vcx);
}

#[core::prelude::v1::test]
fn action_dock_favorite_keeps_the_multi_selection() {
    let (temp_dir, engine) = seed_library("overlay_dock_fav", 8);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    select_first(&view, vcx, 3);

    click(vcx, "batch-fav-dock-btn");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            3,
            "a dock click must not fall through and clear the grid selection"
        );
        assert!(
            app.get_active_slice(0..3)
                .iter()
                .all(|item| item.is_favorite),
            "the dock Favorite button must favourite every selected item"
        );
    });

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[core::prelude::v1::test]
fn action_dock_add_tags_keeps_the_multi_selection_and_starts_input() {
    let (temp_dir, engine) = seed_library("overlay_dock_tags", 8);
    let ids: Vec<_> = engine
        .get_item_slice(0..3)
        .iter()
        .map(|item| item.id)
        .collect();
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    select_first(&view, vcx, 3);

    click(vcx, "batch-tags-dock-btn");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            3,
            "a dock click must not fall through and clear the grid selection"
        );
        assert!(app.inspector_open, "Add Tags must open the inspector");
        assert!(app.tag_input_active, "Add Tags must start typing a tag");
    });

    // Committing the typed tag must persist it on every selected item.
    view.update(vcx, |app, cx| {
        app.tag_input_buffer = "trip".to_string();
        app.commit_tag_input(cx);
        assert!(
            app.error_message.is_none(),
            "the batch tag write must succeed"
        );
    });
    for id in ids {
        assert!(
            engine.get_tags_for_item(id).iter().any(|tag| tag == "trip"),
            "batch tagging from the dock must tag every selected item, missing on {id:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
