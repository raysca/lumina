//! Tests for the parent module, kept beside it.

use super::*;
use crate::{MediaItem, MediaType};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A temp library dir, removed on drop.
pub(crate) struct TempDir(pub PathBuf);

impl TempDir {
    pub(crate) fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("lumina_{prefix}_{nanos}"));
        std::fs::create_dir_all(&dir).expect("temp dir must be creatable");
        // Canonical, so paths match what the OS reports (/private/var).
        Self(dir.canonicalize().expect("temp dir must canonicalize"))
    }

    /// Writes `bytes` at `rel`, creating parent folders.
    pub(crate) fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().expect("file has a parent"))
            .expect("parent dir must be creatable");
        std::fs::write(&path, bytes).expect("file must be writable");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A catalog item for an existing file at `path`.
pub(crate) fn item_at(path: &Path) -> MediaItem {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    MediaItem {
        id: MediaId(0),
        path: path.to_path_buf(),
        parent_dir: path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        file_name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        title: None,
        file_size,
        mtime_epoch_ms: 1_642_200_000_000,
        media_type: MediaType::Image,
        width: 10,
        height: 10,
        duration_ms: None,
        timestamp_epoch_ms: 1_642_200_000_000,
        year: 2022,
        month: 1,
        day: 15,
        latitude: None,
        longitude: None,
        altitude: None,
        country_code: None,
        admin1_region: None,
        city: None,
        make: None,
        model: None,
        lens: None,
        focal_length: None,
        f_number: None,
        iso: None,
        exposure_time: None,
        is_favorite: false,
        is_hidden: false,
        is_trashed: false,
    }
}

/// Marks `id` missing directly, for tests of what follows.
pub(crate) fn set_missing(catalog: &Catalog, id: MediaId) {
    catalog
        .conn
        .lock()
        .execute(
            "INSERT INTO media_file_state (media_id, missing_since_epoch_ms) VALUES (?1, 1)
                 ON CONFLICT(media_id) DO UPDATE SET missing_since_epoch_ms = 1;",
            [id.0 as i64],
        )
        .expect("missing state must be writable");
}

fn stored_inode(catalog: &Catalog, id: MediaId) -> Option<i64> {
    catalog
        .conn
        .lock()
        .query_row(
            "SELECT file_inode FROM media_file_state WHERE media_id = ?1;",
            [id.0 as i64],
            |row| row.get(0),
        )
        .expect("state row must exist")
}

fn ingest(catalog: &Catalog, path: &Path) -> MediaItem {
    catalog
        .insert_or_update_batch(&[item_at(path)])
        .expect("upsert must succeed")
        .remove(0)
}

#[test]
fn upsert_records_inode() {
    let dir = TempDir::new("inode");
    let path = dir.write("a.jpg", b"a");
    let catalog = Catalog::open_in_memory().unwrap();
    let item = ingest(&catalog, &path);
    assert_eq!(
        stored_inode(&catalog, item.id),
        file_inode(&path).map(|i| i as i64),
        "ingest must record the file's inode"
    );
    assert!(catalog.get_missing_ids().unwrap().is_empty());
}

#[test]
fn upsert_reconnects_missing_row_by_inode() {
    let dir = TempDir::new("reconnect");
    let old = dir.write("a.jpg", b"same bytes");
    let catalog = Catalog::open_in_memory().unwrap();
    let item = ingest(&catalog, &old);
    catalog
        .add_tags_batch(&[item.id], &["kept".to_string()])
        .unwrap();
    set_missing(&catalog, item.id);

    let new = dir.0.join("sub/b.jpg");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::rename(&old, &new).unwrap();
    let moved = ingest(&catalog, &new);

    assert_eq!(moved.id, item.id, "a moved file must keep its row");
    assert_eq!(catalog.total_count().unwrap(), 1, "no second row");
    let row = catalog.get_item_by_id(item.id).unwrap().unwrap();
    assert_eq!(row.path, new);
    assert_eq!(row.file_name, "b.jpg");
    assert_eq!(catalog.get_tags_for_item(item.id).unwrap(), vec!["kept"]);
    assert!(
        catalog.get_missing_ids().unwrap().is_empty(),
        "a reconnected row is present"
    );
}

