//! Tests for the parent module, kept beside it.

use super::*;
use crate::text::MAX_NAME_CHARS;
use crate::{MediaType, SortDirection, SortField};

fn media_item(path: &str, timestamp_epoch_ms: i64, hidden: bool, trashed: bool) -> MediaItem {
    let path = std::path::PathBuf::from(path);
    let parent_dir = path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    MediaItem {
        id: MediaId(0),
        path,
        parent_dir,
        file_name,
        title: None,
        file_size: 1024,
        mtime_epoch_ms: timestamp_epoch_ms,
        media_type: MediaType::Image,
        width: 640,
        height: 480,
        duration_ms: None,
        timestamp_epoch_ms,
        year: 2026,
        month: 1,
        day: 1,
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
        is_hidden: hidden,
        is_trashed: trashed,
    }
}

#[test]
fn the_revision_moves_on_writes_and_not_on_reads() {
    let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
    let id = catalog
        .insert_or_update_batch(&[media_item("/lib/a.jpg", 1_000, false, false)])
        .expect("insert media")[0]
        .id;
    let before = catalog.revision();
    catalog.get_tags_for_item(id).expect("read tags");
    catalog.get_item_by_id(id).expect("read item");
    assert_eq!(catalog.revision(), before, "reads leave the revision alone");
    catalog
        .add_tags_batch(&[id], &["trip".to_string()])
        .expect("add tag");
    let after_tag = catalog.revision();
    assert_ne!(after_tag, before, "a tag write moves the revision");
    catalog.set_title(id, Some("Lisbon")).expect("set title");
    assert_ne!(catalog.revision(), after_tag, "a title write moves it too");
    assert_eq!(
        catalog.revision_written_here(),
        catalog.revision(),
        "this thread's own write is recorded as its own"
    );

    let mine = catalog.revision_written_here();
    let other = catalog.clone();
    std::thread::spawn(move || other.set_title(id, Some("Porto")).expect("set title"))
        .join()
        .expect("writer thread");
    assert_ne!(
        catalog.revision(),
        mine,
        "another thread's write moves the revision"
    );
    assert_eq!(
        catalog.revision_written_here(),
        mine,
        "but is not recorded as this thread's write"
    );
}

#[test]
fn every_text_write_stores_one_clean_line() {
    let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
    let id = catalog
        .insert_or_update_batch(&[media_item("/lib/a.jpg", 1_000, false, false)])
        .expect("insert media")[0]
        .id;
    let dirty = "  a\u{202E}b\n\u{FFFF} c  ";

    catalog.set_title(id, Some(dirty)).expect("set title");
    let item = catalog.get_item_by_id(id).expect("read").expect("item");
    assert_eq!(item.title.as_deref(), Some("a b c"), "title is cleaned");

    catalog.set_caption(id, dirty).expect("set caption");
    let captions = catalog.get_captions_map().expect("captions");
    assert_eq!(
        captions[&id], "a b\nc",
        "caption is cleaned, keeping its line break"
    );

    catalog
        .add_tags_batch(&[id], &[dirty.to_string()])
        .expect("add tags");
    assert!(
        catalog.add_tags_batch(&[id], &["x".repeat(150)]).is_err(),
        "an over-long tag is refused: cutting it would store a different keyword"
    );
    assert_eq!(
        catalog.get_tags_for_item(id).expect("tags"),
        vec!["a b c".to_string()],
        "tags are cleaned to one line"
    );

    catalog
        .rename_tag("a b c", "Trip\u{2028}2026")
        .expect("rename");
    assert!(
        catalog
            .get_tags_for_item(id)
            .expect("tags")
            .contains(&"trip 2026".to_string()),
        "a renamed tag is cleaned"
    );

    let album = catalog
        .create_smart_album(dirty, "folder", crate::MatchMode::All, &[])
        .expect("create album");
    assert_eq!(album.title, "a b c", "smart album title is cleaned");
    let stored = catalog
        .get_smart_album(album.id)
        .expect("read album")
        .expect("album");
    assert_eq!(stored.title, "a b c", "the stored album title is cleaned");
    catalog
        .update_smart_album(
            album.id,
            "Edited\t\u{FEFF}",
            "folder",
            crate::MatchMode::All,
            &[],
        )
        .expect("update album");
    let stored = catalog
        .get_smart_album(album.id)
        .expect("read")
        .expect("album");
    assert_eq!(stored.title, "Edited", "an edited album title is cleaned");

    catalog
        .set_titles(&[(id, Some("batch\u{2028}title "))])
        .expect("batch titles");
    let item = catalog.get_item_by_id(id).expect("read").expect("item");
    assert_eq!(
        item.title.as_deref(),
        Some("batch title"),
        "batch titles are cleaned"
    );

    catalog
        .set_captions(&[(id, "Para\u{202E} one\n\nPara two\n")])
        .expect("batch captions");
    assert_eq!(
        catalog.get_captions_map().expect("captions")[&id],
        "Para one\n\nPara two",
        "captions keep their paragraphs but lose disguising characters"
    );
}

