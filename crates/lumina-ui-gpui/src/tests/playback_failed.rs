//! Playback failed: a video the player cannot open says so in
//! the viewer and in PiP, with Try Again and Reveal in Finder.

use crate::app::playback_failed::VIDEO_LOADS;
use crate::app::types::get_sample_items;
use crate::app::viewer_actions::REVEALED_PATHS;
use crate::tests::helpers::{
    app_with_library, click, focus_app, keystrokes, render_ui, right_click, seed_library,
};
use lumina_core::{MediaItem, MediaType};

/// Removes the temp library on success and failure.

/// A library plus a video item whose file exists but is not a video, so
/// every load fails (for real on macOS, and with the dummy player elsewhere).
fn broken_video(
    prefix: &str,
) -> (
    super::helpers::TempLibrary,
    std::sync::Arc<lumina_core::LuminaEngine>,
    MediaItem,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let path = dir.join("broken.mp4");
    std::fs::write(&path, b"not a video").unwrap();
    let mut video = get_sample_items()
        .into_iter()
        .find(|item| item.media_type == MediaType::Video)
        .expect("the deterministic sample set must contain a video");
    video.path = path;
    (dir, engine, video)
}

fn loads() -> usize {
    VIDEO_LOADS.with(|n| n.get())
}

#[core::prelude::v1::test]
fn video_load_failure_says_so() {
    let (_fixture, engine, video) = broken_video("playback_failed");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    REVEALED_PATHS.with(|p| p.borrow_mut().clear());

    view.update(vcx, |app, cx| app.open_viewer(video.clone(), cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-playback-failed").is_some(),
        "a video that fails to load must say so, not show an empty stage"
    );
    assert!(
        vcx.debug_bounds("playback-failed-title-text:Can't play this clip")
            .is_some(),
        "the card names the problem"
    );
    assert!(
        vcx.debug_bounds("playback-failed-error").is_some(),
        "the player's error string shows under the body"
    );
    assert!(
        vcx.debug_bounds("viewer-center-play").is_none(),
        "no play button over a clip that cannot play"
    );

    right_click(vcx, "viewer-playback-failed");
    for selector in [
        "open-with-menu-btn",
        "reveal-menu-btn",
        "edit-external-menu-btn",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "right-clicking the card still opens the viewer menu ({selector})"
        );
    }
    view.update(vcx, |app, _| app.context_menu_state = None);
    render_ui(vcx);

    let before = loads();
    click(vcx, "playback-retry");
    assert_eq!(loads(), before + 1, "Try Again loads the clip again");
    assert!(
        vcx.debug_bounds("viewer-playback-failed").is_some(),
        "a retry that fails again keeps saying so"
    );

    click(vcx, "playback-reveal");
    REVEALED_PATHS.with(|p| {
        assert_eq!(
            *p.borrow(),
            std::slice::from_ref(&video.path),
            "Reveal in Finder reveals the clip's file"
        );
    });

    // Keyboard: Tab reaches Try Again, ↵ presses it; Tab again, Reveal.
    focus_app(vcx, &view);
    let before = loads();
    keystrokes(vcx, "tab enter");
    assert_eq!(loads(), before + 1, "Tab then ↵ presses Try Again");
    keystrokes(vcx, "enter");
    assert_eq!(
        loads(),
        before + 2,
        "a failed retry keeps Try Again focused, so ↵ tries again"
    );
    keystrokes(vcx, "tab enter");
    assert_eq!(
        REVEALED_PATHS.with(|p| p.borrow().len()),
        2,
        "Tab moves to Reveal in Finder and ↵ presses it"
    );

    // Another item: the failure belongs to the clip that failed.
    let other = view.update(vcx, |app, _| {
        let items = app
            .grid_state
            .as_ref()
            .unwrap()
            .engine
            .get_item_slice(0..10);
        items
            .into_iter()
            .find(|item| item.media_type == MediaType::Image)
            .expect("the seeded library has photos")
    });
    view.update(vcx, |app, cx| app.open_viewer(other, cx));
    render_ui(vcx);
    assert!(
        view.update(vcx, |app, _| app.video.load_error.is_none()),
        "moving to another item clears the load error"
    );
    assert!(
        vcx.debug_bounds("viewer-playback-failed").is_none(),
        "the next item does not inherit the failure card"
    );

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer, "Esc still closes the viewer");
    });
}

