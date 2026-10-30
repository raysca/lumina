//! Picture-in-Picture as the detached video player: one session, kept apart
//! from the viewer and grid. These drive the real AVFoundation player against
//! a small committed H.264 clip (`tests/fixtures/clip.mp4`).

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, right_click,
};
use lumina_core::{EngineEvent, LuminaEngine, MediaItem, MediaType};

struct PipLibrary {
    dir: std::path::PathBuf,
    engine: std::sync::Arc<LuminaEngine>,
    video_a: MediaItem,
    video_b: MediaItem,
    photo: MediaItem,
}

impl Drop for PipLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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
    std::fs::write(dir.join("photo_a.jpg"), b"test-jpeg-content").unwrap();
    let (engine, rx) =
        LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), Some(dir.join("cache")))
            .unwrap();
    while let Ok(event) = rx.recv_timeout(std::time::Duration::from_secs(10)) {
        if let EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
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
        dir,
        engine,
        video_a,
        video_b,
        photo,
    }
}

fn card(item: &MediaItem) -> &'static str {
    leak(format!("media-card-{}", item.id.0))
}

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

#[core::prelude::v1::test]
fn viewer_video_moves_into_pip_and_survives_browsing_photos() {
    let lib = pip_library("survive");
    let mut cx = gpui::TestAppContext::single();
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

#[core::prelude::v1::test]
fn opening_another_video_replaces_the_pip_video_without_the_viewer() {
    let lib = pip_library("replace");
    let mut cx = gpui::TestAppContext::single();
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

#[core::prelude::v1::test]
fn expand_returns_the_pip_video_to_the_viewer_at_the_same_position() {
    let lib = pip_library("expand");
    let mut cx = gpui::TestAppContext::single();
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

#[core::prelude::v1::test]
fn toggling_pip_off_stops_playback_and_palette_reaches_it() {
    let lib = pip_library("toggle");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    click(vcx, card(&lib.video_a));
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-k");
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
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    keystrokes(vcx, "backspace");
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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

#[core::prelude::v1::test]
fn a_late_close_message_from_an_old_pip_window_keeps_the_new_session() {
    let lib = pip_library("stale_close");
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
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
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    // The controls appear on hover.
    pip.simulate_mouse_move(
        gpui::point(gpui::px(240.0), gpui::px(135.0)),
        None,
        gpui::Modifiers::default(),
    );
    pip.run_until_parked();
    let close = pip
        .debug_bounds("pip-close")
        .expect("hovering PiP must show its close button");
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

#[core::prelude::v1::test]
fn the_pip_window_expand_button_returns_the_video_to_the_viewer() {
    let lib = pip_library("window_expand");
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, lib.engine.clone());

    open_pip_for(&view, vcx, &lib.video_a);
    click(vcx, card(&lib.photo));
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    pip.simulate_mouse_move(
        gpui::point(gpui::px(240.0), gpui::px(135.0)),
        None,
        gpui::Modifiers::default(),
    );
    pip.run_until_parked();
    let expand = pip
        .debug_bounds("pip-expand")
        .expect("hovering PiP must show Expand");
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
