//! Scrub previews: a small strip of decoded frames for the open video, shown
//! above the scrubber while hovering or dragging.
//!
//! The strip is built once per open video, when the player loads it, on
//! the GPUI background executor (never the UI thread and never the
//! engine's decode workers, so grid thumbnails are not starved). Closing the
//! video or changing media cancels an unfinished request and retires the
//! frames; a result that arrives for a video that is no longer open is
//! dropped before any of its images is painted.

use gpui::*;
use lumina_core::MediaId;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use crate::app::LuminaApp;

/// Decoded preview frames for one open video, in ascending time.
pub(crate) struct ScrubStrip {
    pub item: MediaId,
    pub frames: Vec<(f64, Arc<RenderImage>)>,
}

/// The single strip request for the open video. Kept until the video closes,
/// so an item gets at most one request (and no retry after a failure) per
/// open. `cancel` is non-zero once the request is stale.
pub(crate) struct StripRequest {
    pub item: MediaId,
    pub cancel: Arc<AtomicI32>,
}

/// Outcome of a strip request, as delivered to the UI thread.
pub(crate) type StripResult = anyhow::Result<Vec<(f64, Arc<RenderImage>)>>;

impl LuminaApp {
    /// Starts the preview strip for the open video unless it already has one
    /// (built, in flight or failed). Does nothing until the player has loaded
    /// the item and knows its duration.
    pub(crate) fn request_scrub_strip(&mut self, cx: &mut Context<Self>) {
        if self.video.player.is_none()
            || !self.video.duration.is_finite()
            || self.video.duration <= 0.0
        {
            return;
        }
        let Some(id) = self.video.item.as_ref().map(|item| item.id) else {
            return;
        };
        if self
            .video
            .strip_request
            .as_ref()
            .is_some_and(|r| r.item == id)
        {
            return;
        }
        // A request for another item cannot outlive its player session, but
        // never leave one decoding unowned.
        self.clear_scrub_strip();
        let cancel = Arc::new(AtomicI32::new(0));
        self.video.strip_request = Some(StripRequest {
            item: id,
            cancel: cancel.clone(),
        });

        #[cfg(test)]
        {
            if let Some(result) = test_seam::record_request() {
                self.finish_scrub_strip(id, &cancel, result, cx);
            }
        }
        #[cfg(not(test))]
        {
            /// Long edge, in pixels, of each strip frame.
            const STRIP_FRAME_SIZE: u32 = 256;
            let Some(path) = self.video.item.as_ref().map(|item| item.path.clone()) else {
                return;
            };
            let duration = self.video.duration;
            let decode_cancel = cancel.clone();
            // Blocks one background thread for at most the native timeout;
            // the cancel flag ends it early when the video closes.
            let decode = cx.background_executor().spawn(async move {
                lumina_core::video_preview_strip(&path, duration, STRIP_FRAME_SIZE, &decode_cancel)?
                    .into_iter()
                    .map(|(time, frame)| Ok((time, crate::bridge::frame_to_gpui_image(frame)?)))
                    .collect::<StripResult>()
            });
            cx.spawn(async move |this, cx| {
                let result = decode.await;
                // The app may be gone; the frames are dropped unpainted then.
                let _ = this.update(cx, |this, cx| {
                    this.finish_scrub_strip(id, &cancel, result, cx)
                });
            })
            .detach();
        }
    }

    /// Stores a finished strip if it still belongs to the open video's
    /// current request; otherwise drops it unpainted. A failure is logged and
    /// leaves the scrubber time-only for this open.
    pub(crate) fn finish_scrub_strip(
        &mut self,
        item: MediaId,
        cancel: &Arc<AtomicI32>,
        result: StripResult,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .video
            .strip_request
            .as_ref()
            .is_some_and(|r| r.item == item && Arc::ptr_eq(&r.cancel, cancel));
        if !current || cancel.load(Ordering::Acquire) != 0 {
            // Stale: never painted, so dropping it frees no atlas texture.
            return;
        }
        match result {
            Ok(frames) => {
                self.video.preview_strip = Some(ScrubStrip { item, frames });
                cx.notify();
            }
            // ponytail: no banner; the preview is an enhancement and the
            // request stays in place, so this open never retries.
            Err(err) => log::warn!("scrub preview for {item:?} unavailable: {err:#}"),
        }
    }

    /// The strip frame nearest `time` and its frame time, if the open video
    /// has a strip. Binary search: the transport asks on every render.
    pub(crate) fn scrub_preview_at(&self, time: f64) -> Option<(f64, Arc<RenderImage>)> {
        let strip = self.video.preview_strip.as_ref()?;
        if self.video.item.as_ref().map(|item| item.id) != Some(strip.item) {
            return None;
        }
        let frames = &strip.frames;
        let after = frames.partition_point(|(t, _)| *t < time);
        let nearest = match (after.checked_sub(1), frames.get(after)) {
            (Some(before), Some((t_after, _))) if (t_after - time) < (time - frames[before].0) => {
                after
            }
            (Some(before), _) => before,
            (None, _) => after,
        };
        frames.get(nearest).cloned()
    }

    /// Cancels an unfinished request and retires the strip's images. Runs
    /// whenever the player session stops (close, media change, PiP close).
    pub(crate) fn clear_scrub_strip(&mut self) {
        if let Some(request) = self.video.strip_request.take() {
            request.cancel.store(1, Ordering::Release);
        }
        if let Some(strip) = self.video.preview_strip.take() {
            for (_, image) in strip.frames {
                self.retire_image(image);
            }
        }
    }
}

/// Test seam: tests never decode from the UI crate. A request is counted and
/// either completes at once with an injected result or stays in flight.
#[cfg(test)]
pub(crate) mod test_seam {
    use super::StripResult;
    use std::cell::{Cell, RefCell};

    thread_local! {
        static REQUESTS: Cell<usize> = const { Cell::new(0) };
        static NEXT_RESULT: RefCell<Option<StripResult>> = const { RefCell::new(None) };
    }

    /// Strip requests started on this thread.
    pub(crate) fn requests() -> usize {
        REQUESTS.with(Cell::get)
    }

    /// The next request completes with `result` instead of staying in flight.
    pub(crate) fn inject(result: StripResult) {
        NEXT_RESULT.with(|next| *next.borrow_mut() = Some(result));
    }

    pub(super) fn record_request() -> Option<StripResult> {
        REQUESTS.with(|count| count.set(count.get() + 1));
        NEXT_RESULT.with(|next| next.borrow_mut().take())
    }
}
