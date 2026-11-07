//! Non-destructive quick edits per item (`media_edits`, v18). Rotation stays
//! in `media_items.rotation_quarters`; crop, exposure and auto-enhance live
//! in the side table, so the media SELECT lists and search index are unchanged.
//! Edits are applied after the disk cache, so writing them never touches the
//! original file or a cached decode.

use anyhow::{bail, Context, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};

use super::Catalog;
use crate::{normalize_quarter_turns, MediaId};

/// Smallest crop side, as a fraction of the frame.
const MIN_CROP_SIDE: f32 = 0.02;

/// A crop in normalized `[0, 1]` coordinates of the unrotated decoded frame,
/// so a later rotation never needs to remap it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl CropRect {
    /// Fails unless every value is finite, the rect lies inside the frame and
    /// each side is at least 2 % of it. An invalid crop is never clamped.
    pub fn validate(&self) -> Result<()> {
        let Self { x, y, w, h } = *self;
        let finite = [x, y, w, h].iter().all(|value| value.is_finite());
        if !finite
            || x < 0.0
            || y < 0.0
            || w < MIN_CROP_SIDE
            || h < MIN_CROP_SIDE
            || x + w > 1.0
            || y + h > 1.0
        {
            bail!("invalid crop {x}, {y}, {w} x {h}");
        }
        Ok(())
    }
}

/// The current edit state of one item. `Default` is the unedited original.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edits {
    /// Clockwise quarter turns, 0..=3.
    pub rotation: u8,
    pub crop: Option<CropRect>,
    pub exposure_ev: f32,
    pub auto_enhance: bool,
}

impl Edits {
    /// True when nothing but rotation is set, so no side row is needed.
    fn side_row_is_default(&self) -> bool {
        self.crop.is_none() && self.exposure_ev == 0.0 && !self.auto_enhance
    }
}

