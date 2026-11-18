//! What a duplicate merge changed on the keeper, so undo can put it back.

use std::collections::HashSet;

use anyhow::{Context, Result};
use rusqlite::{params, Transaction};

use super::Catalog;
use crate::MediaId;

/// The keeper's user metadata before a duplicate merge copied the trashed
/// copies' onto it (`Catalog::merge_metadata_in_tx`), and which copies
/// those were. [`Catalog::undo_duplicate_merge`] puts it back;
/// [`Catalog::redo_duplicate_merge`] merges the same copies again.
#[derive(Debug, Clone, PartialEq)]
pub struct KeeperSnapshot {
    keep_id: MediaId,
    /// The copies merged into the keeper and moved to Recently Deleted.
    copies: Vec<MediaId>,
    is_favorite: i64,
    rating: i64,
    color_label: i64,
    title: Option<String>,
    flag: i64,
    rotation_quarters: i64,
    tags: Vec<String>,
    albums: Vec<i64>,
    had_caption: bool,
    had_edits: bool,
    had_rights: bool,
    /// `(album, merged copy)`: albums whose key photo was a merged copy.
    covers: Vec<(i64, MediaId)>,
}

/// One merge's undo record: each keeper as it was, with its copies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DuplicateMerge {
    pub keepers: Vec<KeeperSnapshot>,
}

impl DuplicateMerge {
    /// Every copy the merge moved to Recently Deleted.
    pub fn trashed(&self) -> Vec<MediaId> {
        self.keepers
            .iter()
            .flat_map(|k| k.copies.iter().copied())
            .collect()
    }
}

/// What [`Catalog::undo_duplicate_merge`] did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MergeUndo {
    /// The keepers put back, their copies restored: what a redo merges
    /// again.
    pub restored: DuplicateMerge,
    /// Keepers left merged because a copy was permanently deleted since:
    /// the merged-in metadata is all that is left of it.
    pub kept_merged: usize,
}

impl KeeperSnapshot {
    /// The keeper this snapshot restores.
    pub fn keep_id(&self) -> MediaId {
        self.keep_id
    }

