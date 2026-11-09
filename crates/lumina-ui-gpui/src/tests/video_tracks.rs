//! Audio tracks and chapters of the open video (Task F2).
//!
//! On macOS the player is AVFoundation and the sample files do not exist, so
//! these tests fill the session's track and chapter lists (what a load reads
//! from the player) and use a real, unloaded player. Its test-only record of
//! the last `select_audio_track` call shows what reached the backend.

use crate::app::command_palette::PaletteTarget;
use crate::app::types::get_sample_items;
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use gpui::VisualTestContext;
use lumina_core::MediaType;

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The sample video open in the viewer (60 s), on an unloaded native player.
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
    let (temp_dir, engine) = seed_library("video_tracks", 8);
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.open_viewer(video, cx);
        app.video.player = Some(crate::player::NativeVideoPlayer::new());
        app.video.duration = 60.0;
        cx.notify();
    });
    render_ui(vcx);
    (Fixture(temp_dir), view, vcx)
}

fn set_tracks(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
    names: &[&str],
    selected: Option<usize>,
) {
    view.update(vcx, |app, cx| {
        app.video.tracks.audio = names.iter().map(|n| n.to_string()).collect();
        app.video
            .player
            .as_mut()
            .expect("the stub player is installed")
            .test_selected_audio = selected;
        cx.notify();
    });
    render_ui(vcx);
}

fn set_chapters(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
    chapters: &[(f64, &str)],
) {
    view.update(vcx, |app, cx| {
        app.video.tracks.chapters = chapters.iter().map(|(t, n)| (*t, n.to_string())).collect();
        cx.notify();
    });
    render_ui(vcx);
}

fn backend_selection(
    view: &gpui::Entity<crate::LuminaApp>,
    vcx: &mut VisualTestContext,
) -> Option<usize> {
    view.update(vcx, |app, _| {
        app.video
            .player
            .as_ref()
            .expect("the stub player is installed")
            .test_selected_audio
    })
}

fn time(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> f64 {
    view.update(vcx, |app, _| app.video.playback_time)
}

fn at(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext, seconds: f64) {
    view.update(vcx, |app, _| app.video.playback_time = seconds);
}

fn audio_rows(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut VisualTestContext) -> Vec<String> {
    view.update(vcx, |app, _| {
        app.command_query = "audio".into();
        app.palette_rows()
            .into_iter()
            .filter(|row| matches!(row.target, PaletteTarget::AudioTrack(_)))
            .map(|row| row.title)
            .collect()
    })
}

const CHAPTERS: &[(f64, &str)] = &[(0.0, "Intro"), (20.0, "Middle"), (40.0, "End")];

#[core::prelude::v1::test]
fn audio_menu_hidden_with_one_track() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    assert!(
        vcx.debug_bounds("video-audio").is_none(),
        "no audio choice, no Audio menu"
    );
    set_tracks(&view, vcx, &["English"], Some(0));
    assert!(
        vcx.debug_bounds("video-audio").is_none(),
        "one track is no choice: the Audio menu stays hidden"
    );
    set_tracks(&view, vcx, &["English", "Commentary"], Some(0));
    assert!(
        vcx.debug_bounds("video-audio").is_some(),
        "two tracks show the Audio menu"
    );
}

#[core::prelude::v1::test]
fn select_audio_track_calls_backend() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    set_tracks(&view, vcx, &["English", "Commentary"], Some(0));
    click(vcx, "video-audio");
    let check = vcx
        .debug_bounds("video-audio-check")
        .expect("the open menu marks the selected track");
    let first = vcx
        .debug_bounds("video-audio-track-0")
        .expect("each track is a menu row");
    assert!(
        first.contains(&check.center()),
        "the check sits on the selected track (English)"
    );
    click(vcx, "video-audio-track-1");
    assert_eq!(
        backend_selection(&view, vcx),
        Some(1),
        "choosing a row must reach the player"
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.video.tracks.audio_menu_open,
            "a choice closes the menu"
        );
    });
    click(vcx, "video-audio");
    let check = vcx.debug_bounds("video-audio-check").expect("still marked");
    let second = vcx.debug_bounds("video-audio-track-1").expect("row 1");
    assert!(
        second.contains(&check.center()),
        "the menu now marks Commentary"
    );
}

#[core::prelude::v1::test]
fn escape_closes_the_audio_menu_and_keeps_the_viewer() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_tracks(&view, vcx, &["English", "Commentary"], Some(0));
    click(vcx, "video-audio");
    assert!(vcx.debug_bounds("video-audio-popover").is_some());
    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("video-audio-popover").is_none(),
        "Escape closes the menu"
    );
    assert!(
        view.update(vcx, |app, _| app.is_fullscreen_viewer),
        "the first Escape must not also close the viewer"
    );
}

