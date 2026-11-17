//! Keyword integration tests: tag paths, aliases, rename, merge and
//! delete, and the filters and smart albums that follow them.

mod common;

use common::*;
use lumina_core::{
    write_sidecar, Catalog, MatchMode, MediaId, MediaType, RuleField, RuleOperator, RuleValue,
    SidecarMetadata, SmartAlbumRule, SortSpec, StructuredQuery,
};

#[test]
fn test_batch_operations_and_keywords() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item(
        "/photos/lake_tahoe.jpg",
        MediaType::Image,
        1728560000000,
        Some("Tahoe"),
        Some("A7IV"),
    );
    let item2 = sample_media_item(
        "/photos/yosemite_valley.jpg",
        MediaType::Image,
        1728570000000,
        Some("Yosemite"),
        Some("EOS R5"),
    );
    let item3 = sample_media_item(
        "/photos/zion_canyon.jpg",
        MediaType::Image,
        1728580000000,
        Some("Zion"),
        Some("Z8"),
    );

    let inserted = catalog
        .insert_or_update_batch(&[item1, item2, item3])
        .unwrap();
    let ids: Vec<MediaId> = inserted.iter().map(|it| it.id).collect();
    assert_eq!(ids.len(), 3);

    // Test batch favorite
    catalog.batch_set_favorite(&ids[0..2], true).unwrap();
    let it1 = catalog.get_item_by_id(ids[0]).unwrap().unwrap();
    let it2 = catalog.get_item_by_id(ids[1]).unwrap().unwrap();
    let it3 = catalog.get_item_by_id(ids[2]).unwrap().unwrap();
    assert!(it1.is_favorite);
    assert!(it2.is_favorite);
    assert!(!it3.is_favorite);

    // Test batch tags
    let tags = vec![
        "nature".to_string(),
        "travel".to_string(),
        "vacation".to_string(),
    ];
    catalog.add_tags_batch(&ids, &tags).unwrap();

    let all_tags = catalog.query_tag_counts().unwrap();
    assert_eq!(
        all_tags,
        vec![
            ("nature".to_string(), 3),
            ("travel".to_string(), 3),
            ("vacation".to_string(), 3)
        ]
    );

    let tags_for_1 = catalog.get_tags_for_item(ids[0]).unwrap();
    assert_eq!(tags_for_1.len(), 3);

    // Query by tag
    let nature_items = catalog.get_items_by_tag("nature", 10, 0).unwrap();
    assert_eq!(nature_items.len(), 3);

    // Structured search with tag
    let q = StructuredQuery::parse("tag:nature");
    let search_results = catalog.search_structured(&q, 10, 0).unwrap();
    assert_eq!(search_results.len(), 3);

    // Remove a tag from item 1
    catalog
        .remove_tags_batch(&[ids[0]], &["vacation".to_string()])
        .unwrap();
    let tags_for_1_after = catalog.get_tags_for_item(ids[0]).unwrap();
    assert_eq!(tags_for_1_after.len(), 2);
    assert!(!tags_for_1_after.contains(&"vacation".to_string()));

    // Test query_tag_counts
    let tag_counts = catalog.query_tag_counts().unwrap();
    assert_eq!(
        tag_counts,
        vec![
            ("nature".to_string(), 3),
            ("travel".to_string(), 3),
            ("vacation".to_string(), 2)
        ]
    );

    // Test Tag in Smart Album
    let tag_rule = SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::Equals,
        value: RuleValue::Text("Nature".to_string()),
    };
    let tag_album = catalog
        .create_smart_album("Nature Album", "tag", MatchMode::All, &[tag_rule])
        .unwrap();
    let tag_album_matches = catalog.query_smart_album(&tag_album, 10, 0).unwrap();
    assert_eq!(tag_album_matches.len(), 3);

    // Test batch trash
    catalog.batch_set_trashed(&ids[1..3], true).unwrap();
    let active_slice = catalog.get_item_slice(0, 10).unwrap();
    assert_eq!(active_slice.len(), 1);

    // Tag counts after trashing items 1 and 2: only item 0 remains active (nature, travel)
    let tag_counts_after = catalog.query_tag_counts().unwrap();
    assert_eq!(
        tag_counts_after,
        vec![("nature".to_string(), 1), ("travel".to_string(), 1)]
    );

    let tag_album_matches_after = catalog.query_smart_album(&tag_album, 10, 0).unwrap();
    assert_eq!(tag_album_matches_after.len(), 1);
    assert_eq!(active_slice[0].id, ids[0]);
}