#[test]
fn upsert_ignores_inode_match_with_different_size() {
    let dir = TempDir::new("reconnect_size");
    let path = dir.write("a.jpg", b"short");
    let catalog = Catalog::open_in_memory().unwrap();
    let item = ingest(&catalog, &path);
    set_missing(&catalog, item.id);

    let new = dir.0.join("b.jpg");
    std::fs::rename(&path, &new).unwrap();
    std::fs::write(&new, b"a different length").unwrap();
    let other = ingest(&catalog, &new);

    assert_ne!(other.id, item.id, "same inode, new size is another file");
    assert!(catalog.get_missing_ids().unwrap().contains(&item.id));
}

#[test]
fn signatures_exclude_missing_rows() {
    let dir = TempDir::new("signatures");
    let a = dir.write("a.jpg", b"a");
    let b = dir.write("b.jpg", b"b");
    let catalog = Catalog::open_in_memory().unwrap();
    let item = ingest(&catalog, &a);
    ingest(&catalog, &b);
    set_missing(&catalog, item.id);

    let signatures = catalog.get_file_signatures().unwrap();
    assert!(!signatures.contains_key(&a), "missing rows must be re-read");
    assert!(signatures.contains_key(&b));
}

#[test]
fn mark_missing_under_folder_spares_siblings() {
    let dir = TempDir::new("mark_folder");
    let catalog = Catalog::open_in_memory().unwrap();
    let a = ingest(&catalog, &dir.write("dir/a.jpg", b"a")).id;
    let b = ingest(&catalog, &dir.write("dir/sub/b.jpg", b"bb")).id;
    let c = ingest(&catalog, &dir.write("dir 2/c.jpg", b"ccc")).id;
    let d = ingest(&catalog, &dir.write("dir.old/d.jpg", b"dddd")).id;
    let e = ingest(&catalog, &dir.write("dir0/e.jpg", b"eeeee")).id;

    let change = catalog.mark_missing_under(&dir.0.join("dir")).unwrap();

    let mut missing = change.missing.clone();
    missing.sort_by_key(|id| id.0);
    assert_eq!(missing, vec![a, b], "only rows under dir/ go missing");
    assert!(change.folded.is_empty());
    let all = catalog.get_missing_ids().unwrap();
    for sibling in [c, d, e] {
        assert!(!all.contains(&sibling), "sibling folders are untouched");
    }
}

#[test]
fn mark_missing_keeps_row_and_metadata() {
    let dir = TempDir::new("mark_file");
    let path = dir.write("a.jpg", b"a");
    let catalog = Catalog::open_in_memory().unwrap();
    let id = ingest(&catalog, &path).id;
    catalog
        .add_tags_batch(&[id], &["kept".to_string()])
        .unwrap();
    catalog.set_rating(id, 4).unwrap();

    let change = catalog.mark_missing_under(&path).unwrap();

    assert_eq!(change.missing, vec![id]);
    assert_eq!(catalog.total_count().unwrap(), 1, "the row is kept");
    assert_eq!(catalog.get_tags_for_item(id).unwrap(), vec!["kept"]);
    assert_eq!(catalog.get_rating(id).unwrap(), 4);
    assert!(
        catalog
            .mark_missing_under(&path)
            .unwrap()
            .missing
            .is_empty(),
        "a missing row is not marked twice"
    );
}

#[test]
fn split_rename_folds_new_row() {
    let dir = TempDir::new("fold");
    let old = dir.write("a.jpg", b"same");
    let catalog = Catalog::open_in_memory().unwrap();
    let a = ingest(&catalog, &old).id;
    catalog.add_tags_batch(&[a], &["kept".to_string()]).unwrap();
    // The new path is seen first: a second row with the same inode.
    let new = dir.0.join("b.jpg");
    std::fs::rename(&old, &new).unwrap();
    let b = ingest(&catalog, &new).id;
    catalog.set_rating(b, 3).unwrap();
    assert_ne!(a, b);

    let change = catalog.mark_missing_under(&old).unwrap();

    assert_eq!(
        change.folded,
        vec![(a, b)],
        "the new row folds into the old"
    );
    assert!(change.missing.is_empty());
    assert_eq!(catalog.total_count().unwrap(), 1);
    let row = catalog.get_item_by_id(a).unwrap().unwrap();
    assert_eq!(row.path, new, "the old row takes the new path");
    assert_eq!(catalog.get_tags_for_item(a).unwrap(), vec!["kept"]);
    assert_eq!(
        catalog.get_rating(a).unwrap(),
        3,
        "the new row's edits merge"
    );
    assert!(catalog.get_missing_ids().unwrap().is_empty());
}

