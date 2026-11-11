//! Get Info facts that are not on `MediaItem`: the catalog's added date and
//! the per-file EXIF details, source and rights, loaded for the item the
//! window shows.

use gpui::*;
use lumina_core::scanner::{read_media_details, MediaDetails};
use lumina_core::{MediaId, MediaItem, Rights};
use std::path::PathBuf;
use std::time::Duration;

use crate::app::LuminaApp;

/// How long the docked panel waits for the selection to rest before reading.
pub const INFO_DETAILS_SETTLE: Duration = Duration::from_millis(150);

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
    pub rights: Rights,
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
        self.request_info_details(None, cx);
    }

    /// [`Self::ensure_info_details`] for the docked panel: the read starts
    /// once the selection has rested for [`INFO_DETAILS_SETTLE`], so
    /// arrow-scrubbing the grid reads only the item it stops on.
    pub fn ensure_info_details_settled(&mut self, cx: &mut Context<Self>) {
        self.request_info_details(Some(INFO_DETAILS_SETTLE), cx);
    }

    fn request_info_details(&mut self, settle: Option<Duration>, cx: &mut Context<Self>) {
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
            rights: Rights::default(),
        });
        let read_path = path.clone();
        let read = async move {
            let (added, source, rights) = engine
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
                    let rights = engine
                        .catalog()
                        .get_rights(&[id])
                        .map(|mut rights| rights.remove(&id).unwrap_or_default())
                        .unwrap_or_else(|err| {
                            log::warn!("reading rights for {id:?}: {err:#}");
                            Rights::default()
                        });
                    (added, source, rights)
                })
                .unwrap_or_default();
            (added, source, rights, read_media_details(&read_path))
        };
        cx.spawn(async move |this, cx| {
            if let Some(settle) = settle {
                cx.background_executor().timer(settle).await;
                // A newer selection replaced the placeholder: skip this read.
                let current = this
                    .read_with(cx, |this, _| {
                        this.info_details
                            .as_ref()
                            .is_some_and(|d| d.id == id && d.path == path)
                    })
                    .unwrap_or(false);
                if !current {
                    return;
                }
            }
            let (added, source, rights, file) = cx.background_executor().spawn(read).await;
            // The app may be gone; nothing to show then.
            let _ = this.update(cx, |this, cx| {
                if let Some(details) = this
                    .info_details
                    .as_mut()
                    .filter(|d| d.id == id && d.path == path)
                {
                    details.added_epoch_ms = added;
                    details.source = source;
                    details.rights = rights;
                    details.file = file;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}
