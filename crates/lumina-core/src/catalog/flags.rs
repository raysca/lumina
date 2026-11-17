//! Culling flags (schema v20): one `media_items.flag` column, -1 reject,
//! 0 unflagged, 1 pick. Read as a map like ratings, so `MediaItem` stays
//! unchanged. Flags stay in the catalog, not XMP (Lightroom does not
//! round-trip picks, and `xmp:Rating=-1` would overwrite stars).

use super::view_scope::{view_where, ViewScope};
use super::Catalog;
use crate::search::SqlParts;
use crate::{Flag, MediaId};
use anyhow::{Context, Result};
use std::collections::HashMap;

/// The rejected items of one view, with the Delete dialog's header summary.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RejectedItems {
    pub ids: Vec<MediaId>,
    pub photos: usize,
    pub videos: usize,
    pub total_bytes: u64,
}

/// One query: the reject flag ANDed with the view's own WHERE. Trashed and
/// hidden items are left out (a trashed reject is already deleted), and
/// those two terms also let SQLite seek the partial `idx_media_flag`.
pub(crate) fn rejected_sql(scope: &ViewScope) -> Result<SqlParts> {
    let (view, mut params) = view_where(scope)?;
    params.insert(0, (Flag::Reject as i64).into());
    Ok((
        format!(
            "SELECT m.id, m.media_type, m.file_size FROM media_items m \
             WHERE m.flag = ? AND m.is_trashed = 0 AND m.is_hidden = 0 AND ({view});"
        ),
        params,
    ))
}

impl Catalog {
    /// Sets each item's flag in one transaction. Ids with no row are
    /// skipped without failing the batch.
    pub fn set_flags(&self, changes: &[(MediaId, Flag)]) -> Result<()> {
        self.update_each(
            "UPDATE media_items SET flag = ?1 WHERE id = ?2;",
            changes.iter().map(|&(id, flag)| (id, flag as i64)),
        )
        .context("saving flags")
    }

