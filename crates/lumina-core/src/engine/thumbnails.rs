//! Thumbnail and viewer decode requests through the disk cache.

use crate::*;

/// Largest decode written to the disk cache: the biggest grid thumbnail
/// (`THUMBNAIL_SIZE_MAX` in the UI). Larger decodes (the 2048 px viewer,
/// edit and two-up compare frames) are 12-17 MB raw each and are only ever
/// shown for the item open, so they are decoded per request instead of
/// filling the cache. A cached copy of that size is still read.
const MAX_CACHED_DECODE: u32 = 1280;

impl LuminaEngine {
    pub fn request_thumbnail<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();

        self.decoder.spawn(move || {
            let item = match catalog.get_item_by_id(id) {
                Ok(Some(it)) => it,
                _ => {
                    callback(None);
                    return;
                }
            };

            Self::process_thumbnail_request(catalog, cache, item, target_size, callback);
        });
    }

    /// Requests a thumbnail using an already-fetched `MediaItem`, bypassing the
    /// initial catalog query and eliminating SQLite connection lock contention.
    pub fn request_thumbnail_item<F>(&self, item: MediaItem, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();

        self.decoder.spawn(move || {
            Self::process_thumbnail_request(catalog, cache, item, target_size, callback);
        });
    }

    /// A grid tile's decode: runs before tiles asked for earlier, and only
    /// while `wanted()` still says the tile is on (or near) the screen. A
    /// tile scrolled past calls `skipped` instead and decodes nothing, so
    /// the asker can release its claim and ask again if it comes back.
    pub fn request_grid_thumbnail<W, S, F>(
        &self,
        item: MediaItem,
        target_size: u32,
        wanted: W,
        skipped: S,
        callback: F,
    ) where
        W: FnOnce() -> bool + Send + 'static,
        S: FnOnce() + Send + 'static,
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();
        self.decoder.spawn_newest_first(move || {
            if wanted() {
                Self::process_thumbnail_request(catalog, cache, item, target_size, callback);
            } else {
                skipped();
            }
        });
    }

    /// The open item's frame (viewer, compare), on the viewer's own pool so
    /// it never queues behind grid tiles. Otherwise as [`Self::request_thumbnail`].
    pub fn request_viewer_frame<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();
        self.decoder
            .spawn_viewer(move || match catalog.get_item_by_id(id) {
                Ok(Some(item)) => {
                    Self::process_thumbnail_request(catalog, cache, item, target_size, callback)
                }
                Ok(None) => callback(None),
                Err(err) => {
                    log::warn!("failed to read item {id:?} for the viewer: {err:?}");
                    callback(None);
                }
            });
    }

    /// Decodes `id` at `target_size` with no edits and no rotation: the
    /// source frame for edit mode's live preview.
    pub fn request_unedited_frame<F>(&self, id: MediaId, target_size: u32, callback: F)
    where
        F: FnOnce(Option<DecodedFrame>) + Send + 'static,
    {
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();
        // The edit preview's source: shown now, so the viewer's lane.
        self.decoder
            .spawn_viewer(move || match catalog.get_item_by_id(id) {
                Ok(Some(item)) => {
                    callback(Self::decode_through_cache(&cache, &item, target_size, true))
                }
                Ok(None) => callback(None),
                Err(err) => {
                    log::warn!("failed to read item {id:?} for an unedited frame: {err:?}");
                    callback(None);
                }
            });
    }

    fn process_thumbnail_request<F>(
        catalog: Catalog,
        cache: DiskCache,
        item: MediaItem,
        target_size: u32,
        callback: F,
    ) where
        F: FnOnce(Option<DecodedFrame>),
    {
        let id = item.id;
        // One read: get_edits joins rotation and the side row.
        let edits = match catalog.get_edits(&[id]) {
            Ok(mut edits) => edits.remove(&id).unwrap_or_default(),
            Err(err) => {
                log::warn!("failed to read edits for {id:?}: {err:?}");
                Edits::default()
            }
        };
        let edits = edits::effective_edits(item.media_type, edits);
        let decode_size =
            edits::edit_decode_size(target_size, edits.crop, (item.width, item.height));
        let cacheable = edits::edit_decode_is_cacheable(target_size, decode_size);
        let frame = Self::decode_through_cache(&cache, &item, decode_size, cacheable);
        callback(frame.map(|frame| edits::apply_edits(frame, &edits)));
    }

    /// The unedited decode of `item` at `size`: from the disk cache when
    /// present, else decoded and written back only when `cacheable`.
    fn decode_through_cache(
        cache: &DiskCache,
        item: &MediaItem,
        size: u32,
        cacheable: bool,
    ) -> Option<DecodedFrame> {
        if let Some(frame) = cache.get_cached(
            item.id,
            &item.path,
            item.file_size,
            item.mtime_epoch_ms,
            size,
        ) {
            return Some(frame);
        }
        match DecoderPipeline::decode_media(item.id, item.media_type, &item.path, size) {
            Ok(frame) => {
                if cacheable && size <= MAX_CACHED_DECODE {
                    if let Err(err) = cache.put_cached(
                        &item.path,
                        item.file_size,
                        item.mtime_epoch_ms,
                        size,
                        &frame,
                    ) {
                        log::warn!("failed to cache the decode of {:?}: {err:#}", item.path);
                    }
                }
                Some(frame)
            }
            Err(err) => {
                log::warn!("failed to decode thumbnail for {:?}: {err:?}", item.path);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests;
