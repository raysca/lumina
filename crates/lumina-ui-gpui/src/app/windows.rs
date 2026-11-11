use crate::app::{LuminaApp, UiMessage};
use crate::views::floating_inspector::FloatingInspectorView;
use crate::views::floating_pip::FloatingPipPlayerView;
use gpui::*;

pub fn open_floating_inspector_window(
    weak_app: WeakEntity<LuminaApp>,
    ui_tx: async_channel::Sender<UiMessage>,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(720.0), px(812.0)), cx);
    let weak_app_clone = weak_app.clone();
    let tx_clone = ui_tx.clone();

    match cx.open_window(
        WindowOptions {
            kind: WindowKind::Floating,
            focus: false,
            is_movable: true,
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Inspector".into()),
                appears_transparent: true,
                // Centred in the 38px titlebar the view draws.
                traffic_light_position: Some(point(px(14.0), px(12.0))),
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| FloatingInspectorView::new(weak_app_clone, tx_clone, window, cx)),
    ) {
        Ok(handle) => {
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| {
                    this.floating_inspector_window = Some(handle.into());
                    cx.notify();
                });
            }
        }
        Err(e) => {
            log::error!("failed to open floating inspector window: {e:?}");
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| {
                    this.inspector_open = true;
                    cx.notify();
                });
            }
        }
    }
}

pub fn open_floating_pip_window(
    weak_app: WeakEntity<LuminaApp>,
    ui_tx: async_channel::Sender<UiMessage>,
    cx: &mut App,
) {
    // Two quick requests can queue before the first window registers, and a
    // PiP closed in the meantime needs no window.
    let Some(app) = weak_app.upgrade() else {
        return;
    };
    let state = app.read(cx);
    if !state.video.in_pip || state.floating_pip_window.is_some() {
        return;
    }
    let pip_size = state.video.item.as_ref().map_or_else(
        || pip_window_size(0, 0),
        |item| pip_window_size(item.width, item.height),
    );
    // The main window is the one whose root view is this app.
    let app_id = app.entity_id();
    let main = cx.windows().into_iter().find_map(|handle| {
        let handle = handle.downcast::<LuminaApp>()?;
        handle
            .update(cx, |_, window, cx| {
                (cx.entity_id() == app_id).then(|| (window.bounds(), window.display(cx)))
            })
            .ok()
            .flatten()
    });
    let (main_bounds, display) = match main {
        Some((bounds, display)) => (Some(bounds), display),
        None => (None, None),
    };
    // GPUI window bounds are relative to the window's display, so the PiP
    // opens on the main window's display; without a fit it is centred on
    // the primary display as before.
    let (bounds, display_id) = match pip_window_origin(
        pip_size,
        main_bounds,
        display.as_ref().map(|d| d.visible_bounds()),
    ) {
        Some(origin) => (
            Bounds::new(origin, pip_size),
            display.as_ref().map(|d| d.id()),
        ),
        None => (Bounds::centered(None, pip_size, cx), None),
    };
    let weak_app_clone = weak_app.clone();
    let tx_clone = ui_tx.clone();

    match cx.open_window(
        WindowOptions {
            kind: WindowKind::Floating,
            focus: false,
            is_movable: true,
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            display_id,
            titlebar: Some(TitlebarOptions {
                title: Some(crate::i18n::t("pip.window_title").into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            // macOS hides floating panels while the app is inactive; PiP must stay up.
            #[cfg(target_os = "macos")]
            crate::player::macos::keep_window_visible_when_inactive(window);
            cx.new(|cx| FloatingPipPlayerView::new(weak_app_clone, tx_clone, window, cx))
        },
    ) {
        Ok(handle) => {
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| {
                    this.floating_pip_window = Some(handle.into());
                    cx.notify();
                });
            }
        }
        Err(e) => {
            log::error!("failed to open floating PiP window: {e:?}");
            // No window means no PiP: do not leave the video playing unseen.
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| {
                    this.close_pip(cx);
                    this.set_error(format!("Could not open Picture-in-Picture: {e}"), cx);
                });
            }
        }
    }
}

/// Gap between the PiP window and the main window's bottom-right corner.
const PIP_INSET: Pixels = px(56.0);