#[test]
fn fold_keeps_the_other_rows_edits() {
    let dir = TempDir::new("fold_edits");
    let catalog = Catalog::open_in_memory().unwrap();
    let keep = ingest(&catalog, &dir.write("a.jpg", b"a")).id;
    let other = ingest(&catalog, &dir.write("b.jpg", b"b")).id;
    catalog.set_rotations(&[(other, 1)]).unwrap();
    // 2001-09-09 UTC
    catalog.set_timestamp(other, 1_000_000_000_000).unwrap();
    catalog
        .set_location(other, Some(51.5), Some(-0.12))
        .unwrap();

    catalog.fold_into(keep, other).unwrap();

    assert_eq!(
        catalog.get_rotation(keep).unwrap(),
        1,
        "rotation carries over"
    );
    let row = catalog.get_item_by_id(keep).unwrap().unwrap();
    assert_eq!(
        row.timestamp_epoch_ms, 1_000_000_000_000,
        "an edited date carries over"
    );
    assert_eq!(
        (row.year, row.month, row.day),
        (2001, 9, 9),
        "the date's parts carry over with it"
    );
    assert_eq!(
        (row.latitude, row.longitude),
        (Some(51.5), Some(-0.12)),
        "an edited place carries over"
    );
    let years: Vec<String> = catalog
        .get_timeline_buckets("year")
        .unwrap()
        .into_iter()
        .map(|bucket| bucket.bucket_key)
        .collect();
    assert!(
        years.contains(&"2001".to_string()),
        "buckets follow the new date, got {years:?}"
    );
    assert_eq!(row.path, dir.0.join("b.jpg"));
    assert!(catalog.get_item_by_id(other).unwrap().is_none());

    let hidden = ingest(&catalog, &dir.write("c.jpg", b"c")).id;
    catalog
        .conn
        .lock()
        .execute(
            "UPDATE media_items SET is_hidden = 1 WHERE id = ?1;",
            [hidden.0 as i64],
        )
        .unwrap();
    catalog.fold_into(keep, hidden).unwrap();
    let row = catalog.get_item_by_id(keep).unwrap().unwrap();
    assert!(row.is_hidden, "hidden carries over");
}

#[test]
fn fold_into_keeps_pick() {
    let dir = TempDir::new("fold_flags");
    let catalog = Catalog::open_in_memory().unwrap();
    let keep = ingest(&catalog, &dir.write("a.jpg", b"a")).id;
    let other = ingest(&catalog, &dir.write("b.jpg", b"b")).id;
    catalog
        .set_flags(&[(keep, crate::Flag::Reject), (other, crate::Flag::Pick)])
        .unwrap();

    catalog.fold_into(keep, other).unwrap();

    assert_eq!(
        catalog.get_flags_map().unwrap(),
        std::collections::HashMap::from([(keep, crate::Flag::Pick)]),
        "fold deletes `other`, so its pick must move to `keep` first"
    );
}

#[test]
fn fold_into_carries_album_membership() {
    let dir = TempDir::new("fold_albums");
    let catalog = Catalog::open_in_memory().unwrap();
    let keep = ingest(&catalog, &dir.write("a.jpg", b"a")).id;
    let other = ingest(&catalog, &dir.write("b.jpg", b"b")).id;
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[other]).unwrap();

    catalog.fold_into(keep, other).unwrap();

    assert!(catalog.get_item_by_id(other).unwrap().is_none());
    assert_eq!(
        crate::catalog::tests_support::album_members(&catalog, album.id),
        vec![keep],
        "fold deletes `other`, so its membership must move to `keep` first"
    );
}

