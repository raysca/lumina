//! Picture-in-Picture as the detached video player: one session, kept apart
//! from the viewer and grid. These drive the real AVFoundation player against
//! a small committed H.264 clip (`tests/fixtures/clip.mp4`).

use super::helpers::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click,
};
use lumina_core::{LuminaEngine, MediaItem, MediaType};

struct PipLibrary {
    dir: crate::tests::helpers::TempLibrary,
    engine: std::sync::Arc<LuminaEngine>,
    video_a: MediaItem,
    video_b: MediaItem,
    photo: MediaItem,
}

fn pip_library(prefix: &str) -> PipLibrary {
    let dir = std::env::temp_dir().join(format!(
        "lumina_pip_{prefix}_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
    std::fs::copy(&clip, dir.join("video_a.mp4")).unwrap();
    std::fs::copy(&clip, dir.join("video_b.mp4")).unwrap();
    std::fs::write(dir.join("photo_a.jpg"), crate::tests::helpers::tiny_jpeg()).unwrap();
    let (engine, rx) =
        LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), Some(dir.join("cache")))
            .unwrap();
    crate::tests::helpers::wait_for_scan(|timeout| rx.recv_timeout(timeout).ok());
    let items = engine.get_item_slice(0..10);
    let named = |name: &str| {
        items
            .iter()
            .find(|item| item.file_name == name)
            .unwrap_or_else(|| panic!("{name} must be indexed"))
            .clone()
    };
    let (video_a, video_b, photo) = (
        named("video_a.mp4"),
        named("video_b.mp4"),
        named("photo_a.jpg"),
    );
    assert_eq!(video_a.media_type, MediaType::Video);
    PipLibrary {
        dir: crate::tests::helpers::TempLibrary(dir),
        engine,
        video_a,
        video_b,
        photo,
    }
}

fn card(item: &MediaItem) -> &'static str {
    leak(format!("media-card-{}", item.id.0))
}

#[cfg(target_os = "macos")]
fn playing_id(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
) -> Option<u64> {
    view.update(vcx, |app, _| app.video.item.as_ref().map(|item| item.id.0))
}

/// Selects `item` by clicking its card and presses ⌥P.
fn open_pip_for(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    item: &MediaItem,
) {
    click(vcx, card(item));
    focus_app(vcx, view);
    keystrokes(vcx, "alt-p");
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn viewer_video_moves_into_pip_and_survives_browsing_photos() {
    let lib = pip_library("survive");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    click(vcx, card(&lib.video_a));
    focus_app(vcx, &view);
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer);
        assert!(
            app.video.player.is_some() && app.video.duration > 1.0,
            "the real AVFoundation player must load the fixture clip"
        );
    });

    let viewer_frame = view.update(vcx, |app, _| app.viewer_image.as_ref().map(|f| f.id));
    keystrokes(vcx, "alt-p");
    view.update(vcx, |app, _| {
        assert!(
            app.video.in_pip,
            "⌥P in the viewer must move the video to PiP"
        );
        assert!(
            !app.is_fullscreen_viewer,
            "the viewer closes when PiP opens"
        );
        assert!(app.video.is_playing, "PiP plays the moved video");
        assert_eq!(
            app.video.pip_frame.as_ref().map(|f| f.id),
            viewer_frame,
            "PiP must start from the viewer's last frame, not an empty window"
        );
        assert!(
            app.floating_pip_window.is_some(),
            "the PiP window must open"
        );
    });

    // View a photo: PiP keeps the same session and its own frame.
    click(vcx, card(&lib.photo));
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "photos still open in the viewer");
        assert!(
            app.video.in_pip && app.video.player.is_some(),
            "PiP must keep playing"
        );
        assert_eq!(app.video.item.as_ref().map(|i| i.id), Some(lib.video_a.id));
        if let (Some(viewer), Some(pip)) = (app.viewer_image.as_ref(), app.video.pip_frame.as_ref())
        {
            assert_ne!(viewer.id, pip.id, "the photo must not be drawn into PiP");
        }
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer);
        assert!(
            app.video.in_pip && app.video.player.is_some(),
            "closing the viewer must not stop PiP"
        );
    });
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn opening_another_video_replaces_the_pip_video_without_the_viewer() {
    let lib = pip_library("replace");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    assert_eq!(playing_id(&view, vcx), Some(lib.video_a.id.0));

    click(vcx, card(&lib.video_b));
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "with PiP open a video must not open the viewer"
        );
        assert!(app.video.in_pip && app.video.is_playing);
        assert_eq!(
            app.video.item.as_ref().map(|i| i.id),
            Some(lib.video_b.id),
            "the new video must replace the PiP video"
        );
    });
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn expand_returns_the_pip_video_to_the_viewer_at_the_same_position() {
    let lib = pip_library("expand");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    click(vcx, card(&lib.photo));
    view.update(vcx, |app, cx| {
        app.toggle_video_play(cx);
        app.seek_video_percent(0.5, cx);
    });
    let before = view.update(vcx, |app, _| app.video.playback_time);

    view.update(vcx, |app, cx| app.expand_pip_to_viewer(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.video.in_pip);
        assert!(app.is_fullscreen_viewer, "expand opens the viewer");
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(lib.video_a.id),
            "the viewer must show the PiP video even if a photo was selected"
        );
        assert!(
            app.video.player.is_some(),
            "expand must keep the player, not reload it"
        );
        assert!(
            (app.video.playback_time - before).abs() < 0.01,
            "expand must keep the position ({} vs {before})",
            app.video.playback_time
        );
        assert!(
            app.floating_pip_window.is_none(),
            "expand closes the PiP window"
        );
    });
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn toggling_pip_off_stops_playback_and_palette_reaches_it() {
    let lib = pip_library("toggle");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    click(vcx, card(&lib.video_a));
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    for key in ["t", "o", "g", "g", "l", "e", "space", "p", "i", "c"] {
        keystrokes(vcx, key);
    }
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            app.video.in_pip,
            "⌘K Toggle Picture-in-Picture must open PiP"
        )
    });

    keystrokes(vcx, "alt-p");
    view.update(vcx, |app, _| {
        assert!(!app.video.in_pip, "⌥P must close an open PiP");
        assert!(
            app.video.player.is_none() && !app.video.is_playing,
            "closing PiP stops playback"
        );
        assert!(app.video.pip_frame.is_none());
        assert!(app.floating_pip_window.is_none());
    });
}