#[test]
fn test_tag_management_lifecycle() {
    let catalog = Catalog::open_in_memory().expect("open catalog");

    let item1 = sample_media_item("/photos/img1.jpg", MediaType::Image, 1000, None, None);
    let item2 = sample_media_item("/photos/img2.jpg", MediaType::Image, 2000, None, None);
    let item3 = sample_media_item("/photos/img3.jpg", MediaType::Image, 3000, None, None);

    let inserted = catalog
        .insert_or_update_batch(&[item1, item2, item3])
        .unwrap();
    let ids: Vec<MediaId> = inserted.iter().map(|it| it.id).collect();

    // 1. Tag items: item1 & item2 get "nature", item2 & item3 get "travel", item3 gets "vacation"
    catalog
        .add_tags_batch(&[ids[0], ids[1]], &["nature".to_string()])
        .unwrap();
    catalog
        .add_tags_batch(&[ids[1], ids[2]], &["travel".to_string()])
        .unwrap();
    catalog
        .add_tags_batch(&[ids[2]], &["vacation".to_string()])
        .unwrap();

    let counts = catalog.query_tag_counts().unwrap();
    assert_eq!(
        counts,
        vec![
            ("nature".to_string(), 2),
            ("travel".to_string(), 2),
            ("vacation".to_string(), 1),
        ]
    );

    // 2. Create smart album targeting tag:nature
    let tag_rule = SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::Equals,
        value: RuleValue::Text("nature".to_string()),
    };
    let album = catalog
        .create_smart_album("Nature Shots", "leaf", MatchMode::All, &[tag_rule])
        .unwrap();
    assert_eq!(catalog.query_smart_album_count(&album).unwrap(), 2);

    // 3. Rename "nature" -> "landscape"
    let affected = catalog
        .rename_tag("nature", "landscape")
        .unwrap()
        .removed
        .len();
    assert_eq!(affected, 2);

    // Verify tag counts: "nature" gone, "landscape" exists
    let counts_renamed = catalog.query_tag_counts().unwrap();
    assert_eq!(
        counts_renamed,
        vec![
            ("landscape".to_string(), 2),
            ("travel".to_string(), 2),
            ("vacation".to_string(), 1),
        ]
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["landscape"]
    );

    // Verify Smart Album cascade: album rules now reference "landscape"
    let reloaded_album = catalog.get_smart_album(album.id).unwrap().unwrap();
    assert_eq!(
        reloaded_album.rules[0].value,
        RuleValue::Text("landscape".to_string())
    );
    assert_eq!(catalog.query_smart_album_count(&reloaded_album).unwrap(), 2);

    // 4. Merge "travel" into "landscape"
    let merged_affected = catalog
        .merge_tags("travel", "landscape")
        .unwrap()
        .removed
        .len();
    assert_eq!(merged_affected, 2);

    // Item 1 had landscape. Item 2 had landscape + travel (now deduplicated landscape). Item 3 had travel (now landscape) + vacation.
    let counts_merged = catalog.query_tag_counts().unwrap();
    assert_eq!(
        counts_merged,
        vec![("landscape".to_string(), 3), ("vacation".to_string(), 1),]
    );
    assert_eq!(catalog.query_smart_album_count(&reloaded_album).unwrap(), 3);

    // 5. Delete "vacation" globally
    let deleted_count = catalog
        .delete_tag_globally("vacation")
        .unwrap()
        .removed
        .len();
    assert_eq!(deleted_count, 1);
    let counts_final = catalog.query_tag_counts().unwrap();
    assert_eq!(counts_final, vec![("landscape".to_string(), 3)]);
    assert_eq!(
        catalog.get_tags_for_item(ids[2]).unwrap(),
        vec!["landscape"]
    );
}

#[test]
fn test_tag_counts_for_items_split_full_and_partial_tags() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = (0..3)
        .map(|i| {
            sample_media_item(
                &format!("/photos/count{i}.jpg"),
                MediaType::Image,
                1000 + i as i64,
                None,
                None,
            )
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .unwrap()
        .iter()
        .map(|it| it.id)
        .collect();
    catalog.add_tags_batch(&ids, &["trip".to_string()]).unwrap();
    catalog
        .add_tags_batch(&[ids[0]], &["street".to_string()])
        .unwrap();
    // A tag only on an unselected item must not appear.
    catalog
        .add_tags_batch(&[ids[2]], &["other".to_string()])
        .unwrap();

    assert_eq!(
        catalog.tag_counts_for_items(&ids[..2]).unwrap(),
        vec![("street".to_string(), 1), ("trip".to_string(), 2)],
        "counts must cover only the given items, ordered by tag"
    );
    assert!(catalog.tag_counts_for_items(&[]).unwrap().is_empty());
}

#[test]
fn test_ids_with_tag_returns_only_tagged_subset() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = (0..3)
        .map(|i| {
            sample_media_item(
                &format!("/photos/has{i}.jpg"),
                MediaType::Image,
                2000 + i as i64,
                None,
                None,
            )
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .unwrap()
        .iter()
        .map(|it| it.id)
        .collect();
    catalog
        .add_tags_batch(&[ids[0], ids[2]], &["street".to_string()])
        .unwrap();

    let mut found = catalog.ids_with_tag(&ids[..2], "street").unwrap();
    found.sort_by_key(|id| id.0);
    assert_eq!(
        found,
        vec![ids[0]],
        "only the queried ids that carry the tag are returned"
    );
    assert!(catalog.ids_with_tag(&ids, "missing").unwrap().is_empty());
}

/// The ids a structured search returns, sorted by id.
fn search_ids(catalog: &Catalog, query: &str) -> Vec<MediaId> {
    let mut ids: Vec<MediaId> = catalog
        .search_structured(&StructuredQuery::parse(query), 100, 0)
        .expect("search must run")
        .into_iter()
        .map(|item| item.id)
        .collect();
    ids.sort_by_key(|id| id.0);
    ids
}

#[test]
fn caption_word_shared_with_tag_survives_tag_removal() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let item = sample_media_item("/photos/fts/a.jpg", MediaType::Image, 1000, None, None);
    let id = catalog.insert_or_update_batch(&[item]).unwrap()[0].id;

    catalog.set_caption(id, "kyoto trip").unwrap();
    catalog
        .add_tags_batch(&[id], &["kyoto".to_string()])
        .unwrap();
    catalog
        .remove_tags_batch(&[id], &["kyoto".to_string()])
        .unwrap();

    assert_eq!(
        search_ids(&catalog, "kyoto"),
        vec![id],
        "a caption word must stay searchable after a tag with the same word is removed"
    );
}

#[test]
fn caption_word_shared_with_tag_survives_global_rename_and_delete() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let item = sample_media_item("/photos/fts/b.jpg", MediaType::Image, 1000, None, None);
    let id = catalog.insert_or_update_batch(&[item]).unwrap()[0].id;

    catalog.set_caption(id, "kyoto trip").unwrap();
    catalog
        .add_tags_batch(&[id], &["kyoto".to_string()])
        .unwrap();
    catalog.rename_tag("kyoto", "japan").unwrap();
    assert_eq!(
        search_ids(&catalog, "kyoto"),
        vec![id],
        "a caption word must stay searchable after a tag with the same word is renamed"
    );

    catalog
        .add_tags_batch(&[id], &["trip".to_string()])
        .unwrap();
    catalog.delete_tag_globally("trip").unwrap();
    assert_eq!(
        search_ids(&catalog, "trip"),
        vec![id],
        "a caption word must stay searchable after a tag with the same word is deleted"
    );
}

