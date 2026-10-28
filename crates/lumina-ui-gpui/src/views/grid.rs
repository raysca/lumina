use gpui::RenderImage;
use lru::LruCache;
use lumina_core::{LuminaEngine, MediaId};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

#[derive(Clone)]
pub struct GridState {
    pub engine: Arc<LuminaEngine>,
    pub total_count: usize,
    pub is_scanning: bool,
    pub scan_progress: usize,
    pub texture_cache: Arc<Mutex<LruCache<MediaId, Arc<RenderImage>>>>,
    pub in_flight: Arc<Mutex<HashSet<MediaId>>>,
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
        }
    }
}
