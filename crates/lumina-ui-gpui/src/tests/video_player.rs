//! Video player (Design 06): top bar, viewer right-click menu, and the
//! docked transport.

use crate::app::types::get_sample_items;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, right_click, seed_library,
};
use gpui::{Bounds, Pixels, VisualTestContext};
use lumina_core::MediaType;

/// Library-backed window (the viewer needs a grid to render) showing the
/// sample video. The temp dir is removed on drop.
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open_video(
    cx: &mut gpui::TestAppContext,
) -> (
    Fixture,
    gpui::Entity<crate::LuminaApp>,
    &mut VisualTestContext,
) {
    let video = get_sample_items()
        .into_iter()
        .find(|item| item.media_type == MediaType::Video)
        .expect("the deterministic sample set must contain a video");
    let (temp_dir, engine) = seed_library("video_player", 8);
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.open_viewer(video, cx);
        app.video.duration = 60.0;
        cx.notify();
    });
    render_ui(vcx);
    (Fixture(temp_dir), view, vcx)
}

fn bounds(vcx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    vcx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} must render"))
}

fn size(b: Bounds<Pixels>) -> (f32, f32) {
    (b.size.width.into(), b.size.height.into())
}

fn center_y(b: Bounds<Pixels>) -> f32 {
    f32::from(b.origin.y) + f32::from(b.size.height) / 2.0
}

#[core::prelude::v1::test]
fn video_transport_is_docked_full_width() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, cx| {
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            1920,
            1080,
        ));
        app.inspector_open = true;
        app.window_bounds = Some((0.0, 0.0, 1600.0, 1000.0));
        app.video.playback_time = 34.0;
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    render_ui(vcx);
    render_ui(vcx);

    let transport = bounds(vcx, "video-transport");
    assert_eq!(
        (
            f32::from(transport.left()),
            f32::from(transport.size.width),
            f32::from(transport.bottom())
        ),
        (0.0, 1260.0, 1000.0),
        "the transport docks across the whole stage at its bottom: {transport:?}"
    );

    let play = bounds(vcx, "video-play");
    assert_eq!(size(play), (38.0, 38.0), "play is a 38px round button");
    for (selector, b) in [
        ("video-seek-back", bounds(vcx, "video-seek-back")),
        ("video-seek-fwd", bounds(vcx, "video-seek-fwd")),
    ] {
        assert_eq!(size(b), (30.0, 30.0), "{selector} is a 30px round button");
        assert!(
            play.right() <= b.left(),
            "{selector} follows play in the left group"
        );
        assert!(
            (center_y(b) - center_y(play)).abs() < 1.0,
            "{selector} shares the controls row"
        );
    }
    let time = bounds(vcx, "video-time");
    assert!(
        time.left() >= bounds(vcx, "video-seek-fwd").right(),
        "the time readout follows the buttons"
    );
    assert!(
        vcx.debug_bounds("video-time-text:0:34 / 1:00").is_some(),
        "the readout renders m:ss / m:ss"
    );
    assert_eq!(
        crate::app::types::format_clock(7322.0),
        "2:02:02",
        "a long clip switches to h:mm:ss"
    );
    let right_group = [
        "video-speed",
        "video-mute",
        "video-loop",
        "video-fullscreen",
    ];
    for pair in right_group.windows(2) {
        assert!(
            bounds(vcx, pair[0]).right() <= bounds(vcx, pair[1]).left(),
            "the right group runs speed, mute, loop, full screen"
        );
    }
    assert!(
        bounds(vcx, "video-speed").left() > time.right(),
        "the right group sits after the time"
    );
    assert!(
        center_y(bounds(vcx, "video-scrubber")) < center_y(play),
        "the scrubber sits above the controls"
    );
    for gone in ["video-step-back", "video-step-fwd", "video-pip"] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{gone} left the transport (keys and the palette remain; PiP is in the top bar)"
        );
    }
    let media = bounds(vcx, "viewer-media");
    assert!(
        f32::from(media.size.width) >= 1099.5 && media.bottom() <= transport.top(),
        "the frame is at least 1100 wide and clears the docked transport: {media:?}"
    );
}

#[core::prelude::v1::test]
fn played_fill_tracks_the_playhead() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, cx| {
        app.video.playback_time = 15.0;
        cx.notify();
    });
    render_ui(vcx);

    let track: f32 = bounds(vcx, "video-scrubber").size.width.into();
    let played: f32 = bounds(vcx, "video-played").size.width.into();
    assert!(
        (played / track - 0.25).abs() < 0.02,
        "15s of 60s must fill a quarter of the track, got {played}/{track}"
    );
}

#[core::prelude::v1::test]
fn l_toggles_loop_and_speed_menu_opens_above_its_button() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);

    let looping = |vcx: &mut VisualTestContext| view.update(vcx, |app, _| app.video.is_looping);
    let before = looping(vcx);
    keystrokes(vcx, "l");
    assert_ne!(looping(vcx), before, "L must toggle loop (v3), not seek");
    keystrokes(vcx, "l");
    assert_eq!(looping(vcx), before, "a second L restores loop");

    click(vcx, "video-speed");
    let speed = bounds(vcx, "video-speed");
    let menu = bounds(vcx, "video-speed-popover");
    assert!(
        menu.bottom() <= speed.top(),
        "the speed menu opens above the speed button"
    );
    assert!(
        menu.left() < speed.right() && menu.right() > speed.left(),
        "the speed menu is anchored over the speed button"
    );
    let speed = crate::views::viewer::utils::format_playback_speed;
    assert_eq!(
        [
            speed(1.0),
            speed(0.25),
            speed(0.5),
            speed(1.5),
            speed(2.0),
            speed(4.0)
        ],
        ["1.0×", "0.25×", "0.5×", "1.5×", "2.0×", "4.0×"],
        "the speed pill shows one decimal, two only when needed"
    );
}