#[test]
fn merge_keeps_search_index_in_sync() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let keep = sample_media_item("/photos/fts/keep.jpg", MediaType::Image, 1000, None, None);
    let dup = sample_media_item("/photos/fts/dup.jpg", MediaType::Image, 2000, None, None);
    let inserted = catalog.insert_or_update_batch(&[keep, dup]).unwrap();
    let (keep_id, dup_id) = (inserted[0].id, inserted[1].id);

    // "sunset" is both a tag and the caption: removing the tag later must not
    // take the caption word out of the index with it.
    catalog
        .add_tags_batch(&[dup_id], &["paris".to_string(), "sunset".to_string()])
        .unwrap();
    catalog.set_caption(dup_id, "sunset").unwrap();
    catalog.merge_duplicate_group(keep_id, &[dup_id]).unwrap();

    assert!(
        search_ids(&catalog, "paris").contains(&keep_id),
        "a tag merged onto the keeper must be searchable"
    );
    assert!(
        search_ids(&catalog, "sunset").contains(&keep_id),
        "a caption merged onto the keeper must be searchable"
    );

    catalog
        .remove_tags_batch(&[keep_id], &["paris".to_string(), "sunset".to_string()])
        .unwrap();
    assert!(
        !search_ids(&catalog, "paris").contains(&keep_id),
        "a tag removed from the keeper after a merge must stop matching"
    );
    assert!(
        search_ids(&catalog, "sunset").contains(&keep_id),
        "the merged caption must keep matching after the tag edit"
    );
}

// --- Keyword paths: filters match a keyword and its descendants ---

/// One item per tag, in order; returns the catalog and the ids.
fn catalog_with_tags(tags: &[&str]) -> (Catalog, Vec<MediaId>) {
    let sets: Vec<[&str; 1]> = tags.iter().map(|tag| [*tag]).collect();
    let sets: Vec<&[&str]> = sets.iter().map(|set| set.as_slice()).collect();
    catalog_with_tag_sets(&sets)
}

fn tag_album(operator: RuleOperator, tag: &str) -> lumina_core::SmartAlbum {
    lumina_core::SmartAlbum::new(
        tag.to_string(),
        "tag".to_string(),
        MatchMode::All,
        vec![SmartAlbumRule {
            field: RuleField::Tag,
            operator,
            value: RuleValue::Text(tag.to_string()),
        }],
    )
}

fn album_ids(catalog: &Catalog, album: &lumina_core::SmartAlbum) -> Vec<MediaId> {
    let mut ids: Vec<MediaId> = catalog
        .query_smart_album(album, 100, 0)
        .expect("smart album runs")
        .into_iter()
        .map(|item| item.id)
        .collect();
    ids.sort_by_key(|id| id.0);
    ids
}

const SIBLING_TAGS: [&str; 7] = [
    "places",
    "places/europe/paris",
    "placesx",
    "placesx/a",
    "places-old",
    "places.x",
    "other/places",
];

#[test]
fn tag_filter_matches_descendants_not_siblings() {
    let (catalog, ids) = catalog_with_tags(&SIBLING_TAGS);
    let expected = vec![ids[0], ids[1]];
    assert_eq!(
        search_ids(&catalog, "tag:places"),
        expected,
        "tag:places matches places and places/europe/paris, never a sibling prefix"
    );
    let mut by_tag: Vec<MediaId> = catalog
        .get_items_by_tag("places", 100, 0)
        .expect("items by tag")
        .into_iter()
        .map(|item| item.id)
        .collect();
    by_tag.sort_by_key(|id| id.0);
    assert_eq!(by_tag, expected, "the tag view matches the same subtree");
    assert_eq!(
        search_ids(&catalog, "tag:places/europe"),
        vec![ids[1]],
        "a mid-level keyword matches its descendants"
    );
}

#[test]
fn negated_tag_filter_excludes_descendants_only() {
    let (catalog, ids) = catalog_with_tags(&["places", "places/europe", "places-old", "other"]);
    assert_eq!(
        search_ids(&catalog, "-tag:places"),
        vec![ids[2], ids[3]],
        "-tag:places drops places and places/europe but keeps places-old"
    );
}

#[test]
fn items_by_parent_tag_are_not_duplicated() {
    let (catalog, ids) = catalog_with_tags(&["a/x"]);
    catalog
        .add_tags_batch(&[ids[0]], &["a/y".to_string()])
        .expect("add second child");
    let found = catalog.get_items_by_tag("a", 100, 0).expect("items by tag");
    assert_eq!(
        found.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![ids[0]],
        "an item with two matching children appears once"
    );
    assert_eq!(search_ids(&catalog, "tag:a"), vec![ids[0]]);
}

#[test]
fn smart_album_tag_equals_includes_children() {
    let (catalog, ids) = catalog_with_tags(&SIBLING_TAGS);
    assert_eq!(
        album_ids(&catalog, &tag_album(RuleOperator::Equals, "Places")),
        vec![ids[0], ids[1]],
        "Tag Equals matches the keyword and its descendants, not siblings"
    );
}

#[test]
fn smart_album_tag_not_equals_excludes_children() {
    let (catalog, ids) = catalog_with_tags(&SIBLING_TAGS);
    assert_eq!(
        album_ids(&catalog, &tag_album(RuleOperator::NotEquals, "places")),
        ids[2..].to_vec(),
        "Tag NotEquals excludes the keyword and its descendants, keeps siblings"
    );
}

