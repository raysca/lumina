//! The crop rectangle over the whole (uncropped, rotated) preview while
//! cropping: shade outside it and four corner handles. It covers the media
//! canvas and occludes it, so a click, drag or scroll there never pans,
//! zooms or opens the context menu. Geometry and actions are in
//! `app/crop.rs`.

use gpui::*;

use crate::app::crop::Corner;
use crate::app::LuminaApp;
use crate::theme;

const HANDLE: f32 = 14.0;

/// `size` contain-fitted to a frame of `aspect` (w / h).
fn contain((w, h): (f32, f32), aspect: f32) -> (f32, f32) {
    if w / h > aspect {
        (h * aspect, h)
    } else {
        (w, w / aspect)
    }
}

/// `None` outside crop mode. Laid out by this frame's layout (an aspect
/// box inside the canvas, fractions inside it), never by geometry captured
/// in an earlier frame, so a window resize cannot leave it behind.
pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<AnyElement> {
    let (crop, frame) = this.crop_view()?;
    let aspect = frame.0 / frame.1;
    let r = crop.rect;
    let (x1, y1) = (r.x + r.w, r.y + r.h);
    let shade = |left: f32, top: f32, w: f32, h: f32| {
        div()
            .absolute()
            .left(relative(left))
            .top(relative(top))
            .w(relative(w.max(0.0)))
            .h(relative(h.max(0.0)))
            .bg(theme::scrim())
    };

    let handles = Corner::ALL.into_iter().map(|corner| {
        let name = SharedString::from(format!("crop-handle-{}", corner.name()));
        // Inside the rect's corner, so a handle at the frame edge stays
        // clickable.
        let handle = div()
            .id(name.clone())
            .debug_selector(move || name.to_string())
            .role(Role::Button)
            .aria_label(corner.label())
            .tooltip(theme::tooltip_label(
                "Drag to resize the crop (⌥ arrows resize)",
            ))
            .absolute()
            .size(px(HANDLE))
            .bg(theme::t1())
            .rounded(theme::RADIUS_XS)
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                    let pointer = (f32::from(e.position.x), f32::from(e.position.y));
                    this.start_crop_drag(corner, pointer, cx);
                    cx.stop_propagation();
                }),
            );
        match corner {
            Corner::TopLeft => handle.left(relative(r.x)).top(relative(r.y)),
            Corner::TopRight => handle.right(relative(1.0 - x1)).top(relative(r.y)),
            Corner::BottomLeft => handle.left(relative(r.x)).bottom(relative(1.0 - y1)),
            Corner::BottomRight => handle.right(relative(1.0 - x1)).bottom(relative(1.0 - y1)),
        }
    });

    let frame_box = div()
        .id("crop-overlay")
        .debug_selector(|| "crop-overlay".to_string())
        .role(Role::Group)
        .aria_label("Crop area: arrows move it, ⌥ arrows resize it")
        .relative()
        .flex_none()
        // Contain: as wide as the canvas, capped at its height, at the
        // frame's aspect (the max height also caps the width through it).
        .w_full()
        .max_h_full()
        .aspect_ratio(aspect)
        .child(shade(0.0, 0.0, 1.0, r.y))
        .child(shade(0.0, y1, 1.0, 1.0 - y1))
        .child(shade(0.0, r.y, r.x, r.h))
        .child(shade(x1, r.y, 1.0 - x1, r.h))
        .child(
            div()
                .absolute()
                .left(relative(r.x))
                .top(relative(r.y))
                .w(relative(r.w))
                .h(relative(r.h))
                .border_1()
                .border_color(theme::t1()),
        )
        .children(handles);

    // A held handle follows the pointer anywhere in the window, and any
    // mouse up ends the drag: window-level listeners, registered each frame
    // while cropping, need no pointer capture.
    let app = cx.entity();
    let tracker = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let fitted = contain(size, aspect);
            let mover = app.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _w, cx| {
                if phase == DispatchPhase::Bubble {
                    let pointer = (f32::from(e.position.x), f32::from(e.position.y));
                    let held = e.pressed_button == Some(MouseButton::Left);
                    mover.update(cx, |this, cx| {
                        if held {
                            this.drag_crop_to(pointer, fitted, cx);
                        } else {
                            this.end_crop_drag(cx);
                        }
                    });
                }
            });
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _w, cx| {
                if phase == DispatchPhase::Bubble {
                    app.update(cx, |this, cx| this.end_crop_drag(cx));
                }
            });
        },
    )
    .absolute()
    .size_full();

    Some(
        div()
            .id("crop-layer")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(tracker)
            .child(frame_box)
            .into_any_element(),
    )
}
