//! The immersive viewer's HUD hides after 2 s idle and returns on pointer
//! movement or a key; the windowed viewer always keeps its chrome.

use gpui::{Modifiers, VisualTestContext};

use crate::app::viewer_actions::HUD_IDLE;
use crate::tests::helpers::{app_with_library, keystrokes, render_ui, right_click, seed_library};
use crate::LuminaApp;
use crate::*;

fn idle(vcx: &mut VisualTestContext, by: std::time::Duration) {
    vcx.executor().advance_clock(by);
    vcx.run_until_parked();
    render_ui(vcx);
}

fn hud(view: &gpui::Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.immersive_hud_visible)
}

fn open_viewer<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    crate::tests::helpers::TempLibrary,
    gpui::Entity<LuminaApp>,
    &'a mut VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let (view, vcx) = app_with_library(cx, engine);
    keystrokes(vcx, "home enter");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "Enter opens the viewer")
    });
    (dir, view, vcx)
}

#[core::prelude::v1::test]
fn the_immersive_hud_hides_after_two_idle_seconds_and_wakes_on_input() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_idle", &mut cx);
    view.update(vcx, |app, cx| app.enter_immersive_fullscreen(None, cx));
    render_ui(vcx);

    idle(vcx, HUD_IDLE - std::time::Duration::from_millis(100));
    assert!(hud(&view, vcx), "the HUD stays before 2 s of idle");
    assert!(
        vcx.debug_bounds("viewer-top-bar").is_some(),
        "the visible HUD paints its top bar"
    );
    idle(vcx, std::time::Duration::from_millis(200));
    assert!(!hud(&view, vcx), "the HUD hides after 2 s of idle");
    assert!(
        vcx.debug_bounds("viewer-top-bar").is_none(),
        "the hidden HUD's top bar is no longer painted"
    );

    let stage = vcx
        .debug_bounds("viewer-viewport")
        .expect("viewer-viewport");
    vcx.simulate_mouse_move(stage.center(), None, Modifiers::none());
    render_ui(vcx);
    assert!(hud(&view, vcx), "moving the pointer brings the HUD back");
    idle(vcx, HUD_IDLE);
    assert!(
        !hud(&view, vcx),
        "and it hides again after another idle 2 s"
    );

    keystrokes(vcx, "right");
    assert!(hud(&view, vcx), "a key brings the HUD back");
    keystrokes(vcx, "h");
    assert!(!hud(&view, vcx), "H still hides it by hand");
}

#[core::prelude::v1::test]
fn the_windowed_viewer_never_hides_its_chrome() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_windowed", &mut cx);
    let stage = vcx
        .debug_bounds("viewer-viewport")
        .expect("viewer-viewport");
    vcx.simulate_mouse_move(stage.center(), None, Modifiers::none());
    idle(vcx, HUD_IDLE * 3);
    assert!(hud(&view, vcx), "auto-hide is immersive only");
    view.update(vcx, |app, _| assert!(!app.is_immersive_fullscreen));
}

#[core::prelude::v1::test]
fn the_hud_stays_under_the_pointer_and_over_an_open_menu() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_stays", &mut cx);
    view.update(vcx, |app, cx| app.enter_immersive_fullscreen(None, cx));
    render_ui(vcx);

    let bar = vcx.debug_bounds("viewer-top-bar").expect("viewer-top-bar");
    vcx.simulate_mouse_move(bar.center(), None, Modifiers::none());
    idle(vcx, HUD_IDLE * 2);
    assert!(hud(&view, vcx), "the HUD never hides under the pointer");

    right_click(vcx, "viewer-viewport");
    view.update(vcx, |app, _| assert!(app.context_menu_state.is_some()));
    idle(vcx, HUD_IDLE * 2);
    assert!(hud(&view, vcx), "the HUD never hides behind an open menu");
}