#[test]
fn tag_filter_plan_uses_index() {
    let (catalog, _) = catalog_with_tags(&SIBLING_TAGS);
    let mut queries: Vec<(String, (String, Vec<rusqlite::types::Value>))> =
        ["tag:places", "-tag:places", "tag:places is:favorite"]
            .iter()
            .map(|text| {
                (
                    text.to_string(),
                    StructuredQuery::parse(text).synthesize_query_sorted(
                        50,
                        0,
                        SortSpec::default(),
                    ),
                )
            })
            .collect();
    for operator in [RuleOperator::Equals, RuleOperator::NotEquals] {
        let album = tag_album(operator.clone(), "places");
        queries.push((
            format!("album {operator:?}"),
            album
                .synthesize_query_sorted(50, 0, SortSpec::default())
                .expect("compiles"),
        ));
    }
    for (label, (sql, params)) in queries {
        let rows = catalog
            .explain_query_plan(&inline_sql_params(&sql, &params))
            .expect("explain");
        let plan = rows.join(" | ");
        assert!(
            rows.iter()
                .any(|r| r.contains("media_tags USING") && r.contains("tag>")),
            "{label}: the tag filter must seek a media_tags index by range, got: {plan}"
        );
        assert!(
            !rows.iter().any(|r| r.starts_with("SCAN media_tags")),
            "{label}: the tag filter must not scan media_tags, got: {plan}"
        );
    }
}

#[test]
fn tag_undo_ids_stay_exact() {
    let (catalog, ids) = catalog_with_tags(&["a", "a/x"]);
    assert_eq!(
        catalog.ids_with_tag(&ids, "a").expect("ids with tag"),
        vec![ids[0]],
        "undo records exact rows: an item with only a/x does not carry a"
    );
    catalog
        .remove_tags_batch(&ids, &["a".to_string()])
        .expect("remove");
    assert_eq!(
        catalog.get_tags_for_item(ids[1]).expect("tags"),
        vec!["a/x".to_string()],
        "removing a parent keyword is exact and leaves children alone"
    );
}

#[test]
fn normalize_path_refuses_over_long_paths() {
    let (catalog, ids) = catalog_with_tags(&["kept"]);
    let long = "x".repeat(lumina_core::text::MAX_NAME_CHARS + 1);
    assert!(lumina_core::catalog::keywords::normalize_path(&long).is_err());
    assert!(
        catalog
            .add_tags_batch(&ids, &["fine".to_string(), long])
            .is_err(),
        "an over-long keyword is refused, not truncated"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).expect("tags"),
        vec!["kept".to_string()],
        "a refused batch writes nothing"
    );
}

#[test]
fn tag_writes_store_the_normalized_path() {
    let (catalog, ids) = catalog_with_tags(&[" Places // Europe/ "]);
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).expect("tags"),
        vec!["places/europe".to_string()],
        "add stores the canonical path"
    );
    catalog
        .remove_tags_batch(&ids, &["places / europe".to_string()])
        .expect("remove");
    assert!(
        catalog.get_tags_for_item(ids[0]).expect("tags").is_empty(),
        "remove normalizes the same way, so it finds the stored row"
    );
}

