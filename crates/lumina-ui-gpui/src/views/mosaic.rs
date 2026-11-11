//! Mosaic justified-row layout: a pure, near-linear pass from the view's
//! [`LayoutCell`]s to header and tile rows. It runs off the UI thread once
//! per (view, revision, width, level); a frame only reads the result.

use crate::views::grid::{BAND_TO_ROW, GRID_CARD_GAP};
use crate::views::search_bar::{size_level, SIZE_COLUMNS};
use lumina_core::LayoutCell;
use std::ops::Range;

/// Target row height for each `SIZE_COLUMNS` level (8, 6, 4, 3, 2 columns),
/// so ±, ⌘=/⌘-, the wheel and pinch walk one ladder in both layouts.
pub(crate) const MOSAIC_TARGETS: [f32; SIZE_COLUMNS.len()] = [110.0, 150.0, 190.0, 250.0, 320.0];

/// A full row may not be shorter than this share of the target: the
/// frame's 158-224 band at 190, about 0.83.
const MIN_SHARE: f32 = 158.0 / 190.0;

/// The band's top as a share of the target: 224 at 190.
const MAX_SHARE: f32 = 224.0 / 190.0;

/// A section's short last row is re-flowed with up to this many rows above
/// it so that every row fills the width ([`MosaicLayout::reflow_tail`]).
const TAIL_ROWS: usize = 3;

/// Space above every day header but the first (frame: section gap 26).
pub(crate) const SECTION_GAP: f32 = 26.0;

/// Line height of the day header's date (Inter Tight 15/600). The header
/// row is this plus [`BAND_TO_ROW`]. GPUI's default line height is about
/// 1.618 x the size (about 24 at 15 px), so the header must pin
/// `line_height(px(20.))` or the paint outgrows the layout.
pub(crate) const DAY_HEADER_TEXT: f32 = 20.0;

/// A cell's aspect with a 0, negative or NaN value read as 0.01 (core
/// already clamps to 0.4-3.0; this keeps widths finite and positive).
fn aspect(cell: &LayoutCell) -> f32 {
    cell.aspect.max(0.01)
}

/// Target row height for a saved column count.
pub(crate) fn target_for_columns(columns: usize) -> f32 {
    MOSAIC_TARGETS[size_level(columns)]
}

/// What a row shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RowKind {
    /// A day header: `yyyymmdd` (0 = undated) and its item count.
    Header { day: u32, count: usize },
    /// Tiles `first..first + len`.
    Tiles,
}

/// One list row. `y..y + height` is the whole list item; its content
/// starts `pad` below `y` (the gap above it is part of the row).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MosaicRow {
    /// First item index (for a header: the first item of its day).
    pub first: usize,
    /// Tiles in the row; 0 for a header.
    pub len: usize,
    pub y: f32,
    pub height: f32,
    pub pad: f32,
    pub kind: RowKind,
}

/// Horizontal placement of one tile, relative to the content's left edge.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct TileSpan {
    pub x: f32,
    pub w: f32,
}

/// Rows plus one [`TileSpan`] per item, in item order.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MosaicLayout {
    pub rows: Vec<MosaicRow>,
    pub tiles: Vec<TileSpan>,
}