#[core::prelude::v1::test]
fn a_scrub_or_the_speed_menu_holds_the_hud_and_it_hides_once_they_end() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_scrub", &mut cx);
    view.update(vcx, |app, cx| app.enter_immersive_fullscreen(None, cx));
    render_ui(vcx);

    for hold in ["scrub", "speed menu"] {
        view.update(vcx, |app, cx| {
            app.wake_immersive_hud(cx);
            match hold {
                "scrub" => app.video.is_scrubbing = true,
                _ => app.video.speed_menu_open = true,
            }
        });
        idle(vcx, HUD_IDLE * 3);
        assert!(
            hud(&view, vcx),
            "the HUD stays during a {hold}, or the transport unmounts under it"
        );
        // Ends without a pointer move (release off the bar, or a key).
        view.update(vcx, |app, _| {
            app.video.is_scrubbing = false;
            app.video.speed_menu_open = false;
        });
        idle(vcx, HUD_IDLE * 2);
        assert!(
            !hud(&view, vcx),
            "once the {hold} ends the HUD still hides, with no new input"
        );
    }
}

#[core::prelude::v1::test]
fn a_menu_closed_by_escape_lets_the_hud_hide() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_escape_menu", &mut cx);
    view.update(vcx, |app, cx| app.enter_immersive_fullscreen(None, cx));
    render_ui(vcx);
    right_click(vcx, "viewer-viewport");
    idle(vcx, HUD_IDLE * 2);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.context_menu_state.is_none(), "Escape closes the menu");
        assert!(app.is_immersive_fullscreen, "and only the menu");
    });
    idle(vcx, HUD_IDLE * 2);
    assert!(!hud(&view, vcx), "with the menu gone the HUD hides again");
}

#[core::prelude::v1::test]
fn the_hud_never_hides_under_a_pointer_resting_on_it_after_reentering() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_reenter", &mut cx);
    let bar = vcx.debug_bounds("viewer-top-bar").expect("viewer-top-bar");
    vcx.simulate_mouse_move(bar.center(), None, Modifiers::none());
    view.update(vcx, |app, cx| {
        app.enter_immersive_fullscreen(None, cx);
        app.exit_immersive_fullscreen(None, cx);
        app.enter_immersive_fullscreen(None, cx);
    });
    idle(vcx, HUD_IDLE * 2);
    assert!(
        hud(&view, vcx),
        "the pointer never left the bar, so the HUD must stay"
    );
}

#[core::prelude::v1::test]
fn i_brings_the_hud_back_with_the_info_drawer() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = open_viewer("hud_info_key", &mut cx);
    view.update(vcx, |app, cx| app.enter_immersive_fullscreen(None, cx));
    idle(vcx, HUD_IDLE * 2);
    assert!(!hud(&view, vcx));
    keystrokes(vcx, "i");
    assert!(
        hud(&view, vcx),
        "I wakes the HUD, or the drawer it opens stays invisible"
    );
}

#[core::prelude::v1::test]
fn test_immersive_fullscreen_mode_lifecycle() {
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));

    view.update(cx, |app, cx| {
        app.grid_state = None;

        // 1. Initial State
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // 2. Enter Immersive Fullscreen
        app.enter_immersive_fullscreen(None, cx);
        assert!(app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // 3. Toggle HUD visibility
        app.toggle_immersive_hud(cx);
        assert!(!app.immersive_hud_visible);
        app.toggle_immersive_hud(cx);
        assert!(app.immersive_hud_visible);

        // 4. Toggle Immersive Fullscreen (exits)
        app.toggle_immersive_fullscreen(None, cx);
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);

        // 5. Toggle Immersive Fullscreen back on
        app.toggle_immersive_fullscreen(None, cx);
        assert!(app.is_immersive_fullscreen);

        // 6. Close viewer resets immersive fullscreen
        app.close_viewer(cx);
        assert!(!app.is_immersive_fullscreen);
        assert!(app.immersive_hud_visible);
    });
    cx.run_until_parked();
}
