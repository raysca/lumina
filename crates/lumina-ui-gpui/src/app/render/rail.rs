//! Timeline rail: a scroll scrubber beside the date-sorted All Media grid.
//! One bar per month (width ∝ its count), a lens over the months on screen,
//! and a click on a year or bar scrolls the grid to that month's first item.
//! Everything is derived from the cached month buckets; nothing queries
//! SQLite per frame.

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::theme;
use crate::views::grid::YEAR_RAIL_WIDTH;
use crate::views::page_header::text_marker;
use crate::views::sidebar::AlbumSelection;

/// "TIMELINE" label top.
const LABEL_TOP: f32 = 8.0;
/// First year row.
const FIRST_ROW: f32 = 26.0;
/// Month bar height and the step between bars.
const BAR_H: f32 = 4.0;
const BAR_PITCH: f32 = 7.0;
/// Year label line height. The year's bars start `BARS_TOP` below the
/// label's top, on their own lines, and the next year `YEAR_GAP` below the
/// last bar.
const YEAR_H: f32 = 13.0;
const BARS_TOP: f32 = 15.0;
const YEAR_GAP: f32 = 20.0;
/// Bars are right-aligned to this x; the longest is `BAR_MAX` wide.
const BAR_RIGHT: f32 = 70.0;
const BAR_MAX: f32 = 36.0;
/// The lens spans x 26-78 and is at least `LENS_MIN_H` tall.
const LENS_LEFT: f32 = 26.0;
const LENS_RIGHT: f32 = 78.0;
const LENS_MIN_H: f32 = 29.0;
/// Height of the "{year} ↓" row.
const OLDER_H: f32 = 16.0;
/// Right edges of a year label ("2026") and of the "{year} ↓" label. A lens
/// that reaches a label's line starts right of it, so it never covers a date.
const YEAR_LABEL_RIGHT: f32 = 34.0;
const OLDER_LABEL_RIGHT: f32 = 48.0;

/// One month with items.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailMonth {
    pub year: i32,
    pub month: u32,
    pub count: usize,
    /// Items in newer months: the month's first index in a newest-first view.
    pub newer: usize,
}

/// The rail's cached months.
#[derive(Default)]
pub struct TimelineRail {
    /// Every month with items, newest first. Rebuilt with the month buckets.
    pub months: Vec<RailMonth>,
    /// Items in all months.
    pub total: usize,
    /// The pointer is over the rail (shows the bubble).
    pub hovered: bool,
    /// The rail's painted height, measured at prepaint (0 before the first
    /// frame). Years past it collapse into the "{year} ↓" row.
    pub height: Rc<Cell<f32>>,
}

impl TimelineRail {
    /// Rebuilds the months from the month buckets (keys `(year, month)`).
    /// Bucket keys come from the same UTC timestamp the date sort orders by,
    /// so newest-first keys match the grid's order.
    pub fn set_counts(&mut self, counts: &HashMap<(i32, u32), usize>) {
        let mut keys: Vec<_> = counts.iter().filter(|(_, n)| **n > 0).collect();
        keys.sort_by(|a, b| b.0.cmp(a.0));
        let mut newer = 0;
        self.months = keys
            .into_iter()
            .map(|(&(year, month), &count)| {
                let m = RailMonth {
                    year,
                    month,
                    count,
                    newer,
                };
                newer += count;
                m
            })
            .collect();
        self.total = newer;
    }

    /// First grid index of `month` under `direction`.
    pub(crate) fn offset(&self, month: &RailMonth, direction: SortDirection) -> usize {
        match direction {
            SortDirection::Descending => month.newer,
            SortDirection::Ascending => self.total - month.newer - month.count,
        }
    }

    /// First grid index of `year` (its newest month descending, oldest
    /// ascending).
    pub(crate) fn year_offset(&self, year: i32, direction: SortDirection) -> Option<usize> {
        self.months
            .iter()
            .filter(|m| m.year == year)
            .map(|m| self.offset(m, direction))
            .min()
    }

