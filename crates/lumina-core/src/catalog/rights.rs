//! Creator, copyright and contact details per item (`media_rights`, v17).
//! A side table, not `media_items` columns: rights are not searched, so the
//! search index and its triggers stay as they are.

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::Catalog;
use crate::text::{clean_text, MAX_TEXT_CHARS};
use crate::MediaId;

/// Rights and contact fields, as XMP sidecars carry them. `None` is unset.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rights {
    pub creator: Option<String>,
    pub copyright: Option<String>,
    pub usage_terms: Option<String>,
    pub contact_email: Option<String>,
    pub contact_url: Option<String>,
}

impl Rights {
    /// Every field cleaned to one line of at most [`MAX_TEXT_CHARS`]; a
    /// field left blank becomes `None`.
    pub fn cleaned(&self) -> Rights {
        let clean = |value: &Option<String>| {
            value
                .as_deref()
                .map(|value| clean_text(value, MAX_TEXT_CHARS))
                .filter(|value| !value.is_empty())
        };
        Rights {
            creator: clean(&self.creator),
            copyright: clean(&self.copyright),
            usage_terms: clean(&self.usage_terms),
            contact_email: clean(&self.contact_email),
            contact_url: clean(&self.contact_url),
        }
    }

    /// True when no field holds text.
    pub fn is_empty(&self) -> bool {
        self.fields()
            .iter()
            .all(|value| value.as_deref().is_none_or(|value| value.trim().is_empty()))
    }

    /// `self` with every field `other` sets taken from `other`.
    pub fn merged_with(&self, other: &Rights) -> Rights {
        let pick = |mine: &Option<String>, theirs: &Option<String>| {
            theirs.clone().or_else(|| mine.clone())
        };
        Rights {
            creator: pick(&self.creator, &other.creator),
            copyright: pick(&self.copyright, &other.copyright),
            usage_terms: pick(&self.usage_terms, &other.usage_terms),
            contact_email: pick(&self.contact_email, &other.contact_email),
            contact_url: pick(&self.contact_url, &other.contact_url),
        }
    }

    fn fields(&self) -> [&Option<String>; 5] {
        [
            &self.creator,
            &self.copyright,
            &self.usage_terms,
            &self.contact_email,
            &self.contact_url,
        ]
    }
}

impl Catalog {
    /// The rights of each of `ids` that has any; items without are absent.
    pub fn get_rights(&self, ids: &[MediaId]) -> Result<HashMap<MediaId, Rights>> {
        rights_in(&self.conn.lock(), ids)
    }

    /// Replaces the rights of each item, in one transaction. Values are
    /// cleaned; an item whose rights end up empty loses its row. An id with
    /// no media row fails the whole write (foreign key).
    pub fn set_rights(&self, rights: &[(MediaId, Rights)]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        write_rights_in(&tx, rights)?;
        tx.commit()?;
        Ok(())
    }
}

/// The rights rows of `ids` that have any, read inside `conn` (a
/// transaction, so a template apply sees what it overwrites).
pub(super) fn rights_in(conn: &Connection, ids: &[MediaId]) -> Result<HashMap<MediaId, Rights>> {
    let mut rights = HashMap::with_capacity(ids.len());
    for chunk in ids.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT media_id, creator, copyright, usage_terms, contact_email, contact_url
             FROM media_rights WHERE media_id IN ({marks});"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(chunk.iter().map(|id| id.0 as i64)),
            |row| {
                Ok((
                    MediaId(row.get::<_, i64>(0)? as u64),
                    Rights {
                        creator: row.get(1)?,
                        copyright: row.get(2)?,
                        usage_terms: row.get(3)?,
                        contact_email: row.get(4)?,
                        contact_url: row.get(5)?,
                    },
                ))
            },
        )?;
        for row in rows {
            let (id, value) = row?;
            rights.insert(id, value);
        }
    }
    Ok(rights)
}