#[test]
fn an_over_long_sidecar_keyword_is_reported_and_the_rest_import() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("long_sidecar_keyword");
    let item = engine.get_item_slice(0..1).remove(0);
    write_sidecar(
        &item.path,
        &SidecarMetadata {
            tags: vec![
                "Places/Europe".to_string(),
                "z".repeat(lumina_core::text::MAX_NAME_CHARS + 1),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    let report = engine
        .import_sidecars(&[item.id])
        .expect("one bad keyword must not abort the import");
    assert_eq!(
        engine.get_tags_for_item(item.id),
        vec!["places/europe".to_string()],
        "the valid keyword imports as a normalized path"
    );
    assert_eq!(
        report.failed.len(),
        1,
        "the refused keyword is reported, not silently cut or dropped: {:?}",
        report.failed
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn a_sidecar_keyword_below_an_alias_is_reported_and_the_rest_import() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("alias_sidecar_keyword");
    let item = engine.get_item_slice(0..1).remove(0);
    engine
        .set_alias("nyc", "places/usa/new york")
        .expect("set alias");
    write_sidecar(
        &item.path,
        &SidecarMetadata {
            tags: vec![
                "nyc/soho".to_string(),
                "kept".to_string(),
                "NYC".to_string(),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    let report = engine
        .import_sidecars(&[item.id])
        .expect("a keyword below an alias must not abort the import");
    let mut tags = engine.get_tags_for_item(item.id);
    tags.sort();
    assert_eq!(
        tags,
        vec!["kept".to_string(), "places/usa/new york".to_string()],
        "the other keywords import, and the alias imports as its keyword"
    );
    assert_eq!(
        report.failed.len(),
        1,
        "the keyword below the alias is reported: {:?}",
        report.failed
    );

    drop(engine);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn an_alias_cannot_sit_above_its_keyword_or_point_below_an_alias() {
    let (catalog, _ids) = catalog_with_tags(&["trip"]);
    assert!(
        catalog.set_alias("places", "places/usa").is_err(),
        "an alias above its own keyword would make the alias a parent in use"
    );
    catalog
        .set_alias("nyc", "places/usa/new york")
        .expect("alias");
    assert!(
        catalog.set_alias("soho", "nyc/soho").is_err(),
        "an alias cannot point at a keyword below another alias"
    );
    assert_eq!(
        catalog.aliases().expect("aliases"),
        vec![("nyc".to_string(), "places/usa/new york".to_string())],
        "refused aliases write nothing"
    );
}

#[test]
fn rename_stores_the_normalized_path_and_its_chip_removes() {
    let (catalog, ids) = catalog_with_tags(&["trip"]);
    catalog
        .rename_tag("trip", "Places / Europe")
        .expect("rename");
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).expect("tags"),
        vec!["places/europe".to_string()],
        "rename writes the canonical path, like add"
    );
    assert!(
        catalog
            .rename_tag(
                "places/europe",
                &"r".repeat(lumina_core::text::MAX_NAME_CHARS + 50)
            )
            .is_err(),
        "an over-long rename is refused, not truncated"
    );
    catalog
        .remove_tags_batch(&ids, &["places/europe".to_string()])
        .expect("remove");
    assert!(
        catalog.get_tags_for_item(ids[0]).expect("tags").is_empty(),
        "removing the chip deletes the renamed row"
    );
}

#[test]
fn removing_a_pre_path_spaced_tag_deletes_its_exact_row() {
    let (tmp_dir, db_path, engine, _rx) = scanned_library("spaced_legacy_tag");
    let item = engine.get_item_slice(0..1).remove(0);
    drop(engine);
    // A tag stored before paths were normalized, written with the index in step.
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let fts_row = "rowid, file_name, parent_dir, city, admin1_region, country_code, make, model, lens, year, tags, caption";
    let fts_values = format!(
        "m.id, m.file_name, m.parent_dir, m.city, m.admin1_region, m.country_code, m.make, \
         m.model, m.lens, CAST(m.year AS TEXT), \
         COALESCE((SELECT GROUP_CONCAT(tag, ' ') FROM media_tags WHERE media_id = m.id), ''), \
         COALESCE((SELECT caption FROM media_captions WHERE media_id = m.id), '') \
         FROM media_items m WHERE m.id = {}",
        item.id.0
    );
    conn.execute_batch(&format!(
        "INSERT INTO media_search_fts(media_search_fts, {fts_row}) SELECT 'delete', {fts_values};
         INSERT INTO media_tags (media_id, tag) VALUES ({}, 'places / europe');
         INSERT INTO media_search_fts({fts_row}) SELECT {fts_values};",
        item.id.0
    ))
    .unwrap();
    drop(conn);
    let catalog = Catalog::open(&db_path).expect("reopen");
    catalog
        .remove_tags_batch(&[item.id], &["places / europe".to_string()])
        .expect("remove");
    assert!(
        catalog.get_tags_for_item(item.id).expect("tags").is_empty(),
        "the chip of a pre-path tag removes its exact row"
    );
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn filter_values_are_normalized_like_stored_paths() {
    let (catalog, ids) = catalog_with_tags(&["places/europe", "other"]);
    for text in [
        "tag:places/",
        "tag:\"Places / Europe\"",
        "tag:Places//Europe",
    ] {
        assert_eq!(
            search_ids(&catalog, text),
            vec![ids[0]],
            "{text:?} matches the stored path"
        );
    }
    assert_eq!(search_ids(&catalog, "-tag:places/"), vec![ids[1]]);
    assert_eq!(
        album_ids(
            &catalog,
            &tag_album(RuleOperator::Equals, "Places / Europe")
        ),
        vec![ids[0]],
        "a smart-album Tag value is normalized too"
    );
    assert_eq!(
        catalog
            .get_items_by_tag("Places / ", 10, 0)
            .expect("items")
            .len(),
        1,
        "the tag view normalizes its value"
    );
}

#[test]
fn tag_filter_keeps_spaced_and_unicode_siblings_apart() {
    let (catalog, ids) =
        catalog_with_tags(&["places x", "places/a", "café", "café/paris", "cafée"]);
    assert_eq!(
        search_ids(&catalog, "tag:places"),
        vec![ids[1]],
        "`places x` is a sibling"
    );
    assert_eq!(
        search_ids(&catalog, "tag:café"),
        vec![ids[2], ids[3]],
        "café matches café/paris but not cafée"
    );
}

#[test]
fn a_blank_sidecar_keyword_is_skipped_not_reported() {
    let (tmp_dir, _db_path, engine, _rx) = scanned_library("blank_sidecar_keyword");
    let item = engine.get_item_slice(0..1).remove(0);
    write_sidecar(
        &item.path,
        &SidecarMetadata {
            tags: vec!["/".to_string(), " / / ".to_string(), "kept".to_string()],
            ..Default::default()
        },
    )
    .unwrap();
    let report = engine.import_sidecars(&[item.id]).expect("import");
    assert!(
        report.failed.is_empty(),
        "a keyword with no segments is skipped, not a failure: {:?}",
        report.failed
    );
    assert_eq!(engine.get_tags_for_item(item.id), vec!["kept".to_string()]);
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// ---- Subtree rename/move/delete and aliases ----

/// One item per entry, each carrying the listed tags.
fn catalog_with_tag_sets(sets: &[&[&str]]) -> (Catalog, Vec<MediaId>) {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let items: Vec<_> = (0..sets.len())
        .map(|i| {
            sample_media_item(
                &format!("/photos/kw3/{i}.jpg"),
                MediaType::Image,
                1_000 + i as i64,
                None,
                None,
            )
        })
        .collect();
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .expect("insert items")
        .iter()
        .map(|it| it.id)
        .collect();
    for (id, tags) in ids.iter().zip(sets) {
        let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
        catalog.add_tags_batch(&[*id], &tags).expect("add tags");
    }
    (catalog, ids)
}

#[test]
fn rename_parent_moves_subtree_and_merges() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a/x", "b/x"], &["b/x"]]);
    let change = catalog.rename_tag("a", "b").expect("rename a subtree");
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["b/x".to_string()],
        "the moved child merges into the existing b/x: one row, no a/* left"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[1]).unwrap(),
        vec!["b/x".to_string()]
    );
    assert_eq!(
        change.removed,
        vec![(ids[0], "a/x".to_string())],
        "the change lists exactly the removed row"
    );
    assert!(
        change.added.is_empty(),
        "b/x already existed: nothing added"
    );
    assert_eq!(change.affected_ids(), vec![ids[0]]);
    assert_eq!(
        search_ids(&catalog, "b"),
        ids,
        "the search index finds the new parent"
    );
    assert!(
        search_ids(&catalog, "a").is_empty(),
        "the old parent no longer finds the item"
    );
}

#[test]
fn rename_into_own_subtree_is_refused() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a", "a/x"]]);
    assert!(
        catalog.rename_tag("a", "a/b").is_err(),
        "a keyword cannot move inside itself"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["a".to_string(), "a/x".to_string()],
        "a refused rename writes nothing"
    );
}

#[test]
fn rename_onto_alias_is_refused() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["trip"]]);
    catalog.set_alias("nyc", "places/usa/new york").unwrap();
    assert!(
        catalog.rename_tag("trip", "NYC").is_err(),
        "a keyword cannot take a name that is already an alias"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["trip".to_string()]
    );
}

