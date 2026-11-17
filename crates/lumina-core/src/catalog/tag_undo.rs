//! Replaying a global keyword rename, merge or delete ([`TagChange`]) for
//! undo and redo.

use anyhow::Result;
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};

use std::collections::HashSet;

use super::keyword_ops::refuse_alias_path;
use super::keywords::{subtree_clause, subtree_params};
use super::{Catalog, TagChange};

impl TagChange {
    /// Folds `later` (an operation run right after this one) into this
    /// change, so several merges undo as one step. A row the first added and
    /// `later` removed (or the reverse) cancels out; an album or alias both
    /// touched keeps its first `before` and its last `after`.
    pub fn then(&mut self, later: TagChange) {
        for row in later.removed {
            match self.added.iter().position(|r| *r == row) {
                Some(i) => {
                    self.added.remove(i);
                }
                None => self.removed.push(row),
            }
        }
        for row in later.added {
            match self.removed.iter().position(|r| *r == row) {
                Some(i) => {
                    self.removed.remove(i);
                }
                None => self.added.push(row),
            }
        }
        fold(
            &mut self.albums_before,
            &mut self.albums_after,
            later.albums_before,
            later.albums_after,
        );
        self.aliases_added.extend(later.aliases_added);
        fold(
            &mut self.aliases_before,
            &mut self.aliases_after,
            later.aliases_before,
            later.aliases_after,
        );
    }
}

/// Merges keyed `(before, after)` lists: a key already present keeps its
/// first `before` and takes the later `after`.
fn fold<K: PartialEq, V>(
    before: &mut Vec<(K, V)>,
    after: &mut Vec<(K, V)>,
    later_before: Vec<(K, V)>,
    later_after: Vec<(K, V)>,
) {
    for (b, a) in later_before.into_iter().zip(later_after) {
        match after.iter_mut().find(|(k, _)| *k == a.0) {
            Some(slot) => *slot = a,
            None => {
                before.push(b);
                after.push(a);
            }
        }
    }
}

/// What a [`Catalog::replace_tag_rows`] replay left alone because it changed
/// since, so the caller can say so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TagReplay {
    /// Smart albums whose rules were edited since.
    pub albums: usize,
    /// Aliases pointed at another keyword since.
    pub aliases: usize,
}