#[core::prelude::v1::test]
fn pip_load_failure_shows_a_compact_message() {
    let (_fixture, engine, video) = broken_video("playback_failed_pip");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.play_in_pip(video, cx));
    render_ui(vcx);
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    render_ui(&mut pip);
    assert!(
        pip.debug_bounds("pip-playback-failed").is_some(),
        "a failed load in PiP shows a message, never an empty black window"
    );
    let before = loads();
    let retry = pip
        .debug_bounds("pip-playback-retry")
        .expect("PiP offers Try Again");
    pip.simulate_click(retry.center(), gpui::Modifiers::default());
    render_ui(&mut pip);
    assert_eq!(loads(), before + 1, "PiP's Try Again loads the clip again");
    assert!(
        pip.debug_bounds("pip-playback-failed").is_some(),
        "a retry that fails again keeps saying so"
    );

    // Keyboard: ↵ in the PiP window is Try Again while the load failed.
    pip.simulate_keystrokes("enter");
    pip.run_until_parked();
    assert_eq!(loads(), before + 2, "↵ in PiP retries the failed clip");

    // A portrait PiP (220 wide) still fits Try Again inside the window.
    pip.simulate_resize(gpui::size(gpui::px(220.0), gpui::px(390.0)));
    render_ui(&mut pip);
    let window = pip.update(|window, _| window.viewport_size());
    let retry = pip
        .debug_bounds("pip-playback-retry")
        .expect("a narrow PiP still offers Try Again");
    assert!(
        retry.left() >= gpui::px(0.0)
            && retry.right() <= window.width
            && retry.top() >= gpui::px(0.0)
            && retry.bottom() <= window.height,
        "Try Again must sit fully inside a 220 px PiP: {retry:?} in {window:?}"
    );
}

#[core::prelude::v1::test]
fn failed_video_transport_is_inert() {
    let (_fixture, engine, video) = broken_video("playback_failed_inert");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_viewer(video, cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("video-transport-disabled").is_some(),
        "a clip that failed to load dims its transport"
    );

    click(vcx, "video-play");
    assert!(
        !view.update(vcx, |app, _| app.video.is_playing),
        "the Play button must not start a clip that failed to load"
    );
    focus_app(vcx, &view);
    for key in ["space", "k"] {
        keystrokes(vcx, key);
        view.update(vcx, |app, _| {
            assert!(
                !app.video.is_playing,
                "{key} must not start a clip that failed to load"
            );
            assert!(app.is_fullscreen_viewer, "{key} must not close the viewer");
        });
    }
}

#[core::prelude::v1::test]
fn failed_video_arrows_navigate_and_escape_closes() {
    let (_fixture, engine, video) = broken_video("playback_failed_nav");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_item_at(video.clone(), 0, cx);
        app.open_viewer(video.clone(), cx);
    });
    render_ui(vcx);
    assert!(view.update(vcx, |app, _| app.video_unplayable()));
    focus_app(vcx, &view);

    keystrokes(vcx, "right");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, Some(1), "→ moves to the next item");
        assert_ne!(
            app.selected_item.as_ref().map(|i| i.path.clone()),
            Some(video.path.clone()),
            "→ on a failed clip moves on instead of seeking"
        );
    });
    view.update(vcx, |app, cx| {
        app.select_item_at(video.clone(), 1, cx);
        app.open_viewer(video.clone(), cx);
    });
    render_ui(vcx);
    keystrokes(vcx, "left");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_index,
            Some(0),
            "← on a failed clip moves to the previous item"
        );
    });

    view.update(vcx, |app, cx| app.open_viewer(video.clone(), cx));
    render_ui(vcx);
    keystrokes(vcx, "escape");
    assert!(
        !view.update(vcx, |app, _| app.is_fullscreen_viewer),
        "Esc closes the viewer over a failed clip"
    );
}

