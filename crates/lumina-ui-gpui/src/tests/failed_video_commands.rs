//! A video that failed to load: the palette's loop, volume and frame
//! commands and the PiP window's L and M keys leave it alone, as the
//! viewer's transport keys do.

use crate::app::command_palette::CommandId;
use crate::app::types::get_sample_items;
use crate::tests::ui_automation::{app_with_library, render_ui, seed_library};
use lumina_core::{MediaItem, MediaType};

/// Removes the temp library on success and failure.
struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A library plus a video item whose file is not a video, so every load
/// fails.
fn broken_video(
    prefix: &str,
) -> (
    Fixture,
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
    (Fixture(dir), engine, video)
}

const VIDEO_COMMANDS: [CommandId; 7] = [
    CommandId::SetLoopStart,
    CommandId::SetLoopEnd,
    CommandId::ClearLoopRange,
    CommandId::VolumeUp,
    CommandId::VolumeDown,
    CommandId::PreviousFrame,
    CommandId::NextFrame,
];

#[core::prelude::v1::test]
fn palette_offers_no_loop_or_volume_commands_for_a_failed_video() {
    let (_fixture, engine, video) = broken_video("failed_video_palette");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.open_viewer(video, cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.video_unplayable(), "the clip failed to load");
        for id in VIDEO_COMMANDS {
            assert!(
                !app.command_applies(id),
                "{id:?} must not act on a clip that failed to load"
            );
        }
    });
}

#[core::prelude::v1::test]
fn pip_loop_and_mute_keys_leave_a_failed_video_alone() {
    let (_fixture, engine, video) = broken_video("failed_video_pip_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.play_in_pip(video, cx));
    render_ui(vcx);
    let handle = view
        .update(vcx, |app, _| app.floating_pip_window)
        .expect("the PiP window must open");
    let mut pip = gpui::VisualTestContext::from_window(handle, &vcx.cx);
    render_ui(&mut pip);
    let state = |pip: &mut gpui::VisualTestContext| {
        view.update(pip, |app, _| {
            assert!(app.video_unplayable(), "the clip failed to load");
            (
                app.video.is_looping,
                app.video.is_muted,
                app.config.video_muted,
            )
        })
    };
    let before = state(&mut pip);
    for key in ["l", "m"] {
        pip.simulate_keystrokes(key);
        pip.run_until_parked();
        assert_eq!(
            state(&mut pip),
            before,
            "{key} in PiP must not loop or mute a clip that failed to load"
        );
    }
}
