use gpui::*;
use lumina_core::{MediaItem, MediaType};
use std::path::Path;

use crate::app::types::LayoutMode;
use crate::app::LuminaApp;
use crate::views::viewer_state::ViewerZoomPreset;

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

    /// Zooms the image viewer by `delta` (used by scroll-wheel/pinch).
    pub fn zoom_viewer_by(&mut self, delta: f32, cx: &mut Context<Self>) {
        if self
            .selected_item
            .as_ref()
            .is_some_and(|item| item.media_type == MediaType::Video)
        {
            return;
        }
        self.viewer_state
            .set_zoom(self.viewer_state.zoom_scale + delta);
        cx.notify();
    }

    pub fn open_selected_in_default_app(&self) {
        if let Some(ref item) = self.selected_item {
            self.open_item_in_default_app(&item.path);
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