#[core::prelude::v1::test]
fn palette_offers_audio_tracks_while_a_video_with_a_choice_is_open() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    set_tracks(&view, vcx, &["English"], Some(0));
    assert!(audio_rows(&view, vcx).is_empty(), "one track: no rows");
    set_tracks(&view, vcx, &["English", ""], Some(0));
    assert_eq!(
        audio_rows(&view, vcx),
        ["Audio Track: English", "Audio Track: Track 2"],
        "an unnamed track still gets a readable name"
    );
    view.update(vcx, |app, cx| {
        app.open_command_palette(cx);
        app.run_palette_target(PaletteTarget::AudioTrack(1), cx);
        assert!(
            !app.command_palette_open,
            "running a row closes the palette"
        );
    });
    assert_eq!(backend_selection(&view, vcx), Some(1));
    view.update(vcx, |app, cx| app.close_viewer(cx));
    assert!(
        audio_rows(&view, vcx).is_empty(),
        "with no video open there is nothing to choose"
    );
}

#[core::prelude::v1::test]
fn palette_audio_track_runs_from_the_keyboard() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_tracks(&view, vcx, &["English", "Commentary"], Some(0));
    keystrokes(vcx, "cmd-k");
    crate::tests::ui_automation::type_text(vcx, "commentary");
    keystrokes(vcx, "enter");
    assert_eq!(
        backend_selection(&view, vcx),
        Some(1),
        "⌘K, typing and Enter choose the track"
    );
}

#[core::prelude::v1::test]
fn reloading_the_video_drops_its_tracks_chapters_and_menu() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    set_tracks(&view, vcx, &["English", "Commentary"], Some(1));
    set_chapters(&view, vcx, CHAPTERS);
    view.update(vcx, |app, cx| {
        app.video.tracks.audio_menu_open = true;
        let other = app.selected_item.clone().expect("the video is shown");
        app.load_video_player(&other, cx);
        let tracks = &app.video.tracks;
        assert!(
            tracks.audio.is_empty(),
            "a load reads the tracks afresh; the old list is gone"
        );
        assert_eq!(
            app.current_audio_track(),
            None,
            "the old player and its audio choice are gone (per session)"
        );
        assert!(tracks.chapters.is_empty(), "chapters belong to one video");
        assert!(!tracks.audio_menu_open, "no menu left open");
    });
}

#[core::prelude::v1::test]
fn alt_right_jumps_to_next_chapter() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_chapters(&view, vcx, CHAPTERS);
    at(&view, vcx, 10.0);
    let shown = view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id));
    keystrokes(vcx, "alt-right");
    assert_eq!(time(&view, vcx), 20.0, "⌥→ goes to the next chapter start");
    let toast = view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()));
    assert_eq!(toast.as_deref(), Some("Chapter: Middle"));
    keystrokes(vcx, "alt-right");
    assert_eq!(time(&view, vcx), 40.0);
    keystrokes(vcx, "alt-right");
    assert_eq!(time(&view, vcx), 40.0, "after the last chapter ⌥→ stays");
    assert_eq!(
        view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id)),
        shown,
        "⌥→ must not move to the next item"
    );
}

#[core::prelude::v1::test]
fn alt_left_goes_to_the_chapter_start_then_the_one_before() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_chapters(&view, vcx, CHAPTERS);
    at(&view, vcx, 30.0);
    keystrokes(vcx, "alt-left");
    assert_eq!(
        time(&view, vcx),
        20.0,
        "well into a chapter, ⌥← restarts it"
    );
    at(&view, vcx, 21.0);
    keystrokes(vcx, "alt-left");
    assert_eq!(
        time(&view, vcx),
        0.0,
        "just after a chapter start, ⌥← goes to the one before"
    );
    keystrokes(vcx, "alt-left");
    assert_eq!(time(&view, vcx), 0.0, "before the first chapter ⌥← stays");
}

#[core::prelude::v1::test]
fn alt_arrows_without_chapters_seek_like_the_plain_arrows() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    let shown = view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id));
    let after = |vcx: &mut VisualTestContext, keys: &str| {
        at(&view, vcx, 10.0);
        keystrokes(vcx, keys);
        time(&view, vcx)
    };
    let (alt_right, right) = (after(vcx, "alt-right"), after(vcx, "right"));
    assert_eq!(alt_right, right, "no chapters: ⌥→ seeks forward 5 s like →");
    assert_ne!(alt_right, 10.0, "⌥→ must not be a dead key");
    let (alt_left, left) = (after(vcx, "alt-left"), after(vcx, "left"));
    assert_eq!(alt_left, left, "no chapters: ⌥← seeks back 5 s like ←");
    assert_eq!(
        view.update(vcx, |app, _| app.selected_item.as_ref().map(|i| i.id)),
        shown,
        "⌥←/⌥→ on a video must not change the item"
    );
}