#[core::prelude::v1::test]
fn video_top_bar_has_no_edit_and_actions_live_in_the_right_click_menu() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_video(&mut cx);

    for selector in ["viewer-fav-btn", "viewer-rate-btn", "viewer-info-btn"] {
        assert_eq!(
            size(bounds(vcx, selector)),
            (30.0, 30.0),
            "{selector} is a 30px icon button"
        );
    }
    assert!(
        vcx.debug_bounds("viewer-edit-btn").is_none(),
        "videos cannot be edited, so the top bar has no Edit"
    );
    for removed in ["quick-edit-rotate-left", "quick-edit-rotate-right"] {
        assert!(
            vcx.debug_bounds(removed).is_none(),
            "{removed} moved out of the top bar"
        );
    }

    let before = view.update(vcx, |app, _| app.inspector_open);
    click(vcx, "viewer-info-btn");
    assert_ne!(
        view.update(vcx, |app, _| app.inspector_open),
        before,
        "Info toggles the inspector drawer"
    );

    right_click(vcx, "viewer-viewport");
    for selector in [
        "open-with-menu-btn",
        "reveal-menu-btn",
        "edit-external-menu-btn",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "the viewer right-click menu must offer {selector}"
        );
    }
}

#[core::prelude::v1::test]
fn prev_next_sit_26px_from_the_stage_edges_and_centred_on_the_frame() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, _view, vcx) = open_video(&mut cx);

    // Design 02: 26px from the stage edges, centred on the (narrower) frame.
    let frame = bounds(vcx, "viewer-viewport");
    let prev = bounds(vcx, "viewer-prev");
    let next = bounds(vcx, "viewer-next");
    let window_w = vcx.update(|window, _| window.viewport_size().width);
    assert_eq!(f32::from(prev.left()), 26.0);
    assert_eq!(f32::from(window_w - next.right()), 26.0);
    for b in [prev, next] {
        assert!(
            (center_y(b) - center_y(frame)).abs() < 1.0,
            "nav buttons are vertically centred on the video frame"
        );
    }
}

#[core::prelude::v1::test]
fn viewer_menu_owns_the_keyboard_and_acts_on_the_shown_item_only() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, _| {
        // Viewer opened from a grid multi-selection.
        app.selected_index = Some(1);
        app.selected_indices.extend([0, 1, 2]);
    });
    focus_app(vcx, &view);
    let shown = view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id));

    right_click(vcx, "viewer-viewport");
    view.update(vcx, |app, _| {
        assert!(
            app.context_menu_state.is_some(),
            "right-click opens the menu"
        );
        assert_eq!(
            app.selected_indices.iter().copied().collect::<Vec<_>>(),
            vec![1],
            "the viewer menu must not act on hidden grid selections"
        );
    });

    keystrokes(vcx, "]");
    keystrokes(vcx, "l");
    keystrokes(vcx, "secondary-a");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices.len(),
            1,
            "⌘A must not widen the menu's target"
        );
    });
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            shown,
            "keys must not move the viewer under an open menu"
        );
    });

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.context_menu_state.is_none(), "Escape closes the menu");
        assert!(app.is_fullscreen_viewer, "…and only the menu");
    });

    right_click(vcx, "viewer-viewport");
    assert!(
        vcx.debug_bounds("preview-menu-btn").is_none(),
        "Open in Viewer is meaningless inside the viewer"
    );
    view.update(vcx, |app, cx| app.close_viewer(cx));
    view.update(vcx, |app, _| {
        assert!(
            app.context_menu_state.is_none(),
            "closing the viewer must not leave its menu over the grid"
        );
    });
}

#[core::prelude::v1::test]
fn open_info_drawer_never_covers_the_transport() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, cx| {
        app.inspector_open = true;
        app.window_bounds = Some((0.0, 0.0, 1440.0, 900.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    render_ui(vcx);
    let drawer = bounds(vcx, "viewer-info-drawer");
    let transport = bounds(vcx, "video-transport");
    assert!(
        transport.right() <= drawer.left(),
        "the info drawer must sit beside the transport: {transport:?} drawer={drawer:?}"
    );
}

#[core::prelude::v1::test]
fn video_frame_is_1120_wide_above_the_docked_transport() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, cx| {
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            1920,
            1080,
        ));
        app.window_bounds = Some((0.0, 0.0, 1440.0, 900.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    render_ui(vcx);

    // Design 06: a 1120x630 frame centred above the docked transport.
    let media = bounds(vcx, "viewer-media");
    let transport = bounds(vcx, "video-transport");
    let (w, h) = size(media);
    assert!(
        (w - 1120.0).abs() < 1.0 && (h - 630.0).abs() < 1.5,
        "a 16:9 video fills a 1120x630 frame: {media:?}"
    );
    assert!(
        transport.top() >= media.bottom(),
        "the transport sits below the video: {media:?} {transport:?}"
    );
    // The arrows centre on the video, beside it and above the transport.
    for arrow in ["viewer-prev", "viewer-next"] {
        let b = bounds(vcx, arrow);
        assert!(
            !b.intersects(&transport) && !b.intersects(&media),
            "{arrow} must clear the video and its transport: {b:?} media={media:?} transport={transport:?}"
        );
        assert!(
            (b.center().y - media.center().y).abs() <= gpui::px(1.0),
            "{arrow} centres on the video: {b:?} media={media:?}"
        );
    }
    assert!(
        f32::from(transport.left()) == 0.0 && size(transport).0 == 1440.0,
        "the transport spans the stage: {transport:?}"
    );
    assert!(
        vcx.debug_bounds("viewer-media-shadow").is_some(),
        "the video frame has the 0 16 48 drop shadow, like a photo"
    );
}

/// Shows the video with a decoded frame of `w`x`h` in a `win_w`x`win_h` window.
fn framed_video(
    cx: &mut gpui::TestAppContext,
    (w, h): (u32, u32),
    (win_w, win_h): (f32, f32),
) -> (Fixture, &mut VisualTestContext) {
    let (fixture, view, vcx) = open_video(cx);
    view.update(vcx, |app, cx| {
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            w,
            h,
        ));
        app.window_bounds = Some((0.0, 0.0, win_w, win_h));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(win_w), gpui::px(win_h)));
    render_ui(vcx);
    render_ui(vcx);
    (fixture, vcx)
}