#[core::prelude::v1::test]
fn trashing_the_pip_video_closes_pip() {
    let lib = pip_library("trash");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    keystrokes(vcx, "backspace enter");
    view.update(vcx, |app, _| {
        assert!(!app.video.in_pip, "trashing the PiP video must close PiP");
        assert!(app.video.player.is_none());
        let toast = app
            .toast
            .current()
            .map(|t| t.message.clone())
            .unwrap_or_default();
        assert!(
            toast.contains("Picture-in-Picture closed"),
            "the trash toast must say PiP closed, got {toast:?}"
        );
    });
}

#[core::prelude::v1::test]
fn context_menu_offers_pip_only_for_videos_and_plays_the_clicked_item() {
    let lib = pip_library("menu");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    right_click(vcx, card(&lib.photo));
    assert!(
        vcx.debug_bounds("pip-menu-btn").is_none(),
        "a photo's context menu must not offer Picture-in-Picture"
    );
    keystrokes(vcx, "escape");

    click(vcx, card(&lib.photo));
    right_click(vcx, card(&lib.video_b));
    click(vcx, "pip-menu-btn");
    view.update(vcx, |app, _| {
        assert!(app.video.in_pip);
        assert_eq!(
            app.video.item.as_ref().map(|i| i.id),
            Some(lib.video_b.id),
            "the menu must play the right-clicked video, not the selection"
        );
    });
}

#[core::prelude::v1::test]
fn viewer_shows_a_handoff_for_videos_while_pip_plays() {
    let lib = pip_library("handoff");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    // Reach video B inside the viewer by starting from the photo.
    click(vcx, card(&lib.photo));
    keystrokes(vcx, "space");
    view.update(vcx, |app, cx| app.select_item(lib.video_b.clone(), cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer);
        assert_eq!(
            app.video.item.as_ref().map(|i| i.id),
            Some(lib.video_a.id),
            "landing on another video in the viewer must not touch PiP"
        );
    });
    assert!(
        vcx.debug_bounds("viewer-pip-handoff").is_some(),
        "the viewer must offer to play the video in PiP instead of a transport"
    );

    focus_app(vcx, &view);
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert_eq!(app.video.item.as_ref().map(|i| i.id), Some(lib.video_b.id));
        assert!(
            !app.is_fullscreen_viewer,
            "playing it in PiP closes the viewer"
        );
    });
}

