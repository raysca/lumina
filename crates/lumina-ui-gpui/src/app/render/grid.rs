use std::cmp;

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{SmartAlbum, SystemAlbum};

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::grid::{
    BAND_TO_ROW, CARD_ROW_GAP, GRID_ROW_PADDING_LEFT, GRID_ROW_PADDING_RIGHT,
};
use crate::views::sidebar::AlbumSelection;

/// Actions offered by a grid empty state. Kept as data so the same button helper
/// can render every state without duplicating click wiring.
#[derive(Clone)]
enum EmptyAction {
    ShowAll,
    EditSmartAlbum(SmartAlbum),
    OpenLibrary,
    ManageTags,
    Rescan,
    /// Drops one search term (as `tokenize_query` splits them) from the query.
    RemoveSearchTerm(usize),
}

/// What an empty view says and offers.
struct EmptyCopy {
    icon: Icon,
    title: String,
    body: String,
    /// Library folder, shown on its own mono line.
    path: Option<String>,
    primary: Option<(String, EmptyAction)>,
    secondary: Option<(String, EmptyAction)>,
}

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

        let cols = self.grid_columns.clamp(2, 8);
        let metrics = self.grid_metrics();
        let row_h = metrics.row_height;
        let photo = size(metrics.photo_side, metrics.photo_height);
        let column_gap = metrics.column_gap;
        let total_rows = effective_count.div_ceil(cols);

        let scroll_handle = self
            .grid_state
            .as_ref()
            .map(|state| state.scroll_handle.clone())
            .unwrap_or_default();

        self.grid_surface(cx)
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
                                    // The row gap sits above the cards, so the
                                    // first row's selection ring (RING_OUTSET in
                                    // card.rs, outside the tile) is never clipped by the
                                    // list's top edge.
                                    .pt(px(CARD_ROW_GAP))
                                    .pl(px(GRID_ROW_PADDING_LEFT))
                                    .pr(px(GRID_ROW_PADDING_RIGHT))
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
    pub(crate) fn render_month_header(&self) -> Option<AnyElement> {
        if self.sort.field != lumina_core::SortField::DateTaken {
            return None;
        }
        let (year, month) = self.month_header.top?;
        let count = self.month_header_count_label();
        Some(
            div()
                .id("grid-month-header")
                .debug_selector(|| "grid-month-header".to_string())
                .flex_none()
                .h(px(36.0))
                .pl(px(GRID_ROW_PADDING_LEFT))
                .pr(px(GRID_ROW_PADDING_RIGHT))
                // Text sits on the band's bottom edge; this margin plus the
                // first row's CARD_ROW_GAP make the frame's 10px to the tiles.
                .mb(px(BAND_TO_ROW - CARD_ROW_GAP))
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

    /// Renders an empty view with a clear next action instead of a dead-end line.
    pub(crate) fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let user_album = match &self.selection {
            AlbumSelection::User(id) => self
                .user_smart_albums
                .iter()
                .find(|album| album.id == *id)
                .cloned(),
            _ => None,
        };

        let search_terms = lumina_core::search::tokenize_query(&self.search_query);
        let show_all = || Some(("Show all photos".to_string(), EmptyAction::ShowAll));
        let copy = |icon, title: String, body: String| EmptyCopy {
            icon,
            title,
            body,
            path: None,
            primary: None,
            secondary: None,
        };
        // Whitespace-only queries tokenize to nothing; treat them as no search.
        let EmptyCopy {
            icon,
            title,
            body,
            path,
            primary,
            secondary,
        } = if !search_terms.is_empty() {
            // The chips are the only actions: removing one widens the search.
            copy(
                Icon::Search,
                "No matches for these filters".into(),
                "Remove a filter to widen the search:".into(),
            )
        } else {
            match &self.selection {
                AlbumSelection::Trash => copy(
                    Icon::Trash,
                    "Recently Deleted is empty".into(),
                    "Items you delete stay here until you empty the trash or restore them.".into(),
                ),
                AlbumSelection::Duplicates => copy(
                    Icon::Repeat,
                    "No Duplicates Found".into(),
                    "Lumina scanned your library and found no duplicate photos or videos.".into(),
                ),
                AlbumSelection::System(SystemAlbum::Favorites) => EmptyCopy {
                    primary: show_all(),
                    ..copy(
                        Icon::Heart,
                        "No favorites yet".into(),
                        "Press F on any photo or video to add it here.".into(),
                    )
                },
                AlbumSelection::User(_) => match user_album {
                    Some(album) => EmptyCopy {
                        primary: Some((
                            "Edit Rules…".into(),
                            EmptyAction::EditSmartAlbum(album.clone()),
                        )),
                        ..copy(
                            Icon::Sliders,
                            format!("Nothing matches \u{201c}{}\u{201d} yet", album.title),
                            crate::app::smart_draft::rule_summary(album.match_mode, &album.rules),
                        )
                    },
                    None => EmptyCopy {
                        primary: show_all(),
                        ..copy(
                            Icon::Sliders,
                            "Nothing matches this view".into(),
                            "The smart album may have been removed. Show everything instead."
                                .into(),
                        )
                    },
                },
                AlbumSelection::Album(id) => EmptyCopy {
                    primary: show_all(),
                    ..copy(
                        Icon::Image,
                        match self.album_title(*id) {
                            Some(title) => {
                                crate::i18n::t("album.empty_title").replace("{title}", title)
                            }
                            None => crate::i18n::t("album.empty_title_unknown"),
                        },
                        crate::i18n::t("album.empty_body"),
                    )
                },
                AlbumSelection::Tag(tag) => EmptyCopy {
                    primary: show_all(),
                    secondary: Some(("Manage tags".into(), EmptyAction::ManageTags)),
                    ..copy(
                        Icon::Tag,
                        "No items found".into(),
                        format!(
                            "Nothing is tagged #{tag} yet. Try another tag or show everything."
                        ),
                    )
                },
                _ => {
                    let root = self
                        .grid_state
                        .as_ref()
                        .map(|state| crate::views::preferences::tilde(state.engine.library_root()));
                    let scanning = self.grid_state.as_ref().is_some_and(|s| s.is_scanning);
                    let choose = Some(("Choose Another Folder…".into(), EmptyAction::OpenLibrary));
                    if let (0, Some(root), true) = (self.total_library_count, &root, scanning) {
                        EmptyCopy {
                            path: Some(root.clone()),
                            primary: choose,
                            ..copy(
                                Icon::Folder,
                                "Scanning…".into(),
                                "Photos and videos appear here as they are found.".into(),
                            )
                        }
                    } else if let (0, Some(root)) = (self.total_library_count, root) {
                        EmptyCopy {
                            path: Some(root),
                            primary: choose,
                            secondary: Some(("Rescan".into(), EmptyAction::Rescan)),
                            ..copy(
                                Icon::Folder,
                                "No photos or videos found".into(),
                                "Lumina looks for JPEG, HEIC, PNG, TIFF, WebP, RAW and video \
                                 files, including in subfolders."
                                    .into(),
                            )
                        }
                    } else if self.total_library_count == 0 {
                        EmptyCopy {
                            primary: Some(("Open Library…".into(), EmptyAction::OpenLibrary)),
                            ..copy(
                                Icon::Folder,
                                "Your library is empty".into(),
                                "Open a folder to start building your local photo and video \
                                 library."
                                    .into(),
                            )
                        }
                    } else {
                        EmptyCopy {
                            primary: show_all(),
                            ..copy(
                                Icon::Image,
                                "No media in this view".into(),
                                "Try another view or show everything in the library.".into(),
                            )
                        }
                    }
                }
            }
        };

        let primary_button = primary.map(|(label, action)| {
            self.empty_action_button("empty-primary", label, action, true, cx)
        });
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
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .size(px(44.0))
                            .rounded(theme::RADIUS_LG)
                            .bg(theme::bg_1())
                            .border_1()
                            .border_color(theme::line())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(render_icon(icon, px(20.0), theme::t2())),
                    )
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .text_center()
                            .child(title),
                    )
                    .child(
                        div()
                            .max_w(px(360.0))
                            .text_xs()
                            .text_color(theme::t2())
                            .text_center()
                            .child(body),
                    )
                    .children(path.map(|path| {
                        div()
                            .debug_selector(|| "empty-library-path".to_string())
                            .max_w_full()
                            .truncate()
                            .font_family(theme::FONT_MONO)
                            .text_xs()
                            .text_color(theme::t2())
                            .child(path)
                    }))
                    // Each search term is a chip; removing one widens the search.
                    .when(!search_terms.is_empty(), |col| {
                        col.child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .justify_center()
                                .gap_1p5()
                                .children(search_terms.iter().enumerate().map(|(i, term)| {
                                    div()
                                        .id(("empty-search-term", i as u64))
                                        .h(px(24.0))
                                        .px(px(10.0))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(theme::line_2())
                                        .bg(theme::bg_3())
                                        .hover(|s| s.bg(theme::bg_3()))
                                        .text_xs()
                                        .text_color(theme::t1())
                                        .cursor_pointer()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .debug_selector(move || format!("empty-search-term-{i}"))
                                        .tooltip(theme::tooltip_label(format!("Remove {term}")))
                                        .font_family(theme::FONT_MONO)
                                        .gap_1p5()
                                        .child(term.clone())
                                        .child(render_icon(Icon::X, px(10.0), theme::t1()))
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.perform_empty_action(
                                                    EmptyAction::RemoveSearchTerm(i),
                                                    cx,
                                                );
                                            },
                                        ))
                                })),
                        )
                    })
                    .child(
                        div()
                            .mt_1p5()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .children(primary_button)
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
            EmptyAction::ShowAll => self.select_album(AlbumSelection::AllLibrary, cx),
            EmptyAction::EditSmartAlbum(album) => self.open_album_builder_for_edit(album, cx),
            EmptyAction::OpenLibrary => self.pick_folder_dialog(cx),
            EmptyAction::ManageTags => self.open_tag_manager(cx),
            EmptyAction::Rescan => self.rescan_library(cx),
            EmptyAction::RemoveSearchTerm(index) => {
                let mut rest = lumina_core::search::tokenize_query(&self.search_query);
                if index < rest.len() {
                    rest.remove(index);
                }
                let query = rest.join(" ");
                self.set_search_query(&query, cx);
            }
        }
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
