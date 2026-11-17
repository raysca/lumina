//! Photo viewer: menu rating, stage prev/next, Fit button while zoomed.

use super::ui_automation::{
    app_with_library, click, click_point, focus_app, keystrokes, render_ui, right_click,
    seed_library, seed_library_undecodable,
};

#[core::prelude::v1::test]
fn viewer_stage_rates_navigates_and_steps_zoom() {
    let (temp_dir, engine) = seed_library("viewer_stage", 3);

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    let first = view.update(vcx, |app, cx| {
        let first = app.get_active_slice(0..1)[0].clone();
        app.select_item_at(first.clone(), 0, cx);
        app.open_viewer(first.clone(), cx);
        first
    });
    render_ui(vcx);

    right_click(vcx, "viewer-viewport");
    click(vcx, "context-rating-star-3");
    assert_eq!(
        engine.catalog().get_rating(first.id).unwrap(),
        3,
        "the viewer menu's third star must persist a 3-star rating"
    );

    let back = vcx.debug_bounds("viewer-back").expect("Back renders");
    let position = vcx
        .debug_bounds("viewer-position")
        .expect("the top bar shows the \"N / M\" position counter");
    assert!(
        position.left() > back.right(),
        "the counter sits right of Back"
    );
    click(vcx, "viewer-next");
    let second = view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(1), "Next must advance to item 2");
        assert!(app.is_fullscreen_viewer, "Next must stay in the viewer");
        app.selected_item.clone().expect("Next must select an item")
    });
    assert_ne!(second.id, first.id, "Next must show a different item");
    click(vcx, "viewer-prev");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(0), "Previous must step back");
    });

    assert!(
        vcx.debug_bounds("viewer-zoom-fit").is_none(),
        "at Fit the stage shows no zoom control"
    );
    focus_app(vcx, &view);
    keystrokes(vcx, "=");
    let zoomed = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(zoomed > 1.0, "+ must zoom in from Fit, got {zoomed}");
    click(vcx, "viewer-zoom-fit");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "Fit to window returns to Fit"
        );
    });
    assert!(
        vcx.debug_bounds("viewer-zoom-fit").is_none(),
        "back at Fit the zoom control hides again"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ---- Opening and stepping without a blank frame ----------
//
// Decodes run on the engine's pool, which `run_until_parked` does not wait
// for, and the seed fixtures fail to decode at an unknown moment. These
// tests therefore claim ids in `in_flight` before the first draw (the grid
// never queues them) and give the view a fresh failure set after the
// viewer's own decode is sent (its late `None` marks the old set only).

use crate::app::state::{on_viewer_frame, THUMB_REQUESTS};
use crate::views::grid::GridState;
use crate::LuminaApp;
use gpui::{Entity, TestAppContext, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId};
use std::sync::Arc;

/// Opens the app with every item claimed in flight and `failed` failed
/// before the first draw, so no real grid decode is ever queued.
pub(super) fn app_without_decodes<'a>(
    cx: &'a mut TestAppContext,
    engine: Arc<LuminaEngine>,
    failed: &[MediaId],
) -> (Entity<LuminaApp>, &'a mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    view.update(vcx, |app, cx| {
        let state = GridState::new(engine.clone());
        let count = engine.get_item_slice(0..1000).len();
        for item in engine.get_item_slice(0..count) {
            state.in_flight.lock().insert(item.id);
        }
        for id in failed {
            state.mark_failed(*id);
        }
        app.grid_state = Some(state);
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

/// Sends later decoder answers nowhere, so a seed fixture's real (failed)
/// decode cannot change the stage at an unpredictable moment.
pub(super) fn detach_decoder_answers(app: &mut LuminaApp) {
    app.ui_tx = async_channel::unbounded().0;
}

/// The image stage: the viewport minus its fixed 88x44 inset.
fn stage_size(vcx: &mut VisualTestContext) -> gpui::Size<gpui::Pixels> {
    let viewport = vcx
        .debug_bounds("viewer-viewport")
        .expect("viewport paints");
    gpui::size(
        viewport.size.width - gpui::px(176.0),
        viewport.size.height - gpui::px(88.0),
    )
}

pub(super) fn open_at(
    app: &mut LuminaApp,
    index: usize,
    cx: &mut gpui::Context<LuminaApp>,
) -> MediaId {
    let item = app.get_active_slice(index..index + 1)[0].clone();
    app.select_item_at(item.clone(), index, cx);
    app.open_viewer(item.clone(), cx);
    item.id
}

#[core::prelude::v1::test]
fn viewer_open_with_cached_thumb_paints_media_on_first_draw() {
    let (dir, engine) = seed_library("viewer_cached", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[]);
    view.update(vcx, |app, cx| {
        let id = app.get_active_slice(0..1)[0].id;
        app.handle_ui_message(
            crate::UiMessage::ThumbnailLoaded {
                id,
                frame: super::viewer_and_video::tiny_decoded_frame(id),
            },
            cx,
        );
        open_at(app, 0, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-media").is_some(),
        "a cached grid thumbnail paints on the viewer's first draw"
    );
    assert!(
        vcx.debug_bounds("viewer-stage-placeholder").is_none(),
        "no placeholder when the grid thumbnail is cached"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_step_without_cache_keeps_stage_size() {
    let (dir, engine) = seed_library("viewer_step_stage", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[]);
    view.update(vcx, |app, cx| {
        let id = app.get_active_slice(0..1)[0].id;
        app.handle_ui_message(
            crate::UiMessage::ThumbnailLoaded {
                id,
                frame: super::viewer_and_video::tiny_decoded_frame(id),
            },
            cx,
        );
        open_at(app, 0, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);
    let media = vcx
        .debug_bounds("viewer-media")
        .expect("the cached first item paints");
    view.update(vcx, |app, cx| {
        app.navigate_selection(1, cx);
        detach_decoder_answers(app);
    });
    render_ui(vcx);
    let placeholder = vcx
        .debug_bounds("viewer-stage-placeholder")
        .expect("an uncached item paints the full-stage placeholder");
    assert_eq!(
        placeholder, media,
        "stepping to an uncached item must keep the stage size (no layout jump)"
    );
    assert_eq!(
        placeholder.size,
        stage_size(vcx),
        "the placeholder fills the image stage, not a 400x400 box"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_neighbour_ids_are_the_adjacent_items() {
    let (dir, engine) = seed_library_undecodable("viewer_neighbours", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        let ids: Vec<MediaId> = app.get_active_slice(0..6).iter().map(|i| i.id).collect();
        // The first draw queued the visible cards; start from a clean slate.
        let state = app.grid_state.as_mut().unwrap();
        state.in_flight = Arc::new(parking_lot::Mutex::new(Default::default()));
        state.thumb_failures = Arc::new(parking_lot::Mutex::new(Default::default()));
        THUMB_REQUESTS.with(|n| n.set(0));

        open_at(app, 3, cx);
        assert_eq!(
            app.viewer_neighbour_ids(),
            vec![ids[2], ids[4]],
            "the neighbours of index 3 are 2 and 4"
        );
        assert_eq!(
            THUMB_REQUESTS.with(|n| n.get()),
            2,
            "opening the viewer prefetches both neighbours' grid thumbnails"
        );

        app.navigate_selection(-3, cx);
        assert_eq!(app.selected_index, Some(0));
        assert_eq!(
            app.viewer_neighbour_ids(),
            vec![ids[1]],
            "the first item has only a next neighbour"
        );
        assert_eq!(
            THUMB_REQUESTS.with(|n| n.get()),
            3,
            "stepping prefetches the new neighbour, once"
        );

        app.navigate_selection(5, cx);
        assert_eq!(
            app.viewer_neighbour_ids(),
            vec![ids[4]],
            "the last item has only a previous neighbour"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_on_failed_item_shows_failed_stage() {
    let (dir, engine) = seed_library("viewer_failed", 2);
    let first = engine.get_item_slice(0..2);
    let failed: Vec<MediaId> = first.iter().map(|i| i.id).collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &failed);
    let path = view.update(vcx, |app, cx| {
        open_at(app, 0, cx);
        app.selected_item.as_ref().unwrap().path.clone()
    });
    render_ui(vcx);
    let stage = vcx
        .debug_bounds("viewer-stage-failed")
        .expect("a failed item paints the failed stage");
    assert_eq!(
        stage.size,
        stage_size(vcx),
        "the failed stage fills the image stage"
    );
    assert!(
        vcx.debug_bounds("viewer-stage-placeholder").is_none(),
        "a failed item never shows a placeholder that waits forever"
    );
    assert_eq!(
        crate::i18n::t("viewer.preview_failed"),
        "Lumina can't show a preview of this file."
    );
    assert_eq!(
        crate::i18n::t("viewer.open_default_hint"),
        "Press O to open it in its default app."
    );

    // O must still reach Open in Default App; a vanished file proves it
    // without launching anything.
    std::fs::remove_file(&path).unwrap();
    focus_app(vcx, &view);
    keystrokes(vcx, "o");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.error_message.as_deref(),
            Some(crate::app::viewer_actions::EXTERNAL_EDITOR_FILE_MISSING_MESSAGE),
            "O on the failed stage must reach open_selected_in_default_app"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_decode_failure_shows_failed_stage_but_keeps_the_grid_card() {
    let (dir, engine) = seed_library_undecodable("viewer_decode_failed", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[]);
    let id = view.update(vcx, |app, cx| {
        let id = open_at(app, 0, cx);
        on_viewer_frame(&app.ui_tx, id, None);
        id
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-stage-failed").is_some(),
        "a failed full-size decode paints the failed stage, not a placeholder forever"
    );
    view.update(vcx, |app, cx| {
        assert!(
            !app.grid_state.as_ref().unwrap().is_failed(id),
            "a full-size decode failure must not mark the grid thumbnail failed"
        );
        // A late failure for an item no longer shown changes nothing.
        app.navigate_selection(1, cx);
        on_viewer_frame(&app.ui_tx, id, None);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_ne!(
            app.viewer_failed,
            Some(id),
            "a stale viewer failure must not mark the newly selected item's stage"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Docked InfoPanel -------------------------------------------------

#[core::prelude::v1::test]
fn viewer_info_panel_is_docked_full_height() {
    let (dir, engine) = seed_library_undecodable("viewer_docked_panel", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_without_decodes(&mut cx, engine, &[]);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    view.update(vcx, |app, cx| {
        open_at(app, 0, cx);
        detach_decoder_answers(app);
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);

    let panel = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("an open inspector paints the docked info panel");
    assert_eq!(
        (
            panel.right(),
            panel.size.width,
            panel.top(),
            panel.size.height
        ),
        (
            gpui::px(1600.0),
            gpui::px(340.0),
            gpui::px(0.0),
            gpui::px(1000.0)
        ),
        "the info panel docks at the right edge, 340 wide and full height: {panel:?}"
    );
    let summary = vcx
        .debug_bounds("inspector-summary")
        .expect("the docked panel paints the item summary");
    assert!(
        summary.right() == panel.right() && summary.size.width >= gpui::px(339.0),
        "the summary fills the 340 wide panel, not the docked inspector's saved width: {summary:?}"
    );
    let bar = vcx.debug_bounds("viewer-top-bar").expect("top bar paints");
    assert_eq!(
        bar.right(),
        gpui::px(1260.0),
        "the top bar belongs to the stage column, left of the panel: {bar:?}"
    );

    // Panel closed (I): the stage takes the full width.
    view.update(vcx, |app, cx| {
        app.inspector_open = false;
        cx.notify();
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("viewer-info-drawer").is_none());
    let bar = vcx.debug_bounds("viewer-top-bar").expect("top bar paints");
    assert_eq!(
        bar.right(),
        gpui::px(1600.0),
        "with the panel closed the stage spans the window: {bar:?}"
    );

    // Immersive: the panel floats over the stage below the top bar with the
    // HUD, so the stage keeps the full width, and it hides with the HUD.
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.is_immersive_fullscreen = true;
        app.immersive_hud_visible = true;
        cx.notify();
    });
    render_ui(vcx);
    let panel = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("the visible HUD shows the open panel");
    let viewport = vcx.debug_bounds("viewer-viewport").expect("viewport");
    assert_eq!(
        (panel.right(), viewport.right()),
        (gpui::px(1600.0), gpui::px(1600.0)),
        "immersive keeps a full-width stage under the floating panel"
    );
    view.update(vcx, |app, cx| {
        app.immersive_hud_visible = false;
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-info-drawer").is_none(),
        "the panel hides with the immersive HUD"
    );

    // Narrow window: the panel keeps 340, the stage gets the rest and its
    // controls stay inside it.
    view.update(vcx, |app, cx| {
        app.is_immersive_fullscreen = false;
        cx.notify();
    });
    // The window's minimum size is 960x600.
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(700.0)));
    render_ui(vcx);
    let panel = vcx.debug_bounds("viewer-info-drawer").expect("panel");
    let bar = vcx.debug_bounds("viewer-top-bar").expect("top bar");
    let next = vcx.debug_bounds("viewer-next").expect("next button");
    assert_eq!(
        (panel.left(), panel.size.width, bar.right()),
        (gpui::px(620.0), gpui::px(340.0), gpui::px(620.0)),
        "a narrow window shrinks the stage, not the panel"
    );
    assert!(
        next.right() <= panel.left(),
        "the next button stays on the stage: {next:?} panel={panel:?}"
    );
    let info = vcx.debug_bounds("viewer-info-btn").expect("info button");
    assert!(
        info.right() <= bar.right(),
        "the top bar's buttons stay left of the panel: {info:?} bar={bar:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Opens item 0 of a one-item undecodable library at 1600x1000 with the
/// info panel open.
fn viewer_with_panel<'a>(
    cx: &'a mut TestAppContext,
    prefix: &str,
) -> (
    std::path::PathBuf,
    Entity<LuminaApp>,
    &'a mut VisualTestContext,
) {
    let (dir, engine) = seed_library_undecodable(prefix, 1);
    let (view, vcx) = app_without_decodes(cx, engine, &[]);
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    view.update(vcx, |app, cx| {
        open_at(app, 0, cx);
        detach_decoder_answers(app);
        app.inspector_open = true;
        cx.notify();
    });
    render_ui(vcx);
    (dir, view, vcx)
}

#[core::prelude::v1::test]
fn viewer_immersive_panel_leaves_the_top_bar_buttons_clickable() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_immersive_panel_bar");
    view.update(vcx, |app, cx| {
        app.is_immersive_fullscreen = true;
        app.immersive_hud_visible = true;
        cx.notify();
    });
    render_ui(vcx);

    let panel = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("the visible HUD shows the open panel");
    for selector in ["viewer-info-btn", "viewer-fav-btn"] {
        let button = vcx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} paints"));
        assert!(
            !button.intersects(&panel),
            "the floating panel must not cover {selector}: {button:?} panel={panel:?}"
        );
    }
    click(vcx, "viewer-info-btn");
    view.update(vcx, |app, _| {
        assert!(
            !app.inspector_open,
            "clicking Info in immersive mode must close the panel"
        );
    });
    assert!(vcx.debug_bounds("viewer-info-drawer").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_immersive_panel_clears_the_zoom_navigator() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = super::viewer_and_video::viewer_with_gradient(
        &mut cx,
        "viewer_immersive_nav",
        1600.0,
        1000.0,
    );
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.is_immersive_fullscreen = true;
        app.immersive_hud_visible = true;
        cx.notify();
    });
    render_ui(vcx);
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-2");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.viewer_state.zoom_scale > 1.0,
            "test premise: ⌘2 zooms in"
        );
        assert!(
            app.immersive_hud_visible,
            "test premise: the HUD is visible"
        );
    });

    let nav = vcx
        .debug_bounds("viewer-navigator")
        .expect("zoomed, the navigator shows");
    let panel = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("the visible HUD shows the open panel");
    assert!(
        !nav.intersects(&panel),
        "the zoom navigator must not sit under the floating panel: nav={nav:?} panel={panel:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_i_key_closes_and_reopens_the_docked_panel() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_panel_key");
    focus_app(vcx, &view);
    let docked = vcx.debug_bounds("viewer-viewport").expect("viewport");

    keystrokes(vcx, "i");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-info-drawer").is_none(),
        "I closes the docked panel"
    );
    let full = vcx.debug_bounds("viewer-viewport").expect("viewport");
    assert_eq!(
        full.size.width - docked.size.width,
        gpui::px(340.0),
        "closing the panel widens the stage by its width: docked={docked:?} full={full:?}"
    );

    keystrokes(vcx, "i");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-info-drawer").is_some(),
        "I reopens the panel"
    );
    let reopened = vcx.debug_bounds("viewer-viewport").expect("viewport");
    assert_eq!(
        reopened.size.width, docked.size.width,
        "reopening the panel narrows the stage again"
    );
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "I must not close the viewer")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Top bar ---------------------------------------------------------------

/// The catalog rating of the viewer's item.
fn viewer_rating(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> u8 {
    view.update(vcx, |app, _| {
        let id = app.selected_item.as_ref().expect("viewer item").id;
        let engine = &app.grid_state.as_ref().expect("library").engine;
        engine.catalog().get_rating(id).expect("rating reads")
    })
}

#[core::prelude::v1::test]
fn viewer_top_bar_orders_its_controls_and_edit_opens_edit_mode() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_top_bar");

    assert!(vcx.debug_bounds("viewer-top-bar").is_some(), "top bar");
    assert!(
        vcx.debug_bounds("viewer-filename").is_none(),
        "the date moved to the panel's When line; the bar shows only the position"
    );
    assert_eq!(
        crate::views::viewer::top_bar::position_label(22, 84),
        "23 / 84",
        "the position reads \"N / M\""
    );
    assert!(
        vcx.debug_bounds("viewer-back-label").is_some(),
        "a wide stage shows the Back label"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.view_label(),
            "All Media",
            "Back names the view it returns to"
        )
    });

    let mut order = vec![
        "viewer-back",
        "viewer-position",
        "viewer-fav-btn",
        "viewer-rate-btn",
    ];
    order.push("viewer-edit-btn");
    if cfg!(target_os = "macos") {
        order.push("viewer-share-btn");
    }
    order.push("viewer-info-btn");
    let rects: Vec<_> = order
        .iter()
        .map(|s| {
            vcx.debug_bounds(s)
                .unwrap_or_else(|| panic!("{s} paints in the top bar"))
        })
        .collect();
    for (pair, names) in rects.windows(2).zip(order.windows(2)) {
        assert!(
            pair[0].right() <= pair[1].left(),
            "{} must sit left of {}: {:?} {:?}",
            names[0],
            names[1],
            pair[0],
            pair[1]
        );
    }

    click(vcx, "viewer-edit-btn");
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_some(), "Edit opens edit mode")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_rate_popover_rates_by_key_and_closes_before_the_viewer() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_rate_popover");
    focus_app(vcx, &view);

    click(vcx, "viewer-rate-btn");
    assert!(
        vcx.debug_bounds("viewer-rate-popover").is_some(),
        "the star button opens the rating popover"
    );
    for n in 1..=5 {
        assert!(
            vcx.debug_bounds(super::ui_automation::leak(format!("viewer-rate-star-{n}")))
                .is_some(),
            "the popover shows star {n}"
        );
    }
    let card = vcx.debug_bounds("viewer-rate-popover").unwrap();
    let button = vcx.debug_bounds("viewer-rate-btn").unwrap();
    assert!(
        card.top() >= button.bottom() && (card.right() - button.right()).abs() <= gpui::px(2.0),
        "the popover hangs under the star button, right edges aligned: card={card:?} button={button:?}"
    );
    // Keys the viewer would act on stay in the popover.
    keystrokes(vcx, "f");
    view.update(vcx, |app, _| {
        assert!(
            !app.selected_item.as_ref().unwrap().is_favorite,
            "F must not reach the viewer while the popover is open"
        )
    });
    keystrokes(vcx, "backspace secondary-backspace enter");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        let id = app.selected_item.as_ref().unwrap().id;
        let engine = &app.grid_state.as_ref().unwrap().engine;
        assert!(
            !engine.get_item_by_id(id).unwrap().is_trashed,
            "⌫ and ⌘⌫ must not trash the photo behind the popover"
        );
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ must not open the Delete dialog behind the popover"
        );
        assert!(app.rate_popover_open, "unhandled keys leave it open");
    });
    keystrokes(vcx, "3");
    render_ui(vcx);
    assert_eq!(viewer_rating(&view, vcx), 3, "3 rates three stars");
    assert!(
        vcx.debug_bounds("viewer-rate-popover").is_none(),
        "a rating key closes the popover"
    );

    click(vcx, "viewer-rate-btn");
    keystrokes(vcx, "0");
    render_ui(vcx);
    assert_eq!(viewer_rating(&view, vcx), 0, "0 clears the rating");

    click(vcx, "viewer-rate-btn");
    click(vcx, "viewer-rate-star-4");
    assert_eq!(viewer_rating(&view, vcx), 4, "a star click rates");
    assert!(vcx.debug_bounds("viewer-rate-popover").is_none());

    // The palette's "Rate…" opens it too; Escape closes only the popover.
    view.update(vcx, |app, cx| {
        app.execute_command(crate::app::command_palette::CommandId::RateMenu, cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-rate-popover").is_some(),
        "Rate… opens the popover"
    );
    keystrokes(vcx, "escape");
    render_ui(vcx);
    assert!(vcx.debug_bounds("viewer-rate-popover").is_none());
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "Escape closes the popover first")
    });
    assert_eq!(viewer_rating(&view, vcx), 4, "Escape leaves the rating");

    // The backdrop dismisses it.
    click(vcx, "viewer-rate-btn");
    click_point(vcx, 400.0, 500.0);
    assert!(
        vcx.debug_bounds("viewer-rate-popover").is_none(),
        "a click outside closes the popover"
    );
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "…and leaves the viewer open")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_top_bar_compacts_by_stage_width() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_top_bar_compact");
    // 1300 wide: 960 of stage beside the panel, room for the label.
    view.update(vcx, |app, cx| {
        app.window_bounds = Some((0.0, 0.0, 1300.0, 800.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1300.0), gpui::px(800.0)));
    render_ui(vcx);
    assert!(vcx.debug_bounds("viewer-back-label").is_some());

    // 960 wide, the minimum: 620 of stage, so the label goes.
    view.update(vcx, |app, cx| {
        app.window_bounds = Some((0.0, 0.0, 960.0, 700.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(700.0)));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-back-label").is_none(),
        "a narrow stage hides the Back label"
    );
    let bar = vcx.debug_bounds("viewer-top-bar").expect("top bar");
    let back = vcx.debug_bounds("viewer-back").expect("back");
    let position = vcx.debug_bounds("viewer-position").expect("position");
    let fav = vcx.debug_bounds("viewer-fav-btn").expect("fav");
    let info = vcx.debug_bounds("viewer-info-btn").expect("info");
    assert!(
        back.right() <= position.left() && position.right() <= fav.left(),
        "nothing overlaps: back={back:?} position={position:?} fav={fav:?}"
    );
    assert!(info.right() <= bar.right(), "the buttons stay in the bar");

    // Same window, panel closed: 960 of stage, so the label returns.
    view.update(vcx, |app, cx| {
        app.inspector_open = false;
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-back-label").is_some(),
        "compact follows the stage width, not the window width"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn viewer_rate_popover_holds_off_edit_mode_and_slideshows() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_rate_popover_modes");
    focus_app(vcx, &view);

    // Like the header menus, the open popover holds off edit mode (from the
    // menu bar; E stays in the popover).
    click(vcx, "viewer-rate-btn");
    view.update(vcx, |app, cx| app.enter_edit_mode(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.edits.session.is_none(),
            "edit mode waits for the popover"
        );
    });
    assert!(vcx.debug_bounds("viewer-rate-popover").is_some());
    view.update(vcx, |app, cx| {
        app.close_rate_popover(cx);
        app.enter_edit_mode(cx);
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.edits.session.is_some(), "test premise: edit mode is on")
    });
    // Even a stale flag stays inert while its button is hidden.
    view.update(vcx, |app, cx| {
        app.rate_popover_open = true;
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-rate-backdrop").is_none()
            && vcx.debug_bounds("viewer-rate-popover").is_none(),
        "no popover or invisible backdrop in edit mode"
    );
    keystrokes(vcx, "3");
    assert_eq!(
        viewer_rating(&view, vcx),
        0,
        "1–5 must not rate through a popover edit mode hides"
    );
    view.update(vcx, |app, cx| {
        app.rate_popover_open = false;
        app.cancel_edit(cx);
    });
    render_ui(vcx);

    click(vcx, "viewer-rate-btn");
    view.update(vcx, |app, _| assert!(app.rate_popover_open));
    view.update(vcx, |app, cx| app.start_slideshow_deferred(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "a slideshow waits for the popover");
        assert!(app.rate_popover_open);
    });
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Stage chrome ----------------------------------------------------------

#[core::prelude::v1::test]
fn viewer_nav_buttons_sit_26px_from_stage_edges() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, view, vcx) = viewer_with_panel(&mut cx, "viewer_nav_edges");
    let px = gpui::px;

    let check = |vcx: &mut VisualTestContext, stage_right: f32, case: &str| {
        let prev = vcx.debug_bounds("viewer-prev").expect("prev paints");
        let next = vcx.debug_bounds("viewer-next").expect("next paints");
        let viewport = vcx.debug_bounds("viewer-viewport").expect("viewport");
        assert_eq!(
            (prev.left(), next.right()),
            (px(26.0), px(stage_right - 26.0)),
            "{case}: the arrows sit 26 from the stage edges: prev={prev:?} next={next:?}"
        );
        for b in [prev, next] {
            assert!(
                (b.center().y - viewport.center().y).abs() <= px(0.5),
                "{case}: centred on the viewport: {b:?} viewport={viewport:?}"
            );
        }
    };
    check(vcx, 1260.0, "panel docked");

    view.update(vcx, |app, cx| {
        app.inspector_open = false;
        cx.notify();
    });
    render_ui(vcx);
    check(vcx, 1600.0, "panel closed");

    // Immersive: the panel floats over the stage, so Next moves left of it.
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.is_immersive_fullscreen = true;
        app.immersive_hud_visible = true;
        cx.notify();
    });
    render_ui(vcx);
    let panel = vcx
        .debug_bounds("viewer-info-drawer")
        .expect("panel floats");
    let next = vcx.debug_bounds("viewer-next").expect("next paints");
    assert_eq!(
        panel.left() - next.right(),
        px(26.0),
        "immersive: Next keeps 26 from the floating panel: next={next:?} panel={panel:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
