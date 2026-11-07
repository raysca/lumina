use super::*;
use crate::catalog::files::tests::TempDir;
use crate::catalog::tests_support::{album_members, video_item};
use crate::smart_albums::MatchMode;
use crate::{DeletionTarget, SortDirection, SortField, SCHEMA_VERSION};

fn plain(catalog: &Catalog, path: &str) -> MediaId {
    catalog.insert_or_update_batch(&[video_item(path)]).unwrap()[0].id
}

/// An item named `path` taken at `ts`.
fn taken(catalog: &Catalog, path: &str, ts: i64) -> MediaId {
    let mut item = video_item(path);
    item.timestamp_epoch_ms = ts;
    catalog.insert_or_update_batch(&[item]).unwrap()[0].id
}

fn ids(items: Vec<MediaItem>) -> Vec<MediaId> {
    items.into_iter().map(|item| item.id).collect()
}

fn page(catalog: &Catalog, album: i64, sort: SortSpec) -> Vec<MediaId> {
    ids(catalog.query_album_sorted(album, 100, 0, sort).unwrap())
}

#[test]
fn migration_creates_album_tables_idempotently() {
    // Removed on drop, so a failed assertion cleans up too.
    let dir = TempDir::new("albums_migration");
    let db = dir.0.join("catalog.db");
    {
        let catalog = Catalog::open(&db).unwrap();
        catalog
            .create_smart_album("Trips", "folder", MatchMode::All, &[])
            .unwrap();
        // Back to the v18 layout: no album tables, smart_albums without pinned.
        catalog
            .conn
            .lock()
            .execute_batch(
                r#"
                DROP TABLE IF EXISTS album_items;
                DROP TABLE IF EXISTS albums;
                CREATE TABLE smart_albums_v18 (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    icon_name TEXT NOT NULL DEFAULT 'folder',
                    match_mode TEXT NOT NULL DEFAULT 'all',
                    rules_json TEXT NOT NULL,
                    created_at_epoch_ms INTEGER NOT NULL
                );
                INSERT INTO smart_albums_v18
                    SELECT id, title, icon_name, match_mode, rules_json, created_at_epoch_ms
                    FROM smart_albums;
                DROP TABLE smart_albums;
                ALTER TABLE smart_albums_v18 RENAME TO smart_albums;
                PRAGMA user_version = 18;
                "#,
            )
            .unwrap();
    }
    for pass in 0..2 {
        let catalog = Catalog::open(&db).unwrap();
        assert_eq!(
            catalog.schema_version().unwrap(),
            SCHEMA_VERSION,
            "opening stamps the current schema version"
        );
        let exists = |sql: &str| -> bool {
            catalog
                .conn
                .lock()
                .prepare(sql)
                .unwrap()
                .exists([])
                .unwrap()
        };
        assert!(
            exists("SELECT 1 FROM pragma_table_info('smart_albums') WHERE name = 'pinned'"),
            "the upgrade adds smart_albums.pinned"
        );
        assert!(
            exists(
                "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_album_items_media'"
            ),
            "the upgrade creates the membership index"
        );
        let smart = catalog.list_smart_albums().unwrap();
        assert_eq!(smart.len(), 1, "the existing smart album is kept");
        assert_eq!(smart[0].title, "Trips");
        assert!(!smart[0].pinned, "an upgraded smart album starts unpinned");

        let item = plain(&catalog, &format!("/lib/{pass}.mp4"));
        let album = catalog.create_album(&format!("Picks {pass}")).unwrap();
        assert_eq!(catalog.add_to_album(album.id, &[item]).unwrap(), 1);
        let listed: Vec<(String, usize)> = catalog
            .list_albums_with_counts()
            .unwrap()
            .into_iter()
            .map(|(album, count)| (album.title, count))
            .collect();
        let expected: Vec<(String, usize)> =
            (0..=pass).map(|p| (format!("Picks {p}"), 1)).collect();
        assert_eq!(
            listed, expected,
            "reopening keeps the albums and members written before"
        );
    }
}

#[test]
fn hidden_member_is_excluded_from_slice_count_and_list() {
    let catalog = Catalog::open_in_memory().unwrap();
    let a = plain(&catalog, "/lib/a.mp4");
    let b = plain(&catalog, "/lib/b.mp4");
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[a, b]).unwrap();
    catalog
        .conn
        .lock()
        .execute(
            "UPDATE media_items SET is_hidden = 1 WHERE id = ?1;",
            [b.0 as i64],
        )
        .unwrap();

    assert_eq!(
        page(&catalog, album.id, SortSpec::default()),
        vec![a],
        "a hidden member is not listed"
    );
    assert_eq!(
        catalog.query_album_count(album.id).unwrap(),
        1,
        "a hidden member is not counted"
    );
    assert_eq!(
        catalog.list_albums_with_counts().unwrap(),
        vec![(album.clone(), 1)],
        "the sidebar count leaves out a hidden member"
    );
    assert_eq!(
        album_members(&catalog, album.id),
        vec![a, b],
        "hiding keeps the membership"
    );
}