    /// Every flagged item (picks and rejects); unflagged items are left out.
    pub fn get_flags_map(&self) -> Result<HashMap<MediaId, Flag>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT id, flag FROM media_items WHERE flag != 0;")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                MediaId(row.get::<_, i64>(0)? as u64),
                Flag::from_db(row.get(1)?),
            ))
        })?;
        rows.collect::<rusqlite::Result<_>>()
            .context("reading flags")
    }

    /// The rejected items of `scope`, in one query (Delete Rejected…).
    pub fn rejected_in(&self, scope: &ViewScope) -> Result<RejectedItems> {
        let (sql, params) = rejected_sql(scope)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
        let mut found = RejectedItems::default();
        while let Some(row) = rows.next()? {
            found.ids.push(MediaId(row.get::<_, i64>(0)? as u64));
            if row.get::<_, String>(1)? == "video" {
                found.videos += 1;
            } else {
                found.photos += 1;
            }
            found.total_bytes += row.get::<_, i64>(2)?.max(0) as u64;
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::files::tests::TempDir;
    use crate::catalog::tests_support::video_item;
    use crate::{
        MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbum, SmartAlbumRule, StructuredQuery,
        SCHEMA_VERSION,
    };

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[video_item(path)]).unwrap()[0].id
    }

    fn flag_column(catalog: &Catalog) -> Vec<i64> {
        let conn = catalog.conn.lock();
        let mut stmt = conn
            .prepare("SELECT flag FROM media_items ORDER BY id;")
            .unwrap();
        let rows = stmt.query_map([], |row| row.get(0)).unwrap();
        rows.map(|row| row.unwrap()).collect()
    }

    #[test]
    fn migration_adds_flag_idempotently() {
        let dir = TempDir::new("flag_migration");
        let db = dir.0.join("catalog.db");
        {
            let catalog = Catalog::open(&db).unwrap();
            plain(&catalog, "/lib/a.mp4");
            plain(&catalog, "/lib/b.mp4");
            // Back to the v19 layout: media_items without flag.
            catalog
                .conn
                .lock()
                .execute_batch(
                    r#"
                    DROP INDEX IF EXISTS idx_media_flag;
                    ALTER TABLE media_items DROP COLUMN flag;
                    PRAGMA user_version = 19;
                    "#,
                )
                .unwrap();
        }
        for pass in 0..2 {
            let catalog = Catalog::open(&db).unwrap();
            assert_eq!(
                catalog.schema_version().unwrap(),
                SCHEMA_VERSION,
                "pass {pass}: opening stamps the current schema version"
            );
            assert_eq!(
                flag_column(&catalog),
                vec![0, 0],
                "pass {pass}: existing rows start unflagged"
            );
            assert!(
                catalog.get_flags_map().unwrap().is_empty(),
                "pass {pass}: no flags yet"
            );
            assert!(
                catalog
                    .conn
                    .lock()
                    .prepare("SELECT 1 FROM sqlite_master WHERE name = 'idx_media_flag'")
                    .unwrap()
                    .exists([])
                    .unwrap(),
                "pass {pass}: the upgrade creates the flag index"
            );
        }
        let catalog = Catalog::open(&db).unwrap();
        let ids: Vec<MediaId> = catalog
            .get_item_slice(0, 10)
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect();
        catalog.set_flags(&[(ids[0], Flag::Pick)]).unwrap();
        assert_eq!(
            catalog.get_flags_map().unwrap(),
            HashMap::from([(ids[0], Flag::Pick)]),
            "the upgraded catalog accepts flag writes"
        );
    }

    #[test]
    fn set_flags_round_trips_and_map_omits_unflagged() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        let c = plain(&catalog, "/lib/c.mp4");
        catalog
            .set_flags(&[(a, Flag::Pick), (b, Flag::Reject), (c, Flag::Pick)])
            .unwrap();
        assert_eq!(
            catalog.get_flags_map().unwrap(),
            HashMap::from([(a, Flag::Pick), (b, Flag::Reject), (c, Flag::Pick)]),
            "picks and rejects round-trip"
        );
        catalog
            .set_flags(&[(c, Flag::Unflagged), (b, Flag::Pick)])
            .unwrap();
        assert_eq!(
            catalog.get_flags_map().unwrap(),
            HashMap::from([(a, Flag::Pick), (b, Flag::Pick)]),
            "unflagging removes the item from the map; a flag can change"
        );
        assert_eq!(flag_column(&catalog), vec![1, 1, 0]);
        catalog
            .set_flags(&[(MediaId(9_999), Flag::Reject)])
            .expect("an unknown id is skipped, not an error");
    }

    fn flag_album(operator: RuleOperator, flag: Flag) -> SmartAlbum {
        SmartAlbum::new(
            "Flags".into(),
            "folder".into(),
            MatchMode::All,
            vec![SmartAlbumRule {
                field: RuleField::Flag,
                operator,
                value: RuleValue::Integer(flag as i64),
            }],
        )
    }

    fn smart_ids(catalog: &Catalog, album: &SmartAlbum) -> Vec<MediaId> {
        let mut ids: Vec<MediaId> = catalog
            .query_smart_album(album, 50, 0)
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    fn plan_of(catalog: &Catalog, sql: &str, params: &[rusqlite::types::Value]) -> String {
        let conn = catalog.conn.lock();
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| {
                row.get::<_, String>(3)
            })
            .unwrap();
        rows.map(|row| row.unwrap()).collect::<Vec<_>>().join(" | ")
    }

    #[test]
    fn flag_rule_matches_picks() {
        let catalog = Catalog::open_in_memory().unwrap();
        let pick = plain(&catalog, "/lib/pick.mp4");
        let reject = plain(&catalog, "/lib/reject.mp4");
        let none = plain(&catalog, "/lib/none.mp4");
        let trashed_pick = plain(&catalog, "/lib/trashed.mp4");
        catalog
            .set_flags(&[
                (pick, Flag::Pick),
                (reject, Flag::Reject),
                (trashed_pick, Flag::Pick),
            ])
            .unwrap();
        catalog.set_trashed(trashed_pick, true).unwrap();

        let picks = flag_album(RuleOperator::Equals, Flag::Pick);
        assert_eq!(
            smart_ids(&catalog, &picks),
            vec![pick],
            "Flag is Pick matches the picks, never a trashed one"
        );
        assert_eq!(catalog.query_smart_album_count(&picks).unwrap(), 1);
        assert_eq!(
            smart_ids(&catalog, &flag_album(RuleOperator::NotEquals, Flag::Pick)),
            vec![reject, none],
            "Flag is not Pick matches rejects and unflagged items"
        );
        assert_eq!(
            smart_ids(&catalog, &flag_album(RuleOperator::Equals, Flag::Unflagged)),
            vec![none]
        );

        let (sql, params) = picks.synthesize_count_query().unwrap();
        assert!(
            sql.contains("m.flag = ?"),
            "the rule compiles to m.flag = ?, got {sql}"
        );
        let plan = plan_of(&catalog, &sql, &params);
        assert!(
            plan.contains("idx_media_flag"),
            "a Flag is rule seeks idx_media_flag, got: {plan}"
        );

        let mut bad = flag_album(RuleOperator::AtLeast, Flag::Pick);
        assert!(bad.build_sql().is_err(), "Flag supports only is / is not");
        bad.rules[0].operator = RuleOperator::Equals;
        bad.rules[0].value = RuleValue::Integer(5);
        assert!(
            bad.build_sql().is_err(),
            "a flag value other than pick, reject or unflagged is refused"
        );
    }

    #[test]
    fn rejected_in_scopes_to_the_view() {
        let catalog = Catalog::open_in_memory().unwrap();
        let in_album = plain(&catalog, "/lib/beach.mp4");
        let in_album_kept = plain(&catalog, "/lib/beach2.mp4");
        let outside = plain(&catalog, "/lib/city.mp4");
        let trashed = plain(&catalog, "/lib/gone.mp4");
        catalog
            .set_flags(&[
                (in_album, Flag::Reject),
                (in_album_kept, Flag::Pick),
                (outside, Flag::Reject),
                (trashed, Flag::Reject),
            ])
            .unwrap();
        catalog.set_trashed(trashed, true).unwrap();
        let album = catalog
            .create_album_with("Trip", &[in_album, in_album_kept, trashed])
            .unwrap();

        let ids = |scope: &ViewScope| {
            let mut ids = catalog.rejected_in(scope).unwrap().ids;
            ids.sort_by_key(|id| id.0);
            ids
        };
        assert_eq!(
            ids(&ViewScope::Album(album.id)),
            vec![in_album],
            "an album's rejects only, never one outside it or in the Trash"
        );
        assert_eq!(ids(&ViewScope::Library), vec![in_album, outside]);
        assert_eq!(ids(&ViewScope::Search("beach".into())), vec![in_album]);
        assert_eq!(
            ids(&ViewScope::Search("is:trashed".into())),
            Vec::<MediaId>::new(),
            "trashed rejects are never offered for deletion again"
        );
        assert_eq!(ids(&ViewScope::Untagged), vec![in_album, outside]);
        assert_eq!(
            ids(&ViewScope::System(crate::SystemAlbum::Videos)),
            vec![in_album, outside]
        );
        let smart = flag_album(RuleOperator::NotEquals, Flag::Pick);
        assert_eq!(ids(&ViewScope::Smart(smart)), vec![in_album, outside]);

        let found = catalog.rejected_in(&ViewScope::Library).unwrap();
        assert_eq!(
            (found.photos, found.videos, found.total_bytes),
            (0, 2, 2_000),
            "the dialog header summary comes with the ids"
        );

        let (sql, params) = rejected_sql(&ViewScope::Library).unwrap();
        let plan = plan_of(&catalog, &sql, &params);
        assert!(
            plan.contains("idx_media_flag"),
            "the reject lookup seeks idx_media_flag, got: {plan}"
        );
    }

    #[test]
    fn search_is_pick_and_reject() {
        let catalog = Catalog::open_in_memory().unwrap();
        let pick = plain(&catalog, "/lib/pick.mp4");
        let reject = plain(&catalog, "/lib/reject.mp4");
        let none = plain(&catalog, "/lib/none.mp4");
        let trashed_pick = plain(&catalog, "/lib/trashed.mp4");
        let hidden_pick = plain(&catalog, "/lib/hidden.mp4");
        catalog
            .set_flags(&[
                (pick, Flag::Pick),
                (reject, Flag::Reject),
                (trashed_pick, Flag::Pick),
                (hidden_pick, Flag::Pick),
            ])
            .unwrap();
        catalog.set_trashed(trashed_pick, true).unwrap();
        catalog
            .conn
            .lock()
            .execute(
                "UPDATE media_items SET is_hidden = 1 WHERE id = ?1;",
                [hidden_pick.0 as i64],
            )
            .unwrap();
        let found = |text: &str| -> Vec<MediaId> {
            let mut ids: Vec<MediaId> = catalog
                .search_structured(&StructuredQuery::parse(text), 50, 0)
                .unwrap()
                .into_iter()
                .map(|item| item.id)
                .collect();
            ids.sort_by_key(|id| id.0);
            ids
        };
        assert_eq!(
            found("is:pick"),
            vec![pick],
            "picks, with trashed and hidden ones excluded"
        );
        assert_eq!(found("is:reject"), vec![reject]);
        assert_eq!(found("is:unflagged"), vec![none]);
        assert_eq!(
            catalog
                .search_structured_count(&StructuredQuery::parse("is:pick"))
                .unwrap(),
            1,
            "the count uses the same clause"
        );
    }
}