#[test]
fn items_by_tag_plan_drives_from_the_tag_range() {
    let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
    let items: Vec<_> = (0..2_000)
        .map(|i| media_item(&format!("/lib/{i}.jpg"), i, false, false))
        .collect();
    let ids: Vec<_> = catalog
        .insert_or_update_batch(&items)
        .expect("insert media")
        .iter()
        .map(|it| it.id)
        .collect();
    catalog
        .add_tags_batch(&ids[..2], &["rare".to_string()])
        .expect("tag");
    catalog
        .add_tags_batch(&ids, &["common".to_string()])
        .expect("tag");
    // No ANALYZE: the app never runs it, so plan without statistics.
    // Inline the values (explain binds nothing): four times the tag,
    // then limit and offset.
    let mut values = ["'rare'"; 4].into_iter().chain(["24", "0"]);
    let sql: String = items_by_tag_sql()
        .chars()
        .map(|c| match c {
            '?' => values.next().expect("one value per ?").to_string(),
            c => c.to_string(),
        })
        .collect();
    let rows = catalog.explain_query_plan(&sql).expect("explain");
    let plan = rows.join(" | ");
    assert!(
        rows.iter().any(
            |r| r.starts_with("SEARCH media_tags USING INDEX idx_media_tags_tag")
                && r.contains("tag>")
        ),
        "a rare tag must seek media_tags by tag range: {plan}"
    );
    assert!(
        !rows
            .iter()
            .any(|r| r.starts_with("SCAN m") || r.contains("CORRELATED")),
        "no timeline walk with a per-row subquery: {plan}"
    );
}

#[test]
fn a_legacy_over_long_tag_stays_removable() {
    let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
    let id = catalog
        .insert_or_update_batch(&[media_item("/lib/a.jpg", 1_000, false, false)])
        .expect("insert media")[0]
        .id;
    let legacy = "y".repeat(MAX_NAME_CHARS + 50);
    {
        // Seeded the way the catalog writes tags, keeping the index in step.
        let mut conn = catalog.conn.lock();
        let tx = conn.transaction().expect("begin");
        Catalog::delete_fts_for_items(&tx, &[id.0 as i64]).expect("unindex");
        tx.execute(
            "INSERT INTO media_tags (media_id, tag) VALUES (?1, ?2)",
            params![id.0 as i64, legacy],
        )
        .expect("seed a tag saved before paths were capped");
        Catalog::insert_fts_for_items(&tx, &[id.0 as i64]).expect("reindex");
        tx.commit().expect("commit");
    }
    catalog
        .remove_tags_batch(&[id], &[legacy])
        .expect("removing a legacy tag succeeds");
    assert!(
        catalog.get_tags_for_item(id).expect("tags").is_empty(),
        "a tag too long to normalize is still removed by its exact text"
    );
}