#[test]
fn deleted_folder_with_hard_links_marks_both_missing() {
    let dir = TempDir::new("hardlink_dir");
    let a_path = dir.write("sub/a.jpg", b"same");
    let b_path = dir.0.join("sub/b.jpg");
    std::fs::hard_link(&a_path, &b_path).unwrap();
    let catalog = Catalog::open_in_memory().unwrap();
    let a = ingest(&catalog, &a_path).id;
    let b = ingest(&catalog, &b_path).id;
    std::fs::remove_dir_all(dir.0.join("sub")).unwrap();

    let mut change = catalog.mark_missing_under(&dir.0.join("sub")).unwrap();
    change.missing.sort_by_key(|id| id.0);

    assert_eq!(
        change.missing,
        vec![a, b],
        "links gone with their folder are both missing, not folded"
    );
    assert!(change.folded.is_empty());
}

#[test]
fn old_present_twin_is_not_folded() {
    let dir = TempDir::new("hardlink");
    let a_path = dir.write("a.jpg", b"same");
    let b_path = dir.0.join("b.jpg");
    std::fs::hard_link(&a_path, &b_path).unwrap();
    let catalog = Catalog::open_in_memory().unwrap();
    let a = ingest(&catalog, &a_path).id;
    let b = ingest(&catalog, &b_path).id;
    catalog
        .conn
        .lock()
        .execute(
            "UPDATE media_items SET added_epoch_ms = added_epoch_ms - 120000;",
            [],
        )
        .unwrap();
    std::fs::remove_file(&a_path).unwrap();

    let change = catalog.mark_missing_under(&a_path).unwrap();

    assert_eq!(
        change.missing,
        vec![a],
        "a long-standing hard link is not a rename"
    );
    assert!(change.folded.is_empty());
    assert!(catalog.get_item_by_id(b).unwrap().is_some());
}

#[test]
fn reconcile_marks_gone_files_and_backfills_inodes() {
    let dir = TempDir::new("reconcile");
    let kept = dir.write("kept.jpg", b"k");
    let gone = dir.write("gone.jpg", b"g");
    let catalog = Catalog::open_in_memory().unwrap();
    let kept_id = ingest(&catalog, &kept).id;
    let gone_id = ingest(&catalog, &gone).id;
    // As a catalog from before v12: no file state at all.
    catalog
        .conn
        .lock()
        .execute("DELETE FROM media_file_state;", [])
        .unwrap();
    std::fs::remove_file(&gone).unwrap();

    let missing = catalog.reconcile_files().unwrap();

    assert_eq!(missing, vec![gone_id]);
    assert_eq!(
        stored_inode(&catalog, kept_id),
        file_inode(&kept).map(|i| i as i64),
        "a present file's inode is backfilled"
    );
    assert!(catalog.reconcile_files().unwrap().is_empty(), "idempotent");
}

#[test]
fn missing_rows_are_not_duplicates() {
    let dir = TempDir::new("dup_missing");
    let catalog = Catalog::open_in_memory().unwrap();
    let a = ingest(&catalog, &dir.write("a.jpg", b"identical")).id;
    let b = ingest(&catalog, &dir.write("b.jpg", b"identical")).id;
    catalog
        .batch_insert_hashes(&[(a, 9, "h".into()), (b, 9, "h".into())])
        .unwrap();
    assert_eq!(catalog.query_duplicate_items_count().unwrap(), 2);

    set_missing(&catalog, b);

    assert_eq!(catalog.query_duplicate_items_count().unwrap(), 0);
    assert!(catalog.query_duplicate_groups().unwrap().is_empty());
    assert_eq!(
        catalog.find_items_by_hash("h").unwrap().len(),
        1,
        "an import never matches a missing row"
    );
    assert_eq!(catalog.find_candidates_by_size(9).unwrap().len(), 1);
}

#[test]
fn v11_catalog_migrates_to_file_state() {
    let dir = TempDir::new("migration");
    let db = dir.0.join("catalog.db");
    let path = dir.write("a.jpg", b"a");
    {
        let catalog = Catalog::open(&db).unwrap();
        ingest(&catalog, &path);
        // Back to a v11 catalog: no file state table.
        let conn = catalog.conn.lock();
        conn.execute_batch("DROP TABLE media_file_state; PRAGMA user_version = 11;")
            .unwrap();
    }
    for _ in 0..2 {
        let catalog = Catalog::open(&db).unwrap();
        assert_eq!(catalog.schema_version().unwrap(), crate::SCHEMA_VERSION);
        assert_eq!(catalog.total_count().unwrap(), 1, "migration keeps rows");
        assert!(
            catalog.get_missing_ids().unwrap().is_empty(),
            "rows from before v12 count as present"
        );
    }
}