impl Catalog {
    /// Puts back the keyword rows, aliases and smart-album rules before
    /// (`before = true`, undo) or after (redo) a [`TagChange`], in one
    /// transaction with the search index refreshed for every affected item.
    ///
    /// Refuses the whole replay, writing nothing, when a keyword it would put
    /// back is now an alias or sits below one (the rule
    /// [`refuse_alias_path`] enforces on every write); the error says to
    /// remove the alias first. Skipping those rows instead would delete the
    /// item's current keyword and store none.
    ///
    /// Rows for an item purged since are skipped. An alias or album changed
    /// since (it no longer holds the side being replaced) keeps its later
    /// value, so a later edit is never overwritten; those are counted in the
    /// result.
    pub fn replace_tag_rows(&self, change: &TagChange, before: bool) -> Result<TagReplay> {
        let (drop_rows, put_rows) = if before {
            (&change.added, &change.removed)
        } else {
            (&change.removed, &change.added)
        };
        let (aliases_from, aliases_to) = if before {
            (&change.aliases_after, &change.aliases_before)
        } else {
            (&change.aliases_before, &change.aliases_after)
        };
        let (albums_from, albums_to) = if before {
            (&change.albums_after, &change.albums_before)
        } else {
            (&change.albums_before, &change.albums_after)
        };
        let ids: Vec<i64> = change.affected_ids().iter().map(|id| id.0 as i64).collect();
        let mut skipped = TagReplay::default();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        // Undo drops the aliases a merge added first: the source name it
        // puts back would otherwise be refused as an alias.
        if before {
            let mut delete =
                tx.prepare_cached("DELETE FROM tag_aliases WHERE alias = ?1 AND tag = ?2;")?;
            for (alias, tag) in &change.aliases_added {
                if delete.execute(params![alias, tag])? == 0 {
                    skipped.aliases += 1;
                }
            }
        }
        let paths: HashSet<&str> = put_rows.iter().map(|(_, tag)| tag.as_str()).collect();
        for path in paths {
            refuse_alias_path(&tx, path)?;
        }
        // The index must be deleted with the values it holds now, so before
        // any row changes, and rebuilt after all of them (contentless FTS).
        Self::delete_fts_for_items(&tx, &ids)?;
        {
            let mut delete =
                tx.prepare_cached("DELETE FROM media_tags WHERE media_id = ?1 AND tag = ?2;")?;
            for (id, tag) in drop_rows {
                delete.execute(params![id.0 as i64, tag])?;
            }
            // A purged item has no media row: skip it rather than fail the
            // foreign key and lose the whole undo.
            let mut insert = tx.prepare_cached(
                "INSERT OR IGNORE INTO media_tags (media_id, tag) \
                 SELECT ?1, ?2 WHERE EXISTS (SELECT 1 FROM media_items WHERE id = ?1);",
            )?;
            for (id, tag) in put_rows {
                insert.execute(params![id.0 as i64, tag])?;
            }
        }
        Self::insert_fts_for_items(&tx, &ids)?;
        if !before {
            // Skipped when the name was aliased again since, or it or a
            // keyword below it was tagged again (the `set_alias` rule).
            let mut insert = tx.prepare_cached(&format!(
                "INSERT OR IGNORE INTO tag_aliases (alias, tag) SELECT ?, ? \
                 WHERE NOT EXISTS (SELECT 1 FROM media_tags WHERE {});",
                subtree_clause("tag")
            ))?;
            for (alias, tag) in &change.aliases_added {
                let mut values = vec![Value::Text(alias.clone()), Value::Text(tag.clone())];
                values.extend(subtree_params(alias));
                if insert.execute(params_from_iter(values))? == 0 {
                    skipped.aliases += 1;
                }
            }
        }
        {
            let mut alias = tx
                .prepare_cached("UPDATE tag_aliases SET tag = ?3 WHERE alias = ?1 AND tag = ?2;")?;
            for ((name, from), (_, to)) in aliases_from.iter().zip(aliases_to) {
                if alias.execute(params![name, from, to])? == 0 {
                    skipped.aliases += 1;
                }
            }
        }
        {
            let mut album = tx.prepare_cached(
                "UPDATE smart_albums SET rules_json = ?3 WHERE id = ?1 AND rules_json = ?2;",
            )?;
            for ((id, from), (_, to)) in albums_from.iter().zip(albums_to) {
                if album.execute(params![id, from, to])? == 0 {
                    skipped.albums += 1;
                }
            }
        }
        tx.commit()?;
        Ok(skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::tests_support::video_item as item;
    use crate::search::StructuredQuery;
    use crate::smart_albums::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};
    use crate::MediaId;

    fn plain(catalog: &Catalog, path: &str) -> MediaId {
        catalog.insert_or_update_batch(&[item(path)]).unwrap()[0].id
    }

    fn tags(catalog: &Catalog, id: MediaId) -> Vec<String> {
        let mut tags = catalog.get_tags_for_item(id).unwrap();
        tags.sort();
        tags
    }

    fn search(catalog: &Catalog, query: &str) -> Vec<MediaId> {
        let mut ids: Vec<MediaId> = catalog
            .search_structured(&StructuredQuery::parse(query), 50, 0)
            .unwrap()
            .into_iter()
            .map(|item| item.id)
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    fn ids_of(rows: &[(MediaId, String)]) -> HashSet<MediaId> {
        rows.iter().map(|(id, _)| *id).collect()
    }

    fn tag_rule(value: &str) -> SmartAlbumRule {
        SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::Equals,
            value: RuleValue::Text(value.into()),
        }
    }

    #[test]
    fn replace_tag_rows_is_atomic_and_reindexes_fts() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        catalog.set_caption(a, "sunset").unwrap();
        catalog
            .add_tags_batch(&[a], &["zebrafoo".into(), "zebrafoo/kid".into()])
            .unwrap();
        catalog.add_tags_batch(&[b], &["quokkabar".into()]).unwrap();
        catalog
            .create_smart_album("Z", "tag", MatchMode::All, &[tag_rule("zebrafoo")])
            .unwrap();
        catalog.set_alias("zf", "zebrafoo").unwrap();
        let change = catalog.rename_tag("zebrafoo", "quokkabar").unwrap();
        assert_eq!(ids_of(&change.removed), HashSet::from([a]));
        assert!(search(&catalog, "zebrafoo").is_empty());

        let skipped = catalog.replace_tag_rows(&change, true).unwrap();
        assert_eq!(skipped, TagReplay::default());
        assert_eq!(tags(&catalog, a), ["zebrafoo", "zebrafoo/kid"]);
        assert_eq!(tags(&catalog, b), ["quokkabar"]);
        assert_eq!(
            search(&catalog, "zebrafoo"),
            vec![a],
            "undo re-indexes the restored keyword: only it holds that word"
        );
        assert_eq!(search(&catalog, "quokkabar"), vec![b], "undo unindexes it");
        assert_eq!(search(&catalog, "sunset"), vec![a], "the caption stays");
        assert_eq!(
            catalog.aliases().unwrap(),
            [("zf".to_string(), "zebrafoo".to_string())]
        );
        let rules = &catalog.list_smart_albums().unwrap()[0].rules;
        assert_eq!(rules[0].value, RuleValue::Text("zebrafoo".into()));

        catalog.replace_tag_rows(&change, false).unwrap();
        assert_eq!(tags(&catalog, a), ["quokkabar", "quokkabar/kid"], "redo");
        assert!(search(&catalog, "zebrafoo").is_empty(), "redo unindexes it");
        assert_eq!(search(&catalog, "quokkabar"), vec![a, b]);

        // Atomic: a failure part way (a dropped table here) writes nothing,
        // the row deletes before it included.
        let conn = catalog.conn.lock();
        conn.execute_batch("DROP TABLE tag_aliases;").unwrap();
        drop(conn);
        assert!(catalog.replace_tag_rows(&change, true).is_err());
        assert_eq!(
            tags(&catalog, a),
            ["quokkabar", "quokkabar/kid"],
            "a failed replay leaves every row as it was"
        );
        assert_eq!(search(&catalog, "quokkabar"), vec![a, b]);
    }