#[core::prelude::v1::test]
fn height_limited_videos_clear_the_top_bar_and_keep_the_transport_on_screen() {
    for (frame, window) in [
        ((1080, 1920), (1440.0, 900.0)),
        ((1920, 1080), (1440.0, 640.0)),
    ] {
        let mut cx = crate::tests::TestApp::single();
        let (_fixture, vcx) = framed_video(&mut cx, frame, window);
        let media = bounds(vcx, "viewer-media");
        let transport = bounds(vcx, "video-transport");
        assert!(
            (f32::from(media.top()) - (60.0 + 12.0)).abs() < 0.5
                && (f32::from(transport.top() - media.bottom()) - 12.0).abs() < 0.5
                && f32::from(transport.bottom()) <= window.1,
            "{frame:?} in {window:?}: the frame keeps a 12px gap under the top bar and \
             the whole transport stays on screen: media={media:?} transport={transport:?}"
        );
        let (w, h) = size(media);
        assert!(
            ((w / h) - frame.0 as f32 / frame.1 as f32).abs() < 0.02,
            "{frame:?}: the frame keeps the video's shape, got {w}x{h}"
        );
    }
}

#[core::prelude::v1::test]
fn info_panel_narrows_the_frame_and_the_transport_spans_the_stage() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, cx| {
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            1920,
            1080,
        ));
        app.inspector_open = true;
        app.window_bounds = Some((0.0, 0.0, 1440.0, 900.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    render_ui(vcx);
    render_ui(vcx);
    let media = bounds(vcx, "viewer-media");
    let transport = bounds(vcx, "video-transport");
    assert!(
        size(media).0 < 1120.0 && size(transport).0 == 1440.0 - 340.0,
        "with Info open the frame narrows and the transport spans the stage: \
         {media:?} {transport:?}"
    );
}

#[core::prelude::v1::test]
fn a_catalog_refresh_keeps_the_players_video_dimensions() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, _| {
        let mut played = app.selected_item.clone().expect("the video is shown");
        played.width = 1920;
        played.height = 1080;
        app.video.item = Some(played.clone());
        // A date/location edit or undo reloads the row, which has no size.
        let mut fresh = played;
        fresh.width = 0;
        fresh.height = 0;
        app.set_selected_from_catalog(fresh);
        let shown = app.selected_item.as_ref().unwrap();
        assert_eq!(
            (shown.width, shown.height),
            (1920, 1080),
            "the inspector must not fall back to Dimensions — after a reload"
        );
    });
}

// Loop range (F1). Most of these tests have no player: on macOS the backend is
// AVFoundation and the sample files do not exist, so the position is the
// tracked `playback_time` and a tick moves it the same way a seek does.

/// The open video playing at `start`, with `(in, out)` set through the real
/// commands at those positions.
fn with_range(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
    in_point: Option<f64>,
    out_point: Option<f64>,
) {
    view.update(vcx, |app, cx| {
        if let Some(at) = in_point {
            app.video.playback_time = at;
            app.set_loop_in(cx);
        }
        if let Some(at) = out_point {
            app.video.playback_time = at;
            app.set_loop_out(cx);
        }
    });
}

/// Plays one frame tick at `position` and returns where the playhead went.
fn tick_at(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
    position: f64,
) -> f64 {
    view.update(vcx, |app, cx| {
        app.video.is_playing = true;
        app.video.playback_time = position;
        app.handle_ui_message(crate::app::types::UiMessage::VideoFrameTick, cx);
        app.video.playback_time
    })
}

fn range(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
) -> (Option<f64>, Option<f64>) {
    view.update(vcx, |app, _| {
        (
            app.video.loop_range.in_point,
            app.video.loop_range.out_point,
        )
    })
}

fn toast_text(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
) -> Option<String> {
    view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()))
}

#[core::prelude::v1::test]
fn loop_range_seeks_back_to_in_point() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    assert_eq!(
        tick_at(&view, vcx, 15.0),
        15.0,
        "inside the range playback carries on"
    );
    assert_eq!(
        tick_at(&view, vcx, 20.0),
        10.0,
        "reaching the out point seeks back to the in point"
    );
}

