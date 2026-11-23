//! The viewer's full-size frame: decoded only while the viewer is open,
//! once the user stops stepping, and shown only if it is still the latest
//! request.

use gpui::*;
use lumina_core::MediaId;
use std::time::{Duration, Instant};

use crate::app::types::UiMessage;
use crate::app::LuminaApp;

/// Steps closer together than this (a held arrow key) wait until stepping
/// pauses, so only the item the user lands on is decoded.
const VIEWER_STEP_DEBOUNCE: Duration = Duration::from_millis(100);

#[cfg(test)]
thread_local! {
    /// Full-size viewer decodes queued on this test thread.
    pub(crate) static VIEWER_FRAME_REQUESTS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

/// Viewer frame requests. Each answer carries the generation it was asked
/// with; only the current one is shown, so a slow decode for an item the
/// user stepped past cannot replace the frame of the item shown now.
#[derive(Default)]
pub(crate) struct ViewerFrameRequests {
    pub(crate) generation: u64,
    last_asked: Option<Instant>,
}

impl LuminaApp {
    /// Asks for the selected item's full-size frame for the open viewer.
    /// While the user steps quickly the decode waits for a pause.
    pub(crate) fn request_shown_viewer_frame(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_item.as_ref().map(|item| item.id) else {
            return;
        };
        let now = Instant::now();
        let stepping = self
            .viewer_frame
            .last_asked
            .is_some_and(|last| now.saturating_duration_since(last) < VIEWER_STEP_DEBOUNCE);
        self.viewer_frame.last_asked = Some(now);
        if !stepping {
            self.request_viewer_frame(id);
            return;
        }
        // Invalidate any earlier answer now: the item it is for is gone.
        self.viewer_frame.generation = self.viewer_frame.generation.wrapping_add(1);
        let generation = self.viewer_frame.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(VIEWER_STEP_DEBOUNCE).await;
            // The view is gone when the window closed; nothing to decode.
            let _ = this.update(cx, |app, _| {
                let still_shown = app.viewer_frame.generation == generation
                    && app.is_fullscreen_viewer
                    && app.selected_item.as_ref().is_some_and(|item| item.id == id);
                if still_shown {
                    app.request_viewer_frame(id);
                }
            });
        })
        .detach();
    }

    /// Queues the full-size decode of `id` now, superseding earlier ones.
    pub(crate) fn request_viewer_frame(&mut self, id: MediaId) {
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            return;
        };
        self.viewer_frame.generation = self.viewer_frame.generation.wrapping_add(1);
        let generation = self.viewer_frame.generation;
        #[cfg(test)]
        VIEWER_FRAME_REQUESTS.with(|n| n.set(n.get() + 1));
        let tx = self.ui_tx.clone();
        engine.request_viewer_frame(
            id,
            crate::app::viewer_actions::VIEWER_IMAGE_SIZE,
            move |frame| on_viewer_frame(&tx, id, generation, frame),
        );
    }

    /// Whether a viewer decode answer is for the latest request.
    pub(crate) fn viewer_frame_is_current(&self, generation: u64) -> bool {
        self.viewer_frame.generation == generation
    }
}

/// The viewer decode's answer, on the decoder pool.
pub(crate) fn on_viewer_frame(
    tx: &async_channel::Sender<UiMessage>,
    id: MediaId,
    generation: u64,
    frame: Option<lumina_core::DecodedFrame>,
) {
    // A closed receiver means the app is gone; nothing is left to repaint.
    let _ = match frame {
        Some(frame) => tx.send_blocking(UiMessage::ViewerLoaded {
            id,
            generation,
            frame,
        }),
        None => tx.send_blocking(UiMessage::ViewerFailed { id, generation }),
    };
}