    #[test]
    fn replace_tag_rows_skips_purged_items_and_later_album_edits() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        let b = plain(&catalog, "/lib/b.mp4");
        catalog.add_tags_batch(&[a, b], &["cat".into()]).unwrap();
        let album = catalog
            .create_smart_album("C", "tag", MatchMode::All, &[tag_rule("cat")])
            .unwrap();
        let change = catalog.rename_tag("cat", "kitty").unwrap();
        catalog
            .update_smart_album(album.id, "C", "tag", MatchMode::All, &[tag_rule("dog")])
            .unwrap();
        catalog
            .permanently_delete_items(&[b], crate::DeletionTarget::CatalogOnly)
            .unwrap();

        let skipped = catalog.replace_tag_rows(&change, true).unwrap();
        assert_eq!(
            skipped.albums, 1,
            "the album edited since is skipped and counted"
        );
        assert_eq!(tags(&catalog, a), ["cat"]);
        let rules = &catalog.list_smart_albums().unwrap()[0].rules;
        assert_eq!(rules[0].value, RuleValue::Text("dog".into()));
    }

    #[test]
    fn then_composes_sequential_merges() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog
            .add_tags_batch(&[a], &["x".into(), "y".into()])
            .unwrap();
        catalog
            .create_smart_album("XY", "tag", MatchMode::Any, &[tag_rule("x"), tag_rule("y")])
            .unwrap();
        let mut change = catalog.merge_tags("x", "z").unwrap();
        change.then(catalog.merge_tags("y", "z").unwrap());
        assert_eq!(change.albums_before.len(), 1, "one album, listed once");

        catalog.replace_tag_rows(&change, true).unwrap();
        assert_eq!(tags(&catalog, a), ["x", "y"]);
        let rules = &catalog.list_smart_albums().unwrap()[0].rules;
        assert_eq!(rules[0].value, RuleValue::Text("x".into()));
        assert_eq!(rules[1].value, RuleValue::Text("y".into()));
    }

    #[test]
    fn replace_tag_rows_refuses_a_keyword_that_is_now_an_alias() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog
            .add_tags_batch(&[a], &["cat".into(), "cat/kid".into(), "pet".into()])
            .unwrap();
        let change = catalog.rename_tag("cat", "kitty").unwrap();
        catalog.set_alias("cat", "dog").unwrap();

        let err = catalog
            .replace_tag_rows(&change, true)
            .expect_err("undo must refuse to store a keyword on an alias");
        assert!(
            err.to_string().contains("remove the alias first"),
            "the refusal says what to do: {err}"
        );
        assert_eq!(
            tags(&catalog, a),
            ["kitty", "kitty/kid", "pet"],
            "a refused undo writes nothing, so no keyword is lost"
        );
        assert_eq!(search(&catalog, "kitty"), vec![a], "the index is untouched");

        catalog.remove_alias("cat").unwrap();
        catalog
            .replace_tag_rows(&change, true)
            .expect("undo succeeds once the alias is removed");
        assert_eq!(tags(&catalog, a), ["cat", "cat/kid", "pet"]);
        assert_eq!(search(&catalog, "cat"), vec![a]);
        assert!(search(&catalog, "kitty").is_empty());

        // Redo has the same rule for the new path.
        catalog.set_alias("kitty", "dog").unwrap();
        assert!(catalog.replace_tag_rows(&change, false).is_err());
        assert_eq!(tags(&catalog, a), ["cat", "cat/kid", "pet"]);
    }

    #[test]
    fn redo_skips_the_merge_alias_once_the_name_is_tagged_again() {
        for retag in ["golden", "golden/x"] {
            let catalog = Catalog::open_in_memory().unwrap();
            let a = plain(&catalog, "/lib/a.mp4");
            catalog.add_tags_batch(&[a], &["golden".into()]).unwrap();
            let change = catalog.merge_tags("golden", "hour").unwrap();
            catalog.replace_tag_rows(&change, true).unwrap();
            // Another item: redo deletes only the rows the merge moved.
            let b = plain(&catalog, "/lib/b.mp4");
            catalog.add_tags_batch(&[b], &[retag.into()]).unwrap();

            let skipped = catalog.replace_tag_rows(&change, false).unwrap();
            assert_eq!(skipped.aliases, 1, "redo counts the alias it left out");
            assert!(
                catalog.aliases().unwrap().is_empty(),
                "redo must not alias \"golden\" while {retag} is a keyword in use"
            );
        }
    }

    #[test]
    fn merge_keeps_a_leftover_alias_row_for_the_source() {
        let catalog = Catalog::open_in_memory().unwrap();
        let a = plain(&catalog, "/lib/a.mp4");
        catalog.add_tags_batch(&[a], &["golden".into()]).unwrap();
        // Data from before the alias rules: the keyword is also an alias.
        catalog
            .conn
            .lock()
            .execute(
                "INSERT INTO tag_aliases (alias, tag) VALUES ('golden', 'other');",
                [],
            )
            .unwrap();
        let change = catalog
            .merge_tags("golden", "hour")
            .expect("a leftover alias row must not fail the merge");
        assert!(
            change.aliases_added.is_empty(),
            "nothing added, nothing to undo"
        );
        assert_eq!(tags(&catalog, a), ["hour"]);
        assert_eq!(
            catalog.aliases().unwrap(),
            [("golden".to_string(), "other".to_string())]
        );
    }
}