#[core::prelude::v1::test]
fn expand_moves_position_based_selection_to_the_pip_video() {
    let lib = pip_library("expand_selection");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    click(vcx, card(&lib.photo));
    view.update(vcx, |app, cx| app.expand_pip_to_viewer(cx));
    view.update(vcx, |app, _| {
        let expected = app.navigation_index_for(lib.video_a.id, None);
        assert!(expected.is_some());
        assert_eq!(
            app.selected_index, expected,
            "expand must move the selected index to the PiP video, not leave it on the photo"
        );
        assert_eq!(
            app.selected_indices.iter().copied().collect::<Vec<_>>(),
            expected.into_iter().collect::<Vec<_>>(),
            "batch actions must not still target the photo"
        );
    });
}

/// Expand leaves what holds no PiP video (Similar shots, the map), and so
/// compare: the viewer must not open over compare on an item it does not
/// show, with Esc then returning to compare.
#[core::prelude::v1::test]
fn expand_closes_compare_and_shows_the_pip_video() {
    let lib = pip_library("expand_compare");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    click(vcx, card(&lib.photo));
    keystrokes(vcx, "secondary-a");
    keystrokes(vcx, "c");
    assert!(
        view.update(vcx, |app, _| app.compare.is_some()),
        "C opens compare while PiP plays"
    );

    view.update(vcx, |app, cx| app.expand_pip_to_viewer(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.compare.is_none(), "expand closes compare");
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(lib.video_a.id),
            "the viewer shows the PiP video"
        );
    });
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("compare-view").is_none(),
        "Esc does not return to a compare the video was never part of"
    );
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn a_late_close_message_from_an_old_pip_window_keeps_the_new_session() {
    let lib = pip_library("stale_close");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    let old_window = view.update(vcx, |app, _| app.floating_pip_window.map(|w| w.window_id()));
    assert!(old_window.is_some(), "the first PiP window must open");
    keystrokes(vcx, "alt-p");
    keystrokes(vcx, "alt-p");
    view.update(vcx, |app, cx| {
        assert!(app.video.in_pip, "⌥P twice must reopen PiP");
        app.floating_pip_closed(old_window.unwrap(), cx);
        assert!(
            app.video.in_pip && app.video.player.is_some(),
            "a close message from the old window must not end the new session"
        );
    });
}

#[core::prelude::v1::test]
fn permanently_deleting_the_pip_video_closes_pip_and_says_so() {
    let lib = pip_library("perm_delete");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    view.update(vcx, |app, cx| {
        app.prompt_delete_selected(cx);
        // PermanentDisk only touches the temp library; SystemTrash would move
        // the file into the developer's real Trash.
        app.set_delete_target(lumina_core::DeletionTarget::PermanentDisk, cx);
        app.toggle_delete_permanent_acknowledged(cx);
        app.execute_confirmed_deletion(cx);
    });
    view.update(vcx, |app, _| {
        assert!(!app.video.in_pip, "deleting the PiP video must close PiP");
        let toast = app
            .toast
            .current()
            .map(|t| t.message.clone())
            .unwrap_or_default();
        assert!(
            toast.contains("Picture-in-Picture closed"),
            "the delete must say PiP closed, got {toast:?}"
        );
    });
    assert!(!lib.dir.join("video_a.mp4").exists());
}

#[core::prelude::v1::test]
fn the_pip_window_close_button_ends_the_session() {
    let lib = pip_library("window_close");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    // The chrome is always visible: no hover first.
    pip.run_until_parked();
    let close = pip
        .debug_bounds("pip-close")
        .expect("PiP always shows its close button");
    pip.simulate_click(close.center(), gpui::Modifiers::default());
    pip.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            !app.video.in_pip,
            "the PiP close button must end the session"
        );
        assert!(app.video.player.is_none());
        assert!(app.floating_pip_window.is_none());
    });
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn the_pip_window_expand_button_returns_the_video_to_the_viewer() {
    let lib = pip_library("window_expand");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    click(vcx, card(&lib.photo));
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    pip.run_until_parked();
    let expand = pip
        .debug_bounds("pip-expand")
        .expect("PiP always shows Expand");
    pip.simulate_click(expand.center(), gpui::Modifiers::default());
    pip.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(!app.video.in_pip);
        assert!(app.is_fullscreen_viewer, "Expand opens the viewer");
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(lib.video_a.id)
        );
        assert!(
            app.video.player.is_some(),
            "the window's close message after Expand must not stop the viewer's player"
        );
    });
}

