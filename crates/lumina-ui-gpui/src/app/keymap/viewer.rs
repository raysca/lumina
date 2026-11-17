use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::views::viewer_state::{ViewerRotation, ViewerZoomPreset};

impl LuminaApp {
    /// Handles shortcuts when fullscreen viewer is open.
    ///
    /// Zoom: `+`/`-` step by 0.25, `⌘0` is Fit, `⌘1`/`⌘2`/`⌘4` are
    /// 100%/200%/400% of actual pixels, `⌘3` is Fill, and `Z` toggles
    /// Fit/100%. Plain digits rate and P/X/U flag (`app/culling.rs`, before
    /// this handler).
    /// Returns `true` if handled.
    pub fn handle_viewer_key_down(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // Full screen: ⌃⌘F. F is Favorite, as in the grid.
        if keystroke.modifiers.platform
            && keystroke.modifiers.control
            && key.eq_ignore_ascii_case("f")
        {
            self.toggle_immersive_fullscreen(Some(window), cx);
            return true;
        }
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("f") {
            self.toggle_favorite_selected(cx);
            return true;
        }

        // HUD Visibility Toggle: 'h'
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("h") {
            self.toggle_immersive_hud(cx);
            return true;
        }

        // Viewer Zoom Shortcuts: + / - / ⌘0–⌘4 / Z (images and video)
        if key == "=" || key == "+" {
            self.zoom_in_viewer(cx);
            return true;
        }
        if key == "-" || key == "_" {
            self.zoom_out_viewer(cx);
            return true;
        }
        // Zoom presets: ⌘0 = Fit, ⌘1 = actual pixels, ⌘2 = 2x, ⌘3 = Fill,
        // ⌘4 = 4x. Exactly ⌘: ⌃1-4 is macOS "Switch to Desktop N", and ⌃⌘
        // chords belong to the window (⌃⌘F).
        if crate::app::keymap::secondary_only(&keystroke.modifiers, cx) {
            let preset = match key {
                "0" => Some(ViewerZoomPreset::Fit),
                "1" => Some(ViewerZoomPreset::Actual),
                "2" => Some(ViewerZoomPreset::TwoX),
                "3" => Some(ViewerZoomPreset::Fill),
                "4" => Some(ViewerZoomPreset::FourX),
                _ => None,
            };
            if let Some(preset) = preset {
                self.apply_viewer_zoom_preset(preset, cx);
                return true;
            }
        }
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("z") {
            self.toggle_viewer_zoom(cx);
            return true;
        }

        // E: edit the open photo (exposure, auto, rotation).
        if !cmd_or_ctrl
            && !keystroke.modifiers.alt
            && !keystroke.modifiers.shift
            && key.eq_ignore_ascii_case("e")
        {
            self.enter_edit_mode(cx);
            return true;
        }

        // R / ⇧R: rotate the open still right / left.
        if !cmd_or_ctrl && !keystroke.modifiers.alt && key.eq_ignore_ascii_case("r") {
            let direction = if keystroke.modifiers.shift {
                ViewerRotation::Left
            } else {
                ViewerRotation::Right
            };
            self.rotate_selection(direction, cx);
            return true;
        }

        // Open in Default App (QuickTime / Preview)
        if !cmd_or_ctrl && !keystroke.modifiers.alt && key.eq_ignore_ascii_case("o") {
            self.open_selected_in_default_app(cx);
            return true;
        }

        // Bracket keys [ and ] for prev/next item navigation in viewer
        if key == "[" {
            self.navigate_selection(-1, cx);
            return true;
        }
        if key == "]" {
            self.navigate_selection(1, cx);
            return true;
        }

        let is_video = self
            .selected_item
            .as_ref()
            .map(|i| i.media_type == MediaType::Video)
            .unwrap_or(false);

        if key.eq_ignore_ascii_case("p") && keystroke.modifiers.alt && !cmd_or_ctrl {
            self.toggle_pip(cx);
            return true;
        }
        // While PiP owns the player the viewer shows this video's poster;
        // Space/K play it in PiP instead of controlling PiP's video.
        if is_video
            && self.video.in_pip
            && !cmd_or_ctrl
            && (key == "space" || key.eq_ignore_ascii_case("k"))
        {
            let is_pip_video =
                self.video.item.as_ref().map(|v| v.id) == self.selected_item.as_ref().map(|s| s.id);
            if is_pip_video {
                self.expand_pip_to_viewer(cx);
            } else if let Some(item) = self.selected_item.clone() {
                self.play_in_pip(item, cx);
            }
            return true;
        }

        // ⌥I / ⌥O / ⌥X on a video, playable or not: Option keys can be dead
        // keys, so the input context must not start a composition from them.
        if is_video
            && !self.video.in_pip
            && !cmd_or_ctrl
            && keystroke.modifiers.alt
            && !keystroke.modifiers.shift
            && matches!(key.to_ascii_lowercase().as_str(), "i" | "o" | "x")
        {
            cx.stop_propagation();
        }
        // A clip that failed to load skips the playback keys: Space does
        // nothing and ←/→ move between items, as on a still.
        if is_video && !self.video_unplayable() && !self.video.in_pip && !cmd_or_ctrl {
            // ⌥I / ⌥O / ⌥X: loop range.
            if keystroke.modifiers.alt && !keystroke.modifiers.shift {
                match key.to_ascii_lowercase().as_str() {
                    "i" => {
                        self.set_loop_in(cx);
                        return true;
                    }
                    "o" => {
                        self.set_loop_out(cx);
                        return true;
                    }
                    "x" => {
                        self.clear_loop_range(cx);
                        return true;
                    }
                    _ => {}
                }
            }
            // ⌥← / ⌥→: previous / next chapter. Before the plain arrows
            // below, which ignore modifiers and would seek instead.
            if keystroke.modifiers.alt
                && !keystroke.modifiers.shift
                && (key == "left" || key == "right")
            {
                self.jump_chapter(key == "right", cx);
                return true;
            }
            if key == "space" || key.eq_ignore_ascii_case("k") {
                self.toggle_video_play(cx);
                return true;
            }
            if key.eq_ignore_ascii_case("j") {
                self.seek_video_relative(-5.0, cx);
                return true;
            }
            // L loops (v3 design); ←/→ and J seek.
            if key.eq_ignore_ascii_case("l") {
                self.toggle_video_loop(cx);
                return true;
            }
            if key == "," {
                self.step_video_frame(-1, cx);
                return true;
            }
            if key == "." {
                self.step_video_frame(1, cx);
                return true;
            }
            if key.eq_ignore_ascii_case("m") {
                self.toggle_video_mute(cx);
                return true;
            }
            // Plain ↑/↓: volume (unbound elsewhere in the viewer).
            if (key == "up" || key == "down")
                && !keystroke.modifiers.alt
                && !keystroke.modifiers.shift
            {
                self.nudge_video_volume(key == "up", cx);
                return true;
            }
            if key == "left" {
                self.seek_video_relative(-5.0, cx);
                return true;
            }
            if key == "right" {
                self.seek_video_relative(5.0, cx);
                return true;
            }
        }
        // Space on a still intentionally does nothing: it must not close the
        // viewer. Escape and the viewer Back button are the exits.

        if key == "escape" {
            if self.video.speed_menu_open || self.video.tracks.audio_menu_open {
                self.video.speed_menu_open = false;
                self.video.tracks.audio_menu_open = false;
                cx.notify();
                return true;
            }
            if self.is_immersive_fullscreen {
                self.exit_immersive_fullscreen(Some(window), cx);
            } else {
                self.close_viewer(cx);
            }
            return true;
        }
        if key == "left" {
            self.navigate_selection(-1, cx);
            return true;
        }
        if key == "right" {
            self.navigate_selection(1, cx);
            return true;
        }

        true
    }
}