    /// Indices into `months` (newest, oldest) of the months whose items
    /// overlap the grid indices `items`.
    fn visible_months(
        &self,
        direction: SortDirection,
        items: &Range<usize>,
    ) -> Option<(usize, usize)> {
        let mut on = self.months.iter().enumerate().filter(|(_, m)| {
            let start = self.offset(m, direction);
            start < items.end && start + m.count > items.start
        });
        let first = on.next()?.0;
        let last = on.next_back().map_or(first, |(i, _)| i);
        Some((first, last))
    }
}

/// "JUN 2025", "MAY — JUL 2025" or "DEC 2024 — FEB 2025", oldest first;
/// undated items (year 0) read "UNDATED".
fn range_label(oldest: &RailMonth, newest: &RailMonth) -> String {
    let short = |m: &RailMonth| t(&format!("date.month_short_{}", m.month)).to_uppercase();
    let full = |m: &RailMonth| {
        if m.year > 0 {
            format!("{} {}", short(m), m.year)
        } else {
            t("date.undated").to_uppercase()
        }
    };
    if (oldest.year, oldest.month) == (newest.year, newest.month) {
        full(newest)
    } else if oldest.year == newest.year && newest.year > 0 {
        format!("{} — {} {}", short(oldest), short(newest), newest.year)
    } else {
        format!("{} — {}", full(oldest), full(newest))
    }
}

enum RailRow {
    Year {
        year: i32,
        y: f32,
    },
    Bar {
        index: usize,
        y: f32,
    },
    /// Every month from `first` on (the newest hidden year and older).
    Older {
        year: i32,
        first: usize,
        y: f32,
    },
}

/// Lays out years top-down until `height` runs out; the rest collapse into
/// one "{year} ↓" row.
fn layout(months: &[RailMonth], height: f32) -> Vec<RailRow> {
    let mut rows = Vec::new();
    let mut y = FIRST_ROW;
    let mut i = 0;
    while i < months.len() {
        let year = months[i].year;
        let n = months[i..].iter().take_while(|m| m.year == year).count();
        let block = BARS_TOP + n as f32 * BAR_PITCH - (BAR_PITCH - BAR_H);
        let is_last = i + n == months.len();
        // Keep room for the older row unless this is the last year.
        let needed = block + if is_last { 0.0 } else { YEAR_GAP + OLDER_H };
        if y + needed > height && !rows.is_empty() {
            rows.push(RailRow::Older { year, first: i, y });
            return rows;
        }
        rows.push(RailRow::Year { year, y });
        for k in 0..n {
            rows.push(RailRow::Bar {
                index: i + k,
                y: y + BARS_TOP + k as f32 * BAR_PITCH,
            });
        }
        y += block + YEAR_GAP;
        i += n;
    }
    rows
}

/// Vertical centre of month `index`'s row: its bar, or the "{year} ↓" row
/// when it is collapsed.
fn month_y(rows: &[RailRow], index: usize) -> Option<f32> {
    rows.iter().find_map(|row| match *row {
        RailRow::Bar { index: i, y } if i == index => Some(y + BAR_H / 2.0),
        RailRow::Older { first, y, .. } if index >= first => Some(y + OLDER_H / 2.0),
        _ => None,
    })
}

/// The lens over `ys` (top to bottom, in rail coordinates) as (left, top,
/// bottom). It clears the label of the year its newest month is in, moving
/// down; if it still reaches a label (it spans years, or sits on the
/// "{year} ↓" row) it starts right of that label instead.
fn lens_rect(rows: &[RailRow], newest_year: i32, ys: Range<f32>) -> (f32, f32, f32) {
    let (mut top, mut bottom) = (ys.start, ys.end);
    let own_label = rows.iter().find_map(|row| match *row {
        RailRow::Year { year, y } if year == newest_year => Some(y + YEAR_H),
        _ => None,
    });
    if let Some(label_bottom) = own_label.filter(|b| top < *b) {
        bottom += label_bottom - top;
        top = label_bottom;
    }
    let left = rows
        .iter()
        .filter_map(|row| match *row {
            RailRow::Year { y, .. } if y < bottom && top < y + YEAR_H => Some(YEAR_LABEL_RIGHT),
            RailRow::Older { y, .. } if y < bottom && top < y + OLDER_H => Some(OLDER_LABEL_RIGHT),
            _ => None,
        })
        .fold(LENS_LEFT, f32::max);
    (left, top, bottom)
}

