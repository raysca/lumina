//! A grid view as SQL: the WHERE over `media_items m` that its count uses,
//! shared by Delete Rejected…, the header's date span and the Mosaic
//! layout and day places.

use super::Catalog;
use crate::{MediaId, SmartAlbum, SortSpec, StructuredQuery, SystemAlbum};
use anyhow::{Context, Result};
use rusqlite::types::Value;
use std::collections::HashMap;

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
        // Unsorted form: the `+` only pays off for a LIMIT page that stops
        // early on the sort index; a count or a full read (the Mosaic
        // layout) is cheaper driven from the FTS rowids.
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

/// One item of a view as the Mosaic lays it out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutCell {
    /// The item; its position in the layout is its index in the grid.
    pub id: MediaId,
    /// Width over height as shown: a 0x0 (unprobed) item is 1.0, an odd
    /// `rotation_quarters` swaps it, and it is clamped to 0.4-3.0 so a
    /// panorama or a sliver cannot take over a row.
    pub aspect: f32,
    /// Capture day as `yyyymmdd`, or 0 when undated (capture time 0).
    pub day: u32,
}

/// The most common place of one day in a view: (city, country code).
pub type DayPlace = (String, Option<String>);

/// `yyyymmdd`, or 0 for an undated row. Undated is a capture time of 0, as
/// in [`date_span_sql`]: the scanner stores such a row as 1970-01-01, so
/// the date columns alone would key it as 19700101. Year 0 (hand-written
/// rows) also counts as undated.
const DAY_KEY: &str = "CASE WHEN m.timestamp_epoch_ms <> 0 AND m.year > 0 \
     THEN m.year * 10000 + m.month * 100 + m.day ELSE 0 END";

/// Every row of the view in its sort order: same WHERE as the count, same
/// ORDER BY as the slice, so a cell index is a grid index. Date sort walks
/// `idx_media_timestamp` with no sort for the library (timestamp DESC, id
/// DESC matches the index); an album is driven by `album_items`.
fn layout_sql(scope: &ViewScope, sort: SortSpec) -> Result<(String, Vec<Value>)> {
    let (view, params) = view_where(scope)?;
    Ok((
        format!(
            "SELECT m.id, m.width, m.height, m.rotation_quarters, {DAY_KEY} \
             FROM media_items m WHERE ({view}) ORDER BY {}",
            sort.order_by("m.")
        ),
        params,
    ))
}

/// Per dated day and place, how many of the view's items were taken there.
/// Library plan: `SCAN m USING INDEX idx_media_file_size` (any active-row
/// partial index; an album seeks `album_items`) then `USE TEMP B-TREE FOR
/// GROUP BY`. Accepted: it is one background pass per
/// catalog revision, not a frame query, so no `(year, month, day, city)`
/// index is added while the bench meets its gate.
fn day_places_sql(scope: &ViewScope) -> Result<(String, Vec<Value>)> {
    let (view, params) = view_where(scope)?;
    Ok((
        format!(
            "SELECT {DAY_KEY}, m.city, m.country_code, COUNT(*) FROM media_items m \
             WHERE ({view}) AND m.city IS NOT NULL AND m.city <> '' \
             GROUP BY 1, 2, 3"
        ),
        params,
    ))
}

/// Width over height as the Mosaic shows it; see [`LayoutCell::aspect`].
fn shown_aspect(width: i64, height: i64, rotation_quarters: i64) -> f32 {
    if width <= 0 || height <= 0 {
        return 1.0;
    }
    let (w, h) = if rotation_quarters % 2 != 0 {
        (height, width)
    } else {
        (width, height)
    };
    (w as f32 / h as f32).clamp(0.4, 3.0)
}

impl Catalog {
    /// Every item of the view as a layout cell, in the view's sort order.
    pub fn view_layout(&self, scope: &ViewScope, sort: SortSpec) -> Result<Vec<LayoutCell>> {
        let (sql, params) = layout_sql(scope, sort)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(LayoutCell {
                id: MediaId(row.get::<_, i64>(0)? as u64),
                aspect: shown_aspect(row.get(1)?, row.get(2)?, row.get(3)?),
                day: row.get::<_, i64>(4)?.max(0) as u32,
            })
        })?;
        rows.collect::<rusqlite::Result<_>>()
            .context("reading the view layout")
    }

    /// The most common place of each dated day in the view, keyed by
    /// `yyyymmdd`. Items are counted by (city, country code), so the same
    /// city with and without a country code is two places. A tie goes to
    /// the alphabetically first city. Days with no city, and undated items
    /// (capture time 0), are absent.
    pub fn day_places(&self, scope: &ViewScope) -> Result<HashMap<u32, DayPlace>> {
        let (sql, params) = day_places_sql(scope)?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)? as u32,
                row.get::<_, i64>(3)?,
                (row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?),
            ))
        })?;
        let mut best: HashMap<u32, (i64, DayPlace)> = HashMap::new();
        for row in rows {
            let (day, count, place) = row.context("reading the day places")?;
            // Undated rows (DAY_KEY 0) are dropped here, not in the WHERE:
            // a date filter there makes the planner seek an index (such as
            // idx_media_year_month) and look up nearly every row.
            if day == 0 {
                continue;
            }
            // More items wins; on a tie the smaller (city, country) wins.
            let wins = best
                .get(&day)
                .is_none_or(|(n, p)| count > *n || (count == *n && place < *p));
            if wins {
                best.insert(day, (count, place));
            }
        }
        Ok(best
            .into_iter()
            .map(|(day, (_, place))| (day, place))
            .collect())
    }

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
        explain(catalog, date_span_sql(scope).unwrap())
    }

    fn explain(catalog: &Catalog, (sql, params): (String, Vec<Value>)) -> String {
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

    #[test]
    fn layout_and_place_queries_use_the_view_indexes() {
        let catalog = Catalog::open_in_memory().unwrap();
        let library = explain(
            &catalog,
            layout_sql(&ViewScope::Library, SortSpec::default()).unwrap(),
        );
        assert!(
            library.contains("idx_media_timestamp") && !library.contains("TEMP B-TREE"),
            "the library layout walks the timestamp index in date order, never sorting: {library}"
        );
        let album = explain(
            &catalog,
            layout_sql(&ViewScope::Album(1), SortSpec::default()).unwrap(),
        );
        assert!(
            album.contains("album_items USING PRIMARY KEY"),
            "an album layout is driven by its members, not a library walk: {album}"
        );
        let places = explain(&catalog, day_places_sql(&ViewScope::Library).unwrap());
        assert!(
            places.starts_with("SCAN m") && places.matches("TEMP B-TREE").count() == 1,
            "the library place pass is one scan plus the accepted GROUP BY b-tree, \
             never a per-row index seek: {places}"
        );
        let album_places = explain(&catalog, day_places_sql(&ViewScope::Album(1)).unwrap());
        assert!(
            album_places.contains("album_items USING PRIMARY KEY"),
            "an album's places are driven by its members: {album_places}"
        );
        println!(
            "layout library: {library}\nlayout album: {album}\n\
             day places library: {places}\nday places album: {album_places}"
        );
    }
}