/// Opens PiP on video A, then swaps in an unloaded player on a 60 s clock so
/// seeks land exactly on `playback_time` (as `tests/video_tracks.rs` does).
/// Returns the PiP window's context, drawn and parked.
fn pip_with_stub_player(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    lib: &PipLibrary,
) -> gpui::VisualTestContext {
    open_pip_for(view, vcx, &lib.video_a);
    view.update(vcx, |app, cx| {
        app.stop_video_ticker();
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.video.is_playing = false;
        app.video.duration = 60.0;
        app.video.playback_time = 0.0;
        // No real frame ticks: the 16 ms ticker thread would find the stub
        // not playing and end playback at a moment the test can't control.
        super::viewer_stage::detach_decoder_answers(app);
        cx.notify();
    });
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    pip.run_until_parked();
    pip
}

fn pip_click(pip: &mut gpui::VisualTestContext, selector: &'static str) {
    let bounds = pip
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("the PiP card must render {selector}"));
    pip.simulate_click(bounds.center(), gpui::Modifiers::default());
    pip.run_until_parked();
}

fn pip_time(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext) -> f64 {
    view.update(vcx, |app, _| app.video.playback_time)
}

#[core::prelude::v1::test]
fn pip_card_has_design_controls() {
    let lib = pip_library("card");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let mut pip = pip_with_stub_player(&view, vcx, &lib);

    for selector in [
        "pip-badge",
        "pip-expand",
        "pip-close",
        "pip-title",
        "pip-skip-back",
        "pip-play-pause",
        "pip-skip-forward",
        "pip-scrub-track",
        "pip-mute",
    ] {
        assert!(
            pip.debug_bounds(selector).is_some(),
            "the PiP card must always show {selector}"
        );
    }
    let badge = pip.debug_bounds("pip-badge").expect("badge renders");
    assert_eq!(
        (badge.left(), badge.top()),
        (gpui::px(12.0), gpui::px(12.0)),
        "the window buttons are hidden, so the badge sits at the frame's 12 px corner: {badge:?}"
    );

    let main = vcx.update(|window, _| window.bounds());
    let card = pip.update(|window, _| window.bounds());
    assert_eq!(
        (card.right(), card.bottom()),
        (
            main.right() - gpui::px(56.0),
            main.bottom() - gpui::px(56.0)
        ),
        "PiP opens 56 px in from the main window's bottom-right ({main:?} vs {card:?})"
    );

    // No chapters: the skip buttons seek 5 s, like ⌥← / ⌥→.
    pip_click(&mut pip, "pip-skip-forward");
    assert_eq!(pip_time(&view, vcx), 5.0, "skip forward must match ⌥→");

    // With chapters they jump between chapter starts, like ⌥← / ⌥→.
    view.update(vcx, |app, cx| {
        app.video.tracks.chapters = vec![
            (0.0, "Intro".to_string()),
            (20.0, "Middle".to_string()),
            (40.0, "End".to_string()),
        ];
        app.video.playback_time = 10.0;
        cx.notify();
    });
    pip.run_until_parked();
    pip_click(&mut pip, "pip-skip-forward");
    assert_eq!(pip_time(&view, vcx), 20.0, "skip forward must match ⌥→");
    view.update(vcx, |app, _| app.video.playback_time = 30.0);
    pip_click(&mut pip, "pip-skip-back");
    assert_eq!(pip_time(&view, vcx), 20.0, "skip back must match ⌥←");
}

#[core::prelude::v1::test]
fn pip_mute_button_toggles_mute() {
    let lib = pip_library("mute");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let mut pip = pip_with_stub_player(&view, vcx, &lib);

    let before = view.update(vcx, |app, _| app.video.is_muted);
    pip_click(&mut pip, "pip-mute");
    assert_eq!(
        view.update(vcx, |app, _| app.video.is_muted),
        !before,
        "the PiP mute button must toggle mute"
    );
    pip_click(&mut pip, "pip-mute");
    assert_eq!(
        view.update(vcx, |app, _| app.video.is_muted),
        before,
        "a second click must restore it"
    );
}