#[core::prelude::v1::test]
fn clear_loop_range_restores_full_loop() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    view.update(vcx, |app, cx| app.clear_loop_range(cx));
    assert_eq!(range(&view, vcx), (None, None), "clear forgets both points");
    assert_eq!(
        tick_at(&view, vcx, 25.0),
        25.0,
        "with no range the old out point must not seek"
    );
}

#[core::prelude::v1::test]
fn out_before_in_is_refused() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), None);
    for at in [10.0, 5.0] {
        with_range(&view, vcx, None, Some(at));
        assert_eq!(
            range(&view, vcx),
            (Some(10.0), None),
            "an out point at {at}, not after the in point, must leave the range alone"
        );
    }
    assert_eq!(
        toast_text(&view, vcx),
        Some(crate::i18n::t("video.loop_end_before_start")),
        "a refused out point says why"
    );
}

#[core::prelude::v1::test]
fn in_after_out_is_refused() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, None, Some(20.0));
    for at in [20.0, 30.0] {
        with_range(&view, vcx, Some(at), None);
        assert_eq!(
            range(&view, vcx),
            (None, Some(20.0)),
            "an in point at {at}, not before the out point, must leave the range alone"
        );
    }
    assert_eq!(
        toast_text(&view, vcx),
        Some(crate::i18n::t("video.loop_start_after_end")),
        "a refused in point says why"
    );
}

#[core::prelude::v1::test]
fn an_out_point_at_the_start_and_an_in_point_at_the_end_are_refused() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, None, Some(0.0));
    with_range(&view, vcx, Some(60.0), None);
    assert_eq!(
        range(&view, vcx),
        (None, None),
        "the start acts as in and the end as out, so both would be empty ranges"
    );
    assert_eq!(
        toast_text(&view, vcx),
        Some(crate::i18n::t("video.loop_start_after_end")),
        "the refusal says why"
    );
}

#[core::prelude::v1::test]
fn in_point_alone_loops_from_the_end_of_the_video() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), None);
    assert_eq!(tick_at(&view, vcx, 40.0), 40.0, "the middle plays on");
    assert_eq!(tick_at(&view, vcx, 60.0), 10.0, "the end acts as out");
}

#[core::prelude::v1::test]
fn out_point_alone_loops_from_the_start() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, None, Some(20.0));
    assert_eq!(tick_at(&view, vcx, 20.0), 0.0, "the start acts as in");
}

#[core::prelude::v1::test]
fn seeking_past_the_out_point_wraps_only_while_playing() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    let paused = view.update(vcx, |app, cx| {
        app.video.is_playing = false;
        app.video.playback_time = 40.0;
        app.handle_ui_message(crate::app::types::UiMessage::VideoFrameTick, cx);
        app.video.playback_time
    });
    assert_eq!(paused, 40.0, "paused, a hand seek past the out point stays");
    assert_eq!(
        tick_at(&view, vcx, 40.0),
        10.0,
        "playing from past the out point goes back to the in point"
    );
}

/// Sets when the wrap seek in flight may be reissued.
fn set_settle(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
    until: std::time::Instant,
) {
    view.update(vcx, |app, _| {
        app.video.loop_range.settle_until = Some(until)
    });
}

#[core::prelude::v1::test]
fn a_seek_in_flight_is_not_restarted_on_every_tick() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    assert_eq!(tick_at(&view, vcx, 20.0), 10.0, "the first tick seeks");
    view.read_with(vcx, |app, _| {
        assert!(
            app.video.loop_range.settle_until.is_some(),
            "a wrap must start the hold, or every tick cancels the seek in flight"
        )
    });
    set_settle(
        &view,
        vcx,
        std::time::Instant::now() + std::time::Duration::from_secs(3600),
    );
    assert_eq!(
        tick_at(&view, vcx, 20.01),
        20.01,
        "a player still reporting the old position must not get a new seek, \
         which would cancel the one in flight"
    );
}

#[core::prelude::v1::test]
fn a_wrap_seek_is_reissued_once_the_watchdog_expires() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    assert_eq!(tick_at(&view, vcx, 20.0), 10.0, "the first tick seeks");
    set_settle(&view, vcx, std::time::Instant::now());
    assert_eq!(
        tick_at(&view, vcx, 20.01),
        10.0,
        "a seek that never landed is issued again when the watchdog runs out"
    );
}

#[core::prelude::v1::test]
fn the_settle_clears_once_the_position_is_back_in_range() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(10.1));
    assert_eq!(tick_at(&view, vcx, 10.1), 10.0, "the first tick seeks");
    assert_eq!(tick_at(&view, vcx, 10.05), 10.05, "the seek landed");
    assert_eq!(
        tick_at(&view, vcx, 10.1),
        10.0,
        "a landed seek must not hold back the next wrap, or a short range overshoots"
    );
}

/// The open video with a real, unloaded native player: its looping flag is
/// the bridge's own, and it reports itself as stopped.
fn with_stub_player(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) {
    use crate::player::VideoPlayerBackend;
    view.update(vcx, |app, _| {
        let mut player = crate::player::NativeVideoPlayer::new();
        player.set_looping(true);
        app.video.player = Some(player);
        app.video.is_looping = true;
    });
}

fn native_looping(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    use crate::player::VideoPlayerBackend;
    view.update(vcx, |app, _| {
        app.video
            .player
            .as_ref()
            .expect("the stub player is installed")
            .is_looping()
    })
}

