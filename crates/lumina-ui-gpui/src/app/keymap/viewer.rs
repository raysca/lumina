use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::views::viewer_state::ViewerZoomPreset;

impl LuminaApp {
    /// Handles shortcuts when fullscreen viewer is open.
    ///
    /// Zoom: `+`/`-` step by 0.25, `0` is Fit, `1`/`2`/`4` are 100%/200%/400%
    /// of actual pixels, and `Z` toggles Fit/100%. Returns `true` if handled.
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

        // Viewer Zoom Shortcuts: + / - / 0 / 1 / 2 / 4 / Z (images and video)
        if key == "=" || key == "+" {
            self.zoom_in_viewer(cx);
            return true;
        }
        if key == "-" || key == "_" {
            self.zoom_out_viewer(cx);
            return true;
        }
        // Zoom presets: 0 = Fit, 1 = actual pixels, 2 = 2x, 4 = 4x.
        if !cmd_or_ctrl {
            let preset = match key {
                "0" => Some(ViewerZoomPreset::Fit),
                "1" => Some(ViewerZoomPreset::Actual),
                "2" => Some(ViewerZoomPreset::TwoX),
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

        // Open in Default App (QuickTime / Preview)
        if !cmd_or_ctrl && key.eq_ignore_ascii_case("o") {
            self.open_selected_in_default_app();
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

        if is_video && !self.video.in_pip && !cmd_or_ctrl {
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
            if self.video.speed_menu_open {
                self.video.speed_menu_open = false;
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