#[core::prelude::v1::test]
fn failed_video_loop_keys_are_consumed_and_inert() {
    let (_fixture, engine, video) = broken_video("playback_failed_loop_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_viewer(video.clone(), cx));
    render_ui(vcx);
    focus_app(vcx, &view);
    let before = view.update(vcx, |app, _| {
        (
            app.inspector_open,
            app.item_flags.get(&video.id).copied(),
            app.selected_item.as_ref().map(|i| i.id),
            app.is_fullscreen_viewer,
        )
    });
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
            "{source} on a failed clip must be consumed, or a dead key leaves a pending composition"
        );
        let after = view.update(vcx, |app, _| {
            (
                app.inspector_open,
                app.item_flags.get(&video.id).copied(),
                app.selected_item.as_ref().map(|i| i.id),
                app.is_fullscreen_viewer,
            )
        });
        assert_eq!(
            after, before,
            "{source} on a failed clip must not toggle the inspector, reject or move on"
        );
    }
}

#[core::prelude::v1::test]
fn edit_mode_and_slideshow_hide_the_failure_card() {
    let (_fixture, engine, video) = broken_video("playback_failed_hidden");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_viewer(video.clone(), cx));
    assert!(view.update(vcx, |app, _| app.playback_failed_shown()));

    view.update(vcx, |app, _| {
        app.edits.session = Some(crate::app::quick_edit::QuickEditState {
            item: video.id,
            draft: Default::default(),
            original: Default::default(),
            source: None,
            crop: None,
            generation: 0,
            rendering: None,
            renders: 0,
            shown: None,
        });
        assert!(
            !app.playback_failed_shown(),
            "edit mode hides the failure card"
        );
        app.edits.session = None;
        assert!(app.playback_failed_shown());
    });

    vcx.update(|window, cx| view.update(cx, |app, cx| app.start_slideshow(window, cx)));
    view.update(vcx, |app, cx| {
        assert!(app.slideshow.is_some(), "the slideshow must start");
        app.open_viewer(video.clone(), cx);
        assert!(
            app.slideshow.is_some() && app.video.load_error.is_some(),
            "the failed clip is shown during the slideshow"
        );
        assert!(
            !app.playback_failed_shown(),
            "a slideshow hides the failure card"
        );
    });
}

#[core::prelude::v1::test]
fn vanished_video_says_it_cannot_be_found() {
    let (_fixture, engine, video) = broken_video("playback_failed_gone");
    std::fs::remove_file(&video.path).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_viewer(video, cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("playback-failed-title-text:Original not available")
            .is_some(),
        "a clip whose file is gone says the original is not available"
    );
    assert!(
        vcx.debug_bounds("playback-failed-title-text:Can't play this clip")
            .is_none(),
        "a missing file is not reported as an unplayable clip"
    );
    assert!(
        vcx.debug_bounds("playback-failed-body-text:The original file is missing.")
            .is_some(),
        "the body says the file is missing, not that it may be damaged"
    );
}

#[core::prelude::v1::test]
fn pip_vanished_video_says_it_cannot_be_found() {
    let (_fixture, engine, video) = broken_video("playback_failed_pip_gone");
    std::fs::remove_file(&video.path).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.play_in_pip(video, cx));
    render_ui(vcx);
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    render_ui(&mut pip);
    assert!(
        pip.debug_bounds("pip-playback-failed-title-text:Original not available")
            .is_some(),
        "PiP says the original is not available when the file is gone"
    );
    assert!(
        pip.debug_bounds("pip-playback-failed-title-text:Can't play this clip")
            .is_none(),
        "PiP does not call a missing file unplayable"
    );
}
