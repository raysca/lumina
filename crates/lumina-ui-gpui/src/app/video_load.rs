//! Video loads: opening a file can take seconds (AVFoundation reads the
//! asset and waits for the item to be ready; GStreamer prerolls), so the
//! player loads on a background thread and `finish_video_load` installs the
//! result on the UI thread, unless the session moved on meanwhile. Tests run
//! the background half inline so they never depend on its timing.

use gpui::*;
use lumina_core::{MediaId, MediaItem};

use super::video_actions::{fill_missing_dimensions, VideoSettings};
use crate::app::types::format_duration_badge;
use crate::app::LuminaApp;
use crate::bridge::video_frame_to_gpui_image;
use crate::config::resume_start_time;
use crate::player::{self, VideoPlayerBackend};

/// What a background load hands to the UI thread.
pub(crate) struct LoadedVideo {
    player: player::NativeVideoPlayer,
    meta: player::backend::VideoMetadata,
    tracks: crate::app::video_tracks::VideoTracks,
    /// The first frame, only when the load starts at time zero: a resumed
    /// start must show the destination, not the time-zero frame.
    first_frame: Option<player::VideoFrame>,
    /// Where the load left the player: the resume point, or zero.
    start: f64,
}

/// Loads `path` off the UI thread, applying `settings` and seeking to the
/// resume point from `saved`. The player is created here and then used only
/// from the UI thread.
pub(crate) fn load_video_in_background(
    path: &std::path::Path,
    saved: Option<f64>,
    settings: VideoSettings,
) -> Result<LoadedVideo, String> {
    let mut player = player::create_native_player();
    let meta = player.load(path).map_err(|err| format!("{err:#}"))?;
    settings.apply(&mut player);
    let start = resume_start_time(saved, meta.duration_secs);
    if start > 0.0 {
        player.seek(start);
    }
    let tracks = crate::app::video_tracks::VideoTracks::read(&player, meta.duration_secs);
    let first_frame = if start == 0.0 {
        player.next_frame()
    } else {
        None
    };
    Ok(LoadedVideo {
        player,
        meta,
        tracks,
        first_frame,
        start,
    })
}

fn pending_load_target(pending: Option<f64>, start: f64, duration: f64) -> f64 {
    pending
        .filter(|target| target.is_finite())
        .map(|target| target.clamp(0.0, duration.max(0.0)))
        .unwrap_or(start)
}

