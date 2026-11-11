use gpui::*;
use lumina_core::{MediaItem, MediaType};
use std::path::Path;
use std::process::Command;

use crate::app::LuminaApp;
use crate::config::validate_external_editor;
use crate::views::viewer_state::ViewerZoomPreset;

/// Immersive mode hides the HUD after this long without pointer or key
/// activity (v3 polish: immersive only; the windowed viewer keeps it).
pub(crate) const HUD_IDLE: std::time::Duration = std::time::Duration::from_secs(2);

/// Auto-hide state for the immersive HUD.
#[derive(Default)]
pub(crate) struct HudIdle {
    /// Pending hide; replacing or dropping it cancels the hide.
    task: Option<Task<()>>,
    pointer_on_hud: bool,
}

/// Banner shown when Edit in External Editor has no configured application.
pub(crate) const EXTERNAL_EDITOR_UNSET_MESSAGE: &str =
    "No external editor configured. Choose an external editor in Preferences.";
/// Banner shown when the saved application value or target path is unusable.
pub(crate) const EXTERNAL_EDITOR_INVALID_MESSAGE: &str =
    "The external editor setting is invalid. Choose an external editor in Preferences.";
/// Banner shown when the media file has gone away since it was selected
/// (external editor, Open in Default App and Reveal in Finder).
pub(crate) const EXTERNAL_EDITOR_FILE_MISSING_MESSAGE: &str =
    "The file is no longer available; nothing was launched.";
/// Banner shown when the configured editor could not open the file, for
/// example because the app was deleted.
pub(crate) const EXTERNAL_EDITOR_FAILED_MESSAGE: &str =
    "The external editor could not open this file. Choose an external editor in Preferences.";
/// Banner shown when Finder could not reveal the file.
pub(crate) const REVEAL_FAILED_MESSAGE: &str = "Could not show this file in Finder.";

/// Decode size requested for the full-viewer still, matching the selection path
/// in `select_item_internal`; the grid requests 320 and the viewer 2048.
///
/// Deliberate simplification: one fixed viewer decode instead of per-zoom
/// tiers (2048 for 2x, 4096 for 4x). 2048 already exceeds a 1440-1660 px Fit
/// viewport, so 2x has real pixels and 4x upscales the same texture; tiered
/// decodes are deferred until zoom latency/memory is measured.
pub(crate) const VIEWER_IMAGE_SIZE: u32 = 2048;

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
        // `open -a` takes an app path or name ("Preview", "TextEdit.app");
        // a bundle id ("com.apple.TextEdit") needs `-b`.
        let is_app_bundle = Path::new(app)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"));
        let is_bundle_id = !app.starts_with('/') && !is_app_bundle && app.contains('.');
        let flag = if is_bundle_id { "-b" } else { "-a" };
        let mut command = Command::new("open");
        command.arg(flag).arg(app).arg(file);
        Some(command)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut command = Command::new(app);
        command.arg(file);
        Some(command)
    }
}

/// Evicts images queued by [`LuminaApp::retire_image`] (replaced viewer
/// frames, evicted thumbnails) from every window's GPU atlas. Renders drain
/// the queue here; the UI message loop also drains it, so playback with every
/// window minimized cannot pile frames up.
pub(crate) fn evict_retired_images(
    images: Vec<std::sync::Arc<RenderImage>>,
    window: &mut Window,
    cx: &mut App,
) {
    for image in images {
        cx.drop_image(image, Some(window));
    }
}

impl LuminaApp {
    /// Replaces the viewer image, queueing the old one for GPU eviction.
    ///
    /// Only an image nobody else holds is retired: grid thumbnails shown as a
    /// placeholder stay owned by the thumbnail cache and keep their texture.
    pub(crate) fn set_viewer_image(&mut self, image: Option<std::sync::Arc<RenderImage>>) {
        let old = std::mem::replace(&mut self.viewer_image, image);
        if let Some(old) = old {
            let replaced_by_self = self
                .viewer_image
                .as_ref()
                .is_some_and(|new| new.id == old.id);
            if !replaced_by_self {
                self.retire_image(old);
            }
        }
    }

