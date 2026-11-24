//! Global keyword operations: subtree rename, merge and delete, and
//! aliases. Path rules live in [`super::keywords`].

use std::collections::HashSet;

use anyhow::{bail, Result};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};

use super::keywords::{
    filter_value, is_blank_path, normalize_path, parent_of, subtree_clause, subtree_params,
    subtree_suffix,
};
use super::Catalog;
use crate::smart_albums::{RuleField, RuleValue, SmartAlbumRule};
use crate::text::MAX_NAME_CHARS;
use crate::MediaId;

/// What a global keyword rename, merge or delete changed, as exact rows, so
/// sidecar sync can find the items and undo can replay the rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagChange {
    /// `(media, tag)` rows the operation deleted.
    pub removed: Vec<(MediaId, String)>,
    /// `(media, tag)` rows it inserted; a row that already existed (a merge
    /// collision) is not listed.
    pub added: Vec<(MediaId, String)>,
    /// `(alias, tag)` rows it changed, as they were.
    pub aliases_before: Vec<(String, String)>,
    /// The same alias rows, as they are now.
    pub aliases_after: Vec<(String, String)>,
    /// `(alias, tag)` rows it created (a merge keeps the source name as an
    /// alias of the target); undo deletes them, redo inserts them again.
    pub aliases_added: Vec<(String, String)>,
    /// `(smart album id, rules_json)` of the albums whose rules it rewrote, as
    /// they were.
    pub albums_before: Vec<(i64, String)>,
    /// The same albums' rules, as they are now.
    pub albums_after: Vec<(i64, String)>,
}

impl TagChange {
    /// Every item whose tags changed, each once, in first-seen order.
    pub fn affected_ids(&self) -> Vec<MediaId> {
        let mut seen = HashSet::new();
        self.removed
            .iter()
            .chain(&self.added)
            .map(|(id, _)| *id)
            .filter(|id| seen.insert(*id))
            .collect()
    }
}

fn subtree_rows_sql() -> String {
    format!(
        "SELECT media_id, tag FROM media_tags WHERE {};",
        subtree_clause("tag")
    )
}

fn subtree_delete_sql() -> String {
    format!("DELETE FROM media_tags WHERE {};", subtree_clause("tag"))
}

/// Active (not trashed, not hidden) items, matching the tag manager counts.
fn subtree_count_sql() -> String {
    format!(
        "SELECT COUNT(DISTINCT mt.media_id) FROM media_tags mt \
         JOIN media_items m ON m.id = mt.media_id \
         WHERE {} AND m.is_trashed = 0 AND m.is_hidden = 0;",
        subtree_clause("mt.tag")
    )
}