impl MosaicLayout {
    /// Lays `cells` out in justified rows `width` wide around `target`
    /// height. With `sections`, each run of equal `day` (0 = undated) gets
    /// a header row; without, the whole view is one header-less run.
    ///
    /// Greedy, O(n), plus a small re-flow of each section's tail: a row
    /// takes items while its full-width height stays above `target`. Once the next item would bring it to `target` or
    /// below, the row closes either before or after that item, whichever
    /// full-width height is closer to `target`. A closed row of two or more
    /// tiles always fills the width exactly; with wide or mixed aspects, or
    /// in a narrow pane, its height can leave the 158-224 band (at 190),
    /// which is accepted. The one ragged closed row is a single tile closed early
    /// (a lone portrait in a narrow pane): it is capped at
    /// `target / MIN_SHARE` and left-aligned. A section's last row that
    /// fills the width at or under the band top (`target * MAX_SHARE`) does
    /// so (a panorama shrinks to fit). A shorter one is re-flowed with the rows above it so all fill
    /// the width ([`Self::reflow_tail`]); when no split fits (a small day)
    /// it keeps `target`, left-aligned. No tile ever passes `width`.
    pub(crate) fn justify(cells: &[LayoutCell], width: f32, target: f32, sections: bool) -> Self {
        let width = width.max(1.0);
        let fill = |n: usize, sum: f32| (width - GRID_CARD_GAP * (n - 1) as f32) / sum;
        let mut out = Self {
            rows: Vec::new(),
            tiles: vec![TileSpan::default(); cells.len()],
        };
        let mut y = 0.0;
        let mut start = 0;
        while start < cells.len() {
            let day = cells[start].day;
            let end = if sections {
                cells[start..]
                    .iter()
                    .position(|c| c.day != day)
                    .map_or(cells.len(), |n| start + n)
            } else {
                cells.len()
            };
            if sections {
                let pad = if start == 0 { 0.0 } else { SECTION_GAP };
                let height = pad + DAY_HEADER_TEXT + BAND_TO_ROW;
                out.rows.push(MosaicRow {
                    first: start,
                    len: 0,
                    y,
                    height,
                    pad,
                    kind: RowKind::Header {
                        day,
                        count: end - start,
                    },
                });
                y += height;
            }
            let (mut first, mut sum) = (start, 0.0);
            let mut i = start;
            while i < end {
                let a = aspect(&cells[i]);
                let n = i - first;
                let h_with = fill(n + 1, sum + a);
                if h_with > target {
                    sum += a;
                    i += 1;
                    continue;
                }
                // An empty row never closes before its first item, so item
                // `i` is retried at most once (as the head of a new row).
                let h_now = if n > 0 { fill(n, sum) } else { f32::INFINITY };
                if h_now - target < target - h_with {
                    let h = if n == 1 {
                        h_now.min(target / MIN_SHARE)
                    } else {
                        h_now
                    };
                    out.push_tiles(cells, first..i, h, &mut y);
                    (first, sum) = (i, 0.0);
                } else {
                    out.push_tiles(cells, first..i + 1, h_with, &mut y);
                    (first, sum) = (i + 1, 0.0);
                    i += 1;
                }
            }
            if first < end {
                let h = fill(end - first, sum);
                if h <= target * MAX_SHARE {
                    out.push_tiles(cells, first..end, h, &mut y);
                } else if !out.reflow_tail(cells, first..end, width, target, &mut y) {
                    out.push_tiles(cells, first..end, target, &mut y);
                }
            }
            start = end;
        }
        out
    }

    /// Re-flows a section's short last row (`last`) together with up to
    /// [`TAIL_ROWS`] tile rows above it (a day header stops it) into rows
    /// that all fill the width, each between `target * MIN_SHARE` and
    /// `target * MAX_SHARE` high (the 158-224 band at 190), picking the split with the least
    /// squared distance from `target`. Returns false, changing nothing, when
    /// no such split exists (a small day). O(m²) in the tail's item count.
    fn reflow_tail(
        &mut self,
        cells: &[LayoutCell],
        last: Range<usize>,
        width: f32,
        target: f32,
        y: &mut f32,
    ) -> bool {
        let closed = self.rows.len();
        let mut keep = closed;
        while keep > 0 && closed - keep < TAIL_ROWS && self.rows[keep - 1].kind == RowKind::Tiles {
            keep -= 1;
        }
        let end = last.end;
        // With no closed rows to borrow from, the last row alone.
        let tail = if keep < closed {
            self.rows[keep].first
        } else {
            last.start
        };
        let mut prefix = vec![0.0f32; end - tail + 1];
        for (k, cell) in cells[tail..end].iter().enumerate() {
            prefix[k + 1] = prefix[k] + aspect(cell);
        }
        let (lo, hi) = (target * MIN_SHARE, target * MAX_SHARE);
        let height = |i: usize, j: usize| {
            (width - GRID_CARD_GAP * (j - i - 1) as f32) / (prefix[j] - prefix[i])
        };
        // best[j]: least cost laying out the first j tail items, and where
        // its last row starts.
        let m = end - tail;
        let mut best = vec![(f32::INFINITY, 0usize); m + 1];
        best[0].0 = 0.0;
        for j in 1..=m {
            for i in 0..j {
                let h = height(i, j);
                if best[i].0.is_finite() && (lo..=hi).contains(&h) {
                    let cost = best[i].0 + (h - target).powi(2);
                    if cost < best[j].0 {
                        best[j] = (cost, i);
                    }
                }
            }
        }
        if !best[m].0.is_finite() {
            return false;
        }
        let mut splits = vec![m];
        while let Some(&j) = splits.last().filter(|&&j| j > 0) {
            splits.push(best[j].1);
        }
        splits.reverse();
        if keep < closed {
            *y = self.rows[keep].y;
            self.rows.truncate(keep);
        }
        for pair in splits.windows(2) {
            let h = height(pair[0], pair[1]);
            self.push_tiles(cells, tail + pair[0]..tail + pair[1], h, y);
        }
        true
    }

