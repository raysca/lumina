//! The Mosaic's Years and Months levels: a card per year or month with
//! items, over the zoom pill's Years · Months · Days segments. They apply
//! only where the timeline rail is (All Media, date sort, no query or year
//! filter), because the cards come from the library-wide timeline buckets
//! (`refresh_album_counts`) and the rail's month offsets. A card's click
//! or ↵ opens Days with that period's first day header on top. A frame
//! never queries; covers go through the grid's thumbnail cache.

use std::collections::HashMap;

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaId, TimelineBucket};

use crate::app::LuminaApp;
use crate::config::GroupLevel;
use crate::i18n::t;
use crate::theme;
use crate::views::grid::{GRID_ROW_PADDING_LEFT, GRID_ROW_PADDING_RIGHT};

/// Gap between cards, across and down.
const CARD_GAP: f32 = 12.0;
/// The title and count under a cover.
const CAPTION_H: f32 = 44.0;

/// Covers from the timeline buckets and the keyboard focus over the cards.
#[derive(Default)]
pub(crate) struct GroupLevels {
    year_covers: HashMap<i32, MediaId>,
    month_covers: HashMap<(i32, u32), MediaId>,
    /// The focused card; ↵ opens it. Back to the first on a level change.
    focus: usize,
    scroll: UniformListScrollHandle,
}

impl GroupLevels {
    /// Keeps the covers of buckets read by `refresh_album_counts`.
    pub(crate) fn set_covers(&mut self, years: &[TimelineBucket], months: &[TimelineBucket]) {
        self.year_covers = years
            .iter()
            .filter_map(|b| Some((b.bucket_key.parse().ok()?, b.cover_media_id?)))
            .collect();
        self.month_covers = months
            .iter()
            .filter_map(|b| {
                let (year, month) = b.bucket_key.split_once('-')?;
                Some(((year.parse().ok()?, month.parse().ok()?), b.cover_media_id?))
            })
            .collect();
    }
}

/// One year or month card.
#[derive(Debug, Clone)]
pub(crate) struct BucketCard {
    /// `year-card-2025` or `month-card-202506`.
    pub selector: String,
    pub title: String,
    pub count: String,
    pub aria_label: String,
    pub cover: Option<MediaId>,
    /// The period's first item in the view's order.
    pub offset: usize,
}

impl LuminaApp {
    /// The Years · Months · Days segments render: Mosaic where the rail is.
    pub(crate) fn group_levels_shown(&self) -> bool {
        self.mosaic_shown() && self.timeline_rail_shown()
    }

    /// The level the body shows: the saved one where the segments render,
    /// else Days.
    pub(crate) fn group_level(&self) -> GroupLevel {
        if self.group_levels_shown() {
            self.config.group_level
        } else {
            GroupLevel::Days
        }
    }

    /// A segment or ⌘K Show Years / Months / Days. Saved in the config; a
    /// failed save puts the level back and is in the error banner.
    pub(crate) fn set_group_level(&mut self, level: GroupLevel, cx: &mut Context<Self>) {
        let previous = self.config.group_level;
        if previous == level {
            return;
        }
        self.config.group_level = level;
        if !self.save_config() {
            self.config.group_level = previous;
        }
        self.levels.focus = 0;
        cx.notify();
    }

    /// Cards per row: 3 years or 4 months.
    fn cards_per_row(&self) -> usize {
        match self.group_level() {
            GroupLevel::Years => 3,
            _ => 4,
        }
    }