#[core::prelude::v1::test]
fn native_loop_is_off_exactly_while_a_range_is_set() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_stub_player(&view, vcx);
    with_range(&view, vcx, Some(10.0), None);
    assert!(
        !native_looping(&view, vcx),
        "an in point must turn off the player's restart at zero"
    );
    view.update(vcx, |app, cx| app.clear_loop_range(cx));
    assert!(native_looping(&view, vcx), "clear gives the full loop back");
    with_range(&view, vcx, None, Some(20.0));
    assert!(
        !native_looping(&view, vcx),
        "an out point must turn off the player's restart at zero"
    );
}

#[core::prelude::v1::test]
fn l_with_a_range_set_keeps_the_native_loop_off() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    with_stub_player(&view, vcx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    for expected in [false, true] {
        keystrokes(vcx, "l");
        assert_eq!(
            view.update(vcx, |app, _| app.video.is_looping),
            expected,
            "L still toggles the Loop button"
        );
        assert!(
            !native_looping(&view, vcx),
            "L must not hand the end of the file back to the player"
        );
    }
    view.update(vcx, |app, cx| app.clear_loop_range(cx));
    assert!(
        native_looping(&view, vcx),
        "clearing applies the Loop button L left on"
    );
}

#[core::prelude::v1::test]
fn reaching_the_end_of_the_file_goes_back_to_the_in_point_and_keeps_playing() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_stub_player(&view, vcx);
    with_range(&view, vcx, Some(10.0), None);
    // The stub reports itself stopped at position 0: the "ended" path.
    let (time, playing) = view.update(vcx, |app, cx| {
        app.video.is_playing = true;
        app.handle_ui_message(crate::app::types::UiMessage::VideoFrameTick, cx);
        (app.video.playback_time, app.video.is_playing)
    });
    assert_eq!(time, 10.0, "the end of the file goes back to the in point");
    assert!(playing, "a range keeps playing past the end of the file");
}

#[core::prelude::v1::test]
fn setting_a_point_confirms_it_with_the_time() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(12.5), None);
    assert_eq!(
        toast_text(&view, vcx).as_deref(),
        Some("Loop start set to 0:12.50"),
    );
    with_range(&view, vcx, None, Some(20.0));
    assert_eq!(
        toast_text(&view, vcx).as_deref(),
        Some("Loop end set to 0:20.00"),
    );
}

#[core::prelude::v1::test]
fn a_non_finite_position_never_becomes_a_point() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(f64::NAN), Some(f64::NAN));
    assert_eq!(range(&view, vcx), (None, None), "NaN is not a position");
}

#[core::prelude::v1::test]
fn loop_keys_stop_at_the_viewer_and_never_reach_the_input_context() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    view.update(vcx, |app, _| app.video.playback_time = 12.0);
    for source in ["alt-i->ˆ", "alt-o->ø", "alt-x->≈"] {
        let propagated = vcx.update(|window, cx| {
            let keystroke = gpui::Keystroke::parse(source).expect("a valid keystroke");
            window
                .dispatch_event(
                    gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                        keystroke,
                        is_held: false,
                        prefer_character_input: false,
                    }),
                    cx,
                )
                .propagate
        });
        assert!(
            !propagated,
            "{source} must be consumed, or a dead key leaves a pending composition"
        );
    }
}

#[core::prelude::v1::test]
fn another_video_or_closing_the_viewer_drops_the_range() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    view.update(vcx, |app, cx| {
        let other = app.selected_item.clone().expect("the video is shown");
        app.load_video_player(&other, cx);
    });
    assert_eq!(
        range(&view, vcx),
        (None, None),
        "loading a video starts without a range"
    );

    with_range(&view, vcx, Some(10.0), Some(20.0));
    view.update(vcx, |app, cx| app.close_viewer(cx));
    assert_eq!(
        range(&view, vcx),
        (None, None),
        "closing the viewer ends the range"
    );
}

#[core::prelude::v1::test]
fn alt_i_sets_in_point_without_toggling_inspector() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    view.update(vcx, |app, _| app.video.playback_time = 12.0);
    let before = view.update(vcx, |app, _| app.inspector_open);
    keystrokes(vcx, "alt-i->ˆ");
    assert_eq!(range(&view, vcx).0, Some(12.0), "⌥I sets the in point");
    assert_eq!(
        view.update(vcx, |app, _| app.inspector_open),
        before,
        "⌥I must not toggle the inspector"
    );
}

#[core::prelude::v1::test]
fn alt_o_does_not_open_default_app() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    view.update(vcx, |app, _| app.video.playback_time = 12.0);
    keystrokes(vcx, "alt-o->ø");
    assert_eq!(range(&view, vcx).1, Some(12.0), "⌥O sets the out point");
    assert!(
        view.update(vcx, |app, _| app.error_message.is_none()),
        "⌥O must not try to open the (missing) sample file in the default app"
    );
}

#[core::prelude::v1::test]
fn alt_x_clears_the_range_without_rejecting() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    with_range(&view, vcx, Some(10.0), Some(20.0));
    let shown = view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id));
    keystrokes(vcx, "alt-x->≈");
    assert_eq!(range(&view, vcx), (None, None), "⌥X clears the range");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            shown,
            "⌥X must not act as the X reject key and move on"
        );
    });
}

