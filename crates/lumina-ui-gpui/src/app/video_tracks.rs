//! Audio tracks and chapters of the open video.
//!
//! Both are read from the player once per load and dropped with the session.
//! The audio choice is never saved (owner decision 7): the next load starts
//! on the track the file and system language pick.

use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::player::VideoPlayerBackend;

/// Within this many seconds of a chapter start, "previous chapter" goes to
/// the chapter before; later it restarts the current one, as music and disc
/// players do.
const PREVIOUS_CHAPTER_GRACE_SECS: f64 = 3.0;

/// A chapter start this close ahead of the playhead counts as reached, so a
/// seek that lands a hair early cannot make "next" repeat the same chapter.
const CHAPTER_EPSILON_SECS: f64 = 0.05;

/// The open video's audio tracks and chapters, for this session only.
#[derive(Default)]
pub struct VideoTracks {
    /// Alternative audio track names, in file order. Fewer than two means
    /// there is no choice to offer.
    pub audio: Vec<String>,
    /// Chapter starts in seconds, ascending, and their titles.
    pub chapters: Vec<(f64, String)>,
    pub audio_menu_open: bool,
}

impl VideoTracks {
    /// Reads a freshly loaded player. `duration` drops chapters past the end.
    pub(crate) fn read(player: &impl VideoPlayerBackend, duration: f64) -> Self {
        Self {
            audio: player.audio_tracks(),
            chapters: clean_chapters(player.chapters(), duration),
            audio_menu_open: false,
        }
    }

    pub fn has_audio_choice(&self) -> bool {
        self.audio.len() > 1
    }

    /// The menu and palette name of track `index`; a file can leave it empty.
    pub fn audio_name(&self, index: usize) -> String {
        match self.audio.get(index).map(|name| name.trim()) {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => t("video.audio_track_unnamed").replace("{n}", &(index + 1).to_string()),
        }
    }

    fn chapter_name(&self, index: usize) -> String {
        match self.chapters.get(index).map(|(_, name)| name.trim()) {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => t("video.chapter_unnamed").replace("{n}", &(index + 1).to_string()),
        }
    }
}

/// Sorted, finite, in-file chapter starts with no two at the same time.
fn clean_chapters(mut chapters: Vec<(f64, String)>, duration: f64) -> Vec<(f64, String)> {
    chapters.retain(|(start, _)| {
        start.is_finite() && *start >= 0.0 && (duration <= 0.0 || *start < duration)
    });
    chapters.sort_by(|a, b| a.0.total_cmp(&b.0));
    chapters.dedup_by(|b, a| (b.0 - a.0).abs() < CHAPTER_EPSILON_SECS);
    chapters
}

/// Index of the chapter to jump to from `position`: the next start ahead, or
/// for `forward == false` the current chapter's start (or the one before it
/// within the grace period). `None` when there is nowhere to go.
fn chapter_target(starts: &[f64], position: f64, forward: bool) -> Option<usize> {
    if forward {
        starts
            .iter()
            .position(|&start| start > position + CHAPTER_EPSILON_SECS)
    } else {
        let current = starts
            .iter()
            .rposition(|&start| start <= position + CHAPTER_EPSILON_SECS)?;
        if position - starts[current] > PREVIOUS_CHAPTER_GRACE_SECS {
            Some(current)
        } else {
            current.checked_sub(1).or(Some(current))
        }
    }
}

impl LuminaApp {
    /// Plays audio track `index` of the open video, for this session only.
    pub fn select_audio_track(&mut self, index: usize, cx: &mut Context<Self>) {
        let tracks = &mut self.video.tracks;
        tracks.audio_menu_open = false;
        if index < tracks.audio.len() {
            if let Some(player) = self.video.player.as_mut() {
                player.select_audio_track(index);
            }
        }
        cx.notify();
    }

    /// The track the player plays now. Asked each time rather than kept:
    /// AVPlayer applies its automatic language choice after the load.
    pub(crate) fn current_audio_track(&self) -> Option<usize> {
        self.video
            .player
            .as_ref()
            .and_then(|player| player.selected_audio_track())
    }

    pub fn toggle_audio_menu(&mut self, cx: &mut Context<Self>) {
        self.video.tracks.audio_menu_open = !self.video.tracks.audio_menu_open;
        self.video.speed_menu_open = false;
        cx.notify();
    }

    /// ⌥→ / ⌥←: next chapter, or back to the start of this one (or the one
    /// before). A jump is an ordinary seek, so a loop range treats it like
    /// any other: past the out point, playback wraps to the in point.
    /// Without chapters the keys seek 5 s, as they did before chapters.
    pub fn jump_chapter(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.video.tracks.chapters.is_empty() {
            self.seek_video_relative(if forward { 5.0 } else { -5.0 }, cx);
            return;
        }
        let starts: Vec<f64> = self.video.tracks.chapters.iter().map(|c| c.0).collect();
        let Some(index) = chapter_target(&starts, self.current_player_position(), forward) else {
            return;
        };
        self.seek_video_to_time(starts[index]);
        let name = self.video.tracks.chapter_name(index);
        self.show_toast(t("video.chapter_jump").replace("{name}", &name), cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTS: [f64; 3] = [0.0, 20.0, 40.0];

    #[core::prelude::v1::test]
    fn next_skips_a_start_the_playhead_is_on() {
        assert_eq!(chapter_target(&STARTS, 20.0, true), Some(2));
        assert_eq!(
            chapter_target(&STARTS, 19.98, true),
            Some(2),
            "a hair early"
        );
        assert_eq!(chapter_target(&STARTS, 40.0, true), None);
    }

    #[core::prelude::v1::test]
    fn previous_restarts_then_steps_back() {
        assert_eq!(chapter_target(&STARTS, 30.0, false), Some(1));
        assert_eq!(
            chapter_target(&STARTS, 23.0, false),
            Some(0),
            "at the grace edge"
        );
        assert_eq!(chapter_target(&STARTS, 20.0, false), Some(0), "on a start");
        assert_eq!(chapter_target(&STARTS, 1.0, false), Some(0), "first stays");
        assert_eq!(
            chapter_target(&[5.0], 1.0, false),
            None,
            "before any chapter"
        );
        assert_eq!(chapter_target(&[], 1.0, true), None);
    }

    #[core::prelude::v1::test]
    fn chapters_are_cleaned_once_on_load() {
        let raw = vec![
            (40.0, "c".to_string()),
            (f64::NAN, "nan".to_string()),
            (0.0, "a".to_string()),
            (0.0, "a again".to_string()),
            (-1.0, "before".to_string()),
            (90.0, "past the end".to_string()),
            (20.0, "b".to_string()),
        ];
        let starts: Vec<f64> = clean_chapters(raw.clone(), 60.0)
            .iter()
            .map(|c| c.0)
            .collect();
        assert_eq!(starts, [0.0, 20.0, 40.0]);
        assert_eq!(
            clean_chapters(raw, 0.0).len(),
            4,
            "an unknown duration keeps late chapters"
        );
    }
}
