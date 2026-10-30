use crate::app::{LuminaApp, UiMessage};
use crate::views::floating_inspector::FloatingInspectorView;
use crate::views::floating_pip::FloatingPipPlayerView;
use gpui::*;

pub fn open_floating_inspector_window(
    weak_app: WeakEntity<LuminaApp>,
    ui_tx: async_channel::Sender<UiMessage>,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(480.0), px(640.0)), cx);
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
                ..Default::default()
            }),
            ..Default::default()
        },
        |_, cx| cx.new(|cx| FloatingInspectorView::new(weak_app_clone, tx_clone, cx)),
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
    let wanted = weak_app.upgrade().is_some_and(|app| {
        let app = app.read(cx);
        app.video.in_pip && app.floating_pip_window.is_none()
    });
    if !wanted {
        return;
    }

    let bounds = Bounds::centered(None, size(px(480.0), px(270.0)), cx);
    let weak_app_clone = weak_app.clone();
    let tx_clone = ui_tx.clone();

    match cx.open_window(
        WindowOptions {
            kind: WindowKind::Floating,
            focus: false,
            is_movable: true,
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Picture-in-Picture".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| FloatingPipPlayerView::new(weak_app_clone, tx_clone, window, cx)),
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
