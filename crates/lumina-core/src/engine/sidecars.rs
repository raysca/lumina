//! XMP sidecar export and import for the engine.

use anyhow::{Context, Result};
use std::collections::HashMap;

use crate::*;

impl LuminaEngine {
    /// Writes/updates XMP sidecars next to the original files (metadata-only
    /// export). Safe to call from any thread; passes run one at a time.
    /// A sidecar another app wrote is never rewritten or deleted: it is
    /// listed in `skipped_foreign` and logged once per pass.
    pub fn write_metadata_sidecars(&self, ids: &[MediaId]) -> Result<SidecarWriteReport> {
        let _pass = self.sidecar_writes.lock();
        let captions = self
            .catalog
            .get_captions_map()
            .context("could not read captions")?;
        let ratings = self
            .catalog
            .get_ratings_map()
            .context("could not read ratings")?;
        let labels = self
            .catalog
            .get_color_labels_map()
            .context("could not read color labels")?;
        let rights = self
            .catalog
            .get_rights(ids)
            .context("could not read rights")?;
        let edited = self
            .catalog
            .edited_flags(ids)
            .context("could not read edited dates and places")?;
        let mut report = SidecarWriteReport::default();
        for id in ids {
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                continue;
            };
            let metadata =
                self.sidecar_metadata_for(*id, &captions, &ratings, &labels, &rights, &edited)?;
            match xmp::write_library_sidecar(&item.path, &metadata)? {
                SidecarOutcome::Written(_) => report.written += 1,
                SidecarOutcome::SkippedForeign => {
                    report.skipped_foreign.push(xmp::sidecar_path(&item.path))
                }
                SidecarOutcome::Removed | SidecarOutcome::Unchanged => {}
            }
        }
        if let Some(first) = report.skipped_foreign.first() {
            log::warn!(
                "left {} XMP sidecar(s) written by another app unchanged, first {first:?}",
                report.skipped_foreign.len()
            );
        }
        Ok(report)
    }

    /// Imports XMP sidecars for the given items. Only values present in the
    /// sidecar are applied (ratings always, title/caption/color label, each
    /// rights field, the date and the GPS position when set, tags merged),
    /// so importing never clears metadata that the sidecar does not mention.
    pub fn import_sidecars(&self, ids: &[MediaId]) -> Result<ImportReport> {
        let before = self.edits_snapshot(ids)?;
        self.import_sidecars_since(ids, &before)
    }

    /// The user-editable fields of `ids` as they are now, read before any
    /// sidecar so an edit made while the import runs can be told apart.
    fn edits_snapshot(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, ItemEdits>> {
        let mut before = HashMap::with_capacity(ids.len());
        for &id in ids {
            if let Some(item) = self.catalog.get_item_by_id(id)? {
                before.insert(id, self.item_edits(&item)?);
            }
        }
        Ok(before)
    }

    fn item_edits(&self, item: &MediaItem) -> Result<ItemEdits> {
        Ok(ItemEdits {
            title: item.title.clone(),
            rating: self
                .catalog
                .get_rating(item.id)
                .context("could not read rating")?,
            label: self
                .catalog
                .get_color_label(item.id)
                .context("could not read color label")?,
            caption: self
                .catalog
                .get_caption(item.id)
                .context("could not read caption")?,
            rights: self
                .catalog
                .get_rights(&[item.id])
                .context("could not read rights")?
                .remove(&item.id)
                .unwrap_or_default(),
            timestamp_epoch_ms: item.timestamp_epoch_ms,
            location: (item.latitude, item.longitude),
        })
    }

    /// Applies each item's sidecar, except to a field the user changed since
    /// `before` was taken: the import runs in the background, and an edit made
    /// meanwhile must win over the (older) sidecar value.
    // ponytail: read-then-write per field, so an edit landing in the
    // microseconds between the fresh read and the write can still lose; a
    // conditional `UPDATE ... WHERE rating = ?before` closes that if needed.
    fn import_sidecars_since(
        &self,
        ids: &[MediaId],
        before: &HashMap<MediaId, ItemEdits>,
    ) -> Result<ImportReport> {
        let mut report = ImportReport::default();

        for id in ids {
            let (Some(item), Some(then)) = (self.catalog.get_item_by_id(*id)?, before.get(id))
            else {
                report.skipped += 1;
                continue;
            };
            match xmp::read_sidecar(&item.path) {
                Ok(Some(metadata)) => {
                    report.imported += 1;
                    let mut changed = false;
                    let now = self.item_edits(&item)?;

                    // Compare as stored, so a sidecar with untidy text is not
                    // rewritten (and counted as updated) on every import.
                    let title = metadata
                        .title
                        .as_deref()
                        .map(|title| text::clean_text(title, text::MAX_TEXT_CHARS))
                        .filter(|title| !title.is_empty());
                    if metadata.title.is_some() && title != now.title && now.title == then.title {
                        self.catalog.set_title(*id, metadata.title.as_deref())?;
                        changed = true;
                    }
                    if metadata.rating != now.rating && now.rating == then.rating {
                        self.catalog.set_rating(*id, metadata.rating)?;
                        changed = true;
                    }
                    // Sidecars from before labels were written carry none; that
                    // must not clear a label set in Lumina.
                    if metadata.color_label != 0
                        && metadata.color_label != now.label
                        && now.label == then.label
                    {
                        self.catalog.set_color_label(*id, metadata.color_label)?;
                        changed = true;
                    }
                    let caption = metadata
                        .caption
                        .as_deref()
                        .map(|caption| text::clean_caption(caption, text::MAX_TEXT_CHARS))
                        .filter(|caption| !caption.is_empty());
                    if metadata.caption.is_some()
                        && caption != now.caption
                        && now.caption == then.caption
                    {
                        self.catalog
                            .set_caption(*id, metadata.caption.as_deref().unwrap_or(""))?;
                        changed = true;
                    }
                    // Fields the sidecar sets replace the catalog's; the
                    // others stay (parse_xmp already cleaned and dropped blanks).
                    let rights = now.rights.merged_with(&metadata.rights);
                    if rights != now.rights && now.rights == then.rights {
                        self.catalog.set_rights(&[(*id, rights)])?;
                        changed = true;
                    }
                    // Dates compare by the second (sidecars carry no
                    // milliseconds), places within 1e-5°, so a re-import
                    // of Lumina's own sidecar changes nothing. Both writes
                    // mark the edited flags, so a rescan keeps them.
                    if let Some(mut date) = metadata.date_taken {
                        // A date alone changes the day and keeps the time of
                        // day, as the date field in Get Info does.
                        if metadata.date_only {
                            if let Some(now) =
                                chrono::DateTime::from_timestamp_millis(now.timestamp_epoch_ms)
                            {
                                date = date.date().and_time(now.time());
                            }
                        }
                        let epoch_ms = date.and_utc().timestamp_millis();
                        if epoch_ms.div_euclid(1000) != now.timestamp_epoch_ms.div_euclid(1000)
                            && now.timestamp_epoch_ms == then.timestamp_epoch_ms
                        {
                            self.catalog.set_timestamps(&[(*id, epoch_ms)])?;
                            changed = true;
                        }
                    }
                    if let Some((lat, lon)) = metadata.location {
                        let same = match now.location {
                            (Some(now_lat), Some(now_lon)) => {
                                (now_lat - lat).abs() <= 1e-5 && (now_lon - lon).abs() <= 1e-5
                            }
                            _ => false,
                        };
                        if !same && now.location == then.location {
                            self.catalog
                                .set_locations(&[(*id, (Some(lat), Some(lon)))])?;
                            changed = true;
                        }
                    }
                    // A keyword the catalog refuses (over the length cap, or
                    // at or below an alias) is reported; the item's other
                    // keywords still import. A blank one (no segment left
                    // after cleaning) is skipped.
                    let mut tags = Vec::new();
                    for tag in &metadata.tags {
                        // Only an invalid path is a per-keyword failure; a
                        // database error from the alias lookup aborts.
                        let path = match catalog::keywords::normalize_path(tag) {
                            Ok(path) => self.catalog.resolve_alias(&path)?,
                            Err(_) if catalog::keywords::is_blank_path(tag) => continue,
                            Err(err) => {
                                report.failed.push((
                                    item.path.clone(),
                                    format!("could not import keyword: {err}"),
                                ));
                                continue;
                            }
                        };
                        match self.catalog.keyword_refusal(&path)? {
                            Some(reason) => report.failed.push((
                                item.path.clone(),
                                format!("could not import keyword: {reason}"),
                            )),
                            None => tags.push(path),
                        }
                    }
                    if !tags.is_empty() {
                        self.catalog.add_tags_batch(&[*id], &tags)?;
                        changed = true;
                    }
                    // Other tools write a file name to dc:source; only a
                    // web address is a network attribution.
                    if let Some(ref source_url) = metadata.source_url {
                        if source_url.starts_with("http://") || source_url.starts_with("https://") {
                            self.catalog.record_media_source(
                                *id,
                                source_url,
                                metadata.credit.as_deref(),
                            )?;
                            changed = true;
                        }
                    }
                    if changed {
                        report.updated += 1;
                    }
                }
                Ok(None) => report.skipped += 1,
                Err(err) => report
                    .failed
                    .push((item.path.clone(), format!("could not read sidecar: {err}"))),
            }
        }
        Ok(report)
    }

    pub(crate) fn sidecar_metadata_for(
        &self,
        id: MediaId,
        captions: &HashMap<MediaId, String>,
        ratings: &HashMap<MediaId, u8>,
        labels: &HashMap<MediaId, u8>,
        rights: &HashMap<MediaId, Rights>,
        edited: &HashMap<MediaId, (bool, bool)>,
    ) -> Result<SidecarMetadata> {
        let tags = self.catalog.get_tags_for_item(id)?;
        let item = self.catalog.get_item_by_id(id)?;
        // Decision: only an edited date or place is written; file EXIF stays
        // the source otherwise. The catalog keeps wall time as UTC.
        let (date_edited, place_edited) = edited.get(&id).copied().unwrap_or_default();
        let date_taken = item
            .as_ref()
            .filter(|_| date_edited)
            .and_then(|item| chrono::DateTime::from_timestamp_millis(item.timestamp_epoch_ms))
            .map(|date| date.naive_utc());
        let location = item
            .as_ref()
            .filter(|_| place_edited)
            .and_then(|item| item.latitude.zip(item.longitude));
        let title = item.and_then(|item| item.title);
        let (source_url, credit) = match self.catalog.get_media_source(id)? {
            Some((url, uploader, _)) => (Some(url), uploader),
            None => (None, None),
        };
        Ok(SidecarMetadata {
            title,
            caption: captions.get(&id).cloned(),
            rating: ratings.get(&id).copied().unwrap_or(0),
            color_label: labels.get(&id).copied().unwrap_or(0),
            tags,
            source_url,
            credit,
            rights: rights.get(&id).cloned().unwrap_or_default(),
            date_taken,
            date_only: false,
            location,
        })
    }
}

