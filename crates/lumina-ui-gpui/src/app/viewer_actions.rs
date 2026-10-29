use gpui::*;
use lumina_core::{MediaItem, MediaType};
use std::path::Path;
use std::process::Command;

use crate::app::types::LayoutMode;
use crate::app::LuminaApp;
use crate::config::validate_external_editor;
use crate::views::viewer_state::ViewerZoomPreset;

/// Banner shown when Edit in External Editor has no configured application.
pub(crate) const EXTERNAL_EDITOR_UNSET_MESSAGE: &str =
    "No external editor configured. Choose an external editor in Preferences.";
/// Banner shown when the saved application value or target path is unusable.
pub(crate) const EXTERNAL_EDITOR_INVALID_MESSAGE: &str =
    "The external editor setting is invalid. Choose an external editor in Preferences.";
/// Banner shown when the media file has gone away since it was selected.
pub(crate) const EXTERNAL_EDITOR_FILE_MISSING_MESSAGE: &str =
    "The file is no longer available; nothing was launched.";

/// Builds the launch command for `file` in `editor` without running it.
///
/// `editor` is an absolute application path or bundle identifier and `file` is
/// the media path. Returns `None` when either value is unusable so callers can
/// report the configuration error instead of launching a process. The app and
/// file are separate [`Command`] arguments, never a shell string, so spaces and
/// shell metacharacters in either value have no special meaning.
pub(crate) fn external_editor_command(editor: &str, file: &Path) -> Option<Command> {
    let app = validate_external_editor(Some(editor))?;
    if file.as_os_str().is_empty() {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg("-a").arg(app).arg(file);
        Some(command)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut command = Command::new(app);
        command.arg(file);
        Some(command)
    }
}

