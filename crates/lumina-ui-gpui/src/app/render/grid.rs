use std::cmp;

use gpui::*;
use lumina_core::MediaType;

use crate::app::types::format_duration_badge;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

impl LuminaApp {
    pub(crate) fn render_browser_grid(
        &self,
        total_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let effective_count = total_count;

        if effective_count == 0 {
            let empty_msg = if !self.search_query.is_empty() {
                format!("No matches for \"{}\"", self.search_query)
            } else {
                match &self.selection {
                    crate::views::sidebar::AlbumSelection::Trash => {
                        "Deleted items stay here until you empty the Trash".to_string()
                    }
                    crate::views::sidebar::AlbumSelection::System(
                        lumina_core::SystemAlbum::Favorites,
                    ) => "No favorites yet — press F on any photo to favorite".to_string(),
                    crate::views::sidebar::AlbumSelection::User(_) => {
                        "Nothing matches this album yet — edit rules".to_string()
                    }
                    crate::views::sidebar::AlbumSelection::Tag(t) => {
                        format!("No items tagged with #{t}")
                    }
                    _ => "No photos or videos found in this view".to_string(),
                }
            };
            return div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::text_subtle())
                        .child(empty_msg),
                )
                .into_any_element();
        }

        let cols = self.grid_columns.clamp(2, 8);
        let row_h = self.row_height();
        let total_rows = effective_count.div_ceil(cols);

        let scroll_handle = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone())
            .unwrap_or_default();

        div()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .pt_2()
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

                        // Fetch all visible items for this frame in a single SQLite query
                        let visible_start = range.start * cols;
                        let visible_end = cmp::min(range.end * cols, effective_count);
                        let visible_items =
                            if visible_start >= visible_end || this.grid_state.is_none() {
                                Vec::new()
                            } else {
                                this.get_active_slice(visible_start..visible_end)
                            };

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

                            let mut row_cards = Vec::with_capacity(cols);
                            for (offset_in_row, item) in items.iter().enumerate() {
                                let card_index = start + offset_in_row;
                                let item_id_num = item.id.0 as usize;
                                let is_selected = this.is_index_selected(card_index)
                                    || this
                                        .selected_item
                                        .as_ref()
                                        .map(|i| i.id == item.id)
                                        .unwrap_or(false);
                                let is_fav = item.is_favorite;
                                let item_rating = this.get_item_rating(item);
                                let item_color = this.get_item_color_label(item);
                                let is_video = item.media_type == MediaType::Video;

                                let texture = if let Some(ref state) = this.grid_state {
                                    let mut cache = state.texture_cache.lock();
                                    if let Some(tex) = cache.get(&item.id).cloned() {
                                        Some(tex)
                                    } else {
                                        drop(cache);
                                        this.queue_thumbnail_request(item.id);
                                        None
                                    }
                                } else {
                                    None
                                };

                                let item_for_click = item.clone();
                                let item_for_context = item.clone();
                                let dur_text = format_duration_badge(item.duration_secs());
                                let fit_mode = this.card_fit_mode;

                                row_cards.push(
                                    div()
                                        .id(item_id_num)
                                        .flex_1()
                                        .h(px(row_h - 10.0))
                                        .p(px(8.0))
                                        .bg(theme::bg_pane())
                                        .rounded(px(8.0))
                                        .border_1()
                                        .border_color(if is_selected {
                                            theme::accent()
                                        } else {
                                            theme::border_subtle()
                                        })
                                        .hover(move |s| {
                                            if is_selected {
                                                s.border_color(theme::accent())
                                            } else {
                                                s.border_color(theme::border_strong())
                                            }
                                        })
                                        .cursor_pointer()
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .on_click(cx.listener(
                                            move |this, event: &ClickEvent, window, cx| {
                                                let mods = window.modifiers();
                                                let is_shift = mods.shift;
                                                let is_cmd = mods.secondary() || mods.control;
                                                if !is_shift && !is_cmd && event.click_count() >= 2
                                                {
                                                    this.open_viewer(item_for_click.clone(), cx);
                                                } else {
                                                    this.handle_card_click(
                                                        item_for_click.clone(),
                                                        card_index,
                                                        is_shift,
                                                        is_cmd,
                                                        cx,
                                                    );
                                                }
                                            },
                                        ))
                                        .on_mouse_down(
                                            MouseButton::Right,
                                            cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                                                this.open_card_context_menu(
                                                    item_for_context.clone(),
                                                    card_index,
                                                    e.position,
                                                    cx,
                                                );
                                            }),
                                        )
                                        // Inner Photo Container
                                        .child(
                                            div()
                                                .w_full()
                                                .flex_1()
                                                .bg(theme::bg_app())
                                                .rounded(px(4.0))
                                                .overflow_hidden()
                                                .relative()
                                                .child(if let Some(render_img) = texture {
                                                    div()
                                                        .size_full()
                                                        .child(
                                                            img(render_img).size_full().object_fit(
                                                                if fit_mode {
                                                                    ObjectFit::Contain
                                                                } else {
                                                                    ObjectFit::Cover
                                                                },
                                                            ),
                                                        )
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .size_full()
                                                        .bg(theme::bg_app())
                                                        .into_any_element()
                                                })
                                                // Favorite Heart Overlay (bottom-left)
                                                .child(if is_fav {
                                                    div()
                                                        .absolute()
                                                        .bottom_1p5()
                                                        .left_1p5()
                                                        .text_color(rgb(0xffffff))
                                                        .child(render_icon(
                                                            Icon::HeartFilled,
                                                            px(12.0),
                                                            rgb(0xffffff),
                                                        ))
                                                        .into_any_element()
                                                } else {
                                                    Empty.into_any_element()
                                                })
                                                // Video Duration Overlay (bottom-right)
                                                .child(if is_video && !dur_text.is_empty() {
                                                    div()
                                                        .absolute()
                                                        .bottom_1p5()
                                                        .right_1p5()
                                                        .px_1()
                                                        .py_0p5()
                                                        .rounded_sm()
                                                        .bg(rgba(0x000000d0))
                                                        .text_xs()
                                                        .font_family(theme::FONT_MONO)
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap(px(3.0))
                                                        .text_color(rgb(0xffffff))
                                                        .child(render_icon(
                                                            Icon::Play,
                                                            px(9.0),
                                                            rgb(0xffffff),
                                                        ))
                                                        .child(dur_text)
                                                        .into_any_element()
                                                } else {
                                                    Empty.into_any_element()
                                                }),
                                        )
                                        // Info Area Below Photo
                                        .child(
                                            div()
                                                .pt_1p5()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_family(theme::FONT_MONO)
                                                        .text_color(theme::text_muted())
                                                        .truncate()
                                                        .child(item.file_name.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .h(px(12.0))
                                                        .flex()
                                                        .flex_row()
                                                        .justify_between()
                                                        .items_center()
                                                        .child(if item_rating > 0 {
                                                            div()
                                                                .flex()
                                                                .flex_row()
                                                                .items_center()
                                                                .gap(px(1.0))
                                                                .children((0..item_rating).map(
                                                                    |_| {
                                                                        render_icon(
                                                                            Icon::StarFilled,
                                                                            px(9.0),
                                                                            theme::rating(),
                                                                        )
                                                                    },
                                                                ))
                                                                .into_any_element()
                                                        } else {
                                                            Empty.into_any_element()
                                                        })
                                                        .child(if let Some(c) = item_color {
                                                            div()
                                                                .size(px(6.0))
                                                                .rounded_full()
                                                                .bg(c)
                                                                .into_any_element()
                                                        } else {
                                                            Empty.into_any_element()
                                                        }),
                                                ),
                                        )
                                        .into_any_element(),
                                );
                            }

                            while row_cards.len() < cols {
                                row_cards
                                    .push(div().flex_1().h(px(row_h - 10.0)).into_any_element());
                            }

                            rows.push(
                                div()
                                    .id(row_ix)
                                    .w_full()
                                    .h(px(row_h))
                                    .px_4()
                                    .flex()
                                    .flex_row()
                                    .gap(px(10.0))
                                    .children(row_cards),
                            );
                        }

                        rows
                    }),
                )
                .track_scroll(scroll_handle)
                .h_full(),
            )
            .into_any_element()
    }
}
