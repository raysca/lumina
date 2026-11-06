//! Applying a metadata template (rights plus keywords) to a selection, and
//! replaying what one apply changed for undo and redo. The templates
//! themselves live in the app's user config, not the catalog.

use anyhow::Result;
use rusqlite::params;
use std::collections::HashSet;

use super::keyword_ops::{refuse_alias_path, resolve_in};
use super::keywords::normalize_path;
use super::rights::{rights_in, write_rights_in};
use super::{Catalog, Rights};
use crate::MediaId;

/// What one template apply changed, as exact rows, so undo restores the
/// rights and removes only the keywords the apply added.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateChange {
    /// `(media, before, after)` for every item the template was applied to.
    pub rights: Vec<(MediaId, Rights, Rights)>,
    /// `(media, tag)` rows the apply inserted; a keyword the item already
    /// had is not listed.
    pub added_tags: Vec<(MediaId, String)>,
}

impl TemplateChange {
    /// Every item the apply touched, each once, in selection order.
    pub fn affected_ids(&self) -> Vec<MediaId> {
        self.rights.iter().map(|(id, _, _)| *id).collect()
    }
}

impl Catalog {
    /// Applies a template to `ids` in one transaction: each field `rights`
    /// sets overwrites the item's own (unset fields are left alone), and each
    /// keyword is added the way [`Catalog::add_tags_batch`] adds it (path
    /// cleaned, alias resolved, a path below an alias refused). Any failure,
    /// including an id with no media row, writes nothing.
    pub fn apply_template(
        &self,
        ids: &[MediaId],
        rights: &Rights,
        keywords: &[String],
    ) -> Result<TemplateChange> {
        let template = rights.cleaned();
        let keywords = keywords
            .iter()
            .map(|keyword| normalize_path(keyword))
            .collect::<Result<Vec<String>>>()?;
        let raw_ids: Vec<i64> = ids.iter().map(|id| id.0 as i64).collect();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let keywords = keywords
            .into_iter()
            .map(|keyword| {
                let keyword = resolve_in(&tx, keyword)?;
                refuse_alias_path(&tx, &keyword)?;
                Ok(keyword)
            })
            .collect::<Result<Vec<String>>>()?;

        let mut before = rights_in(&tx, ids)?;
        let change_rights: Vec<(MediaId, Rights, Rights)> = ids
            .iter()
            .map(|&id| {
                let was = before.remove(&id).unwrap_or_default();
                let now = was.merged_with(&template).cleaned();
                (id, was, now)
            })
            .collect();
        // An id with no media row fails the foreign key of whichever row
        // it gets first (rights or keyword), which rolls everything back.
        write_rights_in(
            &tx,
            &change_rights
                .iter()
                .map(|(id, _, now)| (*id, now.clone()))
                .collect::<Vec<_>>(),
        )?;

        let mut added_tags = Vec::new();
        if !keywords.is_empty() {
            Self::delete_fts_for_items(&tx, &raw_ids)?;
            {
                let mut insert = tx.prepare_cached(
                    "INSERT OR IGNORE INTO media_tags (media_id, tag) VALUES (?1, ?2);",
                )?;
                for &id in ids {
                    for keyword in &keywords {
                        if insert.execute(params![id.0 as i64, keyword])? > 0 {
                            added_tags.push((id, keyword.clone()));
                        }
                    }
                }
            }
            Self::insert_fts_for_items(&tx, &raw_ids)?;
        }
        tx.commit()?;
        Ok(TemplateChange {
            rights: change_rights,
            added_tags,
        })
    }