impl Catalog {
    /// The edits of each of `ids` in the catalog; unknown ids are left out.
    pub fn get_edits(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, Edits>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT m.rotation_quarters, e.crop_x, e.crop_y, e.crop_w, e.crop_h,
                    e.exposure_ev, e.auto_enhance
             FROM media_items m LEFT JOIN media_edits e ON e.media_id = m.id
             WHERE m.id = ?1;",
        )?;
        let mut edits = HashMap::with_capacity(ids.len());
        for &id in ids {
            let row = stmt
                .query_row(params![id.0 as i64], |row| {
                    let crop: [Option<f64>; 4] =
                        [row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?];
                    Ok(Edits {
                        rotation: normalize_quarter_turns(row.get(0)?),
                        crop: match crop {
                            [Some(x), Some(y), Some(w), Some(h)] => Some(CropRect {
                                x: x as f32,
                                y: y as f32,
                                w: w as f32,
                                h: h as f32,
                            }),
                            _ => None,
                        },
                        exposure_ev: row.get::<_, Option<f64>>(5)?.unwrap_or(0.0) as f32,
                        auto_enhance: row.get::<_, Option<bool>>(6)?.unwrap_or(false),
                    })
                })
                .optional()?;
            if let Some(row) = row {
                edits.insert(id, row);
            }
        }
        Ok(edits)
    }

    /// Replaces each item's edits in one transaction: the rotation column and
    /// the side row together. Edits with nothing but rotation delete the side
    /// row. Unknown ids are skipped; an invalid crop or a non-finite exposure
    /// fails the whole write.
    pub fn set_edits(&self, changes: &[(MediaId, Edits)]) -> Result<()> {
        for (id, edits) in changes {
            if let Some(crop) = edits.crop {
                crop.validate()
                    .with_context(|| format!("edits for item {}", id.0))?;
            }
            if !edits.exposure_ev.is_finite() {
                bail!("invalid exposure for item {}", id.0);
            }
        }
        let now = chrono::Utc::now().timestamp_millis();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        {
            let mut rotate =
                tx.prepare_cached("UPDATE media_items SET rotation_quarters = ?1 WHERE id = ?2;")?;
            let mut delete = tx.prepare_cached("DELETE FROM media_edits WHERE media_id = ?1;")?;
            let mut upsert = tx.prepare_cached(
                "INSERT OR REPLACE INTO media_edits
                 (media_id, crop_x, crop_y, crop_w, crop_h, exposure_ev, auto_enhance,
                  updated_epoch_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);",
            )?;
            for (id, edits) in changes {
                let quarters = normalize_quarter_turns(edits.rotation as i32);
                if rotate.execute(params![quarters, id.0 as i64])? == 0 {
                    continue; // no media row: skipped, like set_rotations
                }
                if edits.side_row_is_default() {
                    delete.execute(params![id.0 as i64])?;
                } else {
                    let crop = edits.crop;
                    upsert.execute(params![
                        id.0 as i64,
                        crop.map(|c| c.x as f64),
                        crop.map(|c| c.y as f64),
                        crop.map(|c| c.w as f64),
                        crop.map(|c| c.h as f64),
                        edits.exposure_ev as f64,
                        edits.auto_enhance,
                        now
                    ])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every item with an edits row or a non-zero rotation (grid badge).
    pub fn edited_ids(&self) -> Result<HashSet<MediaId>> {
        let conn = self.conn.lock();
        // ponytail: full scan of media_items (~8 ms warm at 100k items). If it
        // shows in view-load timings, add a partial index WHERE
        // rotation_quarters != 0 and query `!= 0` (no index serves `% 4`).
        let mut stmt = conn.prepare_cached(
            "SELECT id FROM media_items WHERE rotation_quarters % 4 != 0
             UNION SELECT media_id FROM media_edits;",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .map(|id| id.map(|id| MediaId(id as u64)))
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::video_item as item;
    use crate::{DeletionTarget, SCHEMA_VERSION};

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[item(path)]).unwrap()[0].id
    }

    fn sample() -> Edits {
        Edits {
            rotation: 1,
            crop: Some(CropRect {
                x: 0.25,
                y: 0.125,
                w: 0.5,
                h: 0.75,
            }),
            exposure_ev: -0.5,
            auto_enhance: true,
        }
    }

    fn edit_rows(catalog: &Catalog) -> i64 {
        catalog
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM media_edits", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn migration_creates_media_edits_idempotently() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_edits_migration_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("catalog.db");
        {
            let catalog = Catalog::open(&db).unwrap();
            // Back to a v17 catalog: no edits table.
            catalog
                .conn
                .lock()
                .execute_batch("DROP TABLE media_edits; PRAGMA user_version = 17;")
                .unwrap();
        }
        for _ in 0..2 {
            let catalog = Catalog::open(&db).unwrap();
            assert_eq!(
                catalog.schema_version().unwrap(),
                SCHEMA_VERSION,
                "opening stamps the current schema version"
            );
            let a = plain(&catalog, "/lib/a.mp4");
            catalog.set_edits(&[(a, sample())]).unwrap();
            assert_eq!(
                catalog.get_edits(&[a]).unwrap()[&a],
                sample(),
                "the migrated catalog stores edits"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn edits_roundtrip_and_default_deletes_row() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        let c = plain(&catalog, "/lib/c.mp4");
        let missing = MediaId(9_999);
        let rotated = Edits {
            rotation: 5,
            ..Default::default()
        };
        catalog
            .set_edits(&[(a, sample()), (b, rotated), (missing, sample())])
            .expect("unknown ids are skipped, not an error");

        let edits = catalog.get_edits(&[a, b, c, missing]).unwrap();
        assert_eq!(edits[&a], sample(), "every field round-trips");
        assert_eq!(edits[&b].rotation, 1, "rotation normalizes to 0..=3");
        assert_eq!(
            edits[&c],
            Edits::default(),
            "an unedited item reads default"
        );
        assert!(!edits.contains_key(&missing), "unknown ids are left out");
        assert_eq!(
            catalog.get_rotations(&[a]).unwrap()[&a],
            1,
            "rotation is written to the media_items column"
        );
        assert_eq!(edit_rows(&catalog), 1, "rotation alone needs no side row");
        assert_eq!(
            catalog.edited_ids().unwrap(),
            HashSet::from([a, b]),
            "an edits row or a rotation marks an item edited"
        );

        catalog.set_edits(&[(a, Edits::default())]).unwrap();
        assert_eq!(edit_rows(&catalog), 0, "default edits delete the row");
        assert_eq!(catalog.get_edits(&[a]).unwrap()[&a], Edits::default());
        assert_eq!(catalog.edited_ids().unwrap(), HashSet::from([b]));
    }

    #[test]
    fn invalid_crop_is_rejected() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let crop = |x, y, w, h| Edits {
            rotation: 2,
            crop: Some(CropRect { x, y, w, h }),
            ..Default::default()
        };
        for bad in [
            crop(f32::NAN, 0.0, 0.5, 0.5),
            crop(0.0, 0.0, f32::INFINITY, 0.5),
            crop(-0.1, 0.0, 0.5, 0.5),
            crop(0.0, -0.1, 0.5, 0.5),
            crop(0.6, 0.0, 0.5, 0.5),
            crop(0.0, 0.6, 0.5, 0.5),
            crop(0.0, 0.0, 0.01, 0.5),
            crop(0.0, 0.0, 0.5, -0.5),
        ] {
            assert!(
                catalog.set_edits(&[(a, bad)]).is_err(),
                "an invalid crop is an error, never clamped: {bad:?}"
            );
        }
        let nan_exposure = Edits {
            exposure_ev: f32::NAN,
            ..Default::default()
        };
        assert!(catalog.set_edits(&[(a, nan_exposure)]).is_err());
        assert_eq!(
            catalog.get_edits(&[a]).unwrap()[&a],
            Edits::default(),
            "a rejected write changes nothing, not even rotation"
        );
        let b = plain(&catalog, "/lib/b.mp4");
        let err = catalog
            .set_edits(&[(b, sample()), (a, crop(0.0, 0.6, 0.5, 0.5))])
            .expect_err("one invalid crop fails the whole batch");
        assert!(
            format!("{err:#}").contains(&format!("item {}", a.0)),
            "the error names the item: {err:#}"
        );
        assert_eq!(
            catalog.get_edits(&[b]).unwrap()[&b],
            Edits::default(),
            "the valid change in a rejected batch is not written"
        );
        catalog
            .set_edits(&[(a, crop(0.0, 0.0, 1.0, 0.02))])
            .expect("a full-width crop at the minimum height is valid");
    }

    #[test]
    fn fold_carries_edits() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let other = plain(&catalog, "/lib/other.mp4");
        catalog.set_edits(&[(other, sample())]).unwrap();
        catalog.fold_into(keep, other).unwrap();
        assert_eq!(
            catalog.get_edits(&[keep]).unwrap()[&keep],
            sample(),
            "a missing-file fold keeps the folded row's edits and rotation"
        );
    }

    #[test]
    fn duplicate_merge_carries_edits_when_keeper_has_none() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.set_edits(&[(dup, sample())]).unwrap();
        catalog.merge_duplicate_group(keep, &[dup]).unwrap();
        assert_eq!(
            catalog.get_edits(&[keep]).unwrap()[&keep],
            sample(),
            "duplicate merge copies the edits row with its rotation onto an unrotated keeper"
        );

        let own = Edits {
            exposure_ev: 1.0,
            ..Default::default()
        };
        let keep2 = plain(&catalog, "/lib/keep2.mp4");
        let dup2 = plain(&catalog, "/lib/dup2.mp4");
        catalog
            .set_edits(&[(keep2, own), (dup2, sample())])
            .unwrap();
        catalog.merge_duplicate_group(keep2, &[dup2]).unwrap();
        assert_eq!(
            catalog.get_edits(&[keep2]).unwrap()[&keep2],
            own,
            "a keeper's own edits win"
        );
    }

    #[test]
    fn cascade_deletes_edits() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog.set_edits(&[(a, sample())]).unwrap();
        catalog
            .permanently_delete_items(&[a], DeletionTarget::CatalogOnly)
            .unwrap();
        assert_eq!(edit_rows(&catalog), 0, "deleting an item deletes its edits");
    }
}
