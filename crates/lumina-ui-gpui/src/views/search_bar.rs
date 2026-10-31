use crate::theme;
use crate::LuminaApp;
use gpui::*;

pub struct SearchBarView;

impl SearchBarView {
    /// Search field that lives in the toolbar.
    pub fn render_field(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let active = !this.search_query.is_empty();
        let focused = this.search_focused;
        let query_text = if focused {
            format!("{}|", this.search_query)
        } else if active {
            this.search_query.clone()
        } else {
            "Search — try camera:sony iso:>3200".to_string()
        };

        let field = div()
            .id("search-input-box")
            .debug_selector(|| "search-input".to_string())
            .w(px(300.0))
            .h(px(28.0))
            .px_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .rounded(theme::RADIUS_MD)
            .border_1()
            .border_color(if focused {
                theme::focus_ring()
            } else if active {
                theme::border_strong()
            } else {
                theme::border()
            })
            .bg(theme::bg_viewer())
            .cursor_text()
            .text_size(theme::TEXT_BODY)
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.search_focused = true;
                this.reset_search_suggestions();
                cx.notify();
            }))
            .child(crate::icons::render_icon(
                crate::icons::Icon::Search,
                px(13.0),
                theme::text_subtle(),
            ))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(if focused || active {
                        theme::text()
                    } else {
                        theme::text_subtle()
                    })
                    .child(query_text),
            )
            .children(active.then(|| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_color(theme::text_muted())
                            .child(format!("{}", this.get_active_count())),
                    )
                    .child(
                        div()
                            .id("clear-search-btn")
                            .debug_selector(|| "clear-search".to_string())
                            .tooltip(theme::tooltip_label("Clear search"))
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_full()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .flex()
                            .justify_center()
                            .items_center()
                            .cursor_pointer()
                            .text_color(theme::text_muted())
                            .child(crate::icons::render_icon(
                                crate::icons::Icon::X,
                                px(10.0),
                                theme::text_muted(),
                            ))
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                cx.stop_propagation();
                                this.clear_search(cx);
                            })),
                    )
            }));

        // Relative wrapper so the suggestion dropdown can anchor under the
        // field and paint above the filter bar.
        div()
            .relative()
            .w(px(300.0))
            .child(field)
            .children(Self::render_suggestions(this, cx))
    }

    /// Compact suggestion dropdown anchored under the focused search field.
    ///
    /// The panel is deferred so it paints after the filter bar even though it is
    /// a toolbar child; the occluding card keeps pointer events off the grid
    /// beneath it.
    fn render_suggestions(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<AnyElement> {
        if !this.search_focused || this.search_suggestions.dismissed {
            return None;
        }
        let items = this.search_suggestion_items();
        if items.is_empty() {
            return None;
        }
        let selected = this.search_suggestions.selected;

        let panel = div()
            .id("search-suggestions-panel")
            .debug_selector(|| "search-suggestions".to_string())
            .absolute()
            .top(px(30.0))
            .right(px(0.0))
            .w(px(320.0))
            .p(px(4.0))
            .bg(theme::bg_pane())
            .border_1()
            .border_color(theme::border())
            .rounded(theme::RADIUS_LG)
            .shadow_2xl()
            .occlude()
            .flex()
            .flex_col()
            .gap(px(1.0))
            .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                // Keep the field focused and the dropdown mounted while the
                // pointer is inside the panel.
                cx.stop_propagation();
            })
            .children(items.into_iter().enumerate().map(|(index, suggestion)| {
                let active = selected == Some(index);
                let row = div()
                    .id(("search-suggestion-row", index))
                    .debug_selector(move || format!("search-suggestion-{index}"))
                    .h(px(24.0))
                    .px_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .rounded(theme::RADIUS_SM)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_xs()
                    .cursor_pointer();
                let row = if active {
                    row.bg(theme::selection_bg())
                        .text_color(theme::selection_fg())
                        .hover(|style| style.bg(theme::selection_bg_hover()))
                } else {
                    row.text_color(theme::text())
                        .hover(|style| style.bg(theme::bg_control_hover()))
                };
                row.child(suggestion).on_click(cx.listener(
                    move |this, _event: &ClickEvent, _window, cx| {
                        cx.stop_propagation();
                        this.apply_search_suggestion_at(index, cx);
                    },
                ))
            }));

        Some(gpui::deferred(panel).with_priority(1).into_any_element())
    }

    /// Filter bar above the grid: filter chips, the fit/square toggle and
    /// the thumbnail size slider.
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        div()
            .debug_selector(|| "filter-bar".to_string())
            .w_full()
            .h(px(44.0))
            .flex_shrink_0()
            .px_4()
            .bg(rgb(0x1A1B1E))
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .children(GridFilter::ALL.into_iter().map(|filter| {
                theme::segmented_item(
                    ElementId::Name(filter.selector().into()),
                    this.grid_filter == filter,
                )
                .debug_selector(move || filter.selector().to_string())
                .child(filter.label())
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.grid_filter = filter;
                    cx.notify();
                }))
            }))
            .child(div().flex_1())
            // [ Fit | Square ] segmented toggle
            .child(
                theme::segmented("fit-square-toggle")
                    .child(
                        theme::segmented_item("fit-mode-fit", this.card_fit_mode)
                            .debug_selector(|| "fit-mode-fit".to_string())
                            .child("Fit")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.card_fit_mode = true;
                                this.save_config();
                                cx.notify();
                            })),
                    )
                    .child(
                        theme::segmented_item("fit-mode-square", !this.card_fit_mode)
                            .debug_selector(|| "fit-mode-square".to_string())
                            .child("Square")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.card_fit_mode = false;
                                this.save_config();
                                cx.notify();
                            })),
                    ),
            )
            .child(render_size_slider(this, cx))
    }
}

