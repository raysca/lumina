//! Overlay isolation regressions.
//!
//! These drive rendered clicks against the fullscreen viewer and the tag
//! manager. Before the occlusion fix, one click could run both the overlay
//! control and the toolbar/grid control painted underneath.

use crate::app::types::get_sample_items;
use crate::tests::helpers::{
    app_with_library, click, click_point, leak, render_ui, right_click, seed_library,
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
    let (_temp_dir, engine) = seed_library("overlay_info", 8);
    let mut cx = crate::tests::TestApp::single();
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
    // At this window width the spot is the viewer's Rate button: it opens
    // its popover. Close it so the next click reaches viewer Info.
    // Linux: the viewer bar has no Share button (no share picker there), so
    // its buttons shift and the spot misses Rate; only close it if open.
    view.update(vcx, |app, cx| {
        #[cfg(not(target_os = "linux"))]
        assert!(
            app.rate_popover_open,
            "the click over the hidden Sort lands on viewer Rate"
        );
        if app.rate_popover_open {
            app.close_rate_popover(cx)
        }
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
}

#[core::prelude::v1::test]
fn viewer_open_click_does_not_open_tag_manager() {
    let (_temp_dir, engine) = seed_library("overlay_open", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_viewer_with_first_item(&view, vcx);

    click(vcx, "tags-toolbar-btn");
    view.update(vcx, |app, _| {
        assert!(
            !app.tags_page_shown(),
            "a click over the hidden toolbar must not reach Tags"
        );
    });

    right_click(vcx, "viewer-viewport");
    click(vcx, "open-with-menu-btn");
    view.update(vcx, |app, _| {
        assert!(
            !app.tags_page_shown(),
            "the Open click must not also open the Tag Manager"
        );
    });
}

#[core::prelude::v1::test]
fn viewer_play_badge_does_not_change_the_selected_media() {
    let (_temp_dir, engine) = seed_library("overlay_play", 8);
    let mut cx = crate::tests::TestApp::single();
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
        // The sample path doesn't exist, so the load failed; this test is
        // about the play badge a loadable paused video shows.
        app.video.load_error = None;
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
}

#[core::prelude::v1::test]
fn info_drawer_click_does_not_change_the_selected_media() {
    let (_temp_dir, engine) = seed_library("overlay_drawer", 8);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn tags_page_click_does_not_select_a_grid_card() {
    let (_temp_dir, engine) = seed_library("overlay_tags", 8);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = card_ids(&view, vcx);
    view.update(vcx, |app, cx| app.open_tag_manager(cx));
    render_ui(vcx);

    let page = vcx
        .debug_bounds("tags-page")
        .expect("the Tags page must paint");
    assert!(
        ids.iter()
            .all(|id| vcx.debug_bounds(leak(format!("media-card-{id}"))).is_none()),
        "the Tags page replaces the grid: no card is painted under it"
    );
    let before = selected_id(&view, vcx);
    let center = page.center();
    click_point(vcx, f32::from(center.x), f32::from(center.y));
    view.update(vcx, |app, _| {
        assert!(app.tags_page_shown(), "a click on the page keeps it");
    });
    assert_eq!(
        selected_id(&view, vcx),
        before,
        "a click on the page must not select a grid card"
    );
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
    let (_temp_dir, engine) = seed_library("overlay_dock_fav", 8);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn action_dock_add_tags_keeps_the_multi_selection_and_starts_input() {
    let (_temp_dir, engine) = seed_library("overlay_dock_tags", 8);
    let ids: Vec<_> = engine
        .get_item_slice(0..3)
        .iter()
        .map(|item| item.id)
        .collect();
    let mut cx = crate::tests::TestApp::single();
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
}

/// Settings is an opaque full-window layer: a toast raised under it must
/// still be seen and clickable, so it is drawn above settings.
#[core::prelude::v1::test]
fn toast_raised_with_settings_open_is_drawn_above_it() {
    let (_temp_dir, engine) = seed_library("settings_toast", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.open_preferences(cx);
        app.show_toast("Settings saved", cx);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("settings-page").is_some(),
        "settings shows"
    );

    // A click on the toast's dismiss control reaches it, not settings.
    // Plain bottom offset, centred in the window: the hidden grid's zoom
    // pill, dock and year rail do not move it.
    let toast = vcx.debug_bounds("toast").expect("the toast renders");
    let page = vcx.debug_bounds("settings-page").unwrap();
    assert_eq!(
        f32::from(page.bottom() - toast.bottom()),
        crate::app::render::overlays::DOCK_BOTTOM,
        "over settings the toast sits at the plain bottom offset"
    );
    assert!(
        (f32::from(toast.center().x) - f32::from(page.center().x)).abs() <= 1.0,
        "over settings the toast is centred, with no year-rail padding"
    );

    click(vcx, "toast-dismiss");
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "the toast is above settings, so its dismiss control takes the click"
        );
        assert!(app.preferences_open, "the click did not reach settings");
    });
}

/// View > Toggle Sidebar would flip and save a setting hidden behind
/// settings; Help > About goes to settings' About and drops the reset
/// question, which belongs to General.
#[core::prelude::v1::test]
fn menu_actions_under_settings_leave_the_hidden_library_alone() {
    let (_temp_dir, engine) = seed_library("settings_menus", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let root = view.read_with(vcx, |app, _| app.focus_handle.clone());
    vcx.update(|window, cx| window.focus(&root, cx));
    view.update(vcx, |app, cx| app.prompt_reset_preferences(cx));
    render_ui(vcx);
    let collapsed = view.read_with(vcx, |app, _| app.sidebar_collapsed);

    vcx.dispatch_action(crate::actions::ToggleSidebar);
    render_ui(vcx);
    view.read_with(vcx, |app, _| {
        assert_eq!(
            app.sidebar_collapsed, collapsed,
            "Toggle Sidebar must do nothing while settings covers the sidebar"
        );
    });

    // Dialogs would draw under the opaque settings layer: their menu items
    // have no handler, so nothing opens and no refusal toast appears.
    let item = view.read_with(vcx, |app, _| {
        app.grid_state.as_ref().unwrap().engine.get_item_slice(0..1)[0].clone()
    });
    view.update(vcx, |app, _| {
        app.selected_item = Some(item);
        app.selected_indices.insert(0);
    });
    vcx.dispatch_action(crate::actions::AddToAlbum);
    vcx.dispatch_action(crate::actions::ManageTemplates);
    vcx.dispatch_action(crate::actions::ImportNetworkVideo);
    render_ui(vcx);
    view.read_with(vcx, |app, _| {
        assert!(app.albums.picker.is_none(), "Add to Album is disabled");
        assert!(
            app.templates.editor.is_none(),
            "Manage Templates is disabled"
        );
        assert!(
            app.network_import.stage.is_none(),
            "Import Video from URL is disabled"
        );
        assert!(
            app.toast.current().is_none(),
            "a disabled item does nothing, not even a refusal: {:?}",
            app.toast.current().map(|t| &t.message)
        );
    });

    vcx.dispatch_action(crate::actions::ShowAbout);
    render_ui(vcx);
    view.read_with(vcx, |app, _| {
        assert!(app.preferences_open, "About stays in settings");
        assert_eq!(
            app.preferences_tab,
            crate::app::preferences::PreferencesTab::About,
            "Help > About goes to the About category"
        );
        assert!(
            !app.preferences_reset_confirm,
            "the reset question is General's; About must not keep it open"
        );
    });
}
