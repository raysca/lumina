//! Playback failed (Design 10): the failed clip's Try Again, Reveal in
//! Finder and their keys, for the viewer card and the PiP message.

use gpui::{Context, Keystroke};

use crate::app::LuminaApp;

#[cfg(test)]
thread_local! {
    /// Video loads attempted on this test thread.
    pub(crate) static VIDEO_LOADS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl LuminaApp {
    /// The viewer shows the failure card: its video failed to load and it
    /// is not playing in PiP. A missing original has its own card instead.
    pub(crate) fn playback_failed_shown(&self) -> bool {
        self.is_fullscreen_viewer
            && !self.video.in_pip
            && self.video.load_error.is_some()
            && self
                .video
                .item
                .as_ref()
                .is_some_and(|item| !self.missing_items.contains(&item.id))
    }

    /// Try Again: loads the failed clip again. PiP plays it if it loads.
    pub(crate) fn retry_video_load(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.video.item.clone() else {
            return;
        };
        // The load resets the card's focus; a retry that fails again keeps
        // it, so ↵ can try again without another Tab.
        let focus = self.video.failed_focus;
        self.load_video_player(&item, cx);
        if self.video.load_error.is_some() {
            self.video.failed_focus = focus;
        }
        if self.video.in_pip && self.video.player.is_some() && !self.video.is_playing {
            self.toggle_video_play(cx);
        }
    }

    /// Reveal in Finder for the failed clip (the ⇧⌘R path).
    pub(crate) fn reveal_failed_video(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.video.item.as_ref().map(|item| item.path.clone()) {
            self.reveal_item_in_finder(&path, cx);
        }
    }

    /// The card's keys: Tab moves between its two buttons, ↵ presses the
    /// focused one. With no button focused ↵ keeps closing the viewer;
    /// Space, the arrows and Esc are untouched.
    pub(crate) fn handle_playback_failed_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.playback_failed_shown() {
            return false;
        }
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt {
            return false;
        }
        match (keystroke.key.as_str(), self.video.failed_focus) {
            ("tab", focus) => {
                // Two buttons: either direction toggles.
                self.video.failed_focus = Some(match focus {
                    Some(index) => 1 - index,
                    None if m.shift => 1,
                    None => 0,
                });
                cx.notify();
            }
            ("enter" | "return", Some(0)) if !m.shift => self.retry_video_load(cx),
            ("enter" | "return", Some(_)) if !m.shift => self.reveal_failed_video(cx),
            _ => return false,
        }
        true
    }
}