#[core::prelude::v1::test]
fn pip_scrub_track_seeks_by_click_and_drag() {
    let lib = pip_library("scrub");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let mut pip = pip_with_stub_player(&view, vcx, &lib);

    let track = pip
        .debug_bounds("pip-scrub-track")
        .expect("the PiP card must render its scrub track");
    let at = |fraction: f32| {
        gpui::point(
            track.origin.x + track.size.width * fraction,
            track.center().y,
        )
    };
    let none = gpui::Modifiers::default();

    pip.simulate_click(at(0.75), none);
    pip.run_until_parked();
    let time = pip_time(&view, vcx);
    assert!(
        (time - 45.0).abs() < 1.0,
        "a click at 75% of the track must seek to 45 s, got {time}"
    );

    pip.simulate_mouse_down(at(0.25), gpui::MouseButton::Left, none);
    pip.run_until_parked();
    pip.simulate_mouse_move(at(0.5), gpui::MouseButton::Left, none);
    pip.run_until_parked();
    pip.simulate_mouse_up(at(0.5), gpui::MouseButton::Left, none);
    pip.run_until_parked();
    let time = pip_time(&view, vcx);
    assert!(
        (time - 30.0).abs() < 1.0,
        "a drag released at 50% must seek to 30 s, got {time}"
    );
    assert!(
        !view.update(vcx, |app, _| app.video.is_scrubbing),
        "release must end the scrub"
    );
}

#[core::prelude::v1::test]
fn pip_window_takes_the_player_keys() {
    let lib = pip_library("keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let mut pip = pip_with_stub_player(&view, vcx, &lib);

    let muted = view.update(vcx, |app, _| app.video.is_muted);
    pip.simulate_keystrokes("m");
    pip.run_until_parked();
    assert_eq!(
        view.update(vcx, |app, _| app.video.is_muted),
        !muted,
        "M in the PiP window must toggle mute"
    );
    pip.simulate_keystrokes("right");
    pip.run_until_parked();
    assert_eq!(pip_time(&view, vcx), 5.0, "→ in the PiP window seeks 5 s");
    pip.simulate_keystrokes("j");
    pip.run_until_parked();
    assert_eq!(pip_time(&view, vcx), 0.0, "J in the PiP window seeks back");
    pip.simulate_keystrokes("space");
    pip.run_until_parked();
    assert!(
        view.update(vcx, |app, _| app.video.is_playing),
        "Space in the PiP window must play"
    );
    pip.simulate_keystrokes("k");
    pip.run_until_parked();
    assert!(
        !view.update(vcx, |app, _| app.video.is_playing),
        "K in the PiP window must pause"
    );

    let looping = view.update(vcx, |app, _| app.video.is_looping);
    pip.simulate_keystrokes("l");
    pip.run_until_parked();
    assert_eq!(
        view.update(vcx, |app, _| app.video.is_looping),
        !looping,
        "L in the PiP window toggles loop, as in the viewer"
    );

    view.update(vcx, |app, cx| {
        app.video.tracks.chapters = vec![(0.0, "Intro".to_string()), (20.0, "Middle".to_string())];
        app.video.playback_time = 10.0;
        cx.notify();
    });
    pip.simulate_keystrokes("alt-right");
    pip.run_until_parked();
    assert_eq!(pip_time(&view, vcx), 20.0, "⌥→ jumps to the next chapter");
    view.update(vcx, |app, _| app.video.playback_time = 10.0);
    pip.simulate_keystrokes("alt-left");
    pip.run_until_parked();
    assert_eq!(pip_time(&view, vcx), 0.0, "⌥← jumps to the chapter start");

    let muted = view.update(vcx, |app, _| app.video.is_muted);
    pip.simulate_keystrokes(&crate::tests::helpers::platform_test_keys(
        "cmd-m ctrl-j shift-right",
    ));
    pip.run_until_parked();
    view.update(vcx, |app, _| {
        assert_eq!(app.video.is_muted, muted, "⌘M is not the PiP mute key");
        assert_eq!(app.video.playback_time, 0.0, "modified keys never seek");
    });

    pip.simulate_keystrokes("escape");
    pip.run_until_parked();
    assert!(
        !view.update(vcx, |app, _| app.video.in_pip),
        "Escape closes the PiP: its window has no window buttons"
    );
}

#[core::prelude::v1::test]
fn pip_drags_from_the_fades_but_not_from_the_controls() {
    use crate::views::floating_pip::TEST_WINDOW_MOVES;
    let lib = pip_library("drag");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let mut pip = pip_with_stub_player(&view, vcx, &lib);
    let moves = || TEST_WINDOW_MOVES.with(|m| m.get());
    let none = gpui::Modifiers::default();
    let press = |pip: &mut gpui::VisualTestContext, selector: &'static str| {
        let at = pip.debug_bounds(selector).unwrap().center();
        pip.simulate_mouse_down(at, gpui::MouseButton::Left, none);
        pip.simulate_mouse_up(at, gpui::MouseButton::Left, none);
        pip.run_until_parked();
    };

    let before = moves();
    press(&mut pip, "pip-title");
    assert_eq!(
        moves(),
        before + 1,
        "a press on the title (bottom fade) must start one window move"
    );
    press(&mut pip, "pip-badge");
    assert_eq!(
        moves(),
        before + 2,
        "a press on the top fade must start one window move"
    );
    for control in ["pip-mute", "pip-skip-back", "pip-scrub-track"] {
        press(&mut pip, control);
        assert_eq!(moves(), before + 2, "{control} must not drag the window");
    }
}