/// PiP window size for a `width`×`height` video: 420×236 for 16:9 (the
/// design card), portrait clips stand up to 400 tall, unknown sizes get 16:9.
fn pip_window_size(width: u32, height: u32) -> Size<Pixels> {
    if width == 0 || height == 0 {
        return size(px(420.0), px(236.0));
    }
    let aspect = width as f32 / height as f32;
    let (w, h) = if aspect >= 1.0 {
        let w = 420.0f32.min(236.0 * aspect).max(280.0);
        (w, (w / aspect).max(160.0))
    } else {
        let h = 400.0f32;
        ((h * aspect).clamp(220.0, 360.0), h)
    };
    size(px(w.round()), px(h.round()))
}

/// Where the PiP window opens: `PIP_INSET` in from the main window's
/// bottom-right corner, kept inside `display` (the main window's display, so
/// a second monitor works). The clamp keeps the inset from the display's
/// right and bottom edges only; on the left and top it stops at the edge.
/// `None` (the caller centres it) when there is no main window or it is too
/// small to hold the PiP plus the inset.
fn pip_window_origin(
    pip: Size<Pixels>,
    main: Option<Bounds<Pixels>>,
    display: Option<Bounds<Pixels>>,
) -> Option<Point<Pixels>> {
    let main = main?;
    if main.size.width < pip.width + PIP_INSET * 2.0
        || main.size.height < pip.height + PIP_INSET * 2.0
    {
        return None;
    }
    let mut origin = point(
        main.right() - pip.width - PIP_INSET,
        main.bottom() - pip.height - PIP_INSET,
    );
    if let Some(display) = display {
        origin.x = origin
            .x
            .min(display.right() - pip.width - PIP_INSET)
            .max(display.left());
        origin.y = origin
            .y
            .min(display.bottom() - pip.height - PIP_INSET)
            .max(display.top());
    }
    Some(origin)
}

#[cfg(test)]
mod tests {
    use super::{pip_window_origin, pip_window_size, PIP_INSET};
    use gpui::{point, px, size, Bounds, Pixels};

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    #[core::prelude::v1::test]
    fn sixteen_by_nine_opens_at_the_design_size() {
        assert_eq!(pip_window_size(1920, 1080), size(px(420.0), px(236.0)));
        assert_eq!(pip_window_size(0, 0), size(px(420.0), px(236.0)));
        let portrait = pip_window_size(1080, 1920);
        assert!(
            portrait.height > portrait.width,
            "portrait clips open portrait"
        );
    }

    #[core::prelude::v1::test]
    fn pip_sits_bottom_right_of_the_main_window() {
        let pip = size(px(420.0), px(236.0));
        let display = rect(0.0, 0.0, 1920.0, 1080.0);
        let main = rect(100.0, 50.0, 1400.0, 900.0);
        assert_eq!(
            pip_window_origin(pip, Some(main), Some(display)),
            Some(point(
                px(100.0 + 1400.0 - 420.0) - PIP_INSET,
                px(50.0 + 900.0 - 236.0) - PIP_INSET
            )),
            "PiP opens inset from the main window's bottom-right corner"
        );
    }

    #[core::prelude::v1::test]
    fn pip_stays_on_the_main_windows_display() {
        let pip = size(px(420.0), px(236.0));
        // A second display to the right; the main window hangs off its bottom.
        let display = rect(1920.0, 0.0, 1440.0, 900.0);
        let main = rect(2000.0, 300.0, 1200.0, 900.0);
        let origin = pip_window_origin(pip, Some(main), Some(display)).unwrap();
        assert!(
            origin.y + pip.height <= px(900.0) && origin.x >= px(1920.0),
            "PiP must stay on the main window's display, got {origin:?}"
        );
    }

    #[core::prelude::v1::test]
    fn no_or_tiny_main_window_falls_back_to_centred() {
        let pip = size(px(420.0), px(236.0));
        let display = rect(0.0, 0.0, 1920.0, 1080.0);
        assert_eq!(pip_window_origin(pip, None, Some(display)), None);
        assert_eq!(
            pip_window_origin(pip, Some(rect(0.0, 0.0, 400.0, 300.0)), Some(display)),
            None,
            "a main window smaller than the PiP gets the centred fallback"
        );
    }
}
