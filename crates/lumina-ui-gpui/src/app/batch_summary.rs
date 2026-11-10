//! The batch inspector's summary of a large selection, read off the UI
//! thread.
//!
//! ⌘A or a long Shift range on a large view would read every selected row
//! on the UI thread (twice, with the tag counts). Above
//! [`BATCH_SYNC_LIMIT`] the count shows at once, the fields read "Mixed",
//! and the rest lands from a background read. It lands only while the
//! selection still has the rows it read and no newer request was made (a
//! request counter, bumped on every summary, catches A→B→A).

use std::collections::BTreeSet;
use std::ops::Range;

use gpui::{BackgroundExecutor, Task};
use lumina_core::{MediaId, MediaItem, MediaType};

use crate::app::active_view::ViewKey;
use crate::app::navigation::{
    summarize_batch_locations, summarize_batch_text, summarize_batch_timestamps,
};
use crate::app::state::{BatchFieldSummary, BatchMetadataSummary};
use crate::app::types::{BatchSelectionSummary, UiMessage};
use crate::app::LuminaApp;

/// Selections up to this many rows are summarised on the UI thread (one
/// bounded read); larger ones in the background.
pub(crate) const BATCH_SYNC_LIMIT: usize = 500;

/// The background summary's executor, the current request and its read.
pub(crate) struct BatchLoad {
    executor: BackgroundExecutor,
    request: u64,
    /// Dropped by the next request, which cancels a read not yet started.
    task: Option<Task<()>>,
}

impl BatchLoad {
    pub(crate) fn new(executor: BackgroundExecutor) -> Self {
        Self {
            executor,
            request: 0,
            task: None,
        }
    }

    /// The selection changed: a summary still loading is stale.
    pub(crate) fn invalidate(&mut self) {
        self.request = self.request.wrapping_add(1);
        self.task = None;
    }
}

/// A summary read in the background, for `request` in view `key`.
pub(crate) struct LoadedSummary {
    request: u64,
    key: ViewKey,
    /// The selected rows it read.
    ranges: Vec<Range<usize>>,
    summary: BatchSelectionSummary,
    /// Everything but the caption, which lives on the UI side.
    metadata: BatchMetadataSummary,
}

/// The selection's rows as contiguous ranges, in order.
pub(crate) fn index_ranges(indices: &BTreeSet<usize>) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for &i in indices {
        match ranges.last_mut() {
            Some(range) if range.end == i => range.end = i + 1,
            _ => ranges.push(i..i + 1),
        }
    }
    ranges
}

/// Counts, bytes, ids and capture dates of `items`.
pub(crate) fn summarize(items: &[MediaItem]) -> BatchSelectionSummary {
    let of = |t: MediaType| items.iter().filter(|i| i.media_type == t).count();
    let times = || items.iter().map(|i| i.timestamp_epoch_ms);
    BatchSelectionSummary {
        count: items.len(),
        total_bytes: items.iter().map(|i| i.file_size).sum(),
        photos_count: of(MediaType::Image),
        videos_count: of(MediaType::Video),
        raw_count: of(MediaType::Raw),
        ids: items.iter().map(|i| i.id).collect(),
        oldest_ms: times().min().unwrap_or(0),
        newest_ms: times().max().unwrap_or(0),
        pending: false,
    }
}

impl LuminaApp {
    /// Shows a large selection's count at once and "Mixed" in its fields
    /// (never the last selection's values), and reads the rest in the
    /// background. The caller has invalidated the last request.
    /// ponytail: each marquee step or Shift+arrow over a large range asks
    /// for a new read; the next request cancels one still queued, but a
    /// read already running finishes and only its result is dropped. A
    /// short read (rows gone meanwhile) stays pending until the selection
    /// changes. The batch writers still read the selected rows when an
    /// edit is committed: an explicit action that must write the right
    /// items.
    pub(crate) fn load_batch_summary(&mut self) {
        self.batch_summary = BatchSelectionSummary {
            count: self.selected_indices.len(),
            pending: true,
            ..Default::default()
        };
        let pending = BatchFieldSummary {
            mixed: true,
            common: None,
        };
        self.batch_metadata = BatchMetadataSummary {
            title: pending.clone(),
            caption: pending.clone(),
            date: pending.clone(),
            location: pending,
        };
        self.sync_batch_inputs();
        // The last selection's tags go now; the result brings these.
        self.refresh_tags_for(Vec::new());
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        let request = self.batch_load.request;
        let key = self.view_key();
        let query = self.view_query();
        let ranges = index_ranges(&self.selected_indices);
        let read = ranges.clone();
        let expected = self.selected_indices.len();
        let tx = self.ui_tx.clone();
        let task = self.batch_load.executor.spawn(async move {
            let items: Vec<MediaItem> = read
                .into_iter()
                .flat_map(|range| query.slice(&engine, range))
                .collect();
            // The engine logs a failed read and returns fewer rows (or
            // none); a partial summary would show wrong totals.
            if items.len() != expected {
                log::warn!(
                    "batch summary read {} of {expected} selected rows; left pending",
                    items.len()
                );
                return;
            }
            let metadata = BatchMetadataSummary {
                title: summarize_batch_text(items.iter().map(|i| i.title.clone())),
                caption: BatchFieldSummary::default(),
                date: summarize_batch_timestamps(items.iter().map(|i| i.timestamp_epoch_ms)),
                location: summarize_batch_locations(
                    items.iter().map(|i| (i.latitude, i.longitude)),
                ),
            };
            let loaded = LoadedSummary {
                request,
                key,
                ranges,
                summary: summarize(&items),
                metadata,
            };
            // A closed channel means the app is gone: nothing to show.
            let _ = tx
                .send(UiMessage::BatchSummaryLoaded(Box::new(loaded)))
                .await;
        });
        self.batch_load.task = Some(task);
    }

    /// Lands a background summary unless the selection or the view changed
    /// since it was asked for (also by a path that does not re-summarise,
    /// such as the viewer's right-click); then loads the tag counts for its
    /// ids.
    pub(crate) fn apply_batch_summary(&mut self, loaded: LoadedSummary) -> bool {
        if loaded.request != self.batch_load.request
            || loaded.key != self.view_key()
            || index_ranges(&self.selected_indices) != loaded.ranges
        {
            return false;
        }
        let caption = self.caption_summary(&loaded.summary.ids);
        self.batch_summary = loaded.summary;
        self.batch_metadata = BatchMetadataSummary {
            caption,
            ..loaded.metadata
        };
        self.sync_batch_inputs();
        self.refresh_tags_for(self.batch_summary.ids.clone());
        true
    }

    /// The shared caption of `ids` from the UI-side caption map.
    fn caption_summary(&self, ids: &[MediaId]) -> BatchFieldSummary {
        let mut captions = ids
            .iter()
            .map(|id| self.item_captions.get(id).filter(|c| !c.is_empty()));
        let Some(first) = captions.next() else {
            return BatchFieldSummary::default();
        };
        let mixed = captions.any(|caption| caption != first);
        BatchFieldSummary {
            mixed,
            common: if mixed { None } else { first.cloned() },
        }
    }
}