#[test]
fn rename_rewrites_smart_album_rules_in_subtree() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["a", "a/x/y"], &["ab"]]);
    let tag_rule = |op: RuleOperator, v: &str| SmartAlbumRule {
        field: RuleField::Tag,
        operator: op,
        value: RuleValue::Text(v.to_string()),
    };
    let album = catalog
        .create_smart_album(
            "Tagged",
            "tag",
            MatchMode::Any,
            &[
                tag_rule(RuleOperator::Equals, "a"),
                tag_rule(RuleOperator::NotEquals, "a/x/y"),
                tag_rule(RuleOperator::Contains, "ab"),
                SmartAlbumRule {
                    field: RuleField::Query,
                    operator: RuleOperator::Equals,
                    value: RuleValue::Text("tag:a".to_string()),
                },
            ],
        )
        .unwrap();
    let change = catalog.rename_tag("a", "c").expect("rename");
    let rules = catalog.get_smart_album(album.id).unwrap().unwrap().rules;
    let values: Vec<_> = rules.iter().map(|r| r.value.clone()).collect();
    assert_eq!(
        values,
        vec![
            RuleValue::Text("c".to_string()),
            RuleValue::Text("c/x/y".to_string()),
            RuleValue::Text("ab".to_string()),
            RuleValue::Text("tag:a".to_string()),
        ],
        "Tag rules on a or below it follow the rename; a sibling and saved searches are untouched"
    );
    assert_eq!(
        change.albums_before.len(),
        1,
        "the touched album is recorded"
    );
    assert_eq!(change.albums_after.len(), 1);
    assert_eq!(change.albums_before[0].0, album.id);
}

#[test]
fn delete_parent_removes_subtree_only() {
    let (catalog, ids) =
        catalog_with_tag_sets(&[&["places", "places/europe/paris", "placesx", "places-old"]]);
    let change = catalog.delete_tag_globally("places").expect("delete");
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["places-old".to_string(), "placesx".to_string()],
        "deleting a parent removes it and its children, not its siblings"
    );
    assert_eq!(change.removed.len(), 2);
    assert!(search_ids(&catalog, "paris").is_empty(), "index refreshed");
    assert_eq!(
        search_ids(&catalog, "placesx"),
        vec![ids[0]],
        "the index is rebuilt with the tags that remain"
    );
}

#[test]
fn bulk_delete_returns_one_change() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a", "a/x", "keep"], &["b/y", "a/x"]]);
    let change = catalog
        .delete_tags_globally(&["a".to_string(), "b".to_string()])
        .expect("bulk delete");
    let mut removed = change.removed.clone();
    removed.sort_by(|l, r| (l.0 .0, &l.1).cmp(&(r.0 .0, &r.1)));
    assert_eq!(
        removed,
        vec![
            (ids[0], "a".to_string()),
            (ids[0], "a/x".to_string()),
            (ids[1], "a/x".to_string()),
            (ids[1], "b/y".to_string()),
        ],
        "one change lists every removed row of every deleted subtree"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["keep".to_string()]
    );
    assert!(catalog.get_tags_for_item(ids[1]).unwrap().is_empty());
}

#[test]
fn alias_resolves_on_add() {
    let (catalog, ids) = catalog_with_tag_sets(&[&[]]);
    catalog.set_alias("NYC", "Places/USA/New York").unwrap();
    assert_eq!(
        catalog.resolve_alias(" nyc ").unwrap(),
        "places/usa/new york"
    );
    assert_eq!(catalog.resolve_alias("paris").unwrap(), "paris");
    catalog.add_tags_batch(&ids, &["nyc".to_string()]).unwrap();
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["places/usa/new york".to_string()],
        "typing an alias stores its keyword, never the alias"
    );
    assert_eq!(
        catalog.aliases().unwrap(),
        vec![("nyc".to_string(), "places/usa/new york".to_string())]
    );
    catalog.remove_alias("nyc").unwrap();
    assert!(catalog.aliases().unwrap().is_empty());
}

#[test]
fn alias_cannot_shadow_keyword() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["places/paris"]]);
    assert!(
        catalog.set_alias("places/paris", "france").is_err(),
        "an alias cannot hide a keyword in use"
    );
    assert!(
        catalog.set_alias("places", "france").is_err(),
        "nor a parent path in use"
    );
    assert!(catalog.aliases().unwrap().is_empty());
}

#[test]
fn rename_retargets_aliases() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["places/usa/new york"]]);
    catalog.set_alias("nyc", "places/usa/new york").unwrap();
    catalog.set_alias("beach", "fun").unwrap();
    let change = catalog.rename_tag("places", "world").unwrap();
    assert_eq!(
        catalog.aliases().unwrap(),
        vec![
            ("beach".to_string(), "fun".to_string()),
            ("nyc".to_string(), "world/usa/new york".to_string()),
        ],
        "aliases into the moved subtree follow it"
    );
    assert_eq!(
        change.aliases_before,
        vec![("nyc".to_string(), "places/usa/new york".to_string())]
    );
    assert_eq!(
        change.aliases_after,
        vec![("nyc".to_string(), "world/usa/new york".to_string())]
    );
}

#[test]
fn subtree_item_count_is_distinct() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["a/x", "a/y"], &["a"], &["ab"]]);
    assert_eq!(
        catalog.subtree_item_count("a").unwrap(),
        2,
        "an item with two children counts once; the sibling ab does not count"
    );
    assert_eq!(catalog.subtree_item_count("a/x").unwrap(), 1);
}

#[test]
fn alias_chain_resolves_to_the_final_keyword_in_either_order() {
    for order in [["a", "b"], ["b", "a"]] {
        let (catalog, ids) = catalog_with_tag_sets(&[&[]]);
        for alias in order {
            match alias {
                "a" => catalog.set_alias("a", "b").unwrap(),
                _ => catalog.set_alias("b", "c").unwrap(),
            }
        }
        assert_eq!(
            catalog.aliases().unwrap(),
            vec![
                ("a".to_string(), "c".to_string()),
                ("b".to_string(), "c".to_string()),
            ],
            "set in order {order:?}: an alias never points at another alias"
        );
        catalog.add_tags_batch(&ids, &["a".to_string()]).unwrap();
        assert_eq!(
            catalog.get_tags_for_item(ids[0]).unwrap(),
            vec!["c".to_string()],
            "set in order {order:?}: an alias is never stored on media"
        );
    }
}