    /// Queues an image the app has stopped showing for GPU eviction.
    ///
    /// Only an image nobody else holds is retired: a thumbnail still shown as
    /// the viewer placeholder (or vice versa) keeps its texture until the last
    /// holder lets go and retires it.
    pub(crate) fn retire_image(&mut self, image: std::sync::Arc<RenderImage>) {
        if std::sync::Arc::strong_count(&image) == 1 {
            self.retired_images.push(image);
        }
    }

    /// Shows `path` in Finder. A missing file, or a launcher that cannot
    /// start or fails, shows in the error banner.
    pub fn reveal_item_in_finder(&mut self, path: &Path, cx: &mut Context<Self>) {
        if path.as_os_str().is_empty() || !path.exists() {
            self.set_error(EXTERNAL_EDITOR_FILE_MISSING_MESSAGE, cx);
        } else {
            // Tests record the request instead of opening a real Finder window.
            #[cfg(test)]
            REVEALED_PATHS.with(|paths| paths.borrow_mut().push(path.to_path_buf()));
            #[cfg(not(test))]
            if let Some(command) = reveal_command(path) {
                Self::launch_reporting(command, REVEAL_FAILED_MESSAGE, cx);
            }
        }
    }

    /// Opens `path` with the system's default app. A missing file, a
    /// launcher that cannot start, or one that reports no app for the file
    /// shows in the error banner.
    pub fn open_item_in_default_app(&mut self, path: &Path, cx: &mut Context<Self>) {
        if path.as_os_str().is_empty() || !path.exists() {
            self.set_error(EXTERNAL_EDITOR_FILE_MISSING_MESSAGE, cx);
        } else {
            // UI tests drive the viewer buttons through synthetic clicks;
            // never launch a real Preview/QuickTime process from the harness.
            #[cfg(not(test))]
            Self::launch_reporting(default_app_command(path), DEFAULT_APP_FAILED_MESSAGE, cx);
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

    /// Zoom by `delta` anchored to the content point under the ⌘-wheel cursor
    /// or the trackpad pinch centre.
    ///
    /// `cursor` is that position in window-content pixels
    /// (`ScrollEvent::position` / `PinchEvent::position`) and `window_viewport` is the drawable
    /// window size (`Window::viewport_size()`). Both are supplied by the
    /// viewport's scroll and pinch handlers.
    ///
    /// The anchor uses the canvas center captured when `media-canvas-fit`
    /// last painted (the window center before the first paint), so it is
    /// exact in the full-screen viewer, the only place the viewer shows.
    ///
    /// The 1..16 clamp (`ZOOM_MIN_SCALE..=ZOOM_MAX_SCALE`) and the
    /// Fit-recenters rule come from `ViewerState::zoom_at`, so keyboard zoom
    /// is unchanged.
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
        let canvas_center = self
            .viewer_state
            .canvas
            .map_or((window_viewport.0 * 0.5, window_viewport.1 * 0.5), |c| {
                c.center
            });
        self.viewer_state.zoom_at(delta, cursor, canvas_center);
        cx.notify();
    }

    pub fn open_selected_in_default_app(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.selected_item.as_ref().map(|item| item.path.clone()) {
            self.open_item_in_default_app(&path, cx);
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
        let Some(command) = external_editor_command(&editor, path) else {
            self.set_error(EXTERNAL_EDITOR_INVALID_MESSAGE, cx);
            return;
        };
        // Tests never launch a real editor; they record the command.
        #[cfg(test)]
        crate::app::external_edit::test_launches::record(&command);
        #[cfg(not(test))]
        Self::launch_reporting(command, EXTERNAL_EDITOR_FAILED_MESSAGE, cx);
    }

    /// Runs a launcher (`open`, `open -a`, `open -R`) off the UI thread. If
    /// it cannot start or exits non-zero, logs why and shows `failed` in the
    /// error banner. On macOS `open` returns as soon as LaunchServices has
    /// the request, so this does not wait for the app.
    /// ponytail: no timeout; on Linux/Windows the editor runs directly and
    /// this waits for it to quit. Spawn without waiting there when those ship.
    #[cfg(not(test))]
    fn launch_reporting(mut command: Command, failed: &'static str, cx: &mut Context<Self>) {
        let launch = cx
            .background_executor()
            .spawn(async move { command.output() });
        cx.spawn(async move |this, cx| {
            if let Err(message) = launch_result(launch.await, failed) {
                let _ = this.update(cx, |this, cx| this.set_error(message, cx));
            }
        })
        .detach();
    }

    /// Ids of the items before and after the selection, from the cached
    /// rows only (no SQL), for the viewer's prefetch.
    pub(crate) fn viewer_neighbour_ids(&self) -> Vec<lumina_core::MediaId> {
        let Some(index) = self.selected_index else {
            return Vec::new();
        };
        let start = index.saturating_sub(1);
        self.displayed_rows(start..index + 2)
            .into_iter()
            .zip(start..)
            .filter(|(_, at)| *at != index)
            .map(|(item, _)| item.id)
            .collect()
    }

    /// Queues the neighbours' grid thumbnails, so stepping in the viewer
    /// shows a picture at once. Cached, in-flight and failed ids are skipped.
    pub(crate) fn prefetch_viewer_neighbours(&self) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        for id in self.viewer_neighbour_ids() {
            if !state.texture_cache.lock().contains(&id) {
                self.queue_thumbnail_request(id);
            }
        }
    }

    pub fn open_viewer(&mut self, item: MediaItem, cx: &mut Context<Self>) {
        // With PiP open, playing a video means playing it there.
        if self.video.in_pip && item.media_type == MediaType::Video {
            self.select_item(item.clone(), cx);
            self.play_in_pip(item, cx);
            return;
        }
        self.select_item(item, cx);
        self.is_fullscreen_viewer = true;
        self.prefetch_viewer_neighbours();
        self.viewer_state.reset();
        self.sync_viewer_player(cx);
        cx.notify();
    }

    #[cfg(test)]
    pub fn toggle_viewer_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.viewer_shortcuts_open = !self.viewer_shortcuts_open;
        cx.notify();
    }