#[core::prelude::v1::test]
fn chapter_keys_do_nothing_in_the_palette_or_a_text_field() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_chapters(&view, vcx, CHAPTERS);
    at(&view, vcx, 10.0);
    view.update(vcx, |app, cx| app.open_command_palette(cx));
    render_ui(vcx);
    keystrokes(vcx, "alt-right");
    assert_eq!(time(&view, vcx), 10.0, "the palette owns the keys");
    view.update(vcx, |app, cx| {
        app.close_command_palette(cx);
        app.search_focused = true;
        cx.notify();
    });
    render_ui(vcx);
    keystrokes(vcx, "alt-right");
    assert_eq!(time(&view, vcx), 10.0, "a text field owns the keys");
}

#[core::prelude::v1::test]
fn chapter_keys_leave_a_video_playing_in_pip_alone() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_chapters(&view, vcx, CHAPTERS);
    at(&view, vcx, 10.0);
    view.update(vcx, |app, _| {
        app.video.in_pip = true;
        // Navigation is by index into the library view.
        app.selected_index = Some(0);
    });
    keystrokes(vcx, "alt-right");
    assert_eq!(
        time(&view, vcx),
        10.0,
        "the viewer keys never drive the PiP video"
    );
    assert_eq!(
        view.update(vcx, |app, _| app.selected_index),
        Some(1),
        "today, while PiP plays, ⌥→ moves the viewer to the next item like →"
    );
}

#[core::prelude::v1::test]
fn a_chapter_past_the_loop_end_behaves_like_any_seek() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    focus_app(vcx, &view);
    set_chapters(&view, vcx, CHAPTERS);
    view.update(vcx, |app, cx| {
        app.video.playback_time = 5.0;
        app.set_loop_in(cx);
        app.video.playback_time = 15.0;
        app.set_loop_out(cx);
        app.video.playback_time = 10.0;
    });
    keystrokes(vcx, "alt-right");
    assert_eq!(time(&view, vcx), 20.0, "paused, the jump lands");
    let wrapped = view.update(vcx, |app, cx| {
        app.video.is_playing = true;
        app.handle_ui_message(crate::app::types::UiMessage::VideoFrameTick, cx);
        app.video.playback_time
    });
    assert_eq!(
        wrapped, 5.0,
        "playing, the range takes the playhead back to its start"
    );
    assert!(
        view.update(vcx, |app, _| app.video.loop_range.is_set()),
        "a chapter jump never clears the range"
    );
}

const TICKS: [&str; 6] = [
    "video-chapter-tick-0",
    "video-chapter-tick-1",
    "video-chapter-tick-2",
    "video-chapter-tick-3",
    "video-chapter-tick-4",
    "video-chapter-tick-5",
];

fn tick_count(vcx: &mut VisualTestContext) -> usize {
    TICKS
        .iter()
        .filter(|selector| vcx.debug_bounds(selector).is_some())
        .count()
}

#[core::prelude::v1::test]
fn no_chapters_no_ticks() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    assert_eq!(tick_count(vcx), 0, "no chapters, no ticks");
    set_chapters(&view, vcx, &[(0.0, "Intro"), (15.0, "A"), (45.0, "B")]);
    assert_eq!(
        tick_count(vcx),
        2,
        "a chapter at the very start draws no tick"
    );
    let track = vcx.debug_bounds("video-scrubber").expect("scrubber");
    let tick = vcx
        .debug_bounds("video-chapter-tick-1")
        .expect("15 s gets a tick");
    let pct = f32::from(tick.left() - track.left()) / f32::from(track.size.width);
    assert!(
        (pct - 0.25).abs() < 0.02,
        "15 s of 60 s is a quarter: {pct}"
    );
    view.update(vcx, |app, cx| {
        app.video.duration = 0.0;
        if let Some(item) = app.selected_item.as_mut() {
            item.duration_ms = None;
        }
        cx.notify();
    });
    render_ui(vcx);
    assert_eq!(
        tick_count(vcx),
        0,
        "with an unknown duration ticks have no place"
    );
}

#[core::prelude::v1::test]
fn the_menu_marks_the_track_the_player_reports_now() {
    let mut cx = crate::tests::TestApp::single();
    let (_fixture, view, vcx) = open_video(&mut cx);
    set_tracks(&view, vcx, &["English", "French"], None);
    // AVPlayer applies its automatic language choice after the load.
    view.update(vcx, |app, _| {
        app.video
            .player
            .as_mut()
            .expect("the stub player is installed")
            .test_selected_audio = Some(1);
    });
    click(vcx, "video-audio");
    let check = vcx
        .debug_bounds("video-audio-check")
        .expect("the menu marks the playing track");
    let row = vcx.debug_bounds("video-audio-track-1").expect("row 1");
    assert!(
        row.contains(&check.center()),
        "the check follows the player, not a value read at load"
    );
    let icons = view.update(vcx, |app, _| {
        app.command_query = "audio".into();
        app.palette_rows()
            .into_iter()
            .filter(|row| matches!(row.target, PaletteTarget::AudioTrack(_)))
            .map(|row| row.icon)
            .collect::<Vec<_>>()
    });
    assert_eq!(
        icons,
        [crate::icons::Icon::Volume, crate::icons::Icon::Check],
        "the palette marks the playing track too"
    );
}