impl LuminaApp {
    /// Starts a player session for `item`, replacing any current one. The
    /// file loads in the background; seeks and play pressed meanwhile are
    /// applied when it lands. Frames go to the viewer or, while
    /// `video.in_pip` is set, to the PiP window.
    pub fn load_video_player(&mut self, item: &MediaItem, cx: &mut Context<Self>) {
        // Finish the previous video before associating the session with the
        // new media id.
        self.stop_video_player();
        let generation = self.video.load_generation;
        if let Some(state) = self.grid_state.as_ref() {
            state.video_resume.begin(item.id);
        }
        self.video.item = Some(item.clone());
        self.video.playback_time = 0.0;
        self.video.duration = 0.0;
        self.video.load_error = None;
        self.video.load_file_gone = false;
        self.video.tracks = Default::default();
        let id = item.id;
        let path = item.path.clone();
        let saved = self
            .grid_state
            .as_ref()
            .and_then(|state| state.video_resume.saved(id));
        let settings = VideoSettings::of(&self.video);
        #[cfg(test)]
        {
            crate::app::playback_failed::VIDEO_LOADS.with(|n| n.set(n.get() + 1));
            let result = load_video_in_background(&path, saved, settings);
            self.finish_video_load(generation, id, result, cx);
        }
        #[cfg(not(test))]
        {
            let task = cx
                .background_executor()
                .spawn(async move { load_video_in_background(&path, saved, settings) });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                // The view is gone when the window closed mid-load; nothing
                // is left to update.
                let _ = this.update(cx, |app, cx| {
                    app.finish_video_load(generation, id, result, cx)
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// Installs a background load's result, unless the session moved on
    /// since it started (`generation` no longer current, or another item
    /// open). Settings changed and seeks or play requested during the load
    /// win over the load's snapshot.
    pub(crate) fn finish_video_load(
        &mut self,
        generation: u64,
        id: MediaId,
        result: Result<LoadedVideo, String>,
        cx: &mut Context<Self>,
    ) {
        if self.video.load_generation != generation
            || self.video.item.as_ref().is_none_or(|item| item.id != id)
        {
            return;
        }
        match result {
            Ok(LoadedVideo {
                mut player,
                meta,
                tracks,
                first_frame,
                start,
            }) => {
                self.video.duration = meta.duration_secs;
                self.video.failed_focus = None;
                let pending_seek = self.video.pending_load_seek.take();
                let target = pending_load_target(pending_seek, start, meta.duration_secs);
                self.video.playback_time = target;
                self.video.tracks = tracks;
                if pending_seek.is_none() && target > 0.0 {
                    self.last_action =
                        Some(format!("Resumed at {}", format_duration_badge(target)));
                }
                for shown in [self.selected_item.as_mut(), self.video.item.as_mut()]
                    .into_iter()
                    .flatten()
                    .filter(|shown| shown.id == id)
                {
                    fill_missing_dimensions(shown, meta.width, meta.height);
                }
                // Settings first: a rate change seeks to the current
                // position, which would undo the pending seek below.
                VideoSettings::of(&self.video).apply(&mut player);
                if target != start {
                    player.seek(target);
                }
                if target == 0.0 {
                    if let Some(frame) =
                        first_frame.and_then(|frame| video_frame_to_gpui_image(frame).ok())
                    {
                        self.show_video_frame(frame);
                    }
                }
                // Play may have been pressed while the player loaded.
                if self.video.is_playing {
                    player.play();
                }
                self.video.player = Some(player);
                #[cfg(feature = "linux-ui-smoke")]
                if std::env::var_os("LUMINA_UI_SMOKE").is_some() {
                    log::info!("LUMINA_UI_SMOKE: video player loaded for {id:?}");
                }
                if self.video.is_playing {
                    self.start_video_ticker();
                } else {
                    // Fetch the frame at the destination: no preroll frame
                    // shows a seek, and the GPU surface never comes with the
                    // load (macOS pulls it in the background).
                    self.arm_seek_refresh();
                }
                self.request_scrub_strip(cx);
            }
            Err(error) => {
                #[cfg(feature = "linux-ui-smoke")]
                if std::env::var_os("LUMINA_UI_SMOKE").is_some() {
                    log::error!("LUMINA_UI_SMOKE: video player failed for {id:?}: {error}");
                }
                self.record_video_failure(error);
            }
        }
        cx.notify();
    }
}

// These drive the GStreamer player directly; the cross-platform load tests
// live in `tests/video_player.rs` and `tests/playback_failed.rs`.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::app::types::UiMessage;
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[core::prelude::v1::test]
    fn settings_changed_during_load_are_applied_to_the_loaded_player() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
        let mut player = crate::player::linux::GStreamerPlayer::new();
        player.load(&fixture).expect("the fixture video loads");

        // Model the user changing controls after load started but before its
        // result is installed in the app state.
        let video = super::super::state::VideoPlaybackState {
            volume: 0.35,
            is_muted: true,
            is_looping: true,
            playback_speed: 1.5,
            ..Default::default()
        };
        VideoSettings::of(&video).apply(&mut player);

        assert_eq!(player.volume(), 0.35);
        assert!(player.is_muted());
        assert!(player.is_looping());
        assert_eq!(player.rate(), 1.5);
    }

    fn fixture() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4")
    }

    fn sample_video() -> MediaItem {
        let mut item = crate::app::types::get_sample_items()
            .into_iter()
            .find(|item| item.media_type == lumina_core::MediaType::Video)
            .expect("sample items include a video");
        item.path = fixture();
        item
    }

    fn normal_settings() -> VideoSettings {
        VideoSettings::of(&super::super::state::VideoPlaybackState {
            volume: 1.0,
            playback_speed: 1.0,
            ..Default::default()
        })
    }

    /// An app with `item` open as a load in progress at `generation`, as
    /// `load_video_player_async` leaves it.
    fn loading(app: &mut LuminaApp, item: &MediaItem, generation: u64) {
        app.video.item = Some(item.clone());
        app.video.load_generation = generation;
        app.video.volume = 1.0;
        app.video.playback_speed = 1.0;
    }

    fn wait_for_position(player: &crate::player::NativeVideoPlayer, target: f64) -> f64 {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut position = player.position();
        while Instant::now() < deadline && (position - target).abs() > 0.05 {
            std::thread::sleep(Duration::from_millis(10));
            position = player.position();
        }
        position
    }

    #[core::prelude::v1::test]
    fn late_load_results_are_rejected_and_current_ones_installed() {
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let item = sample_video();
        view.update(cx, |app, cx| {
            loading(app, &item, 5);
            let stale = load_video_in_background(&fixture(), None, normal_settings());
            app.finish_video_load(4, item.id, stale, cx);
            assert!(
                app.video.player.is_none(),
                "a load from an older generation must not install its player"
            );
            let other = load_video_in_background(&fixture(), None, normal_settings());
            app.finish_video_load(5, MediaId(item.id.0 + 1), other, cx);
            assert!(
                app.video.player.is_none(),
                "a load for another item must not install its player"
            );
            let current = load_video_in_background(&fixture(), None, normal_settings());
            app.finish_video_load(5, item.id, current, cx);
            assert!(
                app.video.player.is_some(),
                "the current load installs its player"
            );
            assert!(
                app.video.duration > 0.0,
                "the current load sets the duration"
            );
            app.stop_video_player();
        });
    }

    #[core::prelude::v1::test]
    fn play_pressed_during_load_starts_the_loaded_player() {
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let item = sample_video();
        view.update(cx, |app, cx| {
            loading(app, &item, 1);
            app.video.is_playing = true;
            let loaded = load_video_in_background(&fixture(), None, normal_settings());
            app.finish_video_load(1, item.id, loaded, cx);
            let player = app
                .video
                .player
                .as_ref()
                .expect("the load installs its player");
            assert!(
                player.is_playing(),
                "play pressed during the load plays the clip"
            );
            assert!(
                app.video.ticker_active.is_some(),
                "a playing clip runs the frame ticker"
            );
            app.stop_video_player();
        });
    }

    /// Regression: installing the player sought to the pending target, then
    /// re-applied settings, and `set_rate` (a flushing seek to the current
    /// position) could pull the playhead back to where the load left it.
    #[core::prelude::v1::test]
    fn pending_seek_during_load_lands_and_stays() {
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let item = sample_video();
        view.update(cx, |app, cx| {
            loading(app, &item, 1);
            app.video.pending_load_seek = Some(1.3);
            let loaded = load_video_in_background(&fixture(), None, normal_settings());
            app.finish_video_load(1, item.id, loaded, cx);
            assert_eq!(
                app.video.playback_time, 1.3,
                "the playhead shows the pending seek"
            );
            assert_eq!(
                app.video.pending_load_seek, None,
                "the pending seek is used once"
            );
            let player = app
                .video
                .player
                .as_ref()
                .expect("the load installs its player");
            let landed = wait_for_position(player, 1.3);
            std::thread::sleep(Duration::from_millis(200));
            let settled = player.position();
            assert!(
                (landed - 1.3).abs() <= 0.05 && (settled - 1.3).abs() <= 0.05,
                "the player lands on the pending seek and stays: {landed} then {settled}"
            );
            app.stop_video_player();
        });
    }

    fn settings_at(rate: f32) -> VideoSettings {
        VideoSettings::of(&super::super::state::VideoPlaybackState {
            volume: 1.0,
            playback_speed: rate,
            ..Default::default()
        })
    }

    /// Loads a 5 s clip with a resume point at 2 s, the background load
    /// applying `load_rate`, then installs it with the UI at `ui_rate`.
    fn resume_at_rates(load_rate: f32, ui_rate: f32) {
        let clip = crate::player::linux::test_clip(5);
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let mut item = sample_video();
        item.path = clip.clone();
        view.update(cx, |app, cx| {
            loading(app, &item, 1);
            app.video.playback_speed = ui_rate;
            let loaded = load_video_in_background(&clip, Some(2.0), settings_at(load_rate));
            app.finish_video_load(1, item.id, loaded, cx);
            assert_eq!(app.video.playback_time, 2.0, "the clip resumes at 2 s");
            let player = app
                .video
                .player
                .as_ref()
                .expect("the load installs its player");
            let landed = wait_for_position(player, 2.0);
            std::thread::sleep(Duration::from_millis(200));
            let settled = player.position();
            assert!(
                (landed - 2.0).abs() <= 0.05 && (settled - 2.0).abs() <= 0.05,
                "the resumed clip stays at its resume point: {landed} then {settled}"
            );
            assert_eq!(
                player.wait_for_segment_rate(f64::from(ui_rate)),
                Some(f64::from(ui_rate)),
                "the resumed clip plays at the chosen rate"
            );
            app.stop_video_player();
        });
        let _ = std::fs::remove_file(&clip);
    }

    /// Regression: the background load set 1.5x, then its resume seek reset
    /// the pipeline to 1.0, and installing skipped the "unchanged" 1.5x.
    #[core::prelude::v1::test]
    fn resumed_load_keeps_the_chosen_rate() {
        resume_at_rates(1.5, 1.5);
    }

    /// Regression: a rate changed during the load sought to the lagging
    /// position right after the resume seek, pulling the playhead back.
    #[core::prelude::v1::test]
    fn rate_changed_during_a_resumed_load_keeps_the_resume_point() {
        resume_at_rates(1.0, 1.5);
    }

    #[core::prelude::v1::test]
    fn failed_background_load_shows_the_playback_failed_state() {
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let item = sample_video();
        view.update(cx, |app, cx| {
            loading(app, &item, 1);
            app.video.is_playing = true;
            app.finish_video_load(1, item.id, Err("could not open".into()), cx);
            assert_eq!(app.video.load_error.as_deref(), Some("could not open"));
            assert!(!app.video.is_playing, "a failed load is not playing");
            assert!(
                app.video_unplayable(),
                "a failed load disables the transport"
            );
            assert!(!app.video.load_file_gone, "the fixture file still exists");
        });
    }

    /// Regression: a decode error mid-clip was dropped from the bus, so the
    /// clip kept "playing" with no frames and no message.
    #[core::prelude::v1::test]
    fn decode_error_while_playing_shows_the_playback_failed_state() {
        let clip = crate::player::linux::test_clip(5);
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let mut item = sample_video();
        item.path = clip.clone();
        view.update(cx, |app, cx| {
            loading(app, &item, 1);
            // The resume map follows the selected video without a library.
            app.selected_item = Some(item.clone());
            let loaded = load_video_in_background(&clip, None, normal_settings());
            app.finish_video_load(1, item.id, loaded, cx);
            let player = app
                .video
                .player
                .as_mut()
                .expect("the load installs its player");
            player.seek(2.0);
            let _ = player.next_frame();
            wait_for_position(player, 2.0);
            app.toggle_video_play(cx);
            assert!(app.video.is_playing, "the clip plays");
            app.video
                .player
                .as_ref()
                .expect("the load installs its player")
                .post_test_error();
            app.handle_ui_message(UiMessage::VideoFrameTick, cx);
            assert!(
                app.video.load_error.is_some(),
                "a decode error is shown in the playback-failed card"
            );
            assert!(!app.video.is_playing, "a failed clip stops playing");
            assert!(app.video.player.is_none(), "a failed clip drops its player");
            assert!(
                app.video.ticker_active.is_none(),
                "a failed clip stops the ticker"
            );
            assert_eq!(
                app.video.item.as_ref().map(|shown| shown.id),
                Some(item.id),
                "the failed clip stays open for Try Again and Reveal"
            );
            let saved = app.config.resume_positions.get(item.id);
            assert!(
                saved.is_some_and(|saved| (saved - 2.0).abs() < 0.5),
                "a mid-clip failure keeps the position reached, as a stop does: {saved:?}"
            );
        });
        let _ = std::fs::remove_file(&clip);
    }
}

#[cfg(test)]
mod load_tests {
    use super::*;