    pub fn close_viewer(&mut self, cx: &mut Context<Self>) {
        // The Back button or a removed item ends a slideshow too.
        self.drop_slideshow(cx);
        self.is_fullscreen_viewer = false;
        self.is_immersive_fullscreen = false;
        self.immersive_hud_visible = true;
        self.hud_idle = Default::default();
        self.viewer_shortcuts_open = false;
        self.rate_popover_open = false;
        self.context_menu_state = None;
        // Closing the viewer (the Back button) discards an open draft.
        self.discard_edit_draft();
        self.viewer_state.reset();
        if !self.video.in_pip {
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
        self.wake_immersive_hud(cx);
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
        // Drop only the pending hide: the windowed top bar stays mounted, so
        // its hover state (and `pointer_on_hud`) is still current.
        self.hud_idle.task = None;
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
        if self.immersive_hud_visible {
            self.immersive_hud_visible = false;
            self.hud_idle = Default::default();
            cx.notify();
        } else {
            self.wake_immersive_hud(cx);
        }
    }

    /// Pointer movement or a key in immersive mode: show the HUD and hide it
    /// again after `HUD_IDLE` without activity. Each wake replaces the timer
    /// task, which cancels the previous one.
    pub(crate) fn wake_immersive_hud(&mut self, cx: &mut Context<Self>) {
        if !self.is_immersive_fullscreen {
            return;
        }
        if !self.immersive_hud_visible {
            self.immersive_hud_visible = true;
            cx.notify();
        }
        self.hud_idle.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HUD_IDLE).await;
            let _ = this.update(cx, |this, cx| {
                if !this.is_immersive_fullscreen {
                    return;
                }
                if this.hud_must_stay() {
                    // Check again later: a menu closed by key or a drag that
                    // ends without a move sends no wake of its own.
                    this.wake_immersive_hud(cx);
                } else {
                    this.immersive_hud_visible = false;
                    cx.notify();
                }
            });
        }));
    }

    /// The HUD never hides under the pointer, over an open menu (context,
    /// speed or audio) or help sheet, mid-scrub or mid-pan (hiding the transport would drop the
    /// release), or while a field takes typing.
    fn hud_must_stay(&self) -> bool {
        self.hud_idle.pointer_on_hud
            || self.context_menu_state.is_some()
            || self.video.speed_menu_open
            || self.video.tracks.audio_menu_open
            || self.video.is_scrubbing
            || self.viewer_state.is_dragging
            || self.viewer_shortcuts_open
            || self.rate_popover_shown()
            || self.text_target().is_some()
    }

    /// Hover tracking for the HUD's top bar and video transport.
    pub(crate) fn set_pointer_on_hud(&mut self, on: bool) {
        self.hud_idle.pointer_on_hud = on;
    }

    #[cfg(test)]
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

    /// Applies an explicit zoom preset (⌘0–⌘4, the navigator buttons, ⌘K).
    pub fn apply_viewer_zoom_preset(&mut self, preset: ViewerZoomPreset, cx: &mut Context<Self>) {
        self.viewer_state.apply_preset(preset);
        cx.notify();
    }

    /// Resets zoom and pan to fit (the Fit to window button).
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

    /// Scroll-pan: pans a zoomed photo by the scroll delta (window pixels).
    /// Nothing at Fit or on a video.
    pub fn pan_viewer_by(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        if self.viewer_zoom_disabled_for_video() || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let before = self.viewer_state.pan_offset;
        self.viewer_state.pan_by(dx, dy);
        if self.viewer_state.pan_offset != before {
            cx.notify();
        }
    }

    pub fn end_viewer_drag(&mut self, cx: &mut Context<Self>) {
        self.viewer_state.end_drag();
        cx.notify();
    }
}

