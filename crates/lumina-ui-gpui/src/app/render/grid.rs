use std::cmp;

use gpui::*;
use lumina_core::{MediaType, SmartAlbum, SystemAlbum};

use crate::app::types::format_duration_badge;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::sidebar::AlbumSelection;

/// Actions offered by a grid empty state. Kept as data so the same button helper
/// can render every state without duplicating click wiring.
#[derive(Clone)]
enum EmptyAction {
    ClearSearch,
    ShowAll,
    EditSmartAlbum(SmartAlbum),
    OpenLibrary,
    ManageTags,
}

impl LuminaApp {
    pub(crate) fn render_browser_grid(
        &self,
        total_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let effective_count = total_count;

        if effective_count == 0 {
            return self.render_empty_state(cx);
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
                        mods.shift || mods.secondary() || mods.control,
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
                                let hover_group: SharedString =
                                    format!("media-card-hover-{}", item_id_num).into();

                                row_cards.push(
                                    div()
                                        .id(item_id_num)
                                        // Stable automation contract for card state:
                                        // media-card-<id>, media-card-photo-<id>,
                                        // media-card-skeleton-<id>, media-card-thumb-<id>,
                                        // media-card-selected-<id>.
                                        .debug_selector(move || {
                                            format!("media-card-{}", item_id_num)
                                        })
                                        // The hover group lets the informational "Open" hint
                                        // appear only while the card is hovered.
                                        .group(hover_group.clone())
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
                                            let s = s.bg(theme::bg_raised());
                                            if is_selected {
                                                s.border_color(theme::accent_hover())
                                            } else {
                                                s.border_color(theme::border_strong())
                                            }
                                        })
                                        // GPUI 0.2.2 has no semantic pressed state, so the press
                                        // cue is a transient surface step. Selection and keyboard
                                        // focus stay structural: accent border + check badge, not
                                        // colour alone.
                                        .active(move |s| {
                                            if is_selected {
                                                s.bg(theme::selection_bg_hover())
                                                    .border_color(theme::accent())
                                            } else {
                                                s.bg(theme::bg_control())
                                                    .border_color(theme::border_strong())
                                            }
                                        })
                                        .cursor_pointer()
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .on_click(cx.listener(
                                            move |this, event: &ClickEvent, window, cx| {
                                                // A sweep that ends over this card must not
                                                // collapse the selection it just built.
                                                if this.consume_grid_drag_click() {
                                                    return;
                                                }
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
                                            MouseButton::Left,
                                            cx.listener(
                                                move |this, event: &MouseDownEvent, window, cx| {
                                                    let mods = window.modifiers();
                                                    let additive = mods.shift
                                                        || mods.secondary()
                                                        || mods.control
                                                        || (this.selected_indices.len() > 1
                                                            && this
                                                                .selected_indices
                                                                .contains(&card_index));
                                                    this.begin_grid_drag(
                                                        event.position,
                                                        Some(card_index),
                                                        additive,
                                                    );
                                                    // Keep the press away from the grid
                                                    // background's own drag start.
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                },
                                            ),
                                        )
                                        .on_mouse_move(cx.listener(
                                            move |this, event: &MouseMoveEvent, _window, cx| {
                                                if event.pressed_button
                                                    == Some(MouseButton::Left)
                                                {
                                                    this.drag_over_card(
                                                        card_index,
                                                        event.position,
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
                                        // Inner Photo Container. The tinted gradient is
                                        // always present so a decoded thumbnail replaces the
                                        // skeleton without a black flash or a layout jump.
                                        .child(
                                            div()
                                                .debug_selector(move || {
                                                    format!("media-card-photo-{}", item_id_num)
                                                })
                                                .w_full()
                                                .flex_1()
                                                .bg(linear_gradient(
                                                    135.0,
                                                    linear_color_stop(theme::bg_raised(), 0.0),
                                                    linear_color_stop(theme::bg_app(), 1.0),
                                                ))
                                                .rounded(px(4.0))
                                                .overflow_hidden()
                                                .relative()
                                                .child(if let Some(render_img) = texture {
                                                    div()
                                                        .debug_selector(move || {
                                                            format!(
                                                                "media-card-thumb-{}",
                                                                item_id_num
                                                            )
                                                        })
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
                                                    // Static skeleton placeholder. The seed
                                                    // fixtures decode to no texture, so this is
                                                    // also the automation signal for "not yet
                                                    // decoded" (media-card-skeleton-<id>).
                                                    div()
                                                        .debug_selector(move || {
                                                            format!(
                                                                "media-card-skeleton-{}",
                                                                item_id_num
                                                            )
                                                        })
                                                        .size_full()
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(
                                                            div()
                                                                .size(px(34.0))
                                                                .rounded_full()
                                                                .bg(theme::bg_control())
                                                                .border_1()
                                                                .border_color(theme::border_subtle())
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .child(render_icon(
                                                                    if is_video {
                                                                        Icon::Film
                                                                    } else {
                                                                        Icon::Image
                                                                    },
                                                                    px(17.0),
                                                                    theme::text_subtle(),
                                                                )),
                                                        )
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
                                                })
                                                // Hover affordance: purely informational and
                                                // mouse-only in appearance, but the same action
                                                // has a keyboard path (Enter or double-click
                                                // opens the viewer).
                                                .child(
                                                    div()
                                                        .id(ElementId::named_usize(
                                                            "media-card-open-hint",
                                                            item_id_num,
                                                        ))
                                                        .absolute()
                                                        .top_1p5()
                                                        .left_1p5()
                                                        .px_1p5()
                                                        .py_0p5()
                                                        .rounded(px(4.0))
                                                        .bg(theme::scrim())
                                                        .border_1()
                                                        .border_color(theme::border_subtle())
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap_1()
                                                        .text_xs()
                                                        .text_color(theme::text())
                                                        .opacity(0.0)
                                                        .group_hover(hover_group, |s| {
                                                            s.opacity(1.0)
                                                        })
                                                        .tooltip(theme::tooltip_label(
                                                            "Open (double-click or Enter)",
                                                        ))
                                                        .child(render_icon(
                                                            Icon::Maximize,
                                                            px(10.0),
                                                            theme::text_muted(),
                                                        ))
                                                        .child("Open"),
                                                )
                                                // Selection marker: a check badge in addition
                                                // to the accent border, so focus/selection do
                                                // not depend on colour alone.
                                                .child(if is_selected {
                                                    div()
                                                        .debug_selector(move || {
                                                            format!(
                                                                "media-card-selected-{}",
                                                                item_id_num
                                                            )
                                                        })
                                                        .absolute()
                                                        .top_1p5()
                                                        .right_1p5()
                                                        .size(px(16.0))
                                                        .rounded_full()
                                                        .bg(theme::accent())
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .child(render_icon(
                                                            Icon::Check,
                                                            px(10.0),
                                                            theme::accent_fg(),
                                                        ))
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
                                                    // Tooltips are an established GPUI
                                                    // pattern here; the full filename stays
                                                    // reachable when the row is truncated.
                                                    div()
                                                        .id(ElementId::named_usize(
                                                            "media-card-name",
                                                            item_id_num,
                                                        ))
                                                        .text_xs()
                                                        .font_family(theme::FONT_MONO)
                                                        .text_color(theme::text_muted())
                                                        .truncate()
                                                        .tooltip(theme::tooltip_label(
                                                            item.file_name.clone(),
                                                        ))
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
            // Marquee rectangle, painted in window coordinates so it follows the
            // pointer regardless of which card is hovered. Card selection
            // feedback (accent border + check badge) remains the structural cue;
            // this is an additive visual on top.
            .child(self.render_grid_drag_rect())
            .into_any_element()
    }

    /// Paints the drag selection rectangle while a moved gesture is live.
    ///
    /// The canvas is non-interactive, so it never steals hover or click events
    /// from the cards underneath.
    fn render_grid_drag_rect(&self) -> AnyElement {
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
                                rgba(0x3b82f61f),
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

    /// Renders an empty view with a clear next action instead of a dead-end line.
    fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let user_album = match &self.selection {
            AlbumSelection::User(id) => self
                .user_smart_albums
                .iter()
                .find(|album| album.id == *id)
                .cloned(),
            _ => None,
        };

        let (icon, title, body, primary, secondary) = if !self.search_query.is_empty() {
            (
                Icon::Search,
                "No matches".to_string(),
                format!(
                    "Nothing matches “{}”. Try a broader search or clear it.",
                    self.search_query
                ),
                ("Clear search".to_string(), EmptyAction::ClearSearch),
                Some(("Show all photos".to_string(), EmptyAction::ShowAll)),
            )
        } else {
            match &self.selection {
                AlbumSelection::Trash => (
                    Icon::Trash,
                    "Trash is empty".to_string(),
                    "Deleted items stay here until you permanently remove them.".to_string(),
                    ("Show all photos".to_string(), EmptyAction::ShowAll),
                    None,
                ),
                AlbumSelection::System(SystemAlbum::Favorites) => (
                    Icon::Heart,
                    "No favorites yet".to_string(),
                    "Press F on any photo or video to add it here.".to_string(),
                    ("Show all photos".to_string(), EmptyAction::ShowAll),
                    None,
                ),
                AlbumSelection::User(_) => match user_album {
                    Some(album) => (
                        Icon::Sliders,
                        "Nothing matches this smart album".to_string(),
                        "Adjust its rules or show everything in the library.".to_string(),
                        ("Edit rules".to_string(), EmptyAction::EditSmartAlbum(album)),
                        Some(("Show all photos".to_string(), EmptyAction::ShowAll)),
                    ),
                    None => (
                        Icon::Sliders,
                        "Nothing matches this view".to_string(),
                        "The smart album may have been removed. Show everything instead."
                            .to_string(),
                        ("Show all photos".to_string(), EmptyAction::ShowAll),
                        None,
                    ),
                },
                AlbumSelection::Tag(tag) => (
                    Icon::Tag,
                    "No items found".to_string(),
                    format!("Nothing is tagged #{tag} yet. Try another tag or show everything."),
                    ("Show all photos".to_string(), EmptyAction::ShowAll),
                    Some(("Manage tags".to_string(), EmptyAction::ManageTags)),
                ),
                _ => {
                    if self.total_library_count == 0 {
                        (
                            Icon::Folder,
                            "Your library is empty".to_string(),
                            "Open a folder to start building your local photo and video library."
                                .to_string(),
                            ("Open Library…".to_string(), EmptyAction::OpenLibrary),
                            None,
                        )
                    } else {
                        (
                            Icon::Image,
                            "No media in this view".to_string(),
                            "Try another view or show everything in the library.".to_string(),
                            ("Show all photos".to_string(), EmptyAction::ShowAll),
                            None,
                        )
                    }
                }
            }
        };

        let primary_button =
            self.empty_action_button("empty-primary", primary.0, primary.1, true, cx);
        let secondary_button = secondary.map(|(label, action)| {
            self.empty_action_button("empty-secondary", label, action, false, cx)
        });

        div()
            .id("empty-state")
            .debug_selector(|| "empty-state".to_string())
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p_8()
            .child(
                div()
                    .w_full()
                    .max_w(px(440.0))
                    .p_6()
                    .rounded_xl()
                    .bg(theme::bg_pane())
                    .border_1()
                    .border_color(theme::border_subtle())
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(44.0))
                            .h(px(44.0))
                            .rounded_lg()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::selection_border())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(render_icon(icon, px(20.0), theme::accent())),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .child(title),
                    )
                    .child(
                        div()
                            .max_w(px(360.0))
                            .text_sm()
                            .text_color(theme::text_muted())
                            .text_center()
                            .child(body),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(primary_button)
                            .children(secondary_button),
                    ),
            )
            .into_any_element()
    }

    fn empty_action_button(
        &self,
        id: &'static str,
        label: String,
        action: EmptyAction,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = if primary {
            theme::control_primary(id)
        } else {
            theme::control(id)
        };
        button
            .debug_selector(move || id.to_string())
            .px_3()
            .child(label)
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.perform_empty_action(action.clone(), cx);
            }))
            .into_any_element()
    }

    fn perform_empty_action(&mut self, action: EmptyAction, cx: &mut Context<Self>) {
        match action {
            EmptyAction::ClearSearch => self.clear_search(cx),
            EmptyAction::ShowAll => self.select_album(AlbumSelection::AllLibrary, cx),
            EmptyAction::EditSmartAlbum(album) => self.open_album_builder_for_edit(album, cx),
            EmptyAction::OpenLibrary => self.pick_folder_dialog(cx),
            EmptyAction::ManageTags => self.open_tag_manager(cx),
        }
    }
}
