//! v3 video player (FIXES §3): slim top bar, viewer right-click menu, and the
//! three-zone transport card.

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

fn center_x(b: Bounds<Pixels>) -> f32 {
    f32::from(b.origin.x) + f32::from(b.size.width) / 2.0
}

fn center_y(b: Bounds<Pixels>) -> f32 {
    f32::from(b.origin.y) + f32::from(b.size.height) / 2.0
}

#[core::prelude::v1::test]
fn transport_is_a_three_zone_card_with_32px_controls_and_a_40px_play() {
    let mut cx = gpui::TestAppContext::single();
    let (_dir, _view, vcx) = open_video(&mut cx);

    let card = bounds(vcx, "video-transport");
    assert!(
        f32::from(card.size.width) <= 1040.0,
        "the transport card is capped at the 1040px video width"
    );

    let order = [
        "video-mute",
        "video-speed",
        "video-seek-back",
        "video-step-back",
        "video-play",
        "video-step-fwd",
        "video-seek-fwd",
        "video-loop",
        "video-pip",
        "video-fullscreen",
    ];
    let row: Vec<_> = order.iter().map(|s| bounds(vcx, s)).collect();
    for (selector, b) in order.iter().zip(&row) {
        assert!(
            card.contains(&b.center()),
            "{selector} sits inside the card"
        );
        assert!(
            (center_y(*b) - center_y(row[0])).abs() < 1.0,
            "{selector} shares the single control row"
        );
        match *selector {
            "video-play" => assert_eq!(size(*b), (40.0, 40.0), "play is a 40px round button"),
            "video-speed" => {}
            _ => assert_eq!(size(*b), (32.0, 32.0), "{selector} is a 32px hit area"),
        }
    }
    for pair in row.windows(2) {
        assert!(
            center_x(pair[0]) < center_x(pair[1]),
            "controls follow [mute·speed] | [−5s·frame·play·frame·+5s] | [loop·PiP·full]"
        );
    }
    assert!(
        (center_x(row[4]) - center_x(card)).abs() < 2.0,
        "play is centred in the card"
    );

    let scrubber = bounds(vcx, "video-scrubber");
    assert!(
        center_y(scrubber) < center_y(row[0]),
        "the scrubber row sits above the controls"
    );
    assert!(
        vcx.debug_bounds("video-volume-slider").is_none(),
        "mute is a toggle only; there is no volume slider"
    );
}

#[core::prelude::v1::test]
fn played_fill_tracks_the_playhead() {
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    assert_eq!(
        crate::views::viewer::utils::format_playback_speed(1.0),
        "1×"
    );
}

#[core::prelude::v1::test]
fn top_bar_has_only_favorite_and_info_and_actions_live_in_the_right_click_menu() {
    let mut cx = gpui::TestAppContext::single();
    let (_dir, view, vcx) = open_video(&mut cx);

    for selector in ["viewer-fav-btn", "viewer-info-btn"] {
        assert_eq!(
            size(bounds(vcx, selector)),
            (28.0, 28.0),
            "{selector} is icon-only"
        );
    }
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
fn prev_next_sit_40px_inside_the_frame_and_centred_on_it() {
    let mut cx = gpui::TestAppContext::single();
    let (_dir, _view, vcx) = open_video(&mut cx);

    // V3Video: 40px from the window edges, centred on the (narrower) frame.
    let frame = bounds(vcx, "viewer-viewport");
    let prev = bounds(vcx, "viewer-prev");
    let next = bounds(vcx, "viewer-next");
    let window_w = vcx.update(|window, _| window.viewport_size().width);
    assert_eq!(f32::from(prev.left()), 40.0);
    assert_eq!(f32::from(window_w - next.right()), 40.0);
    for b in [prev, next] {
        assert!(
            (center_y(b) - center_y(frame)).abs() < 1.0,
            "nav buttons are vertically centred on the video frame"
        );
    }
}

#[core::prelude::v1::test]
fn viewer_menu_owns_the_keyboard_and_acts_on_the_shown_item_only() {
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
fn video_frame_is_1040_wide_with_the_transport_right_below() {
    let mut cx = gpui::TestAppContext::single();
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

    // V3Video: a 1040x585 frame, then the transport card 12px below it.
    let media = bounds(vcx, "viewer-media");
    let transport = bounds(vcx, "video-transport");
    let (w, h) = size(media);
    assert!(
        (w - 1040.0).abs() < 1.0 && (h - 585.0).abs() < 1.5,
        "a 16:9 video fills a 1040x585 frame: {media:?}"
    );
    let gap = f32::from(transport.top() - media.bottom());
    assert!(
        (8.0..=16.0).contains(&gap),
        "the transport sits right below the video, got a {gap}px gap"
    );
    assert!(
        (f32::from(transport.left() - media.left())).abs() < 1.0
            && (size(transport).0 - w).abs() < 1.0,
        "the transport is exactly as wide as the video: {transport:?} {media:?}"
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
        let mut cx = gpui::TestAppContext::single();
        let (_fixture, vcx) = framed_video(&mut cx, frame, window);
        let media = bounds(vcx, "viewer-media");
        let transport = bounds(vcx, "video-transport");
        assert!(
            (f32::from(media.top()) - (52.0 + 12.0)).abs() < 0.5
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
fn transport_matches_a_frame_narrowed_by_the_info_drawer() {
    let mut cx = gpui::TestAppContext::single();
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
        size(media).0 < 1040.0 && (size(transport).0 - size(media).0).abs() < 1.0,
        "with Info open the transport narrows with the frame: {media:?} {transport:?}"
    );
}

#[core::prelude::v1::test]
fn a_catalog_refresh_keeps_the_players_video_dimensions() {
    let mut cx = gpui::TestAppContext::single();
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
