//! Get Info facts that are not on `MediaItem`: the catalog's added date and
//! the per-file EXIF details, loaded for the item the window shows.

use gpui::*;
use lumina_core::scanner::{read_media_details, MediaDetails};
use lumina_core::{MediaId, MediaItem};
use std::path::PathBuf;

use crate::app::LuminaApp;

/// Details for one item. Keyed by id and path: ids restart per catalog, so
/// an id alone could show another library's facts. Fields stay empty until the
/// background read lands.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoDetails {
    pub id: MediaId,
    pub path: PathBuf,
    pub added_epoch_ms: Option<i64>,
    pub file: MediaDetails,
    pub source: Option<(String, Option<String>, i64)>,
}

impl InfoDetails {
    pub fn is_for(&self, item: &MediaItem) -> bool {
        self.id == item.id && self.path == item.path
    }
}

impl LuminaApp {
    /// Loads details for the selected item unless they are already loaded or
    /// in flight. The catalog and file reads run on the background executor
    /// (UI never blocks on IO); a reply for an item no longer shown is dropped.
    pub fn ensure_info_details(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.selected_item.as_ref() else {
            return;
        };
        if self.info_details.as_ref().is_some_and(|d| d.is_for(item)) {
            return;
        }
        let (id, path) = (item.id, item.path.clone());
        let engine = self.grid_state.as_ref().map(|state| state.engine.clone());
        self.info_details = Some(InfoDetails {
            id,
            path: path.clone(),
            added_epoch_ms: None,
            file: MediaDetails::default(),
            source: None,
        });
        let read_path = path.clone();
        let read = cx.background_executor().spawn(async move {
            let (added, source) = engine
                .as_ref()
                .map(|engine| {
                    let added = engine
                        .catalog()
                        .get_added_epoch_ms(id)
                        .unwrap_or_else(|err| {
                            log::warn!("reading the added date for {id:?}: {err:#}");
                            None
                        });
                    let source = engine.get_media_source(id).unwrap_or_else(|err| {
                        log::warn!("reading media source for {id:?}: {err:#}");
                        None
                    });
                    (added, source)
                })
                .unwrap_or((None, None));
            (added, source, read_media_details(&read_path))
        });
        cx.spawn(async move |this, cx| {
            let (added, source, file) = read.await;
            // The app may be gone; nothing to show then.
            let _ = this.update(cx, |this, cx| {
                if let Some(details) = this
                    .info_details
                    .as_mut()
                    .filter(|d| d.id == id && d.path == path)
                {
                    details.added_epoch_ms = added;
                    details.source = source;
                    details.file = file;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