/// [`Catalog::set_rights`] inside a caller's transaction.
pub(super) fn write_rights_in(conn: &Connection, rights: &[(MediaId, Rights)]) -> Result<()> {
    let mut delete = conn.prepare_cached("DELETE FROM media_rights WHERE media_id = ?1;")?;
    let mut upsert = conn.prepare_cached(
        "INSERT OR REPLACE INTO media_rights
         (media_id, creator, copyright, usage_terms, contact_email, contact_url)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6);",
    )?;
    for (id, value) in rights {
        let value = value.cleaned();
        if value.is_empty() {
            delete.execute(params![id.0 as i64])?;
        } else {
            upsert.execute(params![
                id.0 as i64,
                value.creator,
                value.copyright,
                value.usage_terms,
                value.contact_email,
                value.contact_url
            ])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::video_item as item;
    use crate::{DeletionTarget, SCHEMA_VERSION};

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[item(path)]).unwrap()[0].id
    }

    fn sample() -> Rights {
        Rights {
            creator: Some("Ada Lovelace".into()),
            copyright: Some("© 2026 Ada".into()),
            usage_terms: Some("All rights reserved".into()),
            contact_email: Some("ada@example.com".into()),
            contact_url: Some("https://ada.example".into()),
        }
    }

    #[test]
    fn set_and_get_rights_cleans_and_drops_empty_rows() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        let untidy = Rights {
            creator: Some("  Ada \n Lovelace ".into()),
            copyright: Some("   ".into()),
            ..Default::default()
        };
        catalog.set_rights(&[(a, untidy), (b, sample())]).unwrap();
        let rights = catalog.get_rights(&[a, b]).unwrap();
        assert_eq!(
            rights[&a],
            Rights {
                creator: Some("Ada Lovelace".into()),
                ..Default::default()
            },
            "values are cleaned and blank fields stored as NULL"
        );
        assert_eq!(rights[&b], sample());

        catalog.set_rights(&[(a, Rights::default())]).unwrap();
        let count: i64 = catalog
            .conn
            .lock()
            .query_row(
                "SELECT COUNT(*) FROM media_rights WHERE media_id = ?1",
                params![a.0 as i64],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "empty rights delete the row instead of storing NULLs"
        );
        assert!(!catalog.get_rights(&[a]).unwrap().contains_key(&a));
    }

    #[test]
    fn set_rights_is_one_transaction() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let err = catalog.set_rights(&[(a, sample()), (MediaId(9_999), sample())]);
        assert!(
            err.is_err(),
            "an id with no media row fails the foreign key"
        );
        assert!(
            catalog.get_rights(&[a]).unwrap().is_empty(),
            "a failed write leaves no partial rows"
        );
    }

    #[test]
    fn migration_creates_media_rights_idempotently() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_rights_migration_{}_{}",
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
            // Back to a v16 catalog: no rights table.
            catalog
                .conn
                .lock()
                .execute_batch("DROP TABLE media_rights; PRAGMA user_version = 16;")
                .unwrap();
        }
        for _ in 0..2 {
            let catalog = Catalog::open(&db).unwrap();
            assert_eq!(catalog.schema_version().unwrap(), SCHEMA_VERSION);
            let a = plain(&catalog, "/lib/a.mp4");
            catalog.set_rights(&[(a, sample())]).unwrap();
            assert_eq!(catalog.get_rights(&[a]).unwrap()[&a], sample());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cascade_deletes_rights() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog.set_rights(&[(a, sample())]).unwrap();
        catalog
            .permanently_delete_items(&[a], DeletionTarget::CatalogOnly)
            .unwrap();
        let count: i64 = catalog
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM media_rights", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "deleting an item deletes its rights row");
    }

    #[test]
    fn merge_carries_rights() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.set_rights(&[(dup, sample())]).unwrap();
        catalog.merge_duplicate_group(keep, &[dup]).unwrap();
        assert_eq!(
            catalog.get_rights(&[keep]).unwrap().get(&keep),
            Some(&sample()),
            "duplicate merge copies rights onto a keeper that has none"
        );

        let own = Rights {
            creator: Some("Keeper".into()),
            ..Default::default()
        };
        let keep2 = plain(&catalog, "/lib/keep2.mp4");
        let dup2 = plain(&catalog, "/lib/dup2.mp4");
        catalog
            .set_rights(&[(keep2, own.clone()), (dup2, sample())])
            .unwrap();
        catalog.merge_duplicate_group(keep2, &[dup2]).unwrap();
        assert_eq!(
            catalog.get_rights(&[keep2]).unwrap()[&keep2],
            own,
            "a keeper's own rights win"
        );

        let fold_keep = plain(&catalog, "/lib/fold_keep.mp4");
        let fold_other = plain(&catalog, "/lib/fold_other.mp4");
        catalog.set_rights(&[(fold_other, sample())]).unwrap();
        catalog.fold_into(fold_keep, fold_other).unwrap();
        assert_eq!(
            catalog.get_rights(&[fold_keep]).unwrap().get(&fold_keep),
            Some(&sample()),
            "a missing-file fold keeps the folded row's rights"
        );
    }
}