#[core::prelude::v1::test]
fn loop_keys_do_nothing_in_the_palette_or_a_text_field() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| {
        app.video.playback_time = 12.0;
        app.open_command_palette(cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "alt-i->ˆ");
    assert_eq!(range(&view, vcx), (None, None), "the palette owns the keys");
    view.update(vcx, |app, cx| {
        app.close_command_palette(cx);
        app.search_focused = true;
        cx.notify();
    });
    render_ui(vcx);
    keystrokes(vcx, "alt-o->ø");
    assert_eq!(
        range(&view, vcx),
        (None, None),
        "a text field owns the keys"
    );
}

#[core::prelude::v1::test]
fn loop_keys_ignore_a_still_in_the_viewer() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| {
        let still = get_sample_items()
            .into_iter()
            .find(|item| item.media_type != MediaType::Video)
            .expect("the sample set has a still");
        app.open_viewer(still, cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "alt-i");
    assert_eq!(range(&view, vcx), (None, None), "a photo has no range");
}

#[core::prelude::v1::test]
fn palette_offers_loop_commands_only_for_an_open_video() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    let titles = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| {
            app.command_query = "loop".into();
            app.palette_rows()
                .iter()
                .map(|row| format!("{row:?}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
    };
    let open = titles(&view, vcx);
    for title in ["Set Loop Start", "Set Loop End", "Clear Loop Range"] {
        assert!(open.contains(title), "{title} is offered: {open}");
    }
    view.update(vcx, |app, cx| app.close_viewer(cx));
    let closed = titles(&view, vcx);
    assert!(
        !closed.contains("Loop Start"),
        "with no video open the loop commands are hidden: {closed}"
    );
}

#[core::prelude::v1::test]
fn scrubber_draws_the_range() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    assert!(
        vcx.debug_bounds("video-loop-range").is_none(),
        "no range, no band"
    );
    with_range(&view, vcx, Some(15.0), Some(45.0));
    view.update(vcx, |_, cx| cx.notify());
    render_ui(vcx);
    let track = bounds(vcx, "video-scrubber");
    let band = bounds(vcx, "video-loop-range");
    let width = f32::from(track.size.width);
    assert!(
        (f32::from(band.left() - track.left()) / width - 0.25).abs() < 0.02
            && (f32::from(band.size.width) / width - 0.5).abs() < 0.02,
        "15–45 s of 60 s spans the middle half: {band:?} on {track:?}"
    );
}

/// The open video, playing on a real unloaded player, at 30 s of 60 s.
fn play_at_30(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) {
    view.update(vcx, |app, cx| {
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.video.is_playing = true;
        app.video.playback_time = 30.0;
        cx.notify();
    });
}

#[core::prelude::v1::test]
fn switching_library_stops_the_viewer_video() {
    let mut cx = crate::tests::TestApp::single();
    let (fixture, view, vcx) = open_video(&mut cx);
    play_at_30(&view, vcx);
    let other = fixture.0.join("other_library");
    std::fs::create_dir_all(&other).expect("create the second library");
    view.update(vcx, |app, cx| {
        let id = app.video.item.as_ref().expect("a video is open").id;
        let state = app.grid_state.as_ref().expect("a library is open");
        // The sample file does not exist, so its load cleared this; a real
        // load leaves the video tracked. Its point survives the prune on
        // close because this sample is among the seeded rows.
        state.video_resume.begin(id);
        let positions = state.video_resume.positions.clone();
        app.is_immersive_fullscreen = true;
        app.open_library_inner(other.clone(), cx);
        assert_eq!(
            positions.get(id),
            Some(30.0),
            "stopping for the switch must keep the video's resume point"
        );
        assert!(
            app.video.player.is_none() && app.video.item.is_none() && !app.video.is_playing,
            "a video from the closed library must not keep playing with no window"
        );
        assert!(
            !app.is_fullscreen_viewer && !app.is_immersive_fullscreen,
            "the new library opens on its grid, not in a leftover viewer mode"
        );
    });
}

#[core::prelude::v1::test]
fn a_library_that_fails_to_open_leaves_the_video_playing() {
    let mut cx = crate::tests::TestApp::single();
    let (fixture, view, vcx) = open_video(&mut cx);
    play_at_30(&view, vcx);
    // A file where the library folder should be: its catalog cannot be created.
    let not_a_folder = fixture.0.join("not_a_folder");
    std::fs::write(&not_a_folder, b"x").expect("write the blocking file");
    view.update(vcx, |app, cx| {
        app.open_library_inner(not_a_folder.clone(), cx);
        assert!(
            app.video.player.is_some() && app.video.is_playing && app.is_fullscreen_viewer,
            "the current library stays open, so its video keeps playing"
        );
    });
}

#[core::prelude::v1::test]
fn resume_points_do_not_follow_into_another_library() {
    let mut cx = crate::tests::TestApp::single();
    let (fixture, view, vcx) = open_video(&mut cx);
    let other = fixture.0.join("other_library");
    std::fs::create_dir_all(&other).expect("create the second library");
    view.update(vcx, |app, cx| {
        // Media ids restart in every library: id 2 here is another video there.
        let id = lumina_core::MediaId(2);
        app.config.resume_positions.record(id, 30.0, 60.0);
        app.open_library_inner(other.clone(), cx);
        let state = app.grid_state.as_ref().expect("the second library is open");
        assert_eq!(
            state.video_resume.saved(id),
            None,
            "a resume point from one library must not start another library's video"
        );
        assert!(
            app.config.resume_positions.is_empty(),
            "the saved config must not carry the old library's points"
        );
        assert_eq!(app.config.resume_library.as_deref(), Some(other.as_path()));
        // Reopening the same library, by a different spelling of its path,
        // keeps its points.
        app.config.resume_positions.record(id, 30.0, 60.0);
        app.open_library_inner(other.join("."), cx);
        let state = app.grid_state.as_ref().expect("the library reopened");
        assert_eq!(state.video_resume.saved(id), Some(30.0));
    });
}