#[test]
fn paging_across_equal_sort_keys_is_stable() {
    let catalog = Catalog::open_in_memory().unwrap();
    let members: Vec<MediaId> = ["x", "y", "z"]
        .iter()
        .map(|name| taken(&catalog, &format!("/lib/{name}.mp4"), 5_000))
        .collect();
    let album = catalog.create_album("Same second").unwrap();
    catalog.add_to_album(album.id, &members).unwrap();

    let newest_first: Vec<MediaId> = members.iter().rev().copied().collect();
    for (sort, expected) in [
        (SortSpec::default(), newest_first),
        (
            SortSpec {
                field: SortField::DateTaken,
                direction: SortDirection::Ascending,
            },
            members.clone(),
        ),
    ] {
        let paged: Vec<MediaId> = (0..3)
            .flat_map(|offset| {
                ids(catalog
                    .query_album_sorted(album.id, 1, offset, sort)
                    .unwrap())
            })
            .collect();
        assert_eq!(
            paged, expected,
            "{sort:?}: equal keys page by id, in the sort's direction"
        );
        let mut covered = paged.clone();
        covered.sort_by_key(|id| id.0);
        assert_eq!(
            covered, members,
            "{sort:?}: the pages are distinct and cover every member"
        );
        assert_eq!(
            paged,
            ids(catalog.query_album_sorted(album.id, 3, 0, sort).unwrap()),
            "{sort:?}: the order is stable between calls"
        );
    }
}

#[test]
fn album_titles_are_cleaned_and_required() {
    let catalog = Catalog::open_in_memory().unwrap();
    let album = catalog.create_album("  Trip\n2026\u{202E} ").unwrap();
    assert_eq!(album.title, "Trip 2026", "titles go through clean_text");
    assert!(
        catalog.create_album(" \t ").is_err(),
        "an album needs a name"
    );
    assert!(catalog.rename_album(album.id, "Kyoto\tspring").unwrap());
    let listed = catalog.list_albums_with_counts().unwrap();
    assert_eq!(listed[0].0.title, "Kyoto spring", "renames are cleaned");
    assert!(
        !catalog.rename_album(album.id + 1, "Nope").unwrap(),
        "renaming an unknown album reports false"
    );
    assert!(catalog.rename_album(album.id, "").is_err());
}

#[test]
fn add_is_idempotent_and_counts_active_only() {
    let catalog = Catalog::open_in_memory().unwrap();
    let a = plain(&catalog, "/lib/a.mp4");
    let b = plain(&catalog, "/lib/b.mp4");
    let c = plain(&catalog, "/lib/c.mp4");
    let album = catalog.create_album("Picks").unwrap();
    let empty = catalog.create_album("Empty").unwrap();

    assert_eq!(catalog.add_to_album(album.id, &[a, b]).unwrap(), 2);
    assert_eq!(
        catalog
            .add_to_album(album.id, &[a, b, c, MediaId(9_999)])
            .unwrap(),
        1,
        "existing members and unknown ids add nothing"
    );
    catalog.batch_set_trashed(&[c], true).unwrap();

    assert_eq!(
        catalog.query_album_count(album.id).unwrap(),
        2,
        "a trashed member is not counted"
    );
    assert_eq!(
        catalog.list_albums_with_counts().unwrap(),
        vec![(album.clone(), 2), (empty, 0)],
        "the sidebar list counts active members and keeps empty albums"
    );
    let mut shown = page(&catalog, album.id, SortSpec::default());
    shown.sort_by_key(|id| id.0);
    assert_eq!(shown, vec![a, b], "a trashed member is not listed");
    assert!(
        catalog.add_to_album(album.id + 100, &[a]).is_err(),
        "adding to an unknown album is an error, not a silent no-op"
    );

    assert_eq!(catalog.remove_from_album(album.id, &[a, c]).unwrap(), 2);
    assert_eq!(catalog.remove_from_album(album.id, &[a]).unwrap(), 0);
    assert_eq!(album_members(&catalog, album.id), vec![b]);
    assert!(
        catalog.get_item_by_id(a).unwrap().is_some(),
        "removing from an album keeps the media"
    );
}