    /// The cards of the current level, in the sort's direction; empty on
    /// Days. From the cached buckets and rail months only.
    pub(crate) fn bucket_cards(&self) -> Vec<BucketCard> {
        let direction = self.sort.direction;
        let mut cards: Vec<BucketCard> = match self.group_level() {
            GroupLevel::Days => return Vec::new(),
            GroupLevel::Years => self
                .timeline_year_buckets
                .iter()
                .filter(|(_, count)| *count > 0)
                .filter_map(|&(year, count)| {
                    let offset = self.rail.year_offset(year, direction)?;
                    Some(card(
                        format!("year-card-{year}"),
                        year.to_string(),
                        count,
                        self.levels.year_covers.get(&year).copied(),
                        offset,
                    ))
                })
                .collect(),
            GroupLevel::Months => self
                .rail
                .months
                .iter()
                .map(|m| {
                    card(
                        format!("month-card-{}{:02}", m.year, m.month),
                        crate::i18n::month_year(m.year, m.month),
                        m.count,
                        self.levels.month_covers.get(&(m.year, m.month)).copied(),
                        self.rail.offset(m, direction),
                    )
                })
                .collect(),
        };
        cards.sort_by_key(|c| c.offset);
        cards
    }

    /// The focused card, `None` on Days.
    pub(crate) fn bucket_cards_focus(&self) -> Option<usize> {
        let len = self.bucket_cards().len();
        (len > 0).then(|| self.levels.focus.min(len - 1))
    }

    /// Opens Days with the card's first day header on top.
    pub(crate) fn open_bucket_card(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.set_group_level(GroupLevel::Days, cx);
        if self.config.group_level == GroupLevel::Days {
            self.scroll_grid_to_offset(offset, cx);
        }
    }

