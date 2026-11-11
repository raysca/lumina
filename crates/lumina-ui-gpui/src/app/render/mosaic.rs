//! The Mosaic body: a variable-height `list` of the cached layout's rows
//! (day headers and justified tile rows). A frame reads only the cached
//! layout (`app/mosaic_layout.rs`) and the active view's cached rows.

use std::sync::Arc;

use gpui::*;

use crate::app::LuminaApp;
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
        self.refresh_mosaic_layout(cx);
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
            .pr(px(GRID_ROW_PADDING_RIGHT));
        match row.kind {
            // ponytail: the date only; place, count and Select come with
            // plan 01b Task 5.
            RowKind::Header { day, .. } => base
                .child(
                    div()
                        .debug_selector(move || format!("mosaic-day-{day}"))
                        .h(px(DAY_HEADER_TEXT))
                        // Pinned so the paint matches the layout's row height.
                        .line_height(px(DAY_HEADER_TEXT))
                        .font_family(theme::FONT_DISPLAY)
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t1())
                        .child(day_title(day)),
                )
                .into_any_element(),
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
