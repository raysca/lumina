//! Crop mode inside edit mode (C): the rectangle, its aspect presets and the
//! keyboard and handle moves. While cropping the rectangle lives in
//! normalized coordinates of the frame as shown (rotated); it is converted
//! to the catalog's unrotated coordinates once, when crop mode is left or
//! on Done (plan 4 Decision 11). Keys are in `keymap/modals.rs`, the
//! overlay in `views/viewer/crop_overlay.rs`.

use std::sync::Arc;

use gpui::*;
use lumina_core::{DecodedFrame, Edits, MediaId};

use crate::app::crop_geometry::{
    drag_corner, is_full_frame, nudge, preset_rect, resize, rotate_rect, FULL_FRAME,
};
pub use crate::app::crop_geometry::{Corner, CropAspect, CropDrag, CropSession};
use crate::app::LuminaApp;

impl LuminaApp {
    /// The frame as shown, in pixels: the unedited source turned by the
    /// draft rotation. `None` until the source has decoded.
    fn crop_frame_size(&self) -> Option<(f32, f32)> {
        let session = self.edits.session.as_ref()?;
        let source = session.source.as_ref()?;
        let (w, h) = (source.width as f32, source.height as f32);
        Some(if session.draft.rotation % 2 == 1 {
            (h, w)
        } else {
            (w, h)
        })
    }

    /// The overlay's view of crop mode: the rect and the frame size.
    pub(crate) fn crop_view(&self) -> Option<(CropSession, (f32, f32))> {
        let crop = self.edits.session.as_ref()?.crop?;
        Some((crop, self.crop_frame_size()?))
    }

    /// `C` in edit mode: enters crop mode, or cycles the aspect inside it.
    pub fn crop_key(&mut self, cx: &mut Context<Self>) {
        match self.edits.session.as_ref().and_then(|s| s.crop) {
            Some(crop) => self.set_crop_aspect(crop.aspect.next(), cx),
            None => self.enter_crop(cx),
        }
    }

    /// Shows the whole frame with the current crop (or the whole frame, at
    /// Original) drawn over it. Needs the preview source for the frame size.
    pub fn enter_crop(&mut self, cx: &mut Context<Self>) {
        if self.crop_frame_size().is_none() {
            if self.edits.session.is_some() {
                self.set_last_action("Crop is available once the preview has loaded");
                cx.notify();
            }
            return;
        }
        let Some(session) = self.edits.session.as_mut() else {
            return;
        };
        if session.crop.is_some() {
            return;
        }
        session.crop = Some(match session.draft.crop {
            Some(crop) => CropSession {
                aspect: CropAspect::Free,
                rect: rotate_rect(crop, session.draft.rotation),
                dragging: None,
            },
            None => CropSession {
                aspect: CropAspect::Original,
                rect: FULL_FRAME,
                dragging: None,
            },
        });
        // The overlay is laid out on the frame at Fit.
        self.viewer_state.reset();
        self.render_edit_preview(cx);
        self.set_last_action("Cropping: C changes the aspect, arrows move, Esc leaves crop");
        cx.notify();
    }

    /// The edit bar's Crop button: enters, or leaves keeping the crop.
    pub fn toggle_crop(&mut self, cx: &mut Context<Self>) {
        if self
            .edits
            .session
            .as_ref()
            .is_some_and(|s| s.crop.is_some())
        {
            self.leave_crop(true, cx);
        } else {
            self.enter_crop(cx);
        }
    }

    /// Leaves crop mode. `keep` converts the rect to unrotated coordinates
    /// into the draft (the whole frame is "no crop"); otherwise (Esc) the
    /// draft's crop stays as it was when crop mode opened.
    pub fn leave_crop(&mut self, keep: bool, cx: &mut Context<Self>) {
        let left = if keep {
            self.commit_crop()
        } else {
            self.edits
                .session
                .as_mut()
                .and_then(|s| s.crop.take())
                .is_some()
        };
        if left {
            self.render_edit_preview(cx);
            cx.notify();
        }
    }

    /// Leaves crop mode keeping the crop in the draft, without rendering;
    /// true if crop mode was open.
    pub(crate) fn commit_crop(&mut self) -> bool {
        let Some(session) = self.edits.session.as_mut() else {
            return false;
        };
        let Some(crop) = session.crop.take() else {
            return false;
        };
        let rotation = session.draft.rotation;
        session.draft.crop =
            (!is_full_frame(crop.rect)).then(|| rotate_rect(crop.rect, (4 - rotation % 4) % 4));
        true
    }

