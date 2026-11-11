//! The Mosaic body: a variable-height `list` of the cached layout's rows
//! (day headers and justified tile rows). A frame reads only the cached
//! layout (`app/mosaic_layout.rs`) and the active view's cached rows.

use std::ops::Range;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::grid::{GRID_ROW_PADDING_LEFT, GRID_ROW_PADDING_RIGHT};
use crate::views::mosaic::{MosaicLayout, RowKind, DAY_HEADER_TEXT};

impl LuminaApp {
    /// The Mosaic body. Shown only where [`Self::mosaic_shown`]; until the
    /// first layout of a view lands the list is empty, and a newer
    /// revision, width or level keeps the old layout painted until its
    /// replacement lands.
    pub(crate) fn render_mosaic(
        &mut self,
        total_count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if total_count == 0 {
            // Loading and empty states are the Grid's.
            return self.render_browser_grid(0, cx).into_any_element();
        }
        // The day layout keeps building under Years and Months, so a card
        // opens Days at once.
        self.refresh_mosaic_layout(cx);
        if self.group_level() != crate::config::GroupLevel::Days {
            return self.render_bucket_cards(cx);
        }
        let Some(list_state) = self.grid_state.as_ref().map(|s| s.mosaic.clone()) else {
            return Empty.into_any_element();
        };
        let layout = self.mosaic_layout();
        // Here, not in the row callback: the list holds its state borrowed
        // while it lays rows out. Input moved the scroll before this frame.
        if let Some(layout) = &layout {
            // A list replaced with its grid state (another GridState for
            // the same library) starts empty: give it the layout's rows.
            if list_state.item_count() != layout.rows.len() {
                list_state.reset(layout.rows.len());
            }
            self.want_mosaic_rows(layout, &list_state, cx);
        }
        let this = cx.entity().downgrade();
        let rows = list(list_state, move |ix, _window, cx| {
            let Some(layout) = layout.clone() else {
                return Empty.into_any_element();
            };
            this.update(cx, |app, cx| app.render_mosaic_row(ix, &layout, cx))
                .unwrap_or_else(|_| Empty.into_any_element())
        })
        .size_full();

        self.grid_surface(cx)
            .children(
                self.search_screen_shown()
                    .then(|| self.render_results_head(cx)),
            )
            .child(
                div()
                    .debug_selector(|| "mosaic-list".to_string())
                    .flex_1()
                    .min_h_0()
                    .child(rows),
            )
            .child(self.render_grid_drag_rect())
            .into_any_element()
    }

    /// Asks the active view for the items of the rows on screen and one
    /// viewport either side (the list's overdraw), from the list's scroll
    /// position. Never queries; a fetch runs in the background.
    fn want_mosaic_rows(
        &mut self,
        layout: &MosaicLayout,
        list: &ListState,
        cx: &mut Context<Self>,
    ) {
        let top = list.logical_scroll_top();
        let Some(row) = layout.rows.get(top.item_ix) else {
            return;
        };
        let y = row.y + f32::from(top.offset_in_item);
        // Before the list's first layout there is no viewport yet.
        let height = f32::from(list.viewport_bounds().size.height).max(900.0);
        let rows = layout.rows_in((y - height).max(0.0)..y + 2.0 * height);
        let (Some(first), Some(last)) = (layout.rows.get(rows.start), rows.end.checked_sub(1))
        else {
            return;
        };
        let last = layout.rows[last];
        self.want_view_rows(first.first..last.first + last.len, cx);
    }

    /// One layout row: a day header exactly `row.height` tall, or a row of
    /// cards at the layout's x and width. Items whose rows have not been
    /// fetched yet draw nothing until the fetch lands.
    fn render_mosaic_row(
        &mut self,
        ix: usize,
        layout: &MosaicLayout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(&row) = layout.rows.get(ix) else {
            return Empty.into_any_element();
        };
        let base = div()
            .w_full()
            .h(px(row.height))
            .flex_none()
            .relative()
            .pt(px(row.pad))
            .pl(px(GRID_ROW_PADDING_LEFT))
            .pr(px(GRID_ROW_PADDING_RIGHT))
            // ⌘-wheel zooms. Rows paint after the list registers its own
            // scroll handler, so this bubbles first and its stop keeps the
            // list from also scrolling.
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                if this.zoom_grid_with_wheel(event, window, cx) {
                    cx.stop_propagation();
                }
            }));
        match row.kind {
            RowKind::Header { day, count } => {
                self.render_day_header(base, day, row.first..row.first + count, cx)
            }
            RowKind::Tiles => {
                let items = self.displayed_rows(row.first..row.first + row.len);
                let height = row.height - row.pad;
                let mut tiles = Vec::with_capacity(items.len());
                for (offset, item) in items.iter().enumerate() {
                    let index = row.first + offset;
                    let Some(span) = layout.tiles.get(index) else {
                        break;
                    };
                    let texture = self.card_texture(item);
                    tiles.push(
                        div()
                            .absolute()
                            .top(px(row.pad))
                            .left(px(GRID_ROW_PADDING_LEFT + span.x))
                            .w(px(span.w))
                            .h(px(height))
                            .flex()
                            .child(self.render_media_card(
                                item,
                                index,
                                size(span.w, height),
                                texture,
                                cx,
                            )),
                    );
                }
                base.children(tiles).into_any_element()
            }
        }
    }

    /// The item's cached thumbnail, or `None` after queueing its decode.
    fn card_texture(&self, item: &lumina_core::MediaItem) -> Option<Arc<RenderImage>> {
        let texture = self
            .grid_state
            .as_ref()
            .and_then(|state| state.texture_cache.lock().get(&item.id).cloned());
        if texture.is_none() {
            self.queue_thumbnail_request_item(item);
        }
        texture
    }
}