impl LuminaApp {
    /// The rail scrubs library-wide month offsets, so it shows only where
    /// those offsets are the grid's: All Media, date sort, no query, no year
    /// filter. That also hides it on the map (Places) and Duplicates.
    pub(crate) fn timeline_rail_shown(&self) -> bool {
        self.selection == AlbumSelection::AllLibrary
            && self.sort.field == SortField::DateTaken
            && self.search_query.is_empty()
            && self.active_year.is_none()
            && !self.rail.months.is_empty()
    }

    /// Grid indices of the rows on screen, from the scroll handle as
    /// [`Self::grid_top_row`] reads it, never from what a frame drew. Before
    /// the list's first layout (no viewport height) it is the top row.
    pub(crate) fn grid_visible_items(&self) -> Range<usize> {
        if self.mosaic_shown() {
            let items = self.mosaic_visible_items();
            return items.start.min(self.rail.total)..items.end.min(self.rail.total);
        }
        let Some(state) = self.grid_state.as_ref() else {
            return 0..0;
        };
        let (offset_y, view_h) = {
            let handle = state.scroll_handle.0.borrow();
            let view_h = handle
                .last_item_size
                .map_or(0.0, |size| f32::from(size.item.height));
            (f32::from(handle.base_handle.offset().y), view_h)
        };
        let row_h = self.row_height().max(1.0);
        let cols = self.grid_cols();
        let total = self.rail.total;
        let top = self.grid_top_row();
        // Half a pixel of slack, as in `grid_top_row`.
        let bottom = (((view_h - offset_y - 0.5) / row_h).ceil().max(0.0) as usize).max(top + 1);
        (top * cols).min(total)..(bottom * cols).min(total)
    }

    /// The bubble's text: the months on screen.
    pub(crate) fn timeline_bubble_label(&self) -> Option<String> {
        let (newest, oldest) = self
            .rail
            .visible_months(self.sort.direction, &self.grid_visible_items())?;
        Some(range_label(
            &self.rail.months[oldest],
            &self.rail.months[newest],
        ))
    }

    /// Scrolls the grid so item `offset`'s row is at the top: offset /
    /// columns × row height, written to the handle now (not a deferred
    /// scroll) so this frame's rail lens already reads it. Clamped to the
    /// last layout's range; the list's prepaint clamps it again.
    pub(crate) fn scroll_grid_to_offset(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.mosaic_shown() {
            // From Years or Months (a rail click) this opens Days there.
            if self.group_level() != crate::config::GroupLevel::Days {
                self.set_group_level(crate::config::GroupLevel::Days, cx);
            }
            // The item's day header (its first row) at the top.
            self.scroll_mosaic_to_item(offset, cx);
            return;
        }
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let cols = self.grid_cols();
        let mut y = (offset / cols) as f32 * self.row_height();
        let handle = state.scroll_handle.0.borrow();
        let base = &handle.base_handle;
        let max = f32::from(base.max_offset().y);
        if max > 0.0 {
            y = y.min(max);
        }
        base.set_offset(point(base.offset().x, px(-y)));
        cx.notify();
    }

    /// Scrolls to the first item of `year` in the current sort direction.
    pub(crate) fn scroll_to_year(&mut self, year: i32, cx: &mut Context<Self>) {
        if let Some(offset) = self.rail.year_offset(year, self.sort.direction) {
            self.scroll_grid_to_offset(offset, cx);
        }
    }

    pub(crate) fn render_year_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.timeline_rail_shown() {
            return Empty.into_any_element();
        }
        let direction = self.sort.direction;
        let rail = &self.rail;
        // Unmeasured: lay out everything (the rows clip), then the measured
        // height collapses the overflow on the next frame.
        let height = match rail.height.get() {
            h if h > 0.0 => h,
            _ => f32::INFINITY,
        };
        let rows = layout(&rail.months, height);
        let visible = rail.visible_months(direction, &self.grid_visible_items());
        let current_year = visible.map(|(newest, _)| rail.months[newest].year);
        let max = rail.months.iter().map(|m| m.count).max().unwrap_or(1) as f32;