#[core::prelude::v1::test]
fn an_older_config_keeps_its_resume_points_for_the_last_library() {
    let mut cx = crate::tests::TestApp::single();
    let (fixture, view, vcx) = open_video(&mut cx);
    let last = fixture.0.join("last_library");
    std::fs::create_dir_all(&last).expect("create the last library");
    view.update(vcx, |app, cx| {
        let id = lumina_core::MediaId(2);
        // A config written before points were tagged with their library.
        app.config.resume_library = None;
        app.config.last_library_path = Some(last.clone());
        app.config.resume_positions.record(id, 30.0, 60.0);
        app.open_library_inner(last.clone(), cx);
        let state = app.grid_state.as_ref().expect("the last library is open");
        assert_eq!(
            state.video_resume.saved(id),
            Some(30.0),
            "opening the library an untagged config was last used with keeps its points"
        );
        assert_eq!(app.config.resume_library.as_deref(), Some(last.as_path()));
    });
}

#[core::prelude::v1::test]
fn video_and_transport_fit_above_the_filmstrip_at_the_minimum_window() {
    let mut cx = crate::tests::TestApp::single();
    let (fixture, view, vcx) = open_video(&mut cx);
    // The sample video sits outside the seeded view; give it a position so
    // the strip shows (its slots are the seeded items, which is enough for
    // layout).
    view.update(vcx, |app, cx| {
        app.selected_index = Some(0);
        app.viewer_image = Some(crate::app::types::generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            1920,
            1080,
        ));
        app.window_bounds = Some((0.0, 0.0, 960.0, 600.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(960.0), gpui::px(600.0)));
    render_ui(vcx);
    render_ui(vcx);
    let _fixture = fixture;
    let media = bounds(vcx, "viewer-media");
    let transport = bounds(vcx, "video-transport");
    let strip = bounds(vcx, "viewer-filmstrip");
    assert!(
        media.top() >= gpui::px(60.0) && transport.top() >= media.bottom(),
        "the frame sits under the top bar with the transport below: {media:?} {transport:?}"
    );
    assert!(
        transport.bottom() <= strip.top(),
        "the transport stays above the filmstrip at 960x600: {transport:?} strip={strip:?}"
    );
    for arrow in ["viewer-prev", "viewer-next"] {
        assert!(
            !bounds(vcx, arrow).intersects(&media),
            "{arrow} stays beside the video at 960x600"
        );
    }
}

#[core::prelude::v1::test]
fn video_top_bar_shows_title_meta_and_pip() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_video(&mut cx);
    view.update(vcx, |app, _| app.selected_index = Some(0));
    render_ui(vcx);

    let item = view.update(vcx, |app, _| app.selected_item.clone().expect("video open"));
    let (title, meta) = crate::views::viewer::top_bar::video_title_meta(&item);
    assert_eq!(
        title,
        crate::views::inspector::body::display_title(&item),
        "the video top bar shows the display title"
    );
    assert!(
        meta.contains(&item.format_timestamp("%-d %b %Y")),
        "the meta line carries the capture date: {meta}"
    );
    if item.width > 0 && item.height > 0 {
        assert!(
            meta.contains(&crate::views::inspector::summary::resolution_label(
                item.width,
                item.height
            )),
            "the meta line carries the resolution label: {meta}"
        );
    }
    let back = bounds(vcx, "viewer-back");
    let title_b = bounds(vcx, "viewer-title");
    let meta_b = bounds(vcx, "viewer-meta");
    assert!(
        title_b.left() >= back.right() && meta_b.left() >= back.right(),
        "title and meta sit after Back on the left"
    );
    assert!(
        vcx.debug_bounds("viewer-position").is_none(),
        "a video top bar has no position counter"
    );
    assert!(
        vcx.debug_bounds("viewer-trim-btn").is_none(),
        "no trim control until trimming exists"
    );
    assert_eq!(size(bounds(vcx, "viewer-pip-btn")), (30.0, 30.0));
    click(vcx, "viewer-pip-btn");
    assert!(
        view.update(vcx, |app, _| app.video.in_pip),
        "the top bar PiP button hands the video to PiP"
    );
}