/// What a day header says, and its accessible labels.
#[derive(Debug)]
pub(crate) struct DayHeader {
    pub date: String,
    pub place: Option<String>,
    pub count: String,
    pub aria_label: String,
    pub select_label: String,
    /// Select's accessible description while it waits for a current
    /// layout (it is dimmed then); `None` when it is live.
    pub select_updating: Option<String>,
}

impl LuminaApp {
    /// The header of `day` with `count` items: its date, the day's most
    /// common place from the cached layout build, and the accessible labels.
    pub(crate) fn mosaic_day_header(&self, day: u32, count: usize) -> DayHeader {
        let date = day_title(day);
        let place = self.mosaic_day_place(day).map(place_label);
        let count_text = self.count_label(count);
        let aria_label = [Some(date.clone()), place.clone(), Some(count_text.clone())]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");
        let key = if self.videos_view() {
            "mosaic.select_day_clips_label"
        } else {
            "mosaic.select_day_label"
        };
        let select_label = if count == 1 {
            crate::i18n::t_with(&format!("{key}_one"), &[&date])
        } else {
            crate::i18n::t_with(key, &[&crate::i18n::count(count), &date])
        };
        DayHeader {
            date,
            place,
            count: count_text,
            aria_label,
            select_label,
            select_updating: (!self.mosaic_layout_current())
                .then(|| crate::i18n::t("mosaic.select_updating")),
        }
    }

    /// A day header row (frame `t0zQL`): date · pin + place · count, and
    /// "Select >" on the right, all on one line of [`DAY_HEADER_TEXT`] so
    /// the row is exactly its layout height.
    fn render_day_header(
        &mut self,
        base: Div,
        day: u32,
        items: Range<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let header = self.mosaic_day_header(day, items.len());
        let line = px(DAY_HEADER_TEXT);
        base.id(SharedString::from(format!("mosaic-day-row-{day}")))
            .debug_selector(move || format!("mosaic-day-row-{day}"))
            .role(Role::Heading)
            .aria_label(SharedString::from(header.aria_label))
            .child(
                div()
                    .h(line)
                    .line_height(line)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(9.0))
                    // ponytail: the design's -0.2 tracking is skipped; GPUI
                    // 0.3.11 has no letter-spacing style.
                    .child(
                        div()
                            .debug_selector(move || format!("mosaic-day-{day}"))
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .child(header.date),
                    )
                    .children(header.place.map(|place| {
                        div()
                            .debug_selector(move || format!("mosaic-day-place-{day}"))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(render_icon(Icon::MapPin, px(11.0), theme::t2()))
                            .child(
                                div()
                                    .font_family(theme::FONT_SANS)
                                    .text_size(px(12.0))
                                    .text_color(theme::t2())
                                    .child(place),
                            )
                    }))
                    .child(
                        div()
                            .debug_selector(move || format!("mosaic-day-count-{day}"))
                            .font_family(theme::FONT_MONO)
                            .text_size(px(10.0))
                            .text_color(theme::t3())
                            .child(header.count),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .id(SharedString::from(format!("mosaic-day-select-{day}")))
                            .debug_selector(move || format!("mosaic-day-select-{day}"))
                            .role(Role::Button)
                            .aria_label(SharedString::from(header.select_label))
                            // Dimmed while the layout catches up: Select
                            // does nothing until then.
                            .map(|button| match header.select_updating {
                                Some(why) => button.opacity(0.5).aria_description(why),
                                None => button.cursor_pointer(),
                            })
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .font_family(theme::FONT_SANS)
                            .text_size(px(11.5))
                            .text_color(theme::t3())
                            .hover(|s| s.text_color(theme::t1()))
                            .child(crate::i18n::t("mosaic.select"))
                            .child(render_icon(Icon::ChevronRight, px(12.0), theme::t3()))
                            // Keep the press from starting the surface's
                            // marquee, whose still release clears the selection.
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _w, cx| {
                                this.select_mosaic_range(items.clone(), cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}

/// "Lisbon, PT", or "Lisbon" without a country code (as the inspector
/// summary joins them; there is no country-name table).
pub(crate) fn place_label(place: &lumina_core::DayPlace) -> String {
    match &place.1 {
        Some(code) => format!("{}, {code}", place.0),
        None => place.0.clone(),
    }
}

/// A day header's title: "Sat, Jun 14, 2025", or "Undated" for day 0.
fn day_title(day: u32) -> String {
    chrono::NaiveDate::from_ymd_opt((day / 10_000) as i32, day / 100 % 100, day % 100)
        .filter(|_| day != 0)
        .map_or_else(
            || crate::i18n::t("mosaic.undated"),
            |date| date.format(&crate::i18n::t("mosaic.day_date")).to_string(),
        )
}

#[cfg(test)]
mod tests {
    #[core::prelude::v1::test]
    fn day_title_formats_the_day_or_says_undated() {
        assert_eq!(super::day_title(20250614), "Sat, Jun 14, 2025");
        assert_eq!(super::day_title(0), "Undated");
        assert_eq!(super::day_title(20251399), "Undated", "an invalid day");
    }
}
