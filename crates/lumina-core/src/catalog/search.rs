//! Structured search: the query, its count, and the `within:` index choice.

use anyhow::{Context, Result};

use super::Catalog;
use crate::search::{GeoIndex, SqlParts, StructuredQuery};
use crate::{MediaItem, SortSpec};

impl Catalog {
    /// Executes a structured search query with optional FTS5 matching and metadata predicates.
    pub fn search_structured(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>> {
        self.search_structured_sorted(query, limit, offset, SortSpec::default())
    }

    pub fn search_structured_sorted(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<Vec<MediaItem>> {
        let (sql, params) = self.search_sql_sorted(query, limit, offset, sort)?;
        let conn = self.conn.read();
        let mut stmt = conn.prepare_cached(&sql)?;
        Self::map_query_rows(&mut stmt, rusqlite::params_from_iter(params.iter()))
    }

    /// The SQL and parameters [`Self::search_structured_sorted`] runs.
    /// Public so plan tests can `EXPLAIN` the real query.
    ///
    /// A `within:` page either walks the sort index and filters the box, or
    /// seeks `idx_media_geo` and sorts the box's matches. With
    /// `cap = sqrt((offset + limit) * rows)`: a box with fewer than `cap`
    /// matches seeks the index (an index-only count decides). Otherwise a
    /// probe reads the first `cap` rows of the walk: if they already hold
    /// the page, the walk reads at most `cap` rows (proven only when
    /// `within:` is the sole filter: both probes test the box alone, so a
    /// dense box with a rare second filter can still walk far, no worse than
    /// that filter alone) and the box is written so the geo index cannot
    /// serve it; if not, the matches lie far down the sort order (an old
    /// trip) and sorting them is cheaper.
    pub fn search_sql_sorted(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<SqlParts> {
        let geo = if query.within.is_some() {
            self.geo_index_choice(query, limit, offset, sort)?
        } else {
            GeoIndex::Use
        };
        Ok(query.synthesize_query_geo(limit, offset, sort, geo))
    }

    /// The probe behind [`Self::search_sql_sorted`] for a `within:` query.
    fn geo_index_choice(
        &self,
        query: &StructuredQuery,
        limit: usize,
        offset: usize,
        sort: SortSpec,
    ) -> Result<GeoIndex> {
        let conn = self.conn.read();
        // ponytail: max(rowid) stands in for the row count (O(1)); it
        // overcounts after deletes, which only raises the cap. Ceiling: the
        // walk probe reads `cap` rows of the sort index (~2.2k for page 0 at
        // 100k), and runs only for a box with at least `cap` matches; on a
        // deep page `cap` grows (~55k for a world box @30000), so the total
        // stays within ~2 x cap.
        // Follow-up, not done: a deep page of a dense box still sorts every
        // match (100k rows, ~8.9k in the box, @8500: 8.5 ms; `camera:` or
        // `iso:` at that depth take 25-30 ms); an id-only sort through a
        // covering geo index would cut it.
        let rows: i64 = conn
            .prepare_cached("SELECT coalesce(max(rowid), 0) FROM media_items;")?
            .query_row([], |row| row.get(0))?;
        let cap = (((offset + limit) as f64) * rows as f64).sqrt().max(1.0) as usize;
        let Some((probe, params)) = query.walk_probe_sql(cap, sort) else {
            return Ok(GeoIndex::Use);
        };
        // Fewer than `cap` matches: sorting them is already within the bound,
        // and this index-only count is far cheaper than the walk probe on a
        // deep page (which would read `cap` ~ 29k rows at 100k, @8500).
        if let Some((count_sql, count_params)) = query.box_count_sql(cap) {
            let matches: i64 = conn
                .prepare_cached(&count_sql)?
                .query_row(rusqlite::params_from_iter(count_params.iter()), |row| {
                    row.get(0)
                })
                .context("counting the within: box")?;
            if (matches as usize) < cap {
                return Ok(GeoIndex::Use);
            }
        }
        let found: i64 = conn
            .prepare_cached(&probe)?
            .query_row(rusqlite::params_from_iter(params.iter()), |row| row.get(0))
            .context("probing the within: box")?;
        Ok(if found as usize >= offset + limit {
            GeoIndex::Skip
        } else {
            GeoIndex::Use
        })
    }

    pub fn search_structured_count(&self, query: &StructuredQuery) -> Result<usize> {
        let (sql, params) = query.synthesize_count_query();
        let conn = self.conn.read();
        let count: i64 =
            conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |row| {
                row.get(0)
            })?;
        Ok(count as usize)
    }
}