#[core::prelude::v1::test]
fn pip_scrub_released_off_the_track_ends_the_scrub() {
    let lib = pip_library("scrub_out");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let mut pip = pip_with_stub_player(&view, vcx, &lib);
    let track = pip.debug_bounds("pip-scrub-track").unwrap();
    let title = pip.debug_bounds("pip-title").unwrap();
    let none = gpui::Modifiers::default();
    let x = |fraction: f32| track.origin.x + track.size.width * fraction;

    pip.simulate_mouse_down(
        gpui::point(x(0.25), track.center().y),
        gpui::MouseButton::Left,
        none,
    );
    pip.run_until_parked();
    let off_track = gpui::point(x(0.5), title.center().y);
    pip.simulate_mouse_move(off_track, gpui::MouseButton::Left, none);
    pip.run_until_parked();
    pip.simulate_mouse_up(off_track, gpui::MouseButton::Left, none);
    pip.run_until_parked();
    view.update(vcx, |app, _| {
        assert!(
            !app.video.is_scrubbing,
            "a release off the track must end the scrub"
        );
        assert!(
            (app.video.playback_time - 30.0).abs() < 1.0,
            "the release commits the dragged position, got {}",
            app.video.playback_time
        );
    });
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn video_badge_marks_the_clip_playing_in_pip() {
    let lib = pip_library("badge");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());
    let playing = |id: u64| leak(format!("media-card-playing-{id}"));
    let (a, b) = (lib.video_a.id.0, lib.video_b.id.0);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(playing(a)).is_none() && vcx.debug_bounds(playing(b)).is_none(),
        "no card is marked playing before PiP opens"
    );

    open_pip_for(&view, vcx, &lib.video_a);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(playing(a)).is_some(),
        "the clip playing in PiP marks its duration badge"
    );
    let badge = vcx
        .debug_bounds(leak(format!("media-card-duration-{a}")))
        .expect("the duration badge");
    assert!(
        vcx.debug_bounds(playing(b)).is_none(),
        "only the clip in PiP is marked"
    );
    let photo = vcx
        .debug_bounds(leak(format!("media-card-photo-{a}")))
        .expect("the card's thumb");
    assert!(
        (f32::from(badge.left() - photo.left()) - 8.0).abs() < 0.5
            && (f32::from(photo.bottom() - badge.bottom()) - 8.0).abs() < 0.5,
        "the badge sits 8/8 in from the thumb's bottom-left: {badge:?} vs {photo:?}"
    );

    // Opening the other video moves the mark; closing PiP drops it.
    click(vcx, card(&lib.video_b));
    keystrokes(vcx, "space");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(playing(b)).is_some(),
        "the mark follows the PiP video"
    );
    view.update(vcx, |app, cx| app.close_pip(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(!app.video.in_pip);
    });
    assert!(
        vcx.debug_bounds(playing(b)).is_none(),
        "closing PiP drops the mark"
    );
}

