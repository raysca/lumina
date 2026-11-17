use std::cmp;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::grid::{ALBUM_PADDING_TOP, BAND_TO_ROW, GRID_ROW_PADDING_LEFT};
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    pub(crate) fn render_browser_grid(
        &self,
        total_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let effective_count = total_count;

        if effective_count == 0 {
            // Not "No results" before the first fetch for this view lands.
            if self.view_loading() {
                return div().flex_1().into_any_element();
            }
            return self.render_empty_state(cx);
        }

        let cols = self.grid_cols();
        let metrics = self.grid_metrics();
        let row_h = metrics.row_height;
        let photo = size(metrics.photo_side, metrics.photo_height);
        let column_gap = metrics.column_gap;
        let (row_gap, padding_right) = (metrics.row_gap, metrics.padding_right);
        let total_rows = effective_count.div_ceil(cols);

        let scroll_handle = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone())
            .unwrap_or_default();

        self.grid_surface(cx)
            // An open album's first row sits ALBUM_PADDING_TOP below the
            // grid's top edge: this plus the row's own gap above the tiles.
            .when(self.album_tiles(), |surface| {
                surface.pt(px(ALBUM_PADDING_TOP - row_gap))
            })
            // ponytail: the band shows the month recorded by the previous
            // frame's processor; the notify below settles it one frame later.
            .children(if self.search_screen_shown() {
                Some(self.render_results_head(cx))
            } else {
                self.render_month_header()
            })
            .child(
                uniform_list(
                    "media-grid",
                    total_rows,
                    cx.processor(move |this, range: std::ops::Range<usize>, _window, cx| {
                        let mut rows = Vec::with_capacity(range.len());

                        // Prefetch 2 rows ahead for smooth 120 FPS scrolling
                        let prefetch_start = range.end;
                        let prefetch_end = cmp::min(prefetch_start + 2, total_rows);
                        this.prefetch_rows(prefetch_start..prefetch_end);

                        // uniform_list also calls this with row 0 on every
                        // layout, only to measure a row. That call must not move
                        // the fetch window or the month band, or a deep scroll
                        // swaps both between row 0 and the viewport each frame
                        // and never stops redrawing.
                        // A real pass can also be 0..1 (a viewport one row
                        // tall, scrolled to the top), so it only counts as
                        // measuring when the list is scrolled away from row 0.
                        let measuring =
                            range == (0..1) && total_rows > 1 && this.grid_top_row() != 0;

                        // Fetch all visible items for this frame in a single SQLite query
                        let visible_start = range.start * cols;
                        let visible_end = cmp::min(range.end * cols, effective_count);
                        let visible_items =
                            if visible_start >= visible_end || this.grid_state.is_none() {
                                Vec::new()
                            } else {
                                if !measuring {
                                    this.want_view_rows(visible_start..visible_end, cx);
                                }
                                this.displayed_rows(visible_start..visible_end)
                            };
                        // The month band reads the top row from this fetch, so
                        // it costs no extra query.
                        let top = visible_items.first().map(|i| (i.year, i.month));
                        if !measuring && this.month_header.top != top {
                            this.month_header.top = top;
                            cx.spawn(async move |this, cx| {
                                let _ = this.update(cx, |_app, cx| {
                                    cx.notify();
                                });
                            })
                            .detach();
                        }

                        for row_ix in range {
                            let start = row_ix * cols;
                            let end = cmp::min(start + cols, effective_count);
                            let slice_start = start.saturating_sub(visible_start);
                            let slice_end =
                                end.saturating_sub(visible_start).min(visible_items.len());
                            let items = if slice_start < slice_end {
                                &visible_items[slice_start..slice_end]
                            } else {
                                &[][..]
                            };

                            let (row_textures, missing_items) =
                                if let Some(ref state) = this.grid_state {
                                    let mut cache = state.texture_cache.lock();
                                    let mut texs = Vec::with_capacity(items.len());
                                    let mut missing = Vec::new();
                                    for item in items {
                                        if let Some(tex) = cache.get(&item.id).cloned() {
                                            texs.push(Some(tex));
                                        } else {
                                            texs.push(None);
                                            missing.push(item);
                                        }
                                    }
                                    drop(cache);
                                    (texs, missing)
                                } else {
                                    (vec![None; items.len()], Vec::new())
                                };

                            for missing_item in missing_items {
                                this.queue_thumbnail_request_item(missing_item);
                            }

                            let mut row_cards = Vec::with_capacity(cols);
                            for (offset_in_row, item) in items.iter().enumerate() {
                                let texture = row_textures.get(offset_in_row).cloned().flatten();
                                row_cards.push(this.render_media_card(
                                    item,
                                    start + offset_in_row,
                                    photo,
                                    texture,
                                    cx,
                                ));
                            }

                            while row_cards.len() < cols {
                                row_cards.push(div().flex_1().into_any_element());
                            }

                            rows.push(
                                div()
                                    .id(row_ix)
                                    .w_full()
                                    .h(px(row_h))
                                    // The row gap sits above the cards, so the first
                                    // row's square-tile selection ring (RING_OUTSET in
                                    // card.rs, outside the tile) is never clipped by the
                                    // list's top edge. Captioned tiles have no outset
                                    // ring (their outline is on the thumb).
                                    .pt(px(row_gap))
                                    .pl(px(GRID_ROW_PADDING_LEFT))
                                    .pr(px(padding_right))
                                    .flex()
                                    .flex_row()
                                    .gap(px(column_gap))
                                    // Cards keep their own height; a captioned
                                    // row's spare bottom is its row gap.
                                    .items_start()
                                    .children(row_cards),
                            );
                        }

                        rows
                    }),
                )
                .track_scroll(&scroll_handle)
                .debug_selector(|| "media-grid".to_string())
                .flex_1()
                .min_h_0(),
            )
            // Marquee rectangle, painted in window coordinates so it follows the
            // pointer regardless of which card is hovered. Card selection
            // feedback (the accent border) remains the structural cue;
            // this is an additive visual on top.
            .child(self.render_grid_drag_rect())
            .into_any_element()
    }

    /// The browser body's container, shared by Grid and Mosaic: marquee
    /// press/move/release, ⌘-wheel and pinch zoom, and a column for its
    /// children.
    pub(crate) fn grid_surface(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("media-grid-surface")
            .debug_selector(|| "media-grid-surface".to_string())
            .flex_1()
            .h_full()
            .overflow_hidden()
            .relative()
            .pt_2()
            // A press on the grid background starts a marquee drag instead of
            // clearing immediately, so a sweep can build a selection. A still
            // press still clears (see finish_grid_drag).
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    let mods = window.modifiers();
                    this.begin_grid_drag(
                        event.position,
                        None,
                        mods.shift || mods.secondary(cx) || mods.control,
                    );
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                this.grid_pointer_moved(event.position, event.pressed_button, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                    this.finish_grid_drag(cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                    this.finish_grid_drag(cx);
                }),
            )
            // Cmd/Ctrl + wheel changes column density around the pointer. A
            // plain wheel is left alone so the uniform list keeps scrolling.
            // The handler runs in the bubble phase after the list's own scroll
            // listener, and restores the offset it would have scrolled.
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                if this.zoom_grid_with_wheel(event, window, cx) {
                    cx.stop_propagation();
                }
            }))
            // Pinch walks the same density ladder at the pinch centre.
            .on_pinch(cx.listener(|this, event: &PinchEvent, _window, cx| {
                this.zoom_grid_with_pinch(event, cx);
                cx.stop_propagation();
            }))
            .flex()
            .flex_col()
    }

    /// V3Grid section header, as a band pinned above the date-sorted grid
    /// showing the top row's month. ponytail: one sticky band, not a header
    /// per month (that needs a variable-height list).
    /// An open album's Grid has no band.
    pub(crate) fn render_month_header(&self) -> Option<AnyElement> {
        if self.sort.field != lumina_core::SortField::DateTaken || self.album_tiles() {
            return None;
        }
        let (year, month) = self.month_header.top?;
        let count = self.month_header_count_label();
        let metrics = self.grid_metrics();
        Some(
            div()
                .id("grid-month-header")
                .debug_selector(|| "grid-month-header".to_string())
                .flex_none()
                .h(px(36.0))
                .pl(px(GRID_ROW_PADDING_LEFT))
                .pr(px(metrics.padding_right))
                // Text sits on the band's bottom edge; this margin plus the
                // first row's gap make the frame's 10px to the tiles.
                .mb(px(BAND_TO_ROW - metrics.row_gap))
                .flex()
                .flex_col()
                .justify_end()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(9.0))
                        // ponytail: the design's -0.2 tracking is skipped; GPUI
                        // 0.3.11 has no letter-spacing style.
                        .child(
                            div()
                                .font_family(theme::FONT_DISPLAY)
                                .text_size(px(15.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::t1())
                                .child(month_title(year, month)),
                        )
                        .children(count.map(|label| {
                            div()
                                .font_family(theme::FONT_MONO)
                                .text_size(px(10.0))
                                .text_color(theme::t3())
                                .child(label)
                        })),
                )
                .into_any_element(),
        )
    }

    /// The band's count text: "N items", or "N clips" in Videos.
    pub(crate) fn month_header_count_label(&self) -> Option<String> {
        self.month_header_count().map(|n| self.count_label(n))
    }

    /// Item count for the band's month. The cached month counts cover the
    /// whole library, so only the unfiltered view shows them; Videos reads
    /// its own per-month counts from the view totals fetch.
    pub(crate) fn month_header_count(&self) -> Option<usize> {
        let top = self.month_header.top?;
        if self.videos_view() {
            return self.displayed_totals()?.months.get(&top).copied();
        }
        if self.selection != AlbumSelection::AllLibrary || !self.search_query.is_empty() {
            return None;
        }
        self.month_header.counts.get(&top).copied()
    }

    /// Paints the drag selection rectangle while a moved gesture is live.
    ///
    /// The canvas is non-interactive, so it never steals hover or click events
    /// from the cards underneath.
    pub(crate) fn render_grid_drag_rect(&self) -> AnyElement {
        let (Some(start), Some(current)) = (self.grid_drag.start, self.grid_drag.current) else {
            return Empty.into_any_element();
        };
        if !self.grid_drag.active || !self.grid_drag.moved {
            return Empty.into_any_element();
        }
        div()
            .id("grid-selection-rect")
            .debug_selector(|| "grid-selection-rect".to_string())
            .absolute()
            .inset_0()
            .child(
                canvas(
                    move |_bounds, _window, _cx| (),
                    move |_bounds, _prepaint, window, _cx| {
                        let top_left =
                            point(cmp::min(start.x, current.x), cmp::min(start.y, current.y));
                        let bottom_right =
                            point(cmp::max(start.x, current.x), cmp::max(start.y, current.y));
                        window.paint_quad(
                            fill(
                                Bounds::from_corners(top_left, bottom_right),
                                theme::accent_soft(),
                            )
                            .border_widths(px(1.0))
                            .border_color(theme::accent()),
                        );
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}

/// "June 2022"; items without a date (year 0) read "Undated".
fn month_title(year: i32, month: u32) -> String {
    if year > 0 {
        format!("{} {year}", crate::app::types::month_name(month))
    } else {
        "Undated".to_string()
    }
}

#[cfg(test)]
mod month_title_tests {
    #[core::prelude::v1::test]
    fn month_title_names_the_month_or_says_undated() {
        assert_eq!(super::month_title(2022, 6), "June 2022");
        assert_eq!(super::month_title(0, 0), "Undated");
    }
}