#[test]
fn album_items_cascade_on_media_delete() {
    let catalog = Catalog::open_in_memory().unwrap();
    let a = plain(&catalog, "/lib/a.mp4");
    let b = plain(&catalog, "/lib/b.mp4");
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[a, b]).unwrap();

    catalog
        .permanently_delete_items(&[a], DeletionTarget::CatalogOnly)
        .unwrap();
    let rows = |catalog: &Catalog| -> i64 {
        catalog
            .conn
            .lock()
            .query_row("SELECT COUNT(*) FROM album_items", [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(rows(&catalog), 1, "purging media leaves no orphan row");
    assert_eq!(album_members(&catalog, album.id), vec![b]);

    catalog.delete_album(album.id).unwrap();
    assert_eq!(rows(&catalog), 0, "deleting an album leaves no orphan row");
}

#[test]
fn delete_album_returns_members_and_restore_round_trips() {
    let catalog = Catalog::open_in_memory().unwrap();
    let a = plain(&catalog, "/lib/a.mp4");
    let b = plain(&catalog, "/lib/b.mp4");
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[b, a]).unwrap();
    assert!(catalog.set_album_pinned(album.id, true).unwrap());
    let album = Album {
        pinned: true,
        ..album
    };

    let (deleted, members) = catalog
        .delete_album(album.id)
        .unwrap()
        .expect("an existing album is returned");
    assert_eq!(deleted, album);
    assert_eq!(members, vec![a, b]);
    assert!(catalog.list_albums_with_counts().unwrap().is_empty());
    assert!(
        catalog.delete_album(album.id).unwrap().is_none(),
        "deleting it again finds nothing"
    );
    assert!(
        catalog.get_item_by_id(a).unwrap().is_some(),
        "deleting an album keeps its media"
    );

    catalog.restore_album(&deleted, &members).unwrap();
    assert_eq!(
        catalog.list_albums_with_counts().unwrap(),
        vec![(album.clone(), 2)],
        "same id, title, creation time and pin"
    );
    assert_eq!(album_members(&catalog, album.id), vec![a, b]);
    assert!(!catalog.set_album_pinned(album.id + 1, true).unwrap());
}

#[test]
fn restore_album_skips_purged_members() {
    let catalog = Catalog::open_in_memory().unwrap();
    let a = plain(&catalog, "/lib/a.mp4");
    let b = plain(&catalog, "/lib/b.mp4");
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[a, b]).unwrap();
    let (deleted, members) = catalog.delete_album(album.id).unwrap().unwrap();

    catalog
        .permanently_delete_items(&[a], DeletionTarget::CatalogOnly)
        .unwrap();
    catalog
        .restore_album(&deleted, &members)
        .expect("a purged member must not make the restore fail");
    assert_eq!(album_members(&catalog, album.id), vec![b]);
}

#[test]
fn trashed_member_keeps_membership_and_returns_on_restore() {
    let catalog = Catalog::open_in_memory().unwrap();
    let a = plain(&catalog, "/lib/a.mp4");
    let b = plain(&catalog, "/lib/b.mp4");
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[a, b]).unwrap();

    catalog.batch_set_trashed(&[a], true).unwrap();
    assert_eq!(catalog.query_album_count(album.id).unwrap(), 1);
    assert_eq!(
        album_members(&catalog, album.id),
        vec![a, b],
        "trash is a flag, so the member stays"
    );

    catalog.restore_trashed_items(&[a]).unwrap();
    assert_eq!(catalog.query_album_count(album.id).unwrap(), 2);
    let mut shown = page(&catalog, album.id, SortSpec::default());
    shown.sort_by_key(|id| id.0);
    assert_eq!(shown, vec![a, b], "the restored item is back in the album");
}

#[test]
fn smart_album_pinned_round_trips_through_list_and_restore() {
    let catalog = Catalog::open_in_memory().unwrap();
    let smart = catalog
        .create_smart_album("Trips", "folder", MatchMode::All, &[])
        .unwrap();
    assert!(!smart.pinned, "a new smart album is unpinned");
    assert!(catalog.set_smart_album_pinned(smart.id, true).unwrap());
    assert!(!catalog.set_smart_album_pinned(smart.id + 1, true).unwrap());

    assert!(catalog.list_smart_albums().unwrap()[0].pinned);
    let stored = catalog.get_smart_album(smart.id).unwrap().unwrap();
    assert!(stored.pinned, "get_smart_album reads the pin");

    assert!(catalog.delete_smart_album(smart.id).unwrap());
    catalog.restore_smart_album(&stored).unwrap();
    let restored = catalog.list_smart_albums().unwrap();
    assert_eq!(restored[0].id, smart.id);
    assert!(restored[0].pinned, "restore keeps the pin");
}

#[test]
fn album_query_follows_sort_spec() {
    let catalog = Catalog::open_in_memory().unwrap();
    let b = taken(&catalog, "/lib/b.mp4", 3_000);
    let c = taken(&catalog, "/lib/C.mp4", 1_000);
    let a = taken(&catalog, "/lib/a.mp4", 2_000);
    let outsider = taken(&catalog, "/lib/0.mp4", 2_500);
    let album = catalog.create_album("Picks").unwrap();
    catalog.add_to_album(album.id, &[a, b, c]).unwrap();

    let sort = |field, direction| SortSpec { field, direction };
    assert_eq!(
        page(
            &catalog,
            album.id,
            sort(SortField::DateTaken, SortDirection::Descending)
        ),
        vec![b, a, c],
        "newest first"
    );
    assert_eq!(
        page(
            &catalog,
            album.id,
            sort(SortField::DateTaken, SortDirection::Ascending)
        ),
        vec![c, a, b],
        "oldest first"
    );
    assert_eq!(
        page(
            &catalog,
            album.id,
            sort(SortField::FileName, SortDirection::Ascending)
        ),
        vec![a, b, c],
        "names sort without case"
    );
    let second = catalog
        .query_album_sorted(album.id, 1, 1, SortSpec::default())
        .unwrap();
    assert_eq!(ids(second), vec![a], "limit and offset page the album");
    assert!(!page(&catalog, album.id, SortSpec::default()).contains(&outsider));
}