#[test]
fn untagged_count_and_query_exclude_tagged_hidden_and_trashed() {
    let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
    let inserted = catalog
        .insert_or_update_batch(&[
            media_item("/lib/untagged_a.jpg", 1_000, false, false),
            media_item("/lib/tagged.jpg", 2_000, false, false),
            media_item("/lib/hidden.jpg", 3_000, true, false),
            media_item("/lib/trashed.jpg", 4_000, false, true),
            media_item("/lib/untagged_b.jpg", 5_000, false, false),
        ])
        .expect("insert media");

    catalog
        .add_tags_batch(&[inserted[1].id], &["travel".to_string()])
        .expect("tag one item");

    assert_eq!(
        catalog.query_untagged_count().expect("count untagged"),
        2,
        "only active, untagged media may be counted"
    );

    let default_page = catalog
        .query_untagged_sorted(10, 0, SortSpec::default())
        .expect("query untagged");
    assert_eq!(
        default_page.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![inserted[4].id, inserted[0].id],
        "untagged media must follow the requested date-desc order"
    );

    let by_name = catalog
        .query_untagged_sorted(
            10,
            0,
            SortSpec {
                field: SortField::FileName,
                direction: SortDirection::Ascending,
            },
        )
        .expect("query untagged by name");
    assert_eq!(
        by_name.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![inserted[0].id, inserted[4].id],
        "the caller's SortSpec must be honored, not a fixed SQL order"
    );

    let second_page = catalog
        .query_untagged_sorted(1, 1, SortSpec::default())
        .expect("page untagged");
    assert_eq!(second_page.len(), 1, "limit and offset must page");
    assert_eq!(second_page[0].id, inserted[0].id);

    assert!(
        catalog
            .query_untagged_sorted(10, 2, SortSpec::default())
            .expect("page past the end")
            .is_empty(),
        "an offset past the last untagged item must return an empty page"
    );
}

#[test]
fn untagged_count_tracks_add_remove_rename_merge_and_delete() {
    let catalog = Catalog::open_in_memory().expect("open in-memory catalog");
    let inserted = catalog
        .insert_or_update_batch(&[
            media_item("/lib/first.jpg", 1_000, false, false),
            media_item("/lib/second.jpg", 2_000, false, false),
        ])
        .expect("insert media");
    let first = inserted[0].id;

    assert_eq!(
        catalog.query_untagged_count().expect("initial count"),
        2,
        "a fresh catalog has every item untagged"
    );

    catalog
        .add_tags_batch(&[first], &["travel".to_string()])
        .expect("add tag");
    assert_eq!(
        catalog.query_untagged_count().expect("count after add"),
        1,
        "tagging an item must remove it from the untagged count"
    );

    catalog
        .add_tags_batch(&[first], &["vacation".to_string()])
        .expect("add second tag");
    assert_eq!(
        catalog
            .query_untagged_count()
            .expect("count after second add"),
        1,
        "extra tags must not change the untagged count"
    );

    catalog
        .remove_tags_batch(&[first], &["vacation".to_string()])
        .expect("remove tag");
    assert_eq!(
        catalog.query_untagged_count().expect("count after remove"),
        1,
        "removing one of two tags keeps the item tagged"
    );

    assert_eq!(
        catalog
            .rename_tag("travel", "journey")
            .expect("rename tag")
            .removed
            .len(),
        1
    );
    assert_eq!(
        catalog.query_untagged_count().expect("count after rename"),
        1,
        "renaming a tag must not change untagged membership"
    );

    assert_eq!(
        catalog
            .merge_tags("journey", "trip")
            .expect("merge tag")
            .removed
            .len(),
        1
    );
    assert_eq!(
        catalog.query_untagged_count().expect("count after merge"),
        1,
        "merging tags must leave tagged items tagged"
    );

    assert_eq!(
        catalog
            .delete_tag_globally("trip")
            .expect("delete tag globally")
            .removed
            .len(),
        1
    );
    assert_eq!(
        catalog.query_untagged_count().expect("count after delete"),
        2,
        "deleting the last tag must return the item to untagged"
    );
    assert_eq!(
        catalog
            .query_untagged_sorted(10, 0, SortSpec::default())
            .expect("query after delete")
            .len(),
        2,
        "the sorted query must agree with the refreshed count"
    );
}