    /// Puts back the state before (`before = true`, undo) or after (redo) a
    /// template apply, in one transaction: the rights, and exactly the
    /// keyword rows the apply added. An item purged since is skipped, and an
    /// item whose rights changed since (they no longer match the side being
    /// replaced) keeps them, so a later edit is never overwritten.
    pub fn replay_template_change(&self, change: &TemplateChange, before: bool) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let live: HashSet<MediaId> = {
            let mut exists = tx.prepare_cached("SELECT 1 FROM media_items WHERE id = ?1;")?;
            let mut live = HashSet::new();
            for id in change.affected_ids() {
                if exists.exists(params![id.0 as i64])? {
                    live.insert(id);
                }
            }
            live
        };
        let ids: Vec<MediaId> = change
            .affected_ids()
            .into_iter()
            .filter(|id| live.contains(id))
            .collect();
        let mut current = rights_in(&tx, &ids)?;
        let rights: Vec<(MediaId, Rights)> = change
            .rights
            .iter()
            .filter(|(id, _, _)| live.contains(id))
            .filter_map(|(id, was, now)| {
                let (from, to) = if before { (now, was) } else { (was, now) };
                let is = current.remove(id).unwrap_or_default();
                (is == from.cleaned()).then(|| (*id, to.clone()))
            })
            .collect();
        write_rights_in(&tx, &rights)?;
        let tags: Vec<&(MediaId, String)> = change
            .added_tags
            .iter()
            .filter(|(id, _)| live.contains(id))
            .collect();
        if !tags.is_empty() {
            let raw_ids: Vec<i64> = ids.iter().map(|id| id.0 as i64).collect();
            Self::delete_fts_for_items(&tx, &raw_ids)?;
            {
                let sql = if before {
                    "DELETE FROM media_tags WHERE media_id = ?1 AND tag = ?2;"
                } else {
                    "INSERT OR IGNORE INTO media_tags (media_id, tag) VALUES (?1, ?2);"
                };
                let mut stmt = tx.prepare_cached(sql)?;
                for (id, tag) in tags {
                    stmt.execute(params![id.0 as i64, tag])?;
                }
            }
            Self::insert_fts_for_items(&tx, &raw_ids)?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::video_item as item;

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[item(path)]).unwrap()[0].id
    }

    fn tags(catalog: &Catalog, id: MediaId) -> Vec<String> {
        catalog.get_tags_for_item(id).unwrap()
    }

    #[test]
    fn apply_template_overwrites_only_set_fields() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        let own = Rights {
            creator: Some("Grace".into()),
            usage_terms: Some("Editorial only".into()),
            ..Default::default()
        };
        catalog.set_rights(&[(a, own.clone())]).unwrap();
        catalog.add_tags_batch(&[a], &["trip".into()]).unwrap();
        let template = Rights {
            creator: Some("Ada".into()),
            copyright: Some("© 2026 Ada".into()),
            // Blank is unset: it must not clear the item's terms.
            usage_terms: Some("  ".into()),
            ..Default::default()
        };

        let change = catalog
            .apply_template(&[a, b], &template, &["Trip".into(), "work/clients".into()])
            .unwrap();

        let rights = catalog.get_rights(&[a, b]).unwrap();
        assert_eq!(
            rights[&a],
            Rights {
                creator: Some("Ada".into()),
                copyright: Some("© 2026 Ada".into()),
                usage_terms: Some("Editorial only".into()),
                ..Default::default()
            },
            "the template overwrites the fields it sets and leaves the rest"
        );
        assert_eq!(rights[&b].creator.as_deref(), Some("Ada"));
        assert_eq!(rights[&b].usage_terms, None);
        assert_eq!(tags(&catalog, a), vec!["trip", "work/clients"]);
        assert_eq!(tags(&catalog, b), vec!["trip", "work/clients"]);
        assert_eq!(
            change.added_tags,
            vec![
                (a, "work/clients".to_string()),
                (b, "trip".to_string()),
                (b, "work/clients".to_string()),
            ],
            "only keywords an item did not have are recorded as added"
        );
        assert_eq!(change.rights[0], (a, own.clone(), rights[&a].clone()));

        catalog.replay_template_change(&change, true).unwrap();
        assert_eq!(catalog.get_rights(&[a]).unwrap()[&a], own, "undo restores");
        assert!(!catalog.get_rights(&[b]).unwrap().contains_key(&b));
        assert_eq!(tags(&catalog, a), vec!["trip"], "a keyword it had stays");
        assert!(tags(&catalog, b).is_empty());

        catalog.replay_template_change(&change, false).unwrap();
        assert_eq!(catalog.get_rights(&[a, b]).unwrap(), rights, "redo");
        assert_eq!(tags(&catalog, b), vec!["trip", "work/clients"]);
    }

    #[test]
    fn apply_template_is_one_transaction() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let template = Rights {
            creator: Some("Ada".into()),
            ..Default::default()
        };
        let err = catalog.apply_template(&[a, MediaId(9_999)], &template, &["trip".into()]);
        assert!(err.is_err(), "an id with no media row fails the apply");
        assert!(
            catalog.get_rights(&[a]).unwrap().is_empty(),
            "a failed apply leaves the real item without rights"
        );
        assert!(tags(&catalog, a).is_empty(), "and without new keywords");
    }

    #[test]
    fn template_keywords_resolve_aliases_and_paths() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog.set_alias("nyc", "places/new york").unwrap();
        let change = catalog
            .apply_template(
                &[a],
                &Rights::default(),
                &[" NYC ".into(), "Places / Paris /".into()],
            )
            .unwrap();
        assert_eq!(
            tags(&catalog, a),
            vec!["places/new york", "places/paris"],
            "an alias stores its keyword and a path is cleaned"
        );
        assert_eq!(change.added_tags.len(), 2);

        let err = catalog.apply_template(&[a], &Rights::default(), &["nyc/x".into()]);
        assert!(err.is_err(), "a keyword below an alias is refused");
    }
    fn ada() -> Rights {
        Rights {
            creator: Some("Ada".into()),
            ..Default::default()
        }
    }

    fn search(catalog: &Catalog, query: &str) -> Vec<MediaId> {
        let mut ids: Vec<MediaId> = catalog
            .search_fts(query, 50)
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    #[test]
    fn replay_skips_purged_items() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        let change = catalog
            .apply_template(&[a, b], &ada(), &["trip".into()])
            .unwrap();
        catalog
            .permanently_delete_items(&[b], crate::DeletionTarget::CatalogOnly)
            .unwrap();

        catalog
            .replay_template_change(&change, true)
            .expect("undo succeeds for the items that are left");
        assert!(catalog.get_rights(&[a]).unwrap().is_empty());
        assert!(tags(&catalog, a).is_empty());
        catalog
            .replay_template_change(&change, false)
            .expect("redo succeeds for the items that are left");
        assert_eq!(catalog.get_rights(&[a]).unwrap()[&a], ada());
        assert_eq!(tags(&catalog, a), vec!["trip"]);
        assert!(
            catalog.get_rights(&[b]).unwrap().is_empty(),
            "a purged item gets no rows back"
        );
    }

    #[test]
    fn replay_keeps_later_rights_edits() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        let change = catalog.apply_template(&[a, b], &ada(), &[]).unwrap();
        let later = Rights {
            creator: Some("Edited later".into()),
            ..Default::default()
        };
        catalog.set_rights(&[(b, later.clone())]).unwrap();

        catalog.replay_template_change(&change, true).unwrap();
        assert!(
            catalog.get_rights(&[a]).unwrap().is_empty(),
            "undo restores a"
        );
        assert_eq!(
            catalog.get_rights(&[b]).unwrap()[&b],
            later,
            "undo leaves rights edited after the apply alone"
        );

        catalog.set_rights(&[(a, later.clone())]).unwrap();
        catalog.replay_template_change(&change, false).unwrap();
        assert_eq!(
            catalog.get_rights(&[a]).unwrap()[&a],
            later,
            "redo leaves rights edited after the undo alone"
        );
    }

    #[test]
    fn template_keywords_keep_search_index_in_sync() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog.set_caption(a, "sunset").unwrap();
        catalog.add_tags_batch(&[a], &["kyoto".into()]).unwrap();
        let change = catalog
            .apply_template(&[a], &Rights::default(), &["zebrafoo".into()])
            .unwrap();
        assert_eq!(search(&catalog, "zebrafoo"), vec![a], "apply indexes it");
        assert_eq!(search(&catalog, "sunset"), vec![a]);
        catalog.replay_template_change(&change, true).unwrap();
        assert!(search(&catalog, "zebrafoo").is_empty(), "undo unindexes it");
        assert_eq!(search(&catalog, "sunset"), vec![a], "the caption stays");
        catalog.replay_template_change(&change, false).unwrap();
        assert_eq!(
            search(&catalog, "zebrafoo"),
            vec![a],
            "redo indexes it again"
        );
        assert_eq!(search(&catalog, "sunset"), vec![a]);
        // A stale index row (an old one never deleted) would keep a removed
        // keyword findable.
        catalog.remove_tags_batch(&[a], &["kyoto".into()]).unwrap();
        assert!(
            search(&catalog, "kyoto").is_empty(),
            "every index row the apply and replays replaced was deleted"
        );
    }
}