    #[core::prelude::v1::test]
    fn pending_seek_during_load_overrides_resume_position_and_is_clamped() {
        assert_eq!(pending_load_target(Some(42.0), 8.0, 60.0), 42.0);
        assert_eq!(pending_load_target(Some(90.0), 8.0, 60.0), 60.0);
        assert_eq!(pending_load_target(Some(f64::NAN), 8.0, 60.0), 8.0);
        assert_eq!(pending_load_target(None, 8.0, 60.0), 8.0);
    }

    #[core::prelude::v1::test]
    fn scrub_and_relative_seek_are_retained_while_the_player_loads() {
        let mut test_app = crate::tests::TestApp::single();
        let (view, cx) = test_app.add_window_view(|_window, cx| LuminaApp::new(cx));
        let item = crate::app::types::get_sample_items()
            .into_iter()
            .find(|item| item.media_type == lumina_core::MediaType::Video)
            .expect("sample items include a video");
        view.update(cx, |app, cx| {
            app.video.item = Some(item);
            app.video.duration = 60.0;
            app.begin_video_scrub(0.5, cx);
            app.end_video_scrub(cx);
            assert_eq!(app.video.pending_load_seek, Some(30.0));

            app.seek_video_relative(5.0, cx);
            assert_eq!(app.video.pending_load_seek, Some(35.0));
            assert_eq!(app.video.playback_time, 35.0);
        });
    }
}
