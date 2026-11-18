//! What a duplicate merge changed on the keeper, so undo can put it back.

use std::collections::HashSet;

use anyhow::{Context, Result};
use rusqlite::{params, Transaction};

use super::Catalog;
use crate::MediaId;

/// The keeper's user metadata before a duplicate merge copied the trashed
/// copies' onto it (`Catalog::merge_metadata_in_tx`). Undo restores the
/// copies from the trash and hands this to
/// [`Catalog::restore_merged_keepers`].
#[derive(Debug, Clone, PartialEq)]
pub struct KeeperSnapshot {
    keep_id: MediaId,
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

/// One merge's undo record: the copies it trashed and each keeper as it was.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DuplicateMerge {
    pub trashed: Vec<MediaId>,
    pub keepers: Vec<KeeperSnapshot>,
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
    /// Undoes the metadata side of duplicate merges: each keeper gets back
    /// what it had before ([`KeeperSnapshot`]), in one transaction. Putting
    /// the trashed copies back is the caller's separate restore from trash.
    pub fn restore_merged_keepers(&self, keepers: &[KeeperSnapshot]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for snapshot in keepers.iter().rev() {
            snapshot.restore(&tx)?;
        }
        tx.commit()?;
        Ok(())
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

        catalog.restore_merged_keepers(&[snapshot]).unwrap();
        catalog.set_trashed(dup, false).unwrap();

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
        assert_eq!(merge.trashed, vec![a2]);
        assert_eq!(catalog.get_ratings_map().unwrap().get(&a1), Some(&4));
        catalog.restore_merged_keepers(&merge.keepers).unwrap();
        assert_eq!(
            catalog.get_ratings_map().unwrap().get(&a1),
            None,
            "the keeper's rating is back to unrated"
        );
    }
}