    pub fn set_crop_aspect(&mut self, aspect: CropAspect, cx: &mut Context<Self>) {
        if self
            .edits
            .session
            .as_ref()
            .is_some_and(|s| s.crop.is_none())
        {
            self.enter_crop(cx);
        }
        let Some(frame) = self.crop_frame_size() else {
            return;
        };
        self.change_crop(|crop| {
            crop.aspect = aspect;
            crop.rect = preset_rect(aspect, frame, crop.rect);
        });
        self.set_last_action(format!("Crop aspect: {}", aspect.label()));
        cx.notify();
    }

    /// Arrows (1 %, ⇧ 10 %) move the rect; with ⌥ they resize it.
    pub fn crop_arrow(&mut self, step: (f32, f32), resize_rect: bool, cx: &mut Context<Self>) {
        let Some(frame) = self.crop_frame_size() else {
            return;
        };
        self.change_crop(|crop| {
            crop.rect = if resize_rect {
                resize(crop.rect, step, crop.aspect, frame)
            } else {
                nudge(crop.rect, step)
            };
        });
        cx.notify();
    }

    pub(crate) fn start_crop_drag(
        &mut self,
        corner: Corner,
        pointer: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        self.change_crop(|crop| {
            crop.dragging = Some(CropDrag {
                corner,
                start: pointer,
                rect: crop.rect,
            })
        });
        cx.notify();
    }

    /// The held handle follows the pointer (window points); `fitted` is the
    /// frame's size on screen.
    pub(crate) fn drag_crop_to(
        &mut self,
        pointer: (f32, f32),
        fitted: (f32, f32),
        cx: &mut Context<Self>,
    ) {
        let Some(frame) = self.crop_frame_size() else {
            return;
        };
        let mut moved = false;
        self.change_crop(|crop| {
            if let Some(drag) = crop.dragging {
                moved = true;
                let (x, y) = drag.corner.point(drag.rect);
                let point = (
                    x + (pointer.0 - drag.start.0) / fitted.0,
                    y + (pointer.1 - drag.start.1) / fitted.1,
                );
                crop.rect = drag_corner(drag.rect, drag.corner, point, crop.aspect, frame);
            }
        });
        if moved {
            cx.notify();
        }
    }

    pub(crate) fn end_crop_drag(&mut self, cx: &mut Context<Self>) {
        let mut ended = false;
        self.change_crop(|crop| ended = crop.dragging.take().is_some());
        if ended {
            cx.notify();
        }
    }

    /// After a draft rotation: the rect follows the picture (it covers the
    /// same region), so a fixed aspect turns with it.
    pub(crate) fn rotate_crop_rect(&mut self, from: u8, to: u8) {
        self.change_crop(|crop| {
            let unrotated = rotate_rect(crop.rect, (4 - from % 4) % 4);
            crop.rect = rotate_rect(unrotated, to);
        });
    }

    /// Revert to Original while cropping: the whole frame at Original.
    pub(crate) fn reset_crop_rect(&mut self) {
        self.change_crop(|crop| {
            crop.aspect = CropAspect::Original;
            crop.rect = FULL_FRAME;
        });
    }

    /// After Done from crop mode the uncropped crop-mode frame is still on
    /// screen: renders the saved edits from `source` off the UI thread and
    /// shows them, unless the saved frame (or another photo) came first.
    pub(crate) fn show_cropped_after_done(
        &mut self,
        id: MediaId,
        source: Option<Arc<DecodedFrame>>,
        edits: Edits,
        cx: &mut Context<Self>,
    ) {
        let (Some(source), Some(shown)) = (source, self.viewer_image.as_ref().map(|i| i.id)) else {
            return;
        };
        let task = crate::app::quick_edit::render_task(source, edits, cx);
        cx.spawn(async move |this, cx| {
            let image = task.await;
            // The app may be gone; nothing to show then.
            let _ = this.update(cx, |app, cx| {
                let still_shown = app.viewer_image.as_ref().is_some_and(|i| i.id == shown)
                    && app.selected_item.as_ref().is_some_and(|i| i.id == id)
                    && app.edits.session.is_none();
                if let Some(image) = image.filter(|_| still_shown) {
                    app.set_viewer_image(Some(image));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn change_crop(&mut self, change: impl FnOnce(&mut CropSession)) {
        if let Some(crop) = self.edits.session.as_mut().and_then(|s| s.crop.as_mut()) {
            change(crop);
        }
    }
}