        // Lens from the newest to the oldest visible month; collapsed months
        // land on the "{year} ↓" row.
        let lens = visible.and_then(|(newest, oldest)| {
            let top = month_y(&rows, newest)?;
            let bottom = month_y(&rows, oldest)?;
            let h = (bottom - top + BAR_H + 8.0).max(LENS_MIN_H);
            let mid = (top + bottom) / 2.0;
            Some(lens_rect(
                &rows,
                rail.months[newest].year,
                mid - h / 2.0..mid + h / 2.0,
            ))
        });

        let mut children: Vec<AnyElement> = Vec::with_capacity(rows.len() + 2);
        children.push(
            div()
                .debug_selector(|| "timeline-label".to_string())
                .absolute()
                .left(px(10.0))
                .top(px(LABEL_TOP))
                .text_size(theme::TEXT_MICRO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t3())
                .child(t("timeline.label"))
                .into_any_element(),
        );
        if let Some((left, top, bottom)) = lens {
            children.push(
                div()
                    .debug_selector(|| "timeline-lens".to_string())
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(LENS_RIGHT - left))
                    .h(px(bottom - top))
                    .rounded(theme::RADIUS_SM)
                    .bg(theme::accent_soft())
                    .border_1()
                    .border_color(theme::accent())
                    .into_any_element(),
            );
        }
        for row in &rows {
            children.push(match *row {
                RailRow::Year { year, y } => {
                    let is_current = current_year == Some(year);
                    let name = crate::i18n::year_label(year);
                    let label = t("timeline.scroll_to").replace("{year}", &name);
                    div()
                        .id(ElementId::NamedInteger("timeline-year".into(), year as u64))
                        .debug_selector(move || format!("timeline-year-{year}"))
                        .role(Role::Button)
                        .aria_label(label.clone())
                        .tooltip(theme::tooltip_label(label))
                        .absolute()
                        .left(px(10.0))
                        .top(px(y))
                        .h(px(YEAR_H))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .font_family(theme::FONT_SANS)
                        .font_features(theme::tabular_figures())
                        .text_size(theme::TEXT_MICRO)
                        .text_color(if is_current { theme::t1() } else { theme::t3() })
                        .hover(|s| s.text_color(theme::t1()))
                        .children(text_marker("timeline-year", &name.clone().into()))
                        .child(name)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.scroll_to_year(year, cx);
                        }))
                        .into_any_element()
                }
                RailRow::Bar { index, y } => {
                    let m = rail.months[index];
                    let offset = rail.offset(&m, direction);
                    let on_screen = visible.is_some_and(|(a, b)| (a..=b).contains(&index));
                    let color = if on_screen {
                        theme::accent()
                    } else if current_year == Some(m.year) {
                        theme::rail_bar_current()
                    } else {
                        theme::rail_bar()
                    };
                    let (year, month) = (m.year, m.month);
                    let width = (m.count as f32 / max * BAR_MAX).max(2.0);
                    let label = t("timeline.month")
                        .replace("{month}", &crate::i18n::month_year(year, month))
                        .replace("{items}", &crate::i18n::plural("header.items", m.count));
                    let group: SharedString = format!("timeline-bar-{year}-{month}").into();
                    // A taller hit target than the 4 px bar, so it is clickable.
                    div()
                        .id(ElementId::NamedInteger(
                            "timeline-bar".into(),
                            (year as u64) * 100 + month as u64,
                        ))
                        .debug_selector(move || format!("timeline-bar-{year}-{month:02}"))
                        .group(group.clone())
                        .role(Role::Button)
                        .aria_label(label.clone())
                        .tooltip(theme::tooltip_label(label))
                        .absolute()
                        .left(px(BAR_RIGHT - BAR_MAX))
                        .top(px(y - (BAR_PITCH - BAR_H) / 2.0))
                        .w(px(BAR_MAX))
                        .h(px(BAR_PITCH))
                        .flex()
                        .items_center()
                        .justify_end()
                        .cursor_pointer()
                        .child(
                            div()
                                .debug_selector(move || {
                                    format!("timeline-bar-fill-{year}-{month:02}")
                                })
                                .w(px(width))
                                .h(px(BAR_H))
                                .rounded(theme::RADIUS_XS)
                                .bg(color)
                                .group_hover(group, |s| s.bg(theme::accent_hover())),
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.scroll_grid_to_offset(offset, cx);
                        }))
                        .into_any_element()
                }
                RailRow::Older { year, first, y } => {
                    let on_screen = visible.is_some_and(|(_, oldest)| oldest >= first);
                    let name = crate::i18n::year_label(year);
                    let label = t("timeline.scroll_to_older").replace("{year}", &name);
                    div()
                        .id("timeline-older")
                        .debug_selector(|| "timeline-older".to_string())
                        .role(Role::Button)
                        .aria_label(label.clone())
                        .tooltip(theme::tooltip_label(label))
                        .absolute()
                        .left(px(10.0))
                        .top(px(y))
                        .h(px(OLDER_H))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .font_family(theme::FONT_SANS)
                        .font_features(theme::tabular_figures())
                        .text_size(theme::TEXT_MICRO)
                        .text_color(if on_screen {
                            theme::accent()
                        } else {
                            theme::t3()
                        })
                        .hover(|s| s.text_color(theme::t1()))
                        .child(format!("{name} ↓"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.scroll_to_year(year, cx);
                        }))
                        .into_any_element()
                }
            });
        }

        // The rail's real height, for the next layout. A change asks for one
        // more frame (a refresh during a draw is ignored, so it waits).
        let measured = rail.height.clone();
        let measure = canvas(
            move |bounds, window, _| {
                let h = f32::from(bounds.size.height);
                if (measured.get() - h).abs() > 0.5 {
                    measured.set(h);
                    window.on_next_frame(|window, _| window.refresh());
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        let bubble = rail
            .hovered
            .then(|| self.timeline_bubble_label())
            .flatten()
            .map(|label| {
                let top = lens.map_or(FIRST_ROW, |(_, top, _)| top).min(height - 23.0);
                div()
                    .debug_selector(|| "timeline-bubble".to_string())
                    .absolute()
                    .right(px(YEAR_RAIL_WIDTH + 6.0))
                    .top(px(top.max(0.0)))
                    .h(px(23.0))
                    .px(px(9.0))
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .bg(theme::bg_3())
                    .rounded(theme::RADIUS_MD)
                    .border_1()
                    .border_color(theme::line_2())
                    .shadow_md()
                    .font_family(theme::FONT_SANS)
                    .font_features(theme::tabular_figures())
                    .text_size(theme::TEXT_CAPTION)
                    .text_color(theme::accent())
                    .child(label)
            });

        div()
            .id("timeline-rail")
            .debug_selector(|| "timeline-rail".to_string())
            .relative()
            .w(px(YEAR_RAIL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(theme::bg_1())
            .border_l_1()
            .border_color(theme::line())
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.rail.hovered = *hovered;
                cx.notify();
            }))
            .child(measure)
            // Rows clip to the rail, so they never paint over the status bar
            // or a banner; the bubble sits outside the rail and is not clipped.
            .child(
                div()
                    .absolute()
                    .size_full()
                    .overflow_hidden()
                    .children(children),
            )
            .children(bubble)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail(counts: &[((i32, u32), usize)]) -> TimelineRail {
        let mut r = TimelineRail::default();
        r.set_counts(&counts.iter().copied().collect());
        r
    }

    #[core::prelude::v1::test]
    fn offsets_follow_the_sort_direction() {
        // Undated items are timestamp 0: the 1970-01 bucket, oldest.
        let r = rail(&[((2025, 6), 4), ((2024, 3), 2), ((1970, 1), 1)]);
        let june = r.months[0];
        assert_eq!((june.year, june.newer), (2025, 0), "newest first");
        assert_eq!(r.year_offset(2024, SortDirection::Descending), Some(4));
        assert_eq!(r.year_offset(2024, SortDirection::Ascending), Some(1));
        assert_eq!(r.year_offset(1970, SortDirection::Ascending), Some(0));
        assert_eq!(r.year_offset(2025, SortDirection::Ascending), Some(3));
    }

    #[core::prelude::v1::test]
    fn visible_months_and_label() {
        let r = rail(&[((2025, 7), 2), ((2025, 5), 2), ((2024, 12), 2)]);
        let (newest, oldest) = r
            .visible_months(SortDirection::Descending, &(1..4))
            .unwrap();
        assert_eq!(
            range_label(&r.months[oldest], &r.months[newest]),
            "MAY — JUL 2025"
        );
        let (newest, oldest) = r
            .visible_months(SortDirection::Descending, &(3..6))
            .unwrap();
        assert_eq!(
            range_label(&r.months[oldest], &r.months[newest]),
            "DEC 2024 — MAY 2025"
        );
        let (newest, oldest) = r.visible_months(SortDirection::Ascending, &(0..1)).unwrap();
        assert_eq!(
            range_label(&r.months[oldest], &r.months[newest]),
            "DEC 2024"
        );
    }

    /// Regression: undated items (year 0) read "0" on the rail and its
    /// bubble showed a raw month key.
    #[core::prelude::v1::test]
    fn undated_months_read_undated() {
        let r = rail(&[((2025, 7), 2), ((0, 0), 2)]);
        assert_eq!(range_label(&r.months[1], &r.months[1]), "UNDATED");
        assert_eq!(
            range_label(&r.months[1], &r.months[0]),
            "UNDATED — JUL 2025"
        );
    }

    #[core::prelude::v1::test]
    fn lens_keeps_the_design_x_unless_it_reaches_a_label() {
        let r = rail(&[
            ((2025, 7), 1),
            ((2025, 6), 1),
            ((2025, 5), 1),
            ((2024, 1), 1),
        ]);
        let rows = layout(&r.months, f32::INFINITY);
        // June alone: clear of the 2025 label and of 2024's.
        let y = month_y(&rows, 1).unwrap();
        let (left, top, _) = lens_rect(&rows, 2025, y - 14.5..y + 14.5);
        assert_eq!(left, LENS_LEFT, "no label near: the design's x 26");
        assert!(top >= FIRST_ROW + YEAR_H, "below the 2025 label");
        // July, the year's first month: moved below its label, same x.
        let y = month_y(&rows, 0).unwrap();
        let (left, top, bottom) = lens_rect(&rows, 2025, y - 14.5..y + 14.5);
        assert_eq!(top, FIRST_ROW + YEAR_H, "it starts under the label");
        assert!((bottom - top - 29.0).abs() < 0.01, "and keeps its height");
        assert_eq!(left, LENS_LEFT);
        // July to January 2024 spans the 2024 label: it starts right of it.
        let end = month_y(&rows, 3).unwrap();
        let (left, ..) = lens_rect(&rows, 2025, y - 6.0..end + 6.0);
        assert_eq!(left, YEAR_LABEL_RIGHT, "a lens over two years clears 2024");
    }

    #[core::prelude::v1::test]
    fn overflow_years_collapse_into_one_row() {
        let months: Vec<_> = (1970..=2026).map(|y| ((y, 1), 1)).collect();
        let r = rail(&months);
        let rows = layout(&r.months, 300.0);
        let older: Vec<_> = rows
            .iter()
            .filter_map(|row| match row {
                RailRow::Older { year, y, .. } => Some((*year, *y)),
                _ => None,
            })
            .collect();
        assert_eq!(older.len(), 1, "one older row");
        assert!(older[0].1 + OLDER_H <= 300.0, "it fits");
        let shown = rows
            .iter()
            .filter(|row| matches!(row, RailRow::Year { .. }))
            .count();
        assert_eq!(
            older[0].0,
            2026 - shown as i32,
            "it names the newest hidden year"
        );
    }
}