/// Expanding PiP over a page with no media (the Albums overview, the Tags
/// page) shows the video from All Media: closing the viewer must not leave
/// a hidden selection on the page.
fn expand_over_a_page_shows_the_video_from_all_media(prefix: &str, page_row: &'static str) {
    let lib = pip_library(prefix);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    click(vcx, page_row);
    assert!(view.update(vcx, |app, _| app.non_media_page_shown()));

    view.update(vcx, |app, cx| app.expand_pip_to_viewer(cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            crate::views::sidebar::AlbumSelection::AllLibrary,
            "the viewer opens over All Media, not {page_row}'s page"
        );
        assert!(app.is_fullscreen_viewer);
        assert_eq!(
            app.selected_item.as_ref().map(|i| i.id),
            Some(lib.video_a.id)
        );
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            !app.non_media_page_shown(),
            "closing the viewer shows the grid the selection belongs to"
        );
    });
    assert!(vcx.debug_bounds("media-grid-surface").is_some());
}

#[core::prelude::v1::test]
fn expand_over_a_media_less_page_shows_the_video_from_all_media() {
    for (prefix, page_row) in [
        ("expand_overview", "sidebar-albums-header"),
        ("expand_tags", "sidebar-nav-keywords"),
    ] {
        expand_over_a_page_shows_the_video_from_all_media(prefix, page_row);
    }
}

/// Counts notifications of `entity` until the returned subscription drops.
fn count_notifies<T: 'static>(
    vcx: &mut gpui::VisualTestContext,
    entity: &gpui::Entity<T>,
) -> (std::rc::Rc<std::cell::Cell<usize>>, gpui::Subscription) {
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let counter = count.clone();
    let subscription =
        vcx.update(|_, cx| cx.observe(entity, move |_, _| counter.set(counter.get() + 1)));
    (count, subscription)
}

/// Shows `frames` video frames 16 ms apart, as the frame tick does.
fn play_frames(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut gpui::VisualTestContext,
    frames: usize,
) -> std::sync::Arc<gpui::RenderImage> {
    let mut last = None;
    for _ in 0..frames {
        let frame = crate::bridge::video_frame_to_gpui_image(crate::player::VideoFrame {
            width: 2,
            height: 2,
            data: vec![0; 16],
        })
        .unwrap();
        last = Some(frame.clone());
        view.update(vcx, |app, cx| {
            app.show_video_frame(frame);
            app.repaint_for_video_frame(cx);
        });
        vcx.executor()
            .advance_clock(std::time::Duration::from_millis(16));
    }
    last.unwrap()
}

/// A PiP frame repaints only the PiP window: notifying the app would rebuild
/// the main window, library and all, at the video's frame rate. The main
/// window follows at most about 15 times a second.
#[core::prelude::v1::test]
fn pip_frames_repaint_the_pip_window_and_throttle_the_main_window() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let (weak_app, ui_tx) = (
        view.downgrade(),
        view.update(vcx, |app, _| app.ui_tx.clone()),
    );
    view.update(vcx, |app, _| app.video.in_pip = true);
    vcx.update(|_, cx| crate::app::windows::open_floating_pip_window(weak_app, ui_tx, cx));
    vcx.run_until_parked();
    let pip = view
        .update(vcx, |app, _| app.floating_pip_window)
        .and_then(|window| window.downcast::<crate::views::floating_pip::FloatingPipPlayerView>())
        .expect("the PiP window opens for a PiP session");
    let pip_view = vcx.update(|_, cx| pip.entity(cx)).expect("PiP view");

    let (main, _main_sub) = count_notifies(vcx, &view);
    let (pip_repaints, _pip_sub) = count_notifies(vcx, &pip_view);
    // One second of 60 fps playback.
    let last = play_frames(&view, vcx, 60);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.video.pip_frame.as_ref().map(|frame| frame.id),
            Some(last.id),
            "PiP shows the latest frame"
        );
    });
    assert!(
        pip_repaints.get() >= 60,
        "every frame must repaint the PiP window, got {}",
        pip_repaints.get()
    );
    assert!(
        (10..=16).contains(&main.get()),
        "the main window repaints about 15 times a second during PiP, not per frame; got {}",
        main.get()
    );
}

/// The viewer shows the frame in the main window, so it repaints each frame.
#[core::prelude::v1::test]
fn viewer_frames_repaint_the_main_window_every_frame() {
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| crate::LuminaApp::new(cx));
    let (main, _sub) = count_notifies(vcx, &view);
    play_frames(&view, vcx, 30);
    assert_eq!(main.get(), 30, "each viewer frame repaints the main window");
}