impl LuminaApp {
    pub fn reveal_item_in_finder(&self, path: &Path) {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            if let Some(parent) = path.parent() {
                let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
            }
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,\"{}\"", path.display()))
                .spawn();
        }
    }

    pub fn open_item_in_default_app(&self, path: &Path) {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(path).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", ""])
                .arg(path)
                .spawn();
        }
    }

    /// True when the viewer is showing a video, where zoom/pan is disabled so
    /// the wheel stays available to the transport controls.
    fn viewer_zoom_disabled_for_video(&self) -> bool {
        self.selected_item
            .as_ref()
            .is_some_and(|item| item.media_type == MediaType::Video)
    }

    /// Zooms the image viewer by `delta` (used by scroll-wheel/pinch).
    pub fn zoom_viewer_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        if self.viewer_zoom_disabled_for_video() {
            return;
        }
        self.viewer_state
            .set_zoom(self.viewer_state.zoom_scale + delta);
        cx.notify();
    }

    /// Zoom by `delta` anchored to the content point under the wheel cursor.
    ///
    /// `cursor` is the wheel position in window-content pixels
    /// (`ScrollWheelEvent::position`) and `window_viewport` is the drawable
    /// window size (`Window::viewport_size()`). Both are supplied by the
    /// viewport's scroll handler.
    ///
    /// Approximation: GPUI 0.2.2 does not expose the laid-out
    /// `media-canvas-fit` bounds to a scroll listener, so the viewer canvas
    /// center is taken as the center of the window content area. In the
    /// fullscreen viewer the canvas spans the window width and is inset only by
    /// the top bar and bottom tray, so the anchor lands within a few pixels; in
    /// Split/Viewer pane layouts the canvas is offset by the sidebar and year
    /// rail, so the anchor is directionally correct rather than pixel-exact.
    ///
    /// The wheel step, 1.0..5.0 clamp, and Fit-recenters rule all come from
    /// `ViewerState::zoom_at`, so keyboard/HUD zoom is unchanged.
    ///
    /// When geometry is unusable (zero-sized or non-finite viewport, non-finite
    /// cursor) it falls back to center-anchored zoom rather than inventing an
    /// anchor.
    pub fn zoom_viewer_at(
        &mut self,
        delta: f32,
        cursor: (f32, f32),
        window_viewport: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        if self.viewer_zoom_disabled_for_video() {
            return;
        }
        let has_geometry = cursor.0.is_finite()
            && cursor.1.is_finite()
            && window_viewport.0.is_finite()
            && window_viewport.1.is_finite()
            && window_viewport.0 > 0.0
            && window_viewport.1 > 0.0;
        if !has_geometry {
            self.zoom_viewer_by(delta, cx);
            return;
        }
        let canvas_center = (window_viewport.0 * 0.5, window_viewport.1 * 0.5);
        self.viewer_state.zoom_at(delta, cursor, canvas_center);
        cx.notify();
    }

    pub fn open_selected_in_default_app(&self) {
        if let Some(ref item) = self.selected_item {
            self.open_item_in_default_app(&item.path);
        }
    }

    /// Opens `path` in the configured external editor.
    ///
    /// Open in Default App is deliberately separate and never consults this
    /// setting. Missing configuration, an invalid saved app, or a missing file
    /// surface in the existing error banner instead of launching anything.
    pub fn open_item_in_external_editor(&mut self, path: &Path, cx: &mut Context<Self>) {
        let Some(raw) = self.config.external_editor.clone() else {
            self.set_error(EXTERNAL_EDITOR_UNSET_MESSAGE, cx);
            return;
        };
        let Some(editor) = validate_external_editor(Some(&raw)).map(str::to_string) else {
            self.set_error(EXTERNAL_EDITOR_INVALID_MESSAGE, cx);
            return;
        };
        // Never hand an empty or vanished path to the launcher.
        if path.as_os_str().is_empty() || !path.exists() {
            self.set_error(EXTERNAL_EDITOR_FILE_MISSING_MESSAGE, cx);
            return;
        }
        let Some(mut command) = external_editor_command(&editor, path) else {
            self.set_error(EXTERNAL_EDITOR_INVALID_MESSAGE, cx);
            return;
        };
        if let Err(err) = command.spawn() {
            self.set_error(format!("Could not open in external editor: {err}"), cx);
        }
    }

    pub fn open_viewer(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        self.select_item(item, cx);
        self.is_fullscreen_viewer = true;
        self.viewer_state.reset();
        if let Some(ref it) = self.selected_item {
            if it.media_type == MediaType::Video {
                self.load_video_player(&it.path.clone(), cx);
            } else {
                self.stop_video_player();
            }
        }
        cx.notify();
    }

    #[cfg(test)]
    pub fn toggle_viewer_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.viewer_shortcuts_open = !self.viewer_shortcuts_open;
        cx.notify();
    }

    pub fn close_viewer(&mut self, cx: &mut Context<Self>) {
        self.is_fullscreen_viewer = false;
        self.is_immersive_fullscreen = false;
        self.immersive_hud_visible = true;
        self.viewer_shortcuts_open = false;
        if self.layout_mode == LayoutMode::Viewer {
            self.layout_mode = LayoutMode::Browser;
        }
        self.viewer_state.reset();
        if self.floating_pip_window.is_none() {
            self.stop_video_player();
        }
        self.save_config();
        cx.notify();
    }

    pub fn enter_immersive_fullscreen(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.is_immersive_fullscreen = true;
        self.immersive_hud_visible = true;
        if let Some(w) = window {
            if !w.is_fullscreen() {
                w.toggle_fullscreen();
            }
        }
        cx.notify();
    }

    pub fn exit_immersive_fullscreen(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.is_immersive_fullscreen = false;
        self.immersive_hud_visible = true;
        if let Some(w) = window {
            if w.is_fullscreen() {
                w.toggle_fullscreen();
            }
        }
        cx.notify();
    }

    pub fn toggle_immersive_fullscreen(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if self.is_immersive_fullscreen {
            self.exit_immersive_fullscreen(window, cx);
        } else {
            self.enter_immersive_fullscreen(window, cx);
        }
    }

    pub fn toggle_immersive_hud(&mut self, cx: &mut Context<Self>) {
        self.immersive_hud_visible = !self.immersive_hud_visible;
        cx.notify();
    }

    pub fn set_viewer_zoom(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.viewer_state.set_zoom(scale);
        cx.notify();
    }

    pub fn zoom_in_viewer(&mut self, cx: &mut Context<Self>) {
        self.viewer_state.zoom_in();
        cx.notify();
    }

    pub fn zoom_out_viewer(&mut self, cx: &mut Context<Self>) {
        self.viewer_state.zoom_out();
        cx.notify();
    }

    pub fn toggle_viewer_zoom(&mut self, cx: &mut Context<Self>) {
        self.viewer_state.toggle_zoom();
        cx.notify();
    }

    /// Applies an explicit zoom preset from the HUD or the 1/2/4 keys.
    pub fn apply_viewer_zoom_preset(&mut self, preset: ViewerZoomPreset, cx: &mut Context<Self>) {
        self.viewer_state.apply_preset(preset);
        cx.notify();
    }

    /// Resets zoom and pan to fit from the HUD reset control.
    pub fn reset_viewer_zoom(&mut self, cx: &mut Context<Self>) {
        self.viewer_state.reset();
        cx.notify();
    }

    pub fn start_viewer_drag(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        self.viewer_state.start_drag(x, y);
        cx.notify();
    }

    pub fn drag_viewer_to(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        self.viewer_state.drag_to(x, y);
        cx.notify();
    }

    pub fn end_viewer_drag(&mut self, cx: &mut Context<Self>) {
        self.viewer_state.end_drag();
        cx.notify();
    }
}