    /// Appends a tile row of `items` at `height`, left to right from x 0.
    fn push_tiles(&mut self, cells: &[LayoutCell], items: Range<usize>, height: f32, y: &mut f32) {
        let after_tiles = self.rows.last().is_some_and(|r| r.kind == RowKind::Tiles);
        let pad = if after_tiles { GRID_CARD_GAP } else { 0.0 };
        let mut x = 0.0;
        for i in items.clone() {
            let w = height * aspect(&cells[i]);
            self.tiles[i] = TileSpan { x, w };
            x += w + GRID_CARD_GAP;
        }
        self.rows.push(MosaicRow {
            first: items.start,
            len: items.len(),
            y: *y,
            height: pad + height,
            pad,
            kind: RowKind::Tiles,
        });
        *y += pad + height;
    }

    /// Index of the tile row holding item `index`. A header shares `first`
    /// with the tile row after it, so the last row starting at or before
    /// `index` is always a tile row. An index past the end clamps to the
    /// last row; on an empty layout it returns 0, which is not a row, so
    /// callers check `rows` first.
    pub(crate) fn row_of(&self, index: usize) -> usize {
        self.rows
            .partition_point(|r| r.first <= index)
            .saturating_sub(1)
    }

    /// The item in the next tile row down (`down`) or up
    /// whose x-centre is nearest item `index`'s; headers are skipped. None
    /// at either end or for an index outside the layout.
    pub(crate) fn vertical_neighbour(&self, index: usize, down: bool) -> Option<usize> {
        let tile = self.tiles.get(index)?;
        let centre = tile.x + tile.w / 2.0;
        let mut r = self.row_of(index);
        let row = loop {
            r = if down { r + 1 } else { r.checked_sub(1)? };
            let row = self.rows.get(r)?;
            if row.kind == RowKind::Tiles {
                break row;
            }
        };
        let dist = |i: &usize| (self.tiles[*i].x + self.tiles[*i].w / 2.0 - centre).abs();
        (row.first..row.first + row.len).min_by(|a, b| dist(a).total_cmp(&dist(b)))
    }

