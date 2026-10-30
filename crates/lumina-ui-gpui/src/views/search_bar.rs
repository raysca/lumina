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
            .w(px(280.0))
            .h(px(28.0))
            .px_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .rounded(px(6.0))
            .border_1()
            .border_color(if focused || active {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(theme::bg_viewer())
            .cursor_text()
            .text_xs()
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
            .w(px(280.0))
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
            .rounded(px(6.0))
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
                    .rounded(px(4.0))
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

    /// Filter bar under the toolbar: view fit/square toggle and zoom slider.
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        div()
            .w_full()
            .h(px(44.0))
            .flex_shrink_0()
            .px_4()
            .bg(theme::bg_pane())
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
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
            // Zoom slider control
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        theme::control("grid-zoom-out-btn")
                            .debug_selector(|| "grid-zoom-out".to_string())
                            .tooltip(theme::tooltip_label("Smaller thumbnails (⌘-)"))
                            .w(px(22.0))
                            .h(px(22.0))
                            .px_0()
                            .justify_center()
                            .child("−")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.zoom_out_grid(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("zoom-slider-track")
                            .w(px(64.0))
                            .h(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .cursor_pointer()
                            .relative()
                            // Visual track line
                            .child(div().w_full().h(px(4.0)).rounded_full().bg(theme::border()))
                            // Thumb
                            .child({
                                let cols = this.grid_columns.clamp(2, 8);
                                let frac = ((cols - 2) as f32 / 6.0).clamp(0.0, 1.0);
                                let left_offset = (1.0 - frac) * 52.0;
                                div()
                                    .absolute()
                                    .top(px(0.0))
                                    .left(px(left_offset))
                                    .size(px(12.0))
                                    .rounded_full()
                                    .bg(theme::accent())
                            })
                            // Interactive click steps (8 cols on left -> 2 cols on right)
                            .child(div().absolute().inset_0().flex().flex_row().children(
                                (2usize..=8).rev().map(|target_cols| {
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "zoom-slider-step".into(),
                                            target_cols as u64,
                                        ))
                                        .flex_1()
                                        .h_full()
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.grid_columns = target_cols;
                                                this.save_config();
                                                cx.notify();
                                            },
                                        ))
                                }),
                            )),
                    )
                    .child(
                        theme::control("grid-zoom-in-btn")
                            .debug_selector(|| "grid-zoom-in".to_string())
                            .tooltip(theme::tooltip_label("Larger thumbnails (⌘=)"))
                            .w(px(22.0))
                            .h(px(22.0))
                            .px_0()
                            .justify_center()
                            .child("+")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.zoom_in_grid(cx);
                            })),
                    ),
            )
    }
}
