use crate::config::ResumePositions;
use gpui::{RenderImage, UniformListScrollHandle};
use lru::LruCache;
use lumina_core::{LuminaEngine, MediaId};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

/// Grid-owned playback resume state.
///
/// The position map is shared with AppConfig so the existing config save path
/// persists it. The current media id stays with the grid instead of becoming a
/// flat app field, and lets a stop record the video that actually played even
/// when the selection has already moved to a different item.
#[derive(Clone, Default)]
pub struct VideoResumeState {
    pub positions: ResumePositions,
    current: Arc<Mutex<Option<MediaId>>>,
}

impl VideoResumeState {
    /// Marks the media whose player is now loading or loaded.
    pub fn begin(&self, id: MediaId) {
        *self.current.lock() = Some(id);
    }

    /// Media id of the active player, if any.
    pub fn current(&self) -> Option<MediaId> {
        *self.current.lock()
    }

    /// Clears the active player id after its final position is recorded.
    pub fn finish(&self) {
        *self.current.lock() = None;
    }

    /// Saved resume position for a media id.
    pub fn saved(&self, id: MediaId) -> Option<f64> {
        self.positions.get(id)
    }

    /// Records a position for the active player. Returns false when no player
    /// media is tracked, so callers can fall back without inventing an id.
    pub fn record_current(&self, seconds: f64, duration_secs: f64) -> bool {
        let Some(id) = self.current() else {
            return false;
        };
        self.positions.record(id, seconds, duration_secs);
        true
    }

    /// Drops positions whose media no longer exists in the library.
    pub fn prune_missing(&self, exists: impl FnMut(MediaId) -> bool) {
        self.positions.prune_missing(exists);
    }
}

#[derive(Clone)]
pub struct GridState {
    pub engine: Arc<LuminaEngine>,
    pub total_count: usize,
    pub is_scanning: bool,
    pub scan_progress: usize,
    pub texture_cache: Arc<Mutex<LruCache<MediaId, Arc<RenderImage>>>>,
    pub in_flight: Arc<Mutex<HashSet<MediaId>>>,
    pub scroll_handle: UniformListScrollHandle,
    /// Per-video resume positions for the open library.
    pub video_resume: VideoResumeState,
}

impl GridState {
    pub fn new(engine: Arc<LuminaEngine>) -> Self {
        let total_count = engine.total_count();
        let cache_capacity = NonZeroUsize::new(800).unwrap();

        Self {
            engine,
            total_count,
            is_scanning: true,
            scan_progress: 0,
            texture_cache: Arc::new(Mutex::new(LruCache::new(cache_capacity))),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
            scroll_handle: UniformListScrollHandle::new(),
            video_resume: VideoResumeState::default(),
        }
    }

    /// Builds grid state that shares the config's resume map, so positions
    /// recorded during playback are part of the next config save.
    pub fn with_resume_positions(
        engine: Arc<LuminaEngine>,
        resume_positions: ResumePositions,
    ) -> Self {
        let mut state = Self::new(engine);
        state.video_resume.positions = resume_positions;
        state
    }
}
