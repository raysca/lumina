//! Shared helpers for the in-process UI tests: seeded temp libraries,
//! a test window with the app, and synthetic input at rendered bounds.

use crate::*;
use lumina_core::{EngineEvent, LuminaEngine};

/// A real 8×8 JPEG, so seeded photos decode in process.
pub(crate) fn tiny_jpeg() -> &'static [u8] {
    static BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    BYTES.get_or_init(|| shaded_jpeg(90))
}

/// A real 8×8 JPEG whose bytes differ per `shade` (not an exact copy).
pub(crate) fn shaded_jpeg(shade: u8) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(8, 8, image::Rgb([shade, 120, 160]));
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut bytes)
        .encode_image(&image)
        .expect("an 8×8 JPEG encodes");
    bytes
}

/// A library of `count` real JPEGs in a fresh temp dir, scanned.
pub(crate) fn seed_library(
    prefix: &str,
    count: usize,
) -> (TempLibrary, std::sync::Arc<LuminaEngine>) {
    seed_with(prefix, count, tiny_jpeg())
}

/// A library of files named `.jpg` that are not images: no size in the
/// catalog, and every decode of them fails (fast: sips rejects them), so no
/// texture ever lands. Only for tests about failed or texture-less cards
/// and stages; for a card still decoding, see [`app_with_pending`].
pub(crate) fn seed_library_undecodable(
    prefix: &str,
    count: usize,
) -> (TempLibrary, std::sync::Arc<LuminaEngine>) {
    seed_with(prefix, count, b"test-jpeg-content")
}

fn seed_with(
    prefix: &str,
    count: usize,
    bytes: &[u8],
) -> (TempLibrary, std::sync::Arc<LuminaEngine>) {
    // The clock alone is not unique: tests start together and macOS reports
    // microseconds, so two could share a catalog ("database is locked").
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temp_dir = std::env::temp_dir().join(format!(
        "lumina_uitest_{}_{}_{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    for i in 0..count {
        std::fs::write(temp_dir.join(format!("photo_{:02}.jpg", i)), bytes).unwrap();
    }
    let (engine, rx) = LuminaEngine::open_with_paths(
        temp_dir.clone(),
        temp_dir.join("catalog.db"),
        Some(temp_dir.join("cache")),
    )
    .unwrap();
    wait_for_scan(|timeout| rx.recv_timeout(timeout).ok());
    (TempLibrary(temp_dir), engine)
}

/// Waits for the first scan's ScanComplete; `next` is the engine
/// receiver's `recv_timeout`. A scan can be silent longer than one native
/// tool timeout (10 s: a fake `.mp4` asks Spotlight for its duration), so a
/// fixed per-event wait under load used to end early and leave the library
/// half catalogued. This waits for the event itself, bounded only so a
/// broken scan fails here with a clear message.
pub(crate) fn wait_for_scan(mut next: impl FnMut(std::time::Duration) -> Option<EngineEvent>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match next(left) {
            Some(EngineEvent::ScanComplete { .. }) => return,
            Some(_) => {}
            None => panic!("the test library's scan never completed"),
        }
    }
}

pub(crate) fn leak(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

/// Draws the registered root view so layout, hit-testing and debug_bounds are
/// populated for the current app state.
pub(crate) fn render_ui(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

pub(crate) fn focus_app(cx: &mut VisualTestContext, view: &Entity<LuminaApp>) {
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle.clone();
        window.focus(&handle, cx);
    });
}

pub(crate) fn bounds_center(
    cx: &mut VisualTestContext,
    selector: &'static str,
) -> gpui::Point<gpui::Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("no rendered bounds for selector {selector:?}"))
        .center()
}

pub(crate) fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_click(position, gpui::Modifiers::default());
    render_ui(cx);
}

pub(crate) fn click_point(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_click(
        gpui::point(gpui::px(x), gpui::px(y)),
        gpui::Modifiers::default(),
    );
    render_ui(cx);
}

pub(crate) fn keystrokes(cx: &mut VisualTestContext, keys: &str) {
    cx.simulate_keystrokes(&platform_test_keys(keys));
    render_ui(cx);
}

