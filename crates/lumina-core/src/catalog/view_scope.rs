//! A grid view as SQL: the WHERE over `media_items m` that its count uses,
//! shared by Delete Rejected… and the header's date span.

use super::Catalog;
use crate::{SmartAlbum, StructuredQuery, SystemAlbum};
use anyhow::{Context, Result};
use rusqlite::types::Value;

/// One grid view whose rows are a WHERE over `media_items m`. A search, a
/// year and a tag are all search text (`year:2024`, `tag:beach`), as the
/// grid runs them.
#[derive(Debug, Clone)]
pub enum ViewScope {
    Library,
    Search(String),
    System(SystemAlbum),
    Smart(SmartAlbum),
    Album(i64),
    Untagged,
}

/// The first and last capture month, as (year, month), of a view's dated
/// items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateSpan {
    /// (year, month 1-12) of the oldest dated item.
    pub first: (i32, u32),
    /// (year, month 1-12) of the newest dated item.
    pub last: (i32, u32),
}

/// The view's WHERE, with the same rows as its count: active items only,
/// except where the view itself says otherwise (`is:trashed`, the Trash).
pub(crate) fn view_where(scope: &ViewScope) -> Result<(String, Vec<Value>)> {
    const ACTIVE: &str = "m.is_trashed = 0 AND m.is_hidden = 0";
    Ok(match scope {
        ViewScope::Library => (ACTIVE.into(), Vec::new()),
        ViewScope::Search(text) => StructuredQuery::parse(text).build_where_subquery(false),
        ViewScope::System(album) => (album.build_where_clause().into(), Vec::new()),
        ViewScope::Smart(album) => album.where_sql(false)?,
        ViewScope::Album(id) => (
            format!("m.id IN (SELECT media_id FROM album_items WHERE album_id = ?) AND {ACTIVE}"),
            vec![(*id).into()],
        ),
        ViewScope::Untagged => (
            format!(
                "NOT EXISTS (SELECT 1 FROM media_tags mt WHERE mt.media_id = m.id) AND {ACTIVE}"
            ),
            Vec::new(),
        ),
    })
}

/// Oldest and newest dated row of the view, as two `LIMIT 1` walks in
/// timestamp order so the active-row timestamp index answers the
/// unfiltered case with two seeks. A capture time of 0 means undated.
/// One `MIN/MAX(year*100+month)` pass was measured on 50k rows: about 2x
/// faster where the plan sorts (album 1.1 vs 2.5 ms, Videos 4.0 vs 10.4 ms)
/// but 300-8000x slower where the walk stops early (Untagged, a common
/// city: µs vs 6-8 ms), and no better for a rare filter (5.8 vs 4.7 ms).
fn date_span_sql(scope: &ViewScope) -> Result<(String, Vec<Value>)> {
    let (view, params) = view_where(scope)?;
    let end = |dir: &str| {
        format!(
            "(SELECT m.year * 100 + m.month FROM media_items m \
             WHERE ({view}) AND m.timestamp_epoch_ms <> 0 \
             ORDER BY m.timestamp_epoch_ms {dir}, m.id {dir} LIMIT 1)"
        )
    };
    let mut both = params.clone();
    both.extend(params);
    Ok((format!("SELECT {}, {};", end("ASC"), end("DESC")), both))
}

impl Catalog {
    /// The capture months the view spans, for the page header. `None` when
    /// it has no dated items (empty, or every capture time is 0).
    pub fn view_date_span(&self, scope: &ViewScope) -> Result<Option<DateSpan>> {
        let (sql, params) = date_span_sql(scope)?;
        let conn = self.conn.lock();
        let (first, last): (Option<i64>, Option<i64>) = conn
            .prepare_cached(&sql)?
            .query_row(rusqlite::params_from_iter(params.iter()), |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .context("reading the view's date span")?;
        let month = |key: i64| ((key / 100) as i32, (key % 100) as u32);
        Ok(first.zip(last).map(|(first, last)| DateSpan {
            first: month(first),
            last: month(last),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(catalog: &Catalog, scope: &ViewScope) -> String {
        let (sql, params) = date_span_sql(scope).unwrap();
        let conn = catalog.conn.lock();
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |r| {
                r.get::<_, String>(3)
            })
            .unwrap();
        rows.map(|r| r.unwrap()).collect::<Vec<_>>().join(" | ")
    }

    #[test]
    fn date_span_seeks_the_timestamp_indexes() {
        let catalog = Catalog::open_in_memory().unwrap();
        for (scope, index) in [
            (ViewScope::Library, "idx_media_timestamp"),
            (
                ViewScope::System(SystemAlbum::Favorites),
                "idx_media_favorite",
            ),
            (
                ViewScope::System(SystemAlbum::RecentlyDeleted),
                "idx_media_trashed",
            ),
        ] {
            let plan = plan(&catalog, &scope);
            assert!(
                plan.matches(index).count() == 2 && !plan.contains("TEMP B-TREE"),
                "{scope:?}: both ends walk {index} in order and stop at the first row, \
                 never sorting the view: {plan}"
            );
        }
        let album = plan(&catalog, &ViewScope::Album(1));
        assert!(
            album.contains("album_items USING PRIMARY KEY"),
            "an album's span is driven by its members, not a library walk: {album}"
        );
    }
}
