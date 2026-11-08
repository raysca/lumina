//! Picture-in-Picture: the detached video player.
//!
//! There is one player session (`video`), so at most one video plays. PiP
//! presents that session in a floating window. While `video.in_pip` is set
//! the viewer and grid never load, stop or paint over the player: browsing
//! and viewing photos leave PiP playing, and opening another video replaces
//! the PiP video instead of opening the viewer.

use gpui::*;
use lumina_core::{MediaId, MediaItem, MediaType};

use crate::app::types::UiMessage;
use crate::app::LuminaApp;

impl LuminaApp {
    /// Plays `item` in PiP. When the viewer is already playing it, playback
    /// moves across without reloading; otherwise a new session starts.
    pub fn play_in_pip(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        if item.media_type != MediaType::Video {
            return;
        }
        let same_session = self.video.item.as_ref().is_some_and(|c| c.id == item.id);
        if same_session && !self.video.in_pip {
            // Hand the viewer's current frame over so PiP never flashes empty.
            self.video.pip_frame = self.viewer_image.clone();
        }
        self.video.in_pip = true;
        if !same_session {
            self.load_video_player(&item, cx);
        }
        // The viewer does not show a video that plays in PiP.
        if self.is_fullscreen_viewer && self.selected_item.as_ref().is_some_and(|s| s.id == item.id)
        {
            self.close_viewer(cx);
        }
        if !self.video.is_playing {
            if let Some(player) = self.video.player.as_mut() {
                use crate::player::VideoPlayerBackend;
                player.play();
                self.video.is_playing = true;
                self.video.seek_refresh_until = None;
                self.start_video_ticker();
            }
        }
        if self.floating_pip_window.is_none() {
            // Window creation needs `&mut App`; the message loop opens it.
            let _ = self.ui_tx.send_blocking(UiMessage::OpenFloatingPip);
        }
        cx.notify();
    }

    /// ⌥P: closes PiP when open, otherwise plays the selected video in PiP.
    pub fn toggle_pip(&mut self, cx: &mut Context<Self>) {
        if self.video.in_pip {
            self.close_pip(cx);
        } else if let Some(item) = self
            .selected_item
            .clone()
            .filter(|item| item.media_type == MediaType::Video)
        {
            self.play_in_pip(item, cx);
        }
    }

    /// Play/pause for the PiP session; a no-op when PiP is closed.
    pub fn toggle_pip_playback(&mut self, cx: &mut Context<Self>) {
        if self.video.in_pip {
            self.toggle_video_play(cx);
        }
    }

    /// Moves the PiP video back into the viewer at the same position.
    pub fn expand_pip_to_viewer(&mut self, cx: &mut Context<Self>) {
        if !self.video.in_pip {
            return;
        }
        let Some(item) = self.video.item.clone() else {
            return;
        };
        // Similar shots holds no selection: show the video from Exact copies.
        if self.similar_shots_shown() {
            self.set_duplicates_tab(crate::app::stacks::DuplicatesTab::Exact, cx);
        }
        // `in_pip` stays set while selecting so the viewer sync leaves the
        // player running. Position-based selection must follow the item; when
        // the current view no longer contains it, nothing stays selected by
        // index.
        if self.selected_item.as_ref().map(|s| s.id) != Some(item.id) {
            match self.navigation_index_for(item.id, None) {
                Some(index) => self.select_item_at(item, index, cx),
                None => {
                    self.clear_selection_fields();
                    self.select_item(item, cx);
                }
            }
        }
        self.video.in_pip = false;
        if let Some(frame) = self.video.pip_frame.take() {
            self.set_viewer_image(Some(frame));
        }
        self.is_fullscreen_viewer = true;
        self.viewer_state.reset();
        self.remove_pip_window(cx);
        cx.notify();
    }

    /// Stops the PiP video and closes its window.
    pub fn close_pip(&mut self, cx: &mut Context<Self>) {
        self.end_pip_session();
        self.remove_pip_window(cx);
        cx.notify();
    }

    /// A PiP window went away through the window manager. Messages from a
    /// window that is no longer the PiP window (already closed or expanded,
    /// then reopened) are ignored so they cannot end the newer session.
    pub fn floating_pip_closed(&mut self, window: WindowId, cx: &mut Context<Self>) {
        if self.floating_pip_window.map(|w| w.window_id()) != Some(window) {
            return;
        }
        self.floating_pip_window = None;
        self.end_pip_session();
        cx.notify();
    }

    /// Closes PiP when its video is among `ids` (moved to Trash or deleted).
    /// Returns whether it did, so the caller can say so in its toast.
    pub(crate) fn close_pip_for_removed(
        &mut self,
        ids: &[MediaId],
        cx: &mut Context<Self>,
    ) -> bool {
        let playing = self.video.item.as_ref().map(|item| item.id);
        if self.video.in_pip && playing.is_some_and(|id| ids.contains(&id)) {
            self.close_pip(cx);
            return true;
        }
        false
    }

    /// Starts or stops viewer playback for the selected item. While PiP owns
    /// the player the viewer never touches it; a video shows its poster.
    pub(crate) fn sync_viewer_player(&mut self, cx: &mut Context<Self>) {
        if self.video.in_pip {
            return;
        }
        match self.selected_item.clone() {
            Some(item) if item.media_type == MediaType::Video => self.load_video_player(&item, cx),
            _ => self.stop_video_player(),
        }
    }

    fn end_pip_session(&mut self) {
        if !self.video.in_pip {
            return;
        }
        self.stop_video_player();
        self.video.in_pip = false;
        if let Some(frame) = self.video.pip_frame.take() {
            self.retire_image(frame);
        }
        self.save_config();
    }

    fn remove_pip_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.floating_pip_window.take() {
            // Fails harmlessly when called from inside the PiP window's own
            // handler; that handler removes its window itself.
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
}