    /// Reads `keep_id`'s metadata before `trash_ids` merge into it.
    pub(crate) fn take(tx: &Transaction, keep_id: MediaId, trash_ids: &[MediaId]) -> Result<Self> {
        let keep = keep_id.0 as i64;
        let exists = |sql: &str| -> Result<bool> { Ok(tx.prepare_cached(sql)?.exists([keep])?) };
        let list = |sql: &str| -> Result<Vec<String>> {
            let mut stmt = tx.prepare_cached(sql)?;
            let rows = stmt.query_map([keep], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        };
        let mut snapshot = tx
            .query_row(
                "SELECT is_favorite, rating, color_label, title, flag, rotation_quarters \
                 FROM media_items WHERE id = ?1;",
                [keep],
                |row| {
                    Ok(Self {
                        keep_id,
                        copies: trash_ids.to_vec(),
                        is_favorite: row.get(0)?,
                        rating: row.get(1)?,
                        color_label: row.get(2)?,
                        title: row.get(3)?,
                        flag: row.get(4)?,
                        rotation_quarters: row.get(5)?,
                        tags: Vec::new(),
                        albums: Vec::new(),
                        had_caption: false,
                        had_edits: false,
                        had_rights: false,
                        covers: Vec::new(),
                    })
                },
            )
            .context("reading the duplicate keeper before the merge")?;
        snapshot.tags = list("SELECT tag FROM media_tags WHERE media_id = ?1;")?;
        snapshot.albums = {
            let mut stmt =
                tx.prepare_cached("SELECT album_id FROM album_items WHERE media_id = ?1;")?;
            let rows = stmt.query_map([keep], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        snapshot.had_caption = exists("SELECT 1 FROM media_captions WHERE media_id = ?1;")?;
        snapshot.had_edits = exists("SELECT 1 FROM media_edits WHERE media_id = ?1;")?;
        snapshot.had_rights = exists("SELECT 1 FROM media_rights WHERE media_id = ?1;")?;
        let mut covers = tx.prepare_cached("SELECT id FROM albums WHERE cover_media_id = ?1;")?;
        for trash_id in trash_ids {
            let rows = covers.query_map([trash_id.0 as i64], |row| row.get(0))?;
            for album in rows {
                snapshot.covers.push((album?, *trash_id));
            }
        }
        Ok(snapshot)
    }

    /// Puts the keeper back as it was: the fields, and removes the tags,
    /// albums, caption, edits and rights the merge added. Same search index
    /// order as the merge: the media_items UPDATE (its trigger reindexes
    /// with the current tags and caption) runs before the index row is
    /// removed, and nothing touches media_items until it is rebuilt.
    fn restore(&self, tx: &Transaction) -> Result<()> {
        let keep = self.keep_id.0 as i64;
        tx.execute(
            "UPDATE media_items SET is_favorite = ?1, rating = ?2, color_label = ?3, \
             title = ?4, flag = ?5, rotation_quarters = ?6 WHERE id = ?7;",
            params![
                self.is_favorite,
                self.rating,
                self.color_label,
                self.title,
                self.flag,
                self.rotation_quarters,
                keep
            ],
        )?;
        if !self.had_edits {
            tx.execute("DELETE FROM media_edits WHERE media_id = ?1;", [keep])?;
        }
        if !self.had_rights {
            tx.execute("DELETE FROM media_rights WHERE media_id = ?1;", [keep])?;
        }

        Catalog::delete_fts_for_items(tx, &[keep])?;
        let tags: HashSet<&str> = self.tags.iter().map(String::as_str).collect();
        let current: Vec<String> = {
            let mut stmt = tx.prepare_cached("SELECT tag FROM media_tags WHERE media_id = ?1;")?;
            let rows = stmt.query_map([keep], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut drop_tag =
            tx.prepare_cached("DELETE FROM media_tags WHERE media_id = ?1 AND tag = ?2;")?;
        for tag in current.iter().filter(|tag| !tags.contains(tag.as_str())) {
            drop_tag.execute(params![keep, tag])?;
        }
        if !self.had_caption {
            tx.execute("DELETE FROM media_captions WHERE media_id = ?1;", [keep])?;
        }
        Catalog::insert_fts_for_items(tx, &[keep])?;

        let albums: HashSet<i64> = self.albums.iter().copied().collect();
        let current: Vec<i64> = {
            let mut stmt =
                tx.prepare_cached("SELECT album_id FROM album_items WHERE media_id = ?1;")?;
            let rows = stmt.query_map([keep], |row| row.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut leave =
            tx.prepare_cached("DELETE FROM album_items WHERE album_id = ?1 AND media_id = ?2;")?;
        for album in current.iter().filter(|album| !albums.contains(album)) {
            leave.execute(params![album, keep])?;
        }
        let mut cover = tx.prepare_cached(
            "UPDATE albums SET cover_media_id = ?1 WHERE id = ?2 AND cover_media_id = ?3;",
        )?;
        for (album, copy) in &self.covers {
            cover.execute(params![copy.0 as i64, album, keep])?;
        }
        Ok(())
    }
}

impl Catalog {
    /// Undoes duplicate merges in one transaction: each keeper gets back
    /// what it had before ([`KeeperSnapshot`]) and its copies leave
    /// Recently Deleted. A keeper with a copy permanently deleted since
    /// stays merged, copies and all: its merged-in metadata is the only
    /// place that copy's tags, albums and edits still live.
    pub fn undo_duplicate_merge(&self, merge: &DuplicateMerge) -> Result<MergeUndo> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut undo = MergeUndo::default();
        {
            let mut exists = tx.prepare_cached("SELECT 1 FROM media_items WHERE id = ?1;")?;
            let mut untrash =
                tx.prepare_cached("UPDATE media_items SET is_trashed = 0 WHERE id = ?1;")?;
            for snapshot in merge.keepers.iter().rev() {
                let mut all_there = true;
                for copy in &snapshot.copies {
                    all_there &= exists.exists([copy.0 as i64])?;
                }
                if !all_there {
                    undo.kept_merged += 1;
                    continue;
                }
                snapshot.restore(&tx)?;
                for copy in &snapshot.copies {
                    untrash.execute([copy.0 as i64])?;
                }
                undo.restored.keepers.push(snapshot.clone());
            }
        }
        undo.restored.keepers.reverse();
        if !undo.restored.keepers.is_empty() {
            Self::rebuild_timeline_buckets(&tx)?;
        }
        tx.commit()?;
        Ok(undo)
    }

    /// Redoes an undone merge in one transaction: each keeper takes its
    /// copies' metadata again and they go back to Recently Deleted, so a
    /// later Empty Recently Deleted cannot lose it. Returns the fresh
    /// record for the next undo. A keeper deleted since is skipped, as are
    /// copies deleted since.
    pub fn redo_duplicate_merge(&self, merge: &DuplicateMerge) -> Result<DuplicateMerge> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut redone = DuplicateMerge::default();
        for snapshot in &merge.keepers {
            let present: Vec<MediaId> = {
                let mut exists = tx.prepare_cached("SELECT 1 FROM media_items WHERE id = ?1;")?;
                if !exists.exists([snapshot.keep_id.0 as i64])? {
                    continue;
                }
                let mut present = Vec::new();
                for copy in &snapshot.copies {
                    if exists.exists([copy.0 as i64])? {
                        present.push(*copy);
                    }
                }
                present
            };
            if !present.is_empty() {
                redone.keepers.push(Self::merge_duplicate_group_in_tx(
                    &tx,
                    snapshot.keep_id,
                    &present,
                )?);
            }
        }
        if !redone.keepers.is_empty() {
            Self::rebuild_timeline_buckets(&tx)?;
        }
        tx.commit()?;
        Ok(redone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::video_item;
    use crate::catalog::{Edits, Rights};
    use crate::search::StructuredQuery;
    use crate::Flag;
    use std::collections::HashMap;

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[video_item(path)]).unwrap()[0].id
    }

    /// The keeper's state as a snapshot sees it, read outside any merge.
    fn state(catalog: &Catalog, keep: MediaId, copies: &[MediaId]) -> KeeperSnapshot {
        let mut conn = catalog.conn.lock();
        let tx = conn.transaction().unwrap();
        let mut snapshot = KeeperSnapshot::take(&tx, keep, copies).unwrap();
        snapshot.tags.sort();
        snapshot.albums.sort();
        snapshot
    }

    fn search(catalog: &Catalog, text: &str) -> Vec<MediaId> {
        catalog
            .search_structured(&StructuredQuery::parse(text), 10, 0)
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect()
    }

    /// Regression: undoing a merge put the trashed copies back but left the
    /// metadata merged onto the keeper.
    #[test]
    fn restore_puts_the_keeper_back_as_before_the_merge() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.add_tags_batch(&[keep], &["mine".into()]).unwrap();
        catalog.set_rating(keep, 2).unwrap();
        let own = catalog.create_album_with("Own", &[keep]).unwrap();

        catalog.add_tags_batch(&[dup], &["beach".into()]).unwrap();
        catalog.batch_set_favorite(&[dup], true).unwrap();
        catalog.set_rating(dup, 5).unwrap();
        catalog.set_color_label(dup, 3).unwrap();
        catalog.set_title(dup, Some("Dune")).unwrap();
        catalog.set_flags(&[(dup, Flag::Pick)]).unwrap();
        catalog.set_caption(dup, "sunset").unwrap();
        let edits = Edits {
            rotation: 1,
            exposure_ev: 0.5,
            ..Default::default()
        };
        catalog.set_edits(&[(dup, edits)]).unwrap();
        let rights = Rights {
            creator: Some("Ana".into()),
            ..Default::default()
        };
        catalog.set_rights(&[(dup, rights)]).unwrap();
        let theirs = catalog.create_album_with("Theirs", &[dup]).unwrap();
        catalog.set_album_cover(theirs.id, Some(dup)).unwrap();

        let before = state(&catalog, keep, &[dup]);
        let snapshot = catalog
            .merge_duplicate_group(keep, &[dup])
            .unwrap()
            .expect("a merge returns the keeper's snapshot");
        assert_eq!(snapshot.keep_id(), keep);
        assert_ne!(
            state(&catalog, keep, &[dup]),
            before,
            "control: the merge changed the keeper"
        );
        assert_eq!(search(&catalog, "sunset"), vec![keep], "control: merged");

        let undo = catalog
            .undo_duplicate_merge(&DuplicateMerge {
                keepers: vec![snapshot],
            })
            .unwrap();
        assert_eq!(undo.kept_merged, 0, "every copy is still there");
        assert!(
            !catalog.get_item_by_id(dup).unwrap().unwrap().is_trashed,
            "the same undo takes the copy out of Recently Deleted"
        );

        assert_eq!(
            state(&catalog, keep, &[dup]),
            before,
            "fields, tags, albums, caption, edits, rights and key photos are back"
        );
        assert_eq!(
            catalog
                .list_albums_with_counts()
                .unwrap()
                .iter()
                .find(|(album, _)| album.id == theirs.id)
                .and_then(|(album, _)| album.cover_media_id),
            Some(dup),
            "the copy's album shows the copy again"
        );
        assert_eq!(
            search(&catalog, "sunset"),
            vec![dup],
            "the merged caption leaves the keeper's index row"
        );
        assert_eq!(
            search(&catalog, "beach"),
            vec![dup],
            "only the copy has its tag"
        );
        assert_eq!(
            search(&catalog, "mine"),
            vec![keep],
            "the keeper's own tag stays"
        );
        let _ = own;
    }

    #[test]
    fn merge_all_returns_a_snapshot_per_keeper() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a1 = plain(&catalog, "/lib/a1.mp4");
        let a2 = plain(&catalog, "/lib/a2.mp4");
        catalog.set_rating(a2, 4).unwrap();
        catalog
            .batch_insert_hashes(&[(a1, 1_000, "a".into()), (a2, 1_000, "a".into())])
            .unwrap();
        let merge = catalog
            .merge_all_duplicates(&HashMap::from([("a".to_string(), a1)]), &HashSet::new())
            .unwrap();
        assert_eq!(merge.trashed(), vec![a2]);
        assert_eq!(catalog.get_ratings_map().unwrap().get(&a1), Some(&4));
        catalog.undo_duplicate_merge(&merge).unwrap();
        assert_eq!(
            catalog.get_ratings_map().unwrap().get(&a1),
            None,
            "the keeper's rating is back to unrated"
        );
    }

    /// Regression: undo after Empty Recently Deleted stripped the purged
    /// copy's tag from the keeper and restored nothing, losing it for good.
    #[test]
    fn undo_leaves_a_keeper_merged_when_its_copy_was_purged() {
        let catalog = Catalog::open_in_memory().unwrap();
        let (a_keep, a_dup) = (
            plain(&catalog, "/lib/a1.mp4"),
            plain(&catalog, "/lib/a2.mp4"),
        );
        let (b_keep, b_dup) = (
            plain(&catalog, "/lib/b1.mp4"),
            plain(&catalog, "/lib/b2.mp4"),
        );
        catalog.add_tags_batch(&[a_dup], &["beach".into()]).unwrap();
        catalog.add_tags_batch(&[b_dup], &["snow".into()]).unwrap();
        let mut merge = DuplicateMerge::default();
        for (keep, dup) in [(a_keep, a_dup), (b_keep, b_dup)] {
            let snapshot = catalog.merge_duplicate_group(keep, &[dup]).unwrap();
            merge.keepers.extend(snapshot);
        }
        catalog
            .permanently_delete_items(&[a_dup], crate::catalog::DeletionTarget::CatalogOnly)
            .unwrap();

        let undo = catalog.undo_duplicate_merge(&merge).unwrap();
        assert_eq!(undo.kept_merged, 1, "group a stays merged");
        assert_eq!(
            catalog.get_tags_for_item(a_keep).unwrap(),
            vec!["beach".to_string()],
            "the purged copy's tag stays on its keeper"
        );
        assert!(
            catalog.get_tags_for_item(b_keep).unwrap().is_empty(),
            "group b, whose copy is still there, is undone"
        );
        assert!(!catalog.get_item_by_id(b_dup).unwrap().unwrap().is_trashed);
        assert_eq!(
            undo.restored.trashed(),
            vec![b_dup],
            "redo merges only what the undo restored"
        );
    }

    /// Regression: redo only trashed the copies again, so the keeper lacked
    /// their metadata and a later Empty Recently Deleted lost it.
    #[test]
    fn redo_merges_the_copies_into_the_keeper_again() {
        let catalog = Catalog::open_in_memory().unwrap();
        let keep = plain(&catalog, "/lib/keep.mp4");
        let dup = plain(&catalog, "/lib/dup.mp4");
        catalog.add_tags_batch(&[dup], &["beach".into()]).unwrap();
        let merge = DuplicateMerge {
            keepers: catalog
                .merge_duplicate_group(keep, &[dup])
                .unwrap()
                .into_iter()
                .collect(),
        };
        let undone = catalog.undo_duplicate_merge(&merge).unwrap().restored;
        assert!(catalog.get_tags_for_item(keep).unwrap().is_empty());

        let redone = catalog.redo_duplicate_merge(&undone).unwrap();
        assert!(catalog.get_item_by_id(dup).unwrap().unwrap().is_trashed);
        assert_eq!(
            catalog.get_tags_for_item(keep).unwrap(),
            vec!["beach".to_string()],
            "redo copies the tag onto the keeper again"
        );
        catalog
            .permanently_delete_items(&[dup], crate::catalog::DeletionTarget::CatalogOnly)
            .unwrap();
        assert_eq!(
            catalog.get_tags_for_item(keep).unwrap(),
            vec!["beach".to_string()],
            "emptying Recently Deleted after a redo keeps the tag"
        );
        assert_eq!(redone.trashed(), vec![dup]);
        let again = catalog.undo_duplicate_merge(&redone).unwrap();
        assert_eq!(
            again.kept_merged, 1,
            "a fresh record: the next undo sees the purge"
        );
    }
}