    /// Years and Months take arrows, ↵ and Esc (back to Days), keep ⌘= /
    /// ⌘- for the size, and absorb the grid's keys: the selection is not on
    /// screen, so ⌫ or F must not change it.
    pub(crate) fn handle_bucket_cards_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.group_level() == GroupLevel::Days || self.is_fullscreen_viewer {
            return false;
        }
        let m = &keystroke.modifiers;
        let command = m.secondary(cx) || m.control;
        let plain = !(m.platform || m.control || m.alt || m.function || m.shift);
        let per_row = self.cards_per_row() as isize;
        let step = match keystroke.key.as_str() {
            "=" | "+" if command => {
                self.zoom_in_grid(cx);
                return true;
            }
            "-" | "_" if command => {
                self.zoom_out_grid(cx);
                return true;
            }
            "left" if plain => -1,
            "right" if plain => 1,
            "up" if plain => -per_row,
            "down" if plain => per_row,
            "enter" | "return" if plain => {
                let cards = self.bucket_cards();
                if let Some(card) = self.bucket_cards_focus().and_then(|i| cards.get(i)) {
                    self.open_bucket_card(card.offset, cx);
                }
                return true;
            }
            "escape" => {
                self.set_group_level(GroupLevel::Days, cx);
                return true;
            }
            _ => return true,
        };
        if let Some(focus) = self.bucket_cards_focus() {
            let last = self.bucket_cards().len() - 1;
            let next = (focus as isize + step).clamp(0, last as isize) as usize;
            self.levels.focus = next;
            self.levels
                .scroll
                .scroll_to_item(next / per_row as usize, ScrollStrategy::Nearest);
            cx.notify();
        }
        true
    }

    /// The Years or Months body: rows of cards in a uniform list.
    pub(crate) fn render_bucket_cards(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cards = self.bucket_cards();
        let per_row = self.cards_per_row();
        let rows = cards.len().div_ceil(per_row);
        let cover_h = if per_row == 3 { 200.0 } else { 150.0 };
        let focus = self.bucket_cards_focus();
        let list = uniform_list(
            "bucket-cards",
            rows,
            cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                range
                    .map(|row| {
                        let start = row * per_row;
                        let end = (start + per_row).min(cards.len());
                        div()
                            .w_full()
                            .pb(px(CARD_GAP))
                            .flex()
                            .flex_row()
                            .gap(px(CARD_GAP))
                            .children((start..end).map(|i| {
                                this.render_bucket_card(&cards[i], cover_h, focus == Some(i), cx)
                            }))
                            // Pads a short last row so its cards keep their width.
                            .children((end..start + per_row).map(|_| div().flex_1()))
                    })
                    .collect()
            }),
        )
        .track_scroll(&self.levels.scroll)
        .size_full();
        div()
            .debug_selector(|| "bucket-cards".to_string())
            .flex_1()
            .h_full()
            .pt_2()
            .pl(px(GRID_ROW_PADDING_LEFT))
            .pr(px(GRID_ROW_PADDING_RIGHT))
            .child(list)
            .into_any_element()
    }

    fn render_bucket_card(
        &self,
        card: &BucketCard,
        cover_h: f32,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let texture = card.cover.and_then(|id| {
            let texture = self
                .grid_state
                .as_ref()
                .and_then(|state| state.texture_cache.lock().get(&id).cloned());
            if texture.is_none() {
                self.queue_thumbnail_request(id);
            }
            texture
        });
        let selector = card.selector.clone();
        let offset = card.offset;
        div()
            .id(SharedString::from(card.selector.clone()))
            .debug_selector(move || selector.clone())
            .role(Role::Button)
            .aria_label(SharedString::from(card.aria_label.clone()))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .p(px(3.0))
            .rounded(theme::RADIUS_MD)
            .border_2()
            .border_color(if focused {
                Hsla::from(theme::focus_ring())
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .child(
                div()
                    .w_full()
                    .h(px(cover_h))
                    .rounded(theme::RADIUS_SM)
                    .overflow_hidden()
                    .bg(linear_gradient(
                        135.0,
                        linear_color_stop(theme::bg_2(), 0.0),
                        linear_color_stop(theme::bg_0(), 1.0),
                    ))
                    .when_some(texture, |well, texture| {
                        well.child(img(texture).size_full().object_fit(ObjectFit::Cover))
                    }),
            )
            .child(
                div()
                    .h(px(CAPTION_H))
                    .pt(px(8.0))
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(9.0))
                    .child(
                        div()
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .child(card.title.clone()),
                    )
                    .child(
                        div()
                            .font_family(theme::FONT_MONO)
                            .text_size(px(10.0))
                            .text_color(theme::t3())
                            .child(card.count.clone()),
                    ),
            )
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                this.open_bucket_card(offset, cx);
            }))
    }

    /// The zoom pill's Years · Months · Days segments, where they render.
    pub(crate) fn render_group_segments(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.group_levels_shown() {
            return None;
        }
        let current = self.config.group_level;
        let segments = [
            (
                GroupLevel::Years,
                "zoom-level-years",
                "zoom.years",
                "command.show_years",
            ),
            (
                GroupLevel::Months,
                "zoom-level-months",
                "zoom.months",
                "command.show_months",
            ),
            (
                GroupLevel::Days,
                "zoom-level-days",
                "zoom.days",
                "command.show_days",
            ),
        ];
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0))
                .children(segments.into_iter().map(|(level, id, label, tip)| {
                    let on = level == current;
                    div()
                        .id(id)
                        .debug_selector(move || id.to_string())
                        .role(Role::Button)
                        .aria_label(SharedString::from(t(label)))
                        .aria_toggled(if on { Toggled::True } else { Toggled::False })
                        .tooltip(theme::tooltip_label(t(tip)))
                        .h(px(30.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .font_family(theme::FONT_SANS)
                        .text_size(px(11.5))
                        .map(|d| {
                            if on {
                                d.bg(theme::accent())
                                    .text_color(theme::on_accent())
                                    .font_weight(FontWeight::SEMIBOLD)
                            } else {
                                d.text_color(theme::t2())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme::bg_3()))
                            }
                        })
                        .child(t(label))
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.set_group_level(level, cx);
                        }))
                }))
                .child(div().w(px(1.0)).h(px(18.0)).mx(px(4.0)).bg(theme::line_2())),
        )
    }
}

/// A card's texts; the label reads like "2025, 1,234 items".
fn card(
    selector: String,
    title: String,
    count: usize,
    cover: Option<MediaId>,
    offset: usize,
) -> BucketCard {
    let count = crate::i18n::items(count);
    BucketCard {
        aria_label: format!("{title}, {count}"),
        selector,
        title,
        count,
        cover,
        offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn a_card_reads_its_title_and_count() {
        let c = card("year-card-2025".into(), "2025".into(), 1234, None, 0);
        assert_eq!(c.aria_label, "2025, 1,234 items");
    }
}