/// Grid filter chips. A filter dims non-matching items instead of hiding
/// them, so positions and selection stay put.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridFilter {
    #[default]
    All,
    Favorites,
    Videos,
    ThreeStars,
    Labeled,
}

impl GridFilter {
    pub const ALL: [GridFilter; 5] = [
        GridFilter::All,
        GridFilter::Favorites,
        GridFilter::Videos,
        GridFilter::ThreeStars,
        GridFilter::Labeled,
    ];

    fn label(self) -> &'static str {
        match self {
            GridFilter::All => "All",
            GridFilter::Favorites => "Favorites",
            GridFilter::Videos => "Videos",
            GridFilter::ThreeStars => "★ 3+",
            GridFilter::Labeled => "Labeled",
        }
    }

    fn selector(self) -> &'static str {
        match self {
            GridFilter::All => "filter-chip-all",
            GridFilter::Favorites => "filter-chip-favorites",
            GridFilter::Videos => "filter-chip-videos",
            GridFilter::ThreeStars => "filter-chip-3stars",
            GridFilter::Labeled => "filter-chip-labeled",
        }
    }

    /// Whether an item with these facts stays at full opacity.
    pub fn matches(self, favorite: bool, video: bool, rating: u8, labeled: bool) -> bool {
        match self {
            GridFilter::All => true,
            GridFilter::Favorites => favorite,
            GridFilter::Videos => video,
            GridFilter::ThreeStars => rating >= 3,
            GridFilter::Labeled => labeled,
        }
    }
}

/// Thumbnail size 1–5, smallest to largest, as grid columns.
pub(crate) const SIZE_COLUMNS: [usize; 5] = [8, 6, 4, 3, 2];
const SLIDER_WIDTH: f32 = 80.0;
const THUMB: f32 = 12.0;

/// Slider level (0-based) closest to a column count from ⌘-/⌘=.
pub(crate) fn size_level(columns: usize) -> usize {
    SIZE_COLUMNS
        .iter()
        .position(|&c| c <= columns.clamp(2, 8))
        .unwrap_or(SIZE_COLUMNS.len() - 1)
}

fn render_size_slider(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let level = size_level(this.grid_columns);
    let travel = SLIDER_WIDTH - THUMB;
    let thumb_left = travel * level as f32 / (SIZE_COLUMNS.len() - 1) as f32;
    // Click or drag anywhere on the track: the pointer's x picks the level.
    // The painted x is read at event time: a value captured at render would
    // be stale after a layout change (e.g. toggling the inspector).
    let set_from_x = move |this: &mut LuminaApp, x: f32, cx: &mut Context<LuminaApp>| {
        let frac = ((x - this.size_slider_left - THUMB / 2.0) / travel).clamp(0.0, 1.0);
        let level = (frac * (SIZE_COLUMNS.len() - 1) as f32).round() as usize;
        if this.grid_columns != SIZE_COLUMNS[level] {
            this.grid_columns = SIZE_COLUMNS[level];
            this.save_config();
            cx.notify();
        }
    };
    div()
        .id("thumb-size-slider")
        .debug_selector(|| "thumb-size-slider".to_string())
        .tooltip(theme::tooltip_label("Thumbnail size (⌘- / ⌘=)"))
        .w(px(SLIDER_WIDTH))
        .h(px(THUMB))
        .relative()
        .flex()
        .items_center()
        .cursor_pointer()
        .child(div().w_full().h(px(3.0)).rounded_full().bg(theme::border()))
        .child(
            div()
                .absolute()
                .top_0()
                .left(px(thumb_left))
                .size(px(THUMB))
                .rounded_full()
                .bg(theme::text()),
        )
        .child(
            canvas(
                {
                    let app = cx.entity().downgrade();
                    move |b, _w, cx| {
                        let left = f32::from(b.origin.x);
                        app.update(cx, |this, _| this.size_slider_left = left).ok();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                cx.stop_propagation();
                this.size_slider_dragging = true;
                set_from_x(this, f32::from(e.position.x), cx);
            }),
        )
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _w, cx| {
            if e.pressed_button == Some(MouseButton::Left) && this.size_slider_dragging {
                set_from_x(this, f32::from(e.position.x), cx);
            }
        }))
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, _cx| this.size_slider_dragging = false),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, _cx| this.size_slider_dragging = false),
        )
}