    /// Rows overlapping `y` (binary search on the row tops).
    pub(crate) fn rows_in(&self, y: Range<f32>) -> Range<usize> {
        let start = self.rows.partition_point(|r| r.y + r.height <= y.start);
        let end = self.rows.partition_point(|r| r.y < y.end);
        start..end.max(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Height of the whole layout.
    fn total_height(layout: &MosaicLayout) -> f32 {
        layout.rows.last().map_or(0.0, |r| r.y + r.height)
    }
    use lumina_core::MediaId;

    fn cells(aspects: &[f32], day: u32) -> Vec<LayoutCell> {
        aspects
            .iter()
            .enumerate()
            .map(|(i, &aspect)| LayoutCell {
                id: MediaId(i as u64 + 1),
                aspect,
                day,
            })
            .collect()
    }

    fn tile_rows(layout: &MosaicLayout) -> Vec<MosaicRow> {
        layout
            .rows
            .iter()
            .copied()
            .filter(|r| r.kind == RowKind::Tiles)
            .collect()
    }

    /// A row's right edge: last tile's x + w.
    fn right_edge(layout: &MosaicLayout, row: &MosaicRow) -> f32 {
        let last = layout.tiles[row.first + row.len - 1];
        last.x + last.w
    }

    #[core::prelude::v1::test]
    fn rows_justify_to_the_width_within_the_height_band() {
        let set = [1.5, 1.0, 0.667, 1.333, 1.5, 0.75, 1.333, 1.0];
        let aspects: Vec<f32> = (0..400).map(|i| set[(i * 7 + i / 3) % set.len()]).collect();
        let layout = MosaicLayout::justify(&cells(&aspects, 20250614), 1200.0, 190.0, true);
        let rows = tile_rows(&layout);
        assert!(rows.len() > 10, "400 items make many rows");
        assert_eq!(
            rows.iter().map(|r| r.len).sum::<usize>(),
            400,
            "every item is in exactly one row"
        );
        for row in &rows[..rows.len() - 1] {
            assert!(
                (right_edge(&layout, row) - 1200.0).abs() <= 0.5,
                "a full row fills the width: {row:?} ends at {}",
                right_edge(&layout, row)
            );
            let h = row.height - row.pad;
            assert!(
                (158.0..=224.0).contains(&h),
                "a full row's height stays in the 158-224 band: {h}"
            );
            for (i, &aspect) in aspects.iter().enumerate().skip(row.first).take(row.len) {
                let t = layout.tiles[i];
                assert!((t.w / h - aspect).abs() < 0.01, "tile {i} keeps its aspect");
                if i > row.first {
                    let prev = layout.tiles[i - 1];
                    assert!(
                        (t.x - (prev.x + prev.w) - GRID_CARD_GAP).abs() < 0.01,
                        "tiles are {GRID_CARD_GAP} apart"
                    );
                }
            }
        }
    }

    #[core::prelude::v1::test]
    fn a_short_last_row_reflows_with_the_rows_above() {
        // Greedy closes five tiles at ~194, leaving four that would need
        // ~232 to fill: re-flowed, both rows fill the width inside the band.
        let cs = cells(
            &[
                1.5,
                16.0 / 9.0,
                0.667,
                1.5,
                0.667,
                16.0 / 9.0,
                1.0,
                1.5,
                0.5625,
            ],
            20250614,
        );
        let layout = MosaicLayout::justify(&cs, 1212.0, 190.0, true);
        let rows = tile_rows(&layout);
        assert_eq!(rows.len(), 2, "two rows: {rows:?}");
        for row in &rows {
            let h = row.height - row.pad;
            assert!((158.0..=224.0).contains(&h), "row in the band: {row:?}");
            assert!(
                (right_edge(&layout, row) - 1212.0).abs() <= 0.5,
                "the last row fills the width too: {row:?}"
            );
        }
        assert_eq!(rows[1].y, rows[0].y + rows[0].height, "rows stack");
        assert_eq!(rows[1].pad, GRID_CARD_GAP, "tile rows keep their gap");
    }

    #[core::prelude::v1::test]
    fn a_tail_reflow_never_borrows_from_the_previous_day() {
        // Day 1 alone lays out as before; day 2 is one square that can only
        // fill by borrowing, and must not take day 1's rows.
        let day1 = cells(
            &[
                1.5,
                16.0 / 9.0,
                0.667,
                1.5,
                0.667,
                16.0 / 9.0,
                1.0,
                1.5,
                0.5625,
            ],
            20250614,
        );
        let alone = MosaicLayout::justify(&day1, 1212.0, 190.0, true);
        let mut both = day1.clone();
        both.extend(cells(&[1.0], 20250613));
        let layout = MosaicLayout::justify(&both, 1212.0, 190.0, true);
        assert_eq!(
            &layout.rows[..alone.rows.len()],
            &alone.rows[..],
            "day 1's rows are unchanged by day 2"
        );
        assert_eq!(&layout.tiles[..day1.len()], &alone.tiles[..]);
        let last = *layout.rows.last().unwrap();
        assert_eq!(
            (last.first, last.len),
            (day1.len(), 1),
            "day 2 keeps its own row"
        );
        assert_eq!(
            last.height - last.pad,
            190.0,
            "a lone square keeps the target"
        );
    }

    #[core::prelude::v1::test]
    fn a_section_ends_its_last_row_unstretched() {
        // 1200 wide: four squares make a 297-high row, so they never fill it.
        let mut cs = cells(&[1.0; 4], 20250614);
        cs.extend(cells(&[1.0; 2], 20250613));
        let layout = MosaicLayout::justify(&cs, 1200.0, 190.0, true);
        let rows = tile_rows(&layout);
        assert_eq!(rows.len(), 2, "each day ends in its own row");
        for row in rows {
            assert_eq!(row.height - row.pad, 190.0, "the last row keeps the target");
            assert_eq!(layout.tiles[row.first].x, 0.0, "left-aligned");
            assert!(right_edge(&layout, &row) < 1200.0, "not stretched");
        }
    }

    #[core::prelude::v1::test]
    fn sections_follow_day_runs() {
        let mut cs = cells(&[1.0; 3], 20250614);
        cs.extend(cells(&[1.0; 2], 20250613));
        let layout = MosaicLayout::justify(&cs, 1000.0, 190.0, true);
        let kinds: Vec<RowKind> = layout.rows.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            vec![
                RowKind::Header {
                    day: 20250614,
                    count: 3
                },
                RowKind::Tiles,
                RowKind::Header {
                    day: 20250613,
                    count: 2
                },
                RowKind::Tiles,
            ],
            "one header per day run"
        );
        let header = DAY_HEADER_TEXT + BAND_TO_ROW;
        assert_eq!(layout.rows[0].y, 0.0, "the first header is at the top");
        assert_eq!(
            layout.rows[0].pad, 0.0,
            "no section gap above the first day"
        );
        let second = layout.rows[2];
        assert_eq!(
            second.y + second.pad,
            header + 190.0 + SECTION_GAP,
            "the second header sits a section gap below the first day's rows"
        );
        assert_eq!(total_height(&layout), 2.0 * header + SECTION_GAP + 380.0);

        let flat = MosaicLayout::justify(&cs, 1000.0, 190.0, false);
        assert!(
            flat.rows.iter().all(|r| r.kind == RowKind::Tiles),
            "without sections there are no headers"
        );
    }

    #[core::prelude::v1::test]
    fn undated_items_form_their_own_section() {
        let mut cs = cells(&[1.0; 2], 20250614);
        cs.extend(cells(&[1.0; 2], 0));
        let layout = MosaicLayout::justify(&cs, 1000.0, 190.0, true);
        assert_eq!(
            layout.rows[2].kind,
            RowKind::Header { day: 0, count: 2 },
            "undated items get a day-0 header"
        );
    }

    #[core::prelude::v1::test]
    fn vertical_neighbour_picks_the_nearest_x() {
        // Width 600. Day A: 3 squares (196 high; a 4th would make 145, so
        // the row closes), then a square and 4 half-width portraits (the
        // day's last row). Day B: 4 portraits.
        let mut cs = cells(&[1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5], 20250614);
        cs.extend(cells(&[0.5; 4], 20250613));
        let layout = MosaicLayout::justify(&cs, 600.0, 190.0, true);
        let rows = tile_rows(&layout);
        assert_eq!(
            rows.iter().map(|r| r.len).collect::<Vec<_>>(),
            vec![3, 5, 4],
            "rows of 3 and 5, then the next day"
        );
        let centre = |i: usize| layout.tiles[i].x + layout.tiles[i].w / 2.0;
        let down = layout.vertical_neighbour(2, true).expect("a row below");
        let nearest = (3..8)
            .min_by(|&a, &b| {
                (centre(a) - centre(2))
                    .abs()
                    .total_cmp(&(centre(b) - centre(2)).abs())
            })
            .unwrap();
        assert_eq!(down, nearest, "↓ lands on the nearest x-centre");
        assert_eq!(down, 7, "the right-most tile sits over the last of 5");
        assert_eq!(
            layout.vertical_neighbour(0, false),
            None,
            "nothing above row 1"
        );
        assert_eq!(
            layout.vertical_neighbour(7, true),
            Some(11),
            "↓ crosses the header"
        );
        assert_eq!(
            layout.vertical_neighbour(8, false),
            Some(3),
            "↑ crosses back"
        );
        assert_eq!(
            layout.vertical_neighbour(11, true),
            None,
            "nothing below the end"
        );
    }

    #[core::prelude::v1::test]
    fn row_of_and_y_of_item_round_trip() {
        let set = [1.5, 0.75, 1.0, 1.333];
        let mut cs = cells(&(0..40).map(|i| set[i % 4]).collect::<Vec<_>>(), 20250614);
        cs.extend(cells(
            &(0..30).map(|i| set[i % 4]).collect::<Vec<_>>(),
            20250610,
        ));
        let layout = MosaicLayout::justify(&cs, 900.0, 150.0, true);
        for i in 0..cs.len() {
            let r = layout.row_of(i);
            let row = layout.rows[r];
            assert_eq!(row.kind, RowKind::Tiles, "item {i} maps to a tile row");
            assert!(
                (row.first..row.first + row.len).contains(&i),
                "row {r} holds {i}"
            );
            let mid = row.y + row.height / 2.0;
            assert!(
                layout.rows_in(mid..mid + 0.1).contains(&r),
                "rows_in finds the row at its own y"
            );
        }
        assert_eq!(layout.rows_in(-10.0..0.0), 0..0, "nothing above the top");
        let all = layout.rows_in(0.0..total_height(&layout));
        assert_eq!(
            all,
            0..layout.rows.len(),
            "the full height covers every row"
        );
    }

    #[core::prelude::v1::test]
    fn extreme_aspects_and_widths_stay_inside_the_width() {
        // A 10:1 panorama alone at the end of a section: shrinks to fit.
        let layout = MosaicLayout::justify(&cells(&[10.0], 20250614), 800.0, 190.0, true);
        let row = tile_rows(&layout)[0];
        assert!(
            right_edge(&layout, &row) <= 800.0 + 0.01,
            "a panorama never overflows"
        );
        assert!(
            (row.height - row.pad - 80.0).abs() < 0.01,
            "its row shrinks to 80"
        );

        // Width smaller than one tile: one item per row, all inside.
        let layout = MosaicLayout::justify(&cells(&[1.0, 0.4, 3.0, 1.0], 0), 40.0, 190.0, true);
        for row in tile_rows(&layout) {
            assert!(
                right_edge(&layout, &row) <= 40.0 + 0.01,
                "tiny width: {row:?}"
            );
            assert!(row.height > row.pad, "every row has a positive height");
        }

        // 300 wide: 1.2 alone is 250 high, with a 3.0 it is 70, so the
        // single tile closes early and is capped (the one ragged full row).
        let layout = MosaicLayout::justify(&cells(&[1.2, 3.0, 3.0], 0), 300.0, 190.0, false);
        let first = tile_rows(&layout)[0];
        assert_eq!(first.len, 1, "the single tile closes alone");
        assert!(
            (first.height - first.pad - 190.0 / MIN_SHARE).abs() < 0.01,
            "a single closed tile is capped at target / MIN_SHARE: {first:?}"
        );
        assert!(right_edge(&layout, &first) < 300.0, "and left-aligned");

        // NaN, 0 and negative aspects read as slivers: finite, positive, inside.
        let layout = MosaicLayout::justify(&cells(&[f32::NAN, 0.0, -1.0], 0), 800.0, 190.0, true);
        for t in &layout.tiles {
            assert!(
                t.w.is_finite() && t.w > 0.0 && t.x + t.w <= 800.0,
                "bad aspect: {t:?}"
            );
        }

        let empty = MosaicLayout::justify(&[], 800.0, 190.0, true);
        assert!(empty.rows.is_empty() && empty.tiles.is_empty());
        assert_eq!(total_height(&empty), 0.0);
        assert_eq!(empty.vertical_neighbour(0, true), None);
        assert_eq!(empty.rows_in(0.0..100.0), 0..0);
    }

    /// Deterministic xorshift values in 0..1.
    fn xorshift(n: usize, mut s: u32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s % 10_000) as f32 / 10_000.0
            })
            .collect()
    }

    /// Tile rows that end before their section does (not the last row).
    fn closed_rows(layout: &MosaicLayout) -> Vec<MosaicRow> {
        let rows = &layout.rows;
        (0..rows.len())
            .filter(|&r| rows[r].kind == RowKind::Tiles)
            .filter(|&r| rows.get(r + 1).is_some_and(|n| n.kind == RowKind::Tiles))
            .map(|r| rows[r])
            .collect()
    }

    #[core::prelude::v1::test]
    fn closed_rows_fill_the_width_for_any_aspect_mix() {
        // Uniform 0.4-3.0, 50-item days: every closed row of 2+ fills.
        let uniform: Vec<LayoutCell> = xorshift(5_000, 0x1234_5678)
            .iter()
            .enumerate()
            .map(|(i, r)| LayoutCell {
                id: MediaId(i as u64),
                aspect: 0.4 + 2.6 * r,
                day: 20250614 - (i / 50) as u32,
            })
            .collect();
        for width in [800.0, 1200.0] {
            let layout = MosaicLayout::justify(&uniform, width, 190.0, true);
            let closed = closed_rows(&layout);
            assert!(closed.len() > 100, "enough closed rows at {width}");
            for row in closed.iter().filter(|r| r.len >= 2) {
                assert!(
                    (right_edge(&layout, row) - width).abs() <= 0.5,
                    "a closed row of 2+ fills {width}: {row:?} ends at {}",
                    right_edge(&layout, row)
                );
            }
        }

        // The real photo mix at 1200: every closed row stays in the band.
        let mix = [4.0 / 3.0, 1.5, 0.75, 2.0 / 3.0, 16.0 / 9.0, 9.0 / 16.0, 1.0];
        let photos: Vec<LayoutCell> = xorshift(5_000, 0x0BAD_F00D)
            .iter()
            .enumerate()
            .map(|(i, r)| LayoutCell {
                id: MediaId(i as u64),
                aspect: mix[(r * mix.len() as f32) as usize % mix.len()],
                day: 20250614 - (i / 50) as u32,
            })
            .collect();
        let layout = MosaicLayout::justify(&photos, 1200.0, 190.0, true);
        for row in closed_rows(&layout) {
            let h = row.height - row.pad;
            assert!(
                (158.0..=224.0).contains(&h),
                "photo-mix rows stay in the 158-224 band at 1200: {h}"
            );
            assert!(
                (right_edge(&layout, &row) - 1200.0).abs() <= 0.5,
                "and fill the width"
            );
        }
    }

    #[core::prelude::v1::test]
    fn targets_follow_the_size_ladder() {
        assert_eq!(target_for_columns(4), 190.0, "the default level is 190");
        assert_eq!(target_for_columns(8), 110.0);
        assert_eq!(target_for_columns(2), 320.0);
    }

    #[core::prelude::v1::test]
    fn justify_100k_within_budget() {
        // xorshift: deterministic aspects in 0.4-3.0, dates in 100-item days.
        let mut s: u32 = 0x9E37_79B9;
        let cs: Vec<LayoutCell> = (0..100_000u32)
            .map(|i| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                LayoutCell {
                    id: MediaId(u64::from(i)),
                    aspect: 0.4 + (s % 2600) as f32 / 1000.0,
                    day: 20250614 - i / 100,
                }
            })
            .collect();
        let start = std::time::Instant::now();
        let layout = MosaicLayout::justify(&cs, 1400.0, 190.0, true);
        let elapsed = start.elapsed();
        println!(
            "justify 100k: {:.2} ms, {} rows",
            elapsed.as_secs_f64() * 1000.0,
            layout.rows.len()
        );
        assert_eq!(layout.tiles.len(), 100_000, "a span per item");
        assert!(
            elapsed.as_millis() < 60,
            "justify of 100k items stays under 60 ms (debug): {elapsed:?}"
        );
    }
}