#[test]
fn moving_up_keeps_a_child_landing_inside_the_old_subtree() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a/b/x", "a/b/b/y"]]);
    let change = catalog.rename_tag("a/b", "a").expect("move up");
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["a/b/y".to_string(), "a/x".to_string()],
        "a/b/b/y lands on a/b/y, inside the old subtree, and must survive"
    );
    assert_eq!(search_ids(&catalog, "y"), vec![ids[0]]);
    assert_eq!(search_ids(&catalog, "x"), vec![ids[0]]);
    assert!(
        change.removed.iter().all(|row| !change.added.contains(row)),
        "a row both removed and re-added is unchanged and listed in neither: {change:?}"
    );
    assert_eq!(
        change.removed,
        vec![
            (ids[0], "a/b/b/y".to_string()),
            (ids[0], "a/b/x".to_string())
        ]
    );
    assert_eq!(
        change.added,
        vec![(ids[0], "a/b/y".to_string()), (ids[0], "a/x".to_string())]
    );
}

#[test]
fn moving_up_onto_an_existing_row_lists_it_in_neither() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a/b/x", "a/b/b/x"]]);
    let change = catalog.rename_tag("a/b", "a").expect("move up");
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["a/b/x".to_string(), "a/x".to_string()]
    );
    assert_eq!(change.removed, vec![(ids[0], "a/b/b/x".to_string())]);
    assert_eq!(change.added, vec![(ids[0], "a/x".to_string())]);
}

#[test]
fn rename_refuses_an_over_long_moved_child() {
    let child = format!("a/{}", "c".repeat(95));
    let (catalog, ids) = catalog_with_tag_sets(&[&["a", child.as_str()]]);
    assert!(
        catalog.rename_tag("a", "bbbbbbbb").is_err(),
        "a child that would pass the length cap is refused, not cut"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["a".to_string(), child],
        "nothing is written"
    );
}

#[test]
fn rename_refuses_a_moved_child_that_is_an_alias() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a", "a/x"]]);
    catalog.set_alias("b/x", "z").unwrap();
    assert!(catalog.rename_tag("a", "b").is_err());
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["a".to_string(), "a/x".to_string()],
        "nothing is written"
    );
}

#[test]
fn no_keyword_is_stored_below_an_alias() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a"]]);
    catalog.set_alias("nyc", "places/usa/new york").unwrap();
    assert!(
        catalog
            .add_tags_batch(&ids, &["nyc/soho".to_string()])
            .is_err(),
        "a path below an alias is refused"
    );
    assert!(
        catalog.rename_tag("a", "nyc/m").is_err(),
        "a rename below an alias is refused"
    );
    assert_eq!(
        catalog.get_tags_for_item(ids[0]).unwrap(),
        vec!["a".to_string()],
        "nothing is written"
    );
    assert!(
        catalog.set_alias("places", "places/usa").is_err(),
        "an alias cannot be an ancestor of its own keyword"
    );
    assert!(
        catalog.set_alias("places", "elsewhere").is_err(),
        "an alias cannot be an ancestor of another alias's keyword"
    );
    assert_eq!(catalog.aliases().unwrap().len(), 1);
}

#[test]
fn rename_records_only_the_albums_it_rewrote() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["a"]]);
    let rule = |v: &str| SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::Equals,
        value: RuleValue::Text(v.to_string()),
    };
    let touched = catalog
        .create_smart_album("A", "tag", MatchMode::All, &[rule(" A ")])
        .unwrap();
    let untouched = catalog
        .create_smart_album("Other", "tag", MatchMode::All, &[rule("other")])
        .unwrap();
    let change = catalog.rename_tag("a", "b").unwrap();
    let parse = |json: &str| serde_json::from_str::<Vec<SmartAlbumRule>>(json).unwrap();
    assert_eq!(change.albums_before.len(), 1, "only the touched album");
    assert_eq!(change.albums_before[0].0, touched.id);
    assert_eq!(parse(&change.albums_before[0].1), vec![rule(" A ")]);
    assert_eq!(change.albums_after.len(), 1);
    assert_eq!(change.albums_after[0].0, touched.id);
    assert_eq!(
        parse(&change.albums_after[0].1),
        vec![rule("b")],
        "a rule value is matched normalized and stored as the new path"
    );
    assert_eq!(
        catalog
            .get_smart_album(untouched.id)
            .unwrap()
            .unwrap()
            .rules,
        vec![rule("other")]
    );
}

#[test]
fn bulk_delete_of_overlapping_roots_lists_each_row_once() {
    let (catalog, ids) = catalog_with_tag_sets(&[&["a", "a/x"]]);
    let change = catalog
        .delete_tags_globally(&["a".to_string(), "a/x".to_string()])
        .unwrap();
    assert_eq!(
        change.removed,
        vec![(ids[0], "a".to_string()), (ids[0], "a/x".to_string())]
    );
}

#[test]
fn subtree_item_count_skips_trashed_and_hidden() {
    let catalog = Catalog::open_in_memory().expect("open catalog");
    let mut hidden = sample_media_item("/photos/kw3/h.jpg", MediaType::Image, 1, None, None);
    hidden.is_hidden = true;
    let items = [
        sample_media_item("/photos/kw3/v.jpg", MediaType::Image, 2, None, None),
        sample_media_item("/photos/kw3/t.jpg", MediaType::Image, 3, None, None),
        hidden,
    ];
    let ids: Vec<MediaId> = catalog
        .insert_or_update_batch(&items)
        .unwrap()
        .iter()
        .map(|it| it.id)
        .collect();
    catalog.add_tags_batch(&ids, &["a/x".to_string()]).unwrap();
    catalog.set_trashed(ids[1], true).unwrap();
    assert_eq!(
        catalog.subtree_item_count("a").unwrap(),
        1,
        "trashed and hidden items are not counted, as in the tag manager"
    );
}