#[test]
fn inode_lookups_use_the_index() {
    let catalog = Catalog::open_in_memory().unwrap();
    let plan = catalog
        .explain_query_plan(
            "SELECT m.id FROM media_items m JOIN media_file_state s ON s.media_id = m.id \
                 WHERE s.file_inode = 1 AND m.file_size = 1 AND m.id != 1 \
                 AND s.missing_since_epoch_ms IS NULL AND m.added_epoch_ms >= 0",
        )
        .unwrap()
        .join(" | ");
    assert!(
        plan.contains("idx_file_state_inode"),
        "marking a folder missing must not scan the table per file: {plan}"
    );
}

#[cfg(unix)]
#[test]
fn a_non_utf8_filename_round_trips_through_the_catalog() {
    use std::os::unix::ffi::OsStrExt;

    let dir = TempDir::new("non_utf8");
    let catalog = Catalog::open_in_memory().unwrap();
    catalog.ensure_library_root(&dir.0).unwrap();
    let path = dir.0.join(std::ffi::OsStr::from_bytes(b"photo-\xff.jpg"));
    // No file on disk: macOS refuses to create such a name, and the
    // catalog's encoding is what is under test.
    let item = ingest(&catalog, &path);

    let got = catalog.get_item_by_id(item.id).unwrap().unwrap();
    assert_eq!(
        got.path.as_os_str().as_bytes(),
        path.as_os_str().as_bytes(),
        "the exact bytes survive the round trip"
    );
    assert_eq!(
        catalog.id_at_path(&path).unwrap(),
        Some(item.id),
        "lookup by the same bytes finds the row"
    );
    assert!(
        catalog
            .present_signatures(std::slice::from_ref(&path))
            .unwrap()
            .contains_key(&path),
        "the batch signature lookup uses the same spelling"
    );
}

#[test]
fn a_folded_rename_keeps_the_new_root_relative_path() {
    let dir = TempDir::new("fold_root");
    let catalog = Catalog::open_in_memory().unwrap();
    catalog.ensure_library_root(&dir.0).unwrap();
    let old = dir.write("a.jpg", b"same bytes");
    let item = ingest(&catalog, &old);
    let new = dir.0.join("b.jpg");
    std::fs::rename(&old, &new).unwrap();
    // The watcher's order: the new path arrives, then the old goes away.
    let twin = ingest(&catalog, &new);

    let change = catalog.mark_missing_under(&old).unwrap();
    assert_eq!(change.folded, vec![(item.id, twin.id)]);

    let (path, rel, rel_parent): (String, String, String) = catalog
        .conn
        .lock()
        .query_row(
            "SELECT file_path, rel_path, rel_parent FROM media_items WHERE id = ?1;",
            [item.id.0 as i64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(PathBuf::from(path), new, "the row follows the new path");
    assert_eq!(rel, "b.jpg", "and so does its root-relative identity");
    assert_eq!(rel_parent, "", "a file in the root has no relative parent");
}

#[test]
fn a_twin_on_another_volume_does_not_fold() {
    let dir = TempDir::new("fold_volume");
    let catalog = Catalog::open_in_memory().unwrap();
    catalog.ensure_library_root(&dir.0).unwrap();
    let old = dir.write("a.jpg", b"same bytes");
    let item = ingest(&catalog, &old);
    // A hard link shares the inode and size, like the same file seen at a
    // second path; only its recorded volume differs.
    let twin_path = dir.0.join("b.jpg");
    std::fs::hard_link(&old, &twin_path).unwrap();
    let twin = ingest(&catalog, &twin_path);
    catalog
        .conn
        .lock()
        .execute(
            "UPDATE media_file_state SET volume_id = 'dev:other' WHERE media_id = ?1;",
            [twin.id.0 as i64],
        )
        .unwrap();

    let change = catalog.mark_missing_under(&old).unwrap();
    assert!(
        change.folded.is_empty(),
        "an inode from another volume is not the same file"
    );
    assert_eq!(change.missing, vec![item.id]);
    assert_eq!(
        catalog.total_count().unwrap(),
        2,
        "both rows survive as separate files"
    );
}
