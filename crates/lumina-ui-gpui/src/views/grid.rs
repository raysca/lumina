use gpui::RenderImage;
use lumina_core::{LuminaEngine, MediaId};
use parking_lot::Mutex;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use lru::LruCache;

pub const COLS_COUNT: usize = 4;
pub const ROW_HEIGHT_PX: f32 = 180.0;

#[derive(Clone)]
pub struct GridState {
    pub engine: Arc<LuminaEngine>,
    pub library_path: PathBuf,
    pub total_count: usize,
    pub is_scanning: bool,
    pub scan_progress: usize,
    pub texture_cache: Arc<Mutex<LruCache<MediaId, Arc<RenderImage>>>>,
    pub in_flight: Arc<Mutex<HashSet<MediaId>>>,
}

impl GridState {
    pub fn new(engine: Arc<LuminaEngine>, library_path: PathBuf) -> Self {
        let total_count = engine.total_count();
        let cache_capacity = NonZeroUsize::new(800).unwrap();

        Self {
            engine,
            library_path,
            total_count,
            is_scanning: true,
            scan_progress: 0,
            texture_cache: Arc::new(Mutex::new(LruCache::new(cache_capacity))),
            in_flight: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn total_rows(&self) -> usize {
        self.total_count.div_ceil(COLS_COUNT)
    }
}