/// The fields an XMP import may overwrite, as the catalog holds them.
#[derive(Debug, Clone, PartialEq)]
struct ItemEdits {
    title: Option<String>,
    rating: u8,
    label: u8,
    caption: Option<String>,
    rights: Rights,
    timestamp_epoch_ms: i64,
    location: Coordinates,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{temp_library, TEST_WAIT};
    use std::fs;

    #[test]
    fn xmp_import_keeps_a_field_the_user_edited_while_it_ran() {
        let dir = temp_library("xmp_edit_race");
        fs::write(dir.join("a.jpg"), b"test-jpeg-content").expect("fixture writes");
        let (engine, rx) = LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), None)
            .expect("engine opens");
        while let Ok(event) = rx.recv_timeout(TEST_WAIT) {
            if let EngineEvent::ScanComplete { .. } = event {
                break;
            }
        }
        let item = engine.get_item_slice(0..1).remove(0);
        write_sidecar(
            &item.path,
            &SidecarMetadata {
                title: Some("Sidecar title".into()),
                caption: Some("Sidecar caption".into()),
                rating: 5,
                color_label: 4,
                rights: crate::catalog::Rights {
                    creator: Some("Sidecar creator".into()),
                    ..Default::default()
                },
                date_taken: chrono::DateTime::from_timestamp(1_500_000_000, 0)
                    .map(|date| date.naive_utc()),
                location: Some((48.8566, 2.3522)),
                ..Default::default()
            },
        )
        .expect("sidecar writes");

        let before = engine.edits_snapshot(&[item.id]).expect("snapshot");
        engine
            .catalog
            .set_timestamp(item.id, 1_600_000_000_000)
            .expect("date");
        engine
            .catalog
            .set_location(item.id, Some(-33.8688), Some(151.2093))
            .expect("place");
        let mine = crate::catalog::Rights {
            creator: Some("My name".into()),
            ..Default::default()
        };
        engine
            .catalog
            .set_rights(&[(item.id, mine.clone())])
            .expect("rights");
        // The user edits while the background import is still running.
        engine.catalog.set_rating(item.id, 2).expect("rating");
        engine
            .catalog
            .set_title(item.id, Some("Mine"))
            .expect("title");
        engine
            .import_sidecars_since(&[item.id], &before)
            .expect("import runs");

        assert_eq!(
            engine.catalog.get_rating(item.id).unwrap(),
            2,
            "a rating set during the import must not be overwritten by the sidecar"
        );
        let title = engine
            .catalog
            .get_item_by_id(item.id)
            .unwrap()
            .unwrap()
            .title;
        assert_eq!(title.as_deref(), Some("Mine"), "the user's title wins");
        assert_eq!(
            engine.catalog.get_rights(&[item.id]).unwrap().get(&item.id),
            Some(&mine),
            "rights set during the import must not be overwritten by the sidecar"
        );
        assert_eq!(
            engine.catalog.get_color_label(item.id).unwrap(),
            4,
            "an untouched field still imports"
        );
        assert_eq!(
            engine.catalog.get_caption(item.id).unwrap().as_deref(),
            Some("Sidecar caption"),
            "an untouched caption still imports"
        );
        let after = engine.catalog.get_item_by_id(item.id).unwrap().unwrap();
        assert_eq!(
            after.timestamp_epoch_ms, 1_600_000_000_000,
            "a date set during the import must not be overwritten by the sidecar"
        );
        assert_eq!(
            (after.latitude, after.longitude),
            (Some(-33.8688), Some(151.2093)),
            "a place set during the import must not be overwritten by the sidecar"
        );
        drop(engine);
        let _ = fs::remove_dir_all(&dir);
    }
}