// --- Merge into another keyword keeps the source name as an alias ---

#[test]
fn merge_keeps_source_as_alias() {
    let (catalog, ids) = catalog_with_tags(&["golden", "golden hour"]);
    let change = catalog.merge_tags("golden", "golden hour").unwrap();
    assert_eq!(catalog.get_tags_for_item(ids[0]).unwrap(), ["golden hour"]);
    assert_eq!(
        catalog.resolve_alias("Golden").unwrap(),
        "golden hour",
        "typing the merged name must apply the target"
    );
    assert_eq!(
        search_ids(&catalog, "hour"),
        vec![ids[0], ids[1]],
        "the merged item is indexed under the target"
    );

    catalog.replace_tag_rows(&change, true).unwrap();
    assert!(
        catalog.aliases().unwrap().is_empty(),
        "undo must remove the alias the merge added"
    );
    assert_eq!(catalog.resolve_alias("golden").unwrap(), "golden");
    assert_eq!(catalog.get_tags_for_item(ids[0]).unwrap(), ["golden"]);

    catalog.replace_tag_rows(&change, false).unwrap();
    assert_eq!(
        catalog.resolve_alias("golden").unwrap(),
        "golden hour",
        "redo must restore the alias"
    );
}

#[test]
fn merge_skips_the_alias_for_a_parent_with_children() {
    let (catalog, ids) = catalog_with_tags(&["trip", "trip/rome", "places"]);
    catalog.merge_tags("trip", "places").unwrap();
    assert_eq!(catalog.get_tags_for_item(ids[1]).unwrap(), ["places/rome"]);
    assert!(
        catalog.aliases().unwrap().is_empty(),
        "an alias on a parent would refuse every keyword typed below it"
    );
}

#[test]
fn merge_refuses_itself_and_a_keyword_below_it() {
    let (catalog, _) = catalog_with_tags(&["trip", "trip/rome"]);
    let err = catalog.merge_tags("trip", "Trip").unwrap_err();
    assert!(err.to_string().contains("into itself"), "{err}");
    let err = catalog.merge_tags("trip", "trip/rome").unwrap_err();
    assert!(err.to_string().contains("below it"), "{err}");
    assert!(catalog.aliases().unwrap().is_empty());
}

fn any_of(tags: &[&str]) -> SmartAlbumRule {
    SmartAlbumRule {
        field: RuleField::Tag,
        operator: RuleOperator::AnyOf,
        value: RuleValue::List(tags.iter().map(|t| t.to_string()).collect()),
    }
}

#[test]
fn tag_any_of_matches_either_keyword_and_subtrees() {
    let (catalog, ids) = catalog_with_tag_sets(&[
        &["beach"],
        &["places/paris"],
        &["placesx"],
        &["mountain"],
        &[],
    ]);
    let album = lumina_core::SmartAlbum::new(
        "Trips".into(),
        "tag".into(),
        MatchMode::All,
        vec![any_of(&["Beach", "places"])],
    );
    let mut got: Vec<u64> = catalog
        .query_smart_album(&album, 50, 0)
        .unwrap()
        .iter()
        .map(|m| m.id.0)
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![ids[0].0, ids[1].0],
        "either keyword or a keyword below it matches; a sibling prefix does not"
    );
    assert_eq!(catalog.query_smart_album_count(&album).unwrap(), 2);

    let empty = lumina_core::SmartAlbum::new(
        String::new(),
        String::new(),
        MatchMode::All,
        vec![any_of(&[])],
    );
    assert!(
        empty.build_sql().is_err(),
        "an empty list is refused, not a panic"
    );
    let blank = lumina_core::SmartAlbum::new(
        String::new(),
        String::new(),
        MatchMode::All,
        vec![any_of(&["  "])],
    );
    assert!(blank.build_sql().is_err(), "a list of blanks is refused");
}

#[test]
fn rename_rewrites_any_of_members() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["a", "a/x"], &["b"], &["ab"]]);
    let album = catalog
        .create_smart_album("Any", "tag", MatchMode::All, &[any_of(&["a/x", "b", "ab"])])
        .unwrap();
    catalog.rename_tag("a", "c").expect("rename");
    let rules = catalog.get_smart_album(album.id).unwrap().unwrap().rules;
    assert_eq!(
        rules[0].value,
        RuleValue::List(vec!["c/x".into(), "b".into(), "ab".into()]),
        "members in the renamed subtree follow it; a sibling prefix stays"
    );
    catalog.merge_tags("b", "c/x").expect("merge");
    let rules = catalog.get_smart_album(album.id).unwrap().unwrap().rules;
    assert_eq!(
        rules[0].value,
        RuleValue::List(vec!["c/x".into(), "ab".into()]),
        "a merge rewrites members and drops the duplicate it makes"
    );
}

#[test]
fn tag_any_of_plan_seeks_the_tag_index() {
    let (catalog, _ids) = catalog_with_tag_sets(&[&["beach"], &["places/paris"]]);
    let album = lumina_core::SmartAlbum::new(
        String::new(),
        String::new(),
        MatchMode::All,
        vec![any_of(&["beach", "places"])],
    );
    let (sql, params) = album.synthesize_count_query().unwrap();
    let rows = catalog
        .explain_query_plan(&inline_sql_params(&sql, &params))
        .expect("explain");
    let plan = rows.join(" | ");
    println!("any-of count plan: {plan}");
    assert!(
        rows.iter()
            .any(|r| r.contains("media_tags") && r.contains("INDEX idx_media_tags_tag")),
        "each keyword seeks idx_media_tags_tag: {plan}"
    );
    assert!(
        !rows.iter().any(|r| r.starts_with("SCAN media_tags")),
        "media_tags is never scanned: {plan}"
    );
}