fn subtree_rows(conn: &Connection, root: &str) -> Result<Vec<(MediaId, String)>> {
    let mut stmt = conn.prepare_cached(&subtree_rows_sql())?;
    let rows = stmt.query_map(params_from_iter(subtree_params(root)), |row| {
        Ok((MediaId(row.get::<_, i64>(0)? as u64), row.get(1)?))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn is_alias(conn: &Connection, path: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM tag_aliases WHERE alias = ?1;",
            params![path],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Refuses `path` when it, or any keyword above it, is an alias: typing the
/// alias would apply another keyword, so a keyword there could never be
/// typed and would be shadowed.
pub(crate) fn refuse_alias_path(conn: &Connection, path: &str) -> Result<()> {
    match alias_refusal(conn, path)? {
        Some(reason) => bail!(reason),
        None => Ok(()),
    }
}

/// Why `path` cannot be stored as a keyword (it is, or sits below, an
/// alias), or `None`. A database error is an `Err`, never a refusal.
fn alias_refusal(conn: &Connection, path: &str) -> Result<Option<String>> {
    let mut current = Some(path);
    while let Some(p) = current {
        if is_alias(conn, p)? {
            return Ok(Some(if p == path {
                format!("\"{p}\" is already an alias; remove the alias first")
            } else {
                format!("\"{path}\" is below the alias \"{p}\"; remove the alias first")
            }));
        }
        current = parent_of(p);
    }
    Ok(None)
}

/// The keyword `path` is an alias of, or `path` itself.
pub(crate) fn resolve_in(conn: &Connection, path: String) -> Result<String> {
    Ok(conn
        .query_row(
            "SELECT tag FROM tag_aliases WHERE alias = ?1;",
            params![path],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(path))
}

fn distinct_raw_ids(rows: &[(MediaId, String)]) -> Vec<i64> {
    let mut seen = HashSet::new();
    rows.iter()
        .map(|(id, _)| id.0 as i64)
        .filter(|id| seen.insert(*id))
        .collect()
}

type AlbumRows = Vec<(i64, String)>;

/// Rewrites every smart-album `Tag` rule (any operator) whose value, or a
/// member of whose "any of" list, is one of `roots` or below it: moved
/// under `new`, or with `new` `None` (a delete) dropped from "any of"
/// lists. Saved-search (`Query`) rules are the user's own text and are left
/// alone. Returns the touched albums' rules before and after.
fn rewrite_tag_rules(
    tx: &Transaction,
    roots: &[String],
    new: Option<&str>,
) -> Result<(AlbumRows, AlbumRows)> {
    let albums: AlbumRows = {
        let mut stmt = tx.prepare("SELECT id, rules_json FROM smart_albums;")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let (mut before, mut after) = (Vec::new(), Vec::new());
    for (id, json) in albums {
        // A rule set that does not parse is left as stored; the album
        // loader reports it.
        let Ok(mut rules) = serde_json::from_str::<Vec<SmartAlbumRule>>(&json) else {
            continue;
        };
        let mut changed = false;
        // `None`: not in a root's subtree. `Some(None)`: deleted.
        let moved = |value: &str| {
            let value = filter_value(value);
            roots
                .iter()
                .find_map(|root| subtree_suffix(&value, root))
                .map(|rest| new.map(|new| format!("{new}{rest}")))
        };
        for rule in rules.iter_mut().filter(|r| r.field == RuleField::Tag) {
            match &mut rule.value {
                // A deleted keyword stays in an "is" rule: dropping the rule
                // would widen the album to every item.
                RuleValue::Text(value) => {
                    if let Some(Some(path)) = moved(value) {
                        *value = path;
                        changed = true;
                    }
                }
                // "include any of": each member follows or, deleted, goes; a
                // merge can make two members equal, and the second goes.
                RuleValue::List(items) if items.iter().any(|item| moved(item).is_some()) => {
                    let mut seen = HashSet::new();
                    let kept: Vec<String> = items
                        .iter()
                        .filter_map(|item| moved(item).unwrap_or_else(|| Some(item.clone())))
                        .filter(|item| seen.insert(item.clone()))
                        .collect();
                    // ponytail: a list whose every member was deleted stays as
                    // stored: it matches nothing either way, and an empty list
                    // would not compile.
                    if !kept.is_empty() {
                        *items = kept;
                        changed = true;
                    }
                }
                _ => {}
            }
        }
        if changed {
            let new_json = serde_json::to_string(&rules)?;
            tx.execute(
                "UPDATE smart_albums SET rules_json = ?1 WHERE id = ?2;",
                params![new_json, id],
            )?;
            before.push((id, json));
            after.push((id, new_json));
        }
    }
    Ok((before, after))
}

type AliasRows = Vec<(String, String)>;

/// Points every alias of `old` or a keyword below it at the moved keyword.
fn retarget_aliases(tx: &Transaction, old: &str, new: &str) -> Result<(AliasRows, AliasRows)> {
    let before: AliasRows = {
        let sql = format!(
            "SELECT alias, tag FROM tag_aliases WHERE {} ORDER BY alias;",
            subtree_clause("tag")
        );
        let mut stmt = tx.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(subtree_params(old)), |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let after: AliasRows = before
        .iter()
        .map(|(alias, tag)| (alias.clone(), format!("{new}{}", &tag[old.len()..])))
        .collect();
    let mut stmt = tx.prepare("UPDATE tag_aliases SET tag = ?2 WHERE alias = ?1;")?;
    for (alias, tag) in &after {
        stmt.execute(params![alias, tag])?;
    }
    Ok((before, after))
}

impl Catalog {
    /// Renames `old_tag` and every keyword below it (`old/x` becomes
    /// `new/x`) in one transaction. A moved row that already exists on the
    /// item merges into it. Smart-album `Tag` rules and aliases in the
    /// subtree follow the move; the search index is refreshed for every
    /// affected item.
    ///
    /// The new name is stored as its [`normalize_path`] form; the old one is
    /// only trimmed and lowercased, so a tag stored before cleaning still
    /// matches. Refuses a move inside the keyword's own subtree, a name that
    /// is an alias, and a move that would make a child path too long.
    pub fn rename_tag(&self, old_tag: &str, new_tag: &str) -> Result<TagChange> {
        self.move_subtree(old_tag, new_tag, false)
    }

    /// [`Catalog::rename_tag`], and with `keep_alias` the old name becomes an
    /// alias of the new one in the same transaction, unless the old keyword
    /// had children (an alias there would refuse every keyword below it).
    fn move_subtree(&self, old_tag: &str, new_tag: &str, keep_alias: bool) -> Result<TagChange> {
        let old = old_tag.trim().to_lowercase();
        if old.is_empty() || is_blank_path(new_tag) {
            return Ok(TagChange::default());
        }
        let new = normalize_path(new_tag)?;
        if old == new {
            return Ok(TagChange::default());
        }
        if subtree_suffix(&new, &old).is_some() {
            bail!("A keyword cannot move inside itself");
        }

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        refuse_alias_path(&tx, &new)?;
        let mut removed = subtree_rows(&tx, &old)?;
        if removed.is_empty() {
            return Ok(TagChange::default());
        }
        let moved: Vec<String> = removed
            .iter()
            .map(|(_, tag)| format!("{new}{}", &tag[old.len()..]))
            .collect();
        for path in moved.iter() {
            refuse_alias_path(&tx, path)?;
            if path.chars().count() > MAX_NAME_CHARS {
                bail!("\"{path}\" would be longer than {MAX_NAME_CHARS} characters");
            }
        }

        let ids = distinct_raw_ids(&removed);
        Self::delete_fts_for_items(&tx, &ids)?;
        tx.execute(
            &subtree_delete_sql(),
            params_from_iter(subtree_params(&old)),
        )?;
        // Insert after the delete: moving up (`a/b` to `a`) can land a child
        // on a path inside the old subtree (`a/b/b/x` to `a/b/x`).
        let mut added = Vec::new();
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO media_tags (media_id, tag) VALUES (?1, ?2);",
            )?;
            for ((id, _), path) in removed.iter().zip(moved) {
                if stmt.execute(params![id.0 as i64, path])? == 1 {
                    added.push((*id, path));
                }
            }
        }
        // Moving up can delete a row and insert it again; it did not change,
        // so it is listed in neither (undo replays both lists).
        let (was, now): (HashSet<_>, HashSet<_>) = (
            removed.iter().cloned().collect(),
            added.iter().cloned().collect(),
        );
        removed.retain(|row| !now.contains(row));
        added.retain(|row| !was.contains(row));
        Self::insert_fts_for_items(&tx, &ids)?;
        let (albums_before, albums_after) =
            rewrite_tag_rules(&tx, std::slice::from_ref(&old), Some(&new))?;
        let (aliases_before, aliases_after) = retarget_aliases(&tx, &old, &new)?;
        // `was` holds every subtree row: any not on `old` itself is a child.
        // The old name is free now (its rows moved, its aliases retargeted).
        let mut aliases_added = Vec::new();
        // A stored name that no longer normalizes (an old, unclean tag)
        // keeps no alias: it could not be typed anyway.
        let alias = normalize_path(&old).ok().filter(|clean| *clean == old);
        if let Some(alias) = alias.filter(|_| keep_alias && was.iter().all(|(_, t)| *t == old)) {
            // A leftover alias row for the name (only possible in data that
            // predates the alias rules) is kept as it is: nothing to add or undo.
            if tx.execute(
                "INSERT OR IGNORE INTO tag_aliases (alias, tag) VALUES (?1, ?2);",
                params![alias, new],
            )? == 1
            {
                aliases_added.push((alias, new));
            }
        }
        tx.commit()?;
        Ok(TagChange {
            removed,
            added,
            aliases_before,
            aliases_after,
            aliases_added,
            albums_before,
            albums_after,
        })
    }

    /// Merges `source_tag` (and its subtree) into `target_tag`, keeping the
    /// source name as an alias of the target when it has no children; see
    /// [`Catalog::rename_tag`]. Refuses a merge into itself or below itself.
    pub fn merge_tags(&self, source_tag: &str, target_tag: &str) -> Result<TagChange> {
        let source = source_tag.trim().to_lowercase();
        if let Ok(target) = normalize_path(target_tag) {
            if target == source {
                bail!("A keyword cannot merge into itself");
            }
            if subtree_suffix(&target, &source).is_some() {
                bail!("A keyword cannot merge into a keyword below it");
            }
        }
        self.move_subtree(source_tag, target_tag, true)
    }

    /// Deletes `tag` and every keyword below it from all media; see
    /// [`Catalog::delete_tags_globally`].
    pub fn delete_tag_globally(&self, tag: &str) -> Result<TagChange> {
        self.delete_tags_globally(&[tag.to_string()])
    }

    /// Deletes each tag and its subtree from all media in one transaction.
    /// Aliases pointing into a deleted subtree are kept (typing one
    /// re-creates the keyword, which is what an alias is for). Smart-album
    /// "include any of" lists drop the deleted keywords in the same
    /// transaction (a list left empty is kept as it was); "is" rules are
    /// left as they are.
    pub fn delete_tags_globally(&self, tags: &[String]) -> Result<TagChange> {
        let roots: Vec<String> = tags
            .iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut seen = HashSet::new();
        let mut removed = Vec::new();
        for root in &roots {
            for row in subtree_rows(&tx, root)? {
                // Overlapping roots (`a` and `a/x`) select a row twice.
                if seen.insert((row.0 .0, row.1.clone())) {
                    removed.push(row);
                }
            }
        }
        let ids = distinct_raw_ids(&removed);
        Self::delete_fts_for_items(&tx, &ids)?;
        for root in &roots {
            tx.execute(
                &subtree_delete_sql(),
                params_from_iter(subtree_params(root)),
            )?;
        }
        Self::insert_fts_for_items(&tx, &ids)?;
        // Also when no item had the keyword: a list may still name it.
        let (albums_before, albums_after) = rewrite_tag_rules(&tx, &roots, None)?;
        tx.commit()?;
        Ok(TagChange {
            removed,
            albums_before,
            albums_after,
            ..TagChange::default()
        })
    }

    /// Makes typing `alias` apply `tag`. Both are stored normalized and an
    /// alias always points at a keyword, never at another alias: a target
    /// that is an alias is resolved, and aliases of `alias` move to `tag`.
    ///
    /// Refuses an alias that is, or is above, a keyword in use, its own
    /// keyword, or another alias's keyword (the alias would shadow it), and
    /// a keyword that is, or is below, an alias.
    ///
    /// Aliases resolve when set: after `ny -> nyc -> x`, `ny` stores `x`, and
    /// re-pointing `nyc` later leaves `ny` on `x`.
    pub fn set_alias(&self, alias: &str, tag: &str) -> Result<()> {
        let alias = normalize_path(alias)?;
        let tag = normalize_path(tag)?;
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let tag = resolve_in(&tx, tag)?;
        if subtree_suffix(&tag, &alias).is_some() {
            bail!("An alias cannot be its keyword or a keyword above it");
        }
        refuse_alias_path(&tx, &tag)?;
        let exists = |sql: String, extra: Option<&str>| -> Result<bool> {
            let mut values: Vec<rusqlite::types::Value> = subtree_params(&alias).into();
            values.extend(extra.map(|v| rusqlite::types::Value::Text(v.to_string())));
            Ok(tx.query_row(&sql, params_from_iter(values), |row| row.get(0))?)
        };
        if exists(
            format!(
                "SELECT EXISTS (SELECT 1 FROM media_tags WHERE {});",
                subtree_clause("tag")
            ),
            None,
        )? {
            bail!("\"{alias}\" is already a keyword, so it cannot be an alias");
        }
        // Equal to another alias's keyword is a chain, retargeted below.
        if exists(
            format!(
                "SELECT EXISTS (SELECT 1 FROM tag_aliases WHERE {} AND tag != ?);",
                subtree_clause("tag")
            ),
            Some(&alias),
        )? {
            bail!("\"{alias}\" is above another alias's keyword, so it cannot be an alias");
        }
        tx.execute(
            "UPDATE tag_aliases SET tag = ?1 WHERE tag = ?2;",
            params![tag, alias],
        )?;
        tx.execute(
            "INSERT INTO tag_aliases (alias, tag) VALUES (?1, ?2) \
             ON CONFLICT(alias) DO UPDATE SET tag = excluded.tag;",
            params![alias, tag],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Removes an alias; the keyword it pointed at is untouched.
    pub fn remove_alias(&self, alias: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM tag_aliases WHERE alias = ?1;",
            params![filter_value(alias)],
        )?;
        Ok(())
    }

    /// Every `(alias, keyword)` pair, ordered by alias.
    pub fn aliases(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached("SELECT alias, tag FROM tag_aliases ORDER BY alias;")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The keyword typing `raw` applies: its alias target, or its
    /// [`normalize_path`] form. Errors when `raw` is not a valid path.
    pub fn resolve_alias(&self, raw: &str) -> Result<String> {
        let path = normalize_path(raw)?;
        resolve_in(&self.conn.lock(), path)
    }

    /// Why [`Catalog::add_tags_batch`] would refuse the resolved keyword
    /// `path` (it is, or sits below, an alias), or `None` when it can be
    /// stored. Lets a bulk import report one keyword and keep the rest.
    pub fn keyword_refusal(&self, path: &str) -> Result<Option<String>> {
        alias_refusal(&self.conn.lock(), path)
    }

    /// How many active items carry `path` or a keyword below it, each once.
    pub fn subtree_item_count(&self, path: &str) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            &subtree_count_sql(),
            params_from_iter(subtree_params(&filter_value(path))),
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sql` with each `?` replaced by `'p'` (explain binds nothing).
    fn inline(sql: &str) -> String {
        sql.replace('?', "'p'")
    }

    fn assert_range_seek(sql: &str, what: &str) {
        let catalog = super::super::Catalog::open_in_memory().expect("open catalog");
        let rows = catalog
            .explain_query_plan(&inline(sql))
            .expect("explain must run");
        let plan = rows.join(" | ");
        assert!(
            rows.iter().any(|r| r.starts_with("SEARCH")
                && r.contains("INDEX idx_media_tags_tag")
                && r.contains("tag>")),
            "{what} must seek idx_media_tags_tag by range: {plan}"
        );
        assert!(
            !rows.iter().any(|r| r.starts_with("SCAN")),
            "{what} must not scan a table: {plan}"
        );
    }

    #[test]
    fn subtree_statements_seek_the_tag_index() {
        assert_range_seek(&subtree_count_sql(), "subtree_item_count");
        assert_range_seek(&subtree_rows_sql(), "the subtree row read");
        assert_range_seek(&subtree_delete_sql(), "the subtree delete");
    }
}