#[core::prelude::v1::test]
fn volume_slider_sets_player_volume() {
    use crate::player::VideoPlayerBackend;
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_stub_player(&view, vcx);
    render_ui(vcx);

    let slider = bounds(vcx, "video-volume");
    assert_eq!(size(slider).0, 62.0, "the slider is 62px wide");
    assert!(
        bounds(vcx, "video-mute").right() <= slider.left()
            && slider.right() <= bounds(vcx, "video-loop").left(),
        "the slider follows mute in the right group"
    );
    vcx.simulate_click(
        gpui::point(slider.left() + slider.size.width * 0.25, slider.center().y),
        gpui::Modifiers::default(),
    );
    let (volume, player) = view.update(vcx, |app, _| {
        (
            app.video.volume,
            app.video.player.as_ref().map(|p| p.volume()),
        )
    });
    assert!(
        (volume - 0.25).abs() < 0.03,
        "a click at a quarter sets the volume to 0.25, got {volume}"
    );
    assert!(
        player.is_some_and(|p| (p - volume).abs() < 0.01),
        "the player follows the slider: {player:?}"
    );
    assert_eq!(
        view.update(vcx, |app, _| app.config.video_volume),
        Some(volume),
        "the volume is kept for the next video"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k v o l u m e space u p enter");
    let up = view.update(vcx, |app, _| app.video.volume);
    assert!(
        (up - (volume + 0.1)).abs() < 0.001,
        "palette Volume Up raises the volume by 0.1: {volume} -> {up}"
    );
    for _ in 0..12 {
        keystrokes(vcx, "up");
    }
    assert_eq!(
        view.update(vcx, |app, _| app.video.volume),
        1.0,
        "↑ raises the volume, clamped to 1.0"
    );
    keystrokes(vcx, "down");
    assert!(
        (view.update(vcx, |app, _| app.video.volume) - 0.9).abs() < 0.001,
        "↓ lowers the volume by 0.1"
    );
    view.update(vcx, |app, _| app.command_query = "volume".into());
    let titles = view.update(vcx, |app, _| {
        app.palette_rows()
            .into_iter()
            .map(|r| r.title)
            .collect::<Vec<_>>()
    });
    assert!(
        titles.iter().any(|t| t == "Volume Down"),
        "the palette offers Volume Down: {titles:?}"
    );
}

#[core::prelude::v1::test]
fn a_volume_save_error_shows_in_the_banner() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    crate::app::state::SAVE_FAILS.with(|fails| fails.set(true));
    view.update(vcx, |app, cx| app.set_video_volume(0.3, cx));
    crate::app::state::SAVE_FAILS.with(|fails| fails.set(false));
    assert!(
        view.update(vcx, |app, _| app.error_message.is_some()),
        "a failed volume save is surfaced, not swallowed"
    );
}

fn saves() -> usize {
    crate::app::state::SAVE_COUNT.with(std::cell::Cell::get)
}

#[core::prelude::v1::test]
fn a_volume_drag_saves_once_on_release() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_stub_player(&view, vcx);
    render_ui(vcx);
    let slider = bounds(vcx, "video-volume");
    let at = |pct: f32| gpui::point(slider.left() + slider.size.width * pct, slider.center().y);
    let before = saves();
    vcx.simulate_mouse_down(
        at(0.25),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    vcx.simulate_mouse_move(
        at(0.75),
        Some(gpui::MouseButton::Left),
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    let mid = view.update(vcx, |app, _| app.video.volume);
    assert!((mid - 0.75).abs() < 0.03, "the drag sets the level: {mid}");
    assert_eq!(saves(), before, "nothing is saved mid-drag");
    vcx.simulate_mouse_up(
        at(0.75),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    assert_eq!(saves(), before + 1, "the release saves once");
    assert_eq!(
        view.update(vcx, |app, _| app.config.video_volume),
        Some(mid),
        "the saved level is where the drag ended"
    );
    let far = bounds(vcx, "video-play").center();
    vcx.simulate_mouse_down(
        gpui::point(far.x, far.y - gpui::px(200.0)),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    vcx.simulate_mouse_up(
        gpui::point(far.x, far.y - gpui::px(200.0)),
        gpui::MouseButton::Left,
        gpui::Modifiers::default(),
    );
    render_ui(vcx);
    assert_eq!(saves(), before + 1, "an unrelated mouse-up saves nothing");
}

#[core::prelude::v1::test]
fn volume_keys_are_plain_and_an_unchanged_level_is_not_saved() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_stub_player(&view, vcx);
    view.update(vcx, |app, cx| app.set_video_volume(1.0, cx));
    focus_app(vcx, &view);
    let before = saves();
    keystrokes(vcx, "up");
    assert_eq!(saves(), before, "↑ at full volume writes nothing");
    for chord in ["alt-down", "shift-down"] {
        keystrokes(vcx, chord);
        assert_eq!(
            view.update(vcx, |app, _| app.video.volume),
            1.0,
            "{chord} does not change the volume"
        );
    }
    keystrokes(vcx, "down");
    assert!((view.update(vcx, |app, _| app.video.volume) - 0.9).abs() < 0.001);
    assert_eq!(saves(), before + 1, "a real change saves once");
}

#[core::prelude::v1::test]
fn palette_steps_frames_for_an_open_video() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    with_stub_player(&view, vcx);
    focus_app(vcx, &view);
    for (query, label) in [
        ("secondary-k n e x t space f r a m e enter", "Next Frame"),
        (
            "secondary-k p r e v i o u s space f r a m e enter",
            "Previous Frame",
        ),
    ] {
        view.update(vcx, |app, _| {
            app.video.step_from = None;
            app.video.is_playing = true;
        });
        keystrokes(vcx, query);
        view.update(vcx, |app, _| {
            assert!(
                app.video.step_from.is_some(),
                "{label} runs a frame step from the palette"
            );
            assert!(!app.video.is_playing, "{label} pauses like , and .");
        });
    }
    view.update(vcx, |app, cx| app.close_viewer(cx));
    let titles = view.update(vcx, |app, _| {
        app.command_query = "frame".into();
        app.palette_rows()
            .into_iter()
            .map(|r| r.title)
            .collect::<Vec<_>>()
    });
    assert!(
        !titles.iter().any(|t| t.ends_with("Frame")),
        "frame steps are offered only for an open video: {titles:?}"
    );
}