/// The platform's "open with the default app" launcher for `path`.
#[cfg(not(test))]
fn default_app_command(path: &Path) -> Command {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/c", "start", ""]);
        command
    };
    command.arg(path);
    command
}

/// Banner shown when the launcher ran but reported it could not open the
/// file (typically no app is set for its type).
pub(crate) const DEFAULT_APP_FAILED_MESSAGE: &str =
    "No app could open this file. Choose one with Open With in Finder.";

/// The banner message for a finished launch, if it failed: `failed`, plus
/// the error when the launcher could not start. The launcher's own stderr
/// (a file:// URL and a LaunchServices code on macOS) goes to the log, not
/// the banner.
pub(crate) fn launch_result(
    output: std::io::Result<std::process::Output>,
    failed: &str,
) -> Result<(), String> {
    let output = match output {
        Ok(output) => output,
        Err(err) => {
            log::warn!("launcher could not start: {err}");
            return Err(format!("{failed} Reason: {err}."));
        }
    };
    if output.status.success() {
        return Ok(());
    }
    log::warn!(
        "launch failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Err(failed.to_string())
}

/// The platform's "show in the file manager" launcher for `path`.
#[cfg(not(test))]
fn reveal_command(path: &Path) -> Option<Command> {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        Some(command)
    }
    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent()?);
        Some(command)
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("explorer");
        command.arg(format!("/select,\"{}\"", path.display()));
        Some(command)
    }
}

#[cfg(test)]
thread_local! {
    /// Paths the app asked Finder to reveal on this test thread.
    pub(crate) static REVEALED_PATHS: std::cell::RefCell<Vec<std::path::PathBuf>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