/// The test data uses Command and GPUI's secondary modifier notation. Map both
/// to Control on Linux so the tests exercise the same app actions there.
pub(crate) fn platform_test_keys(keys: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        keys.split_whitespace()
            .map(|chord| {
                let parts = chord.split('-').collect::<Vec<_>>();
                if (parts.contains(&"cmd") || parts.contains(&"secondary"))
                    && !parts.contains(&"ctrl")
                {
                    parts
                        .into_iter()
                        .map(|part| {
                            if part == "cmd" || part == "secondary" {
                                "ctrl"
                            } else {
                                part
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("-")
                } else {
                    chord.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    #[cfg(not(target_os = "linux"))]
    {
        keys.to_string()
    }
}

pub(crate) fn primary_modifier() -> gpui::Modifiers {
    #[cfg(target_os = "linux")]
    {
        gpui::Modifiers {
            control: true,
            ..Default::default()
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        gpui::Modifiers::command()
    }
}

pub(crate) fn type_text(cx: &mut VisualTestContext, text: &str) {
    cx.simulate_input(text);
    render_ui(cx);
}

pub(crate) fn double_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
    });
    render_ui(cx);
}

pub(crate) fn right_click(cx: &mut VisualTestContext, selector: &'static str) {
    let position = bounds_center(cx, selector);
    cx.simulate_mouse_down(
        position,
        gpui::MouseButton::Right,
        gpui::Modifiers::default(),
    );
    render_ui(cx);
}

pub(crate) fn app_with_library(
    cx: &mut TestAppContext,
    engine: std::sync::Arc<LuminaEngine>,
) -> (Entity<LuminaApp>, &mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine));
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}

/// [`app_with_library`] with the decodes of `pending` claimed before the
/// first draw, so none is queued: their cards stay "still decoding".
pub(crate) fn app_with_pending<'a>(
    cx: &'a mut TestAppContext,
    engine: std::sync::Arc<LuminaEngine>,
    pending: &[lumina_core::MediaId],
) -> (Entity<LuminaApp>, &'a mut VisualTestContext) {
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    view.update(vcx, |app, cx| {
        let state = crate::views::grid::GridState::new(engine);
        state.in_flight.lock().extend(pending.iter().copied());
        app.grid_state = Some(state);
        app.refresh_album_counts();
        cx.notify();
    });
    render_ui(vcx);
    focus_app(vcx, &view);
    (view, vcx)
}
/// Sends a wheel event at `position` after moving the pointer there, because
/// GPUI hit-tests scroll events against the last mouse position.
pub(crate) fn wheel(
    cx: &mut VisualTestContext,
    position: gpui::Point<gpui::Pixels>,
    delta: gpui::ScrollDelta,
    modifiers: gpui::Modifiers,
) {
    cx.simulate_mouse_move(position, None, gpui::Modifiers::default());
    cx.simulate_event(gpui::ScrollEvent {
        position,
        delta,
        modifiers,
        touch_phase: gpui::TouchPhase::Moved,
        source: gpui::ScrollSource::Pointer,
    });
    render_ui(cx);
}

/// A temp test folder, deleted when it drops, so a failing test cleans up
/// too. Derefs to the path.
pub(crate) struct TempLibrary(pub(crate) std::path::PathBuf);

impl std::ops::Deref for TempLibrary {
    type Target = std::path::PathBuf;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TempLibrary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn press_viewer_key(
    app: &mut LuminaApp,
    key: &'static str,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<LuminaApp>,
) {
    let key = crate::tests::helpers::platform_test_keys(key);
    let key: &'static str = Box::leak(key.into_boxed_str());
    app.handle_key_down(
        &KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("test keystroke must parse"),
            is_held: false,
            prefer_character_input: false,
        },
        window,
        cx,
    );
}

pub(crate) fn render_viewer_ui(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

/// Synthetic double-click at a window-space position. The click count is read
/// from the mouse-up event, matching the platform driver used by the app.
pub(crate) fn double_click_at(
    cx: &mut gpui::VisualTestContext,
    position: gpui::Point<gpui::Pixels>,
) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        modifiers: gpui::Modifiers::default(),
        button: gpui::MouseButton::Left,
        click_count: 2,
    });
    render_viewer_ui(cx);
}

/// Opens the overlay viewer with a known gradient texture so the painted media
/// element has deterministic bounds independent of real decoder output.
pub(crate) fn viewer_with_gradient<'a>(
    cx: &'a mut gpui::TestAppContext,
    prefix: &str,
    width: f32,
    height: f32,
) -> (
    crate::tests::helpers::TempLibrary,
    gpui::Entity<LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (temp_dir, engine) = seed_library(prefix, 4);
    let (view, vcx) = app_with_library(cx, engine);
    let photo = view.update(vcx, |app, _| {
        app.get_active_slice(0..1)
            .into_iter()
            .next()
            .expect("seeded library must have an item")
    });
    view.update(vcx, |app, app_cx| {
        app.open_viewer(photo, app_cx);
        // The seeded JPEG payload is not a real image, so this gradient stays
        // the displayed texture for the whole test.
        app.viewer_image = Some(generate_gradient_image(
            (40, 60, 90),
            (10, 15, 30),
            800,
            600,
        ));
        app.window_bounds = Some((0.0, 0.0, width, height));
        app_cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(width), gpui::px(height)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);
    (temp_dir, view, vcx)
}

pub(crate) fn tiny_decoded_frame(id: lumina_core::MediaId) -> lumina_core::DecodedFrame {
    lumina_core::DecodedFrame {
        id,
        width: 2,
        height: 2,
        format: lumina_core::PixelFormat::Rgba8,
        data: vec![0; 16],
    }
}
