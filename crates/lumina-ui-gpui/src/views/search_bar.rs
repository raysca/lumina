use crate::theme;
use crate::LuminaApp;
use gpui::*;

pub struct SearchBarView;

impl SearchBarView {
    /// Search field that lives in the toolbar.
    pub fn render_field(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let active = !this.search_query.is_empty();
        let focused = this.search_focused;
        let input = this.field_input_for(crate::app::text_input::TextTarget::Search);
        let query_text = if active {
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
                    .child(match input {
                        Some(input) => input.into_any_element(),
                        None => query_text.into_any_element(),
                    }),
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
                            .child(format!("{}", this.displayed_count())),
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
        if !this.search_focused {
            return None;
        }
        // Escape hides the rows but keeps the explanation while focused.
        let items = if this.search_suggestions.dismissed {
            Vec::new()
        } else {
            this.search_suggestion_items()
        };
        let explanation = this.search_explanation();
        if items.is_empty() && explanation.is_none() {
            return None;
        }
        let selected = this.search_suggestions.selected;
        let recent: Vec<bool> = items.iter().map(|s| this.is_recent_search(s)).collect();

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
                let row = row.gap_1p5().children(recent[index].then(|| {
                    // ponytail: no clock icon in the set; rotate-ccw reads as history.
                    div()
                        .debug_selector(move || format!("search-suggestion-recent-{index}"))
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::RotateCcw,
                            px(11.0),
                            theme::text_subtle(),
                        ))
                }));
                row.child(suggestion).on_click(cx.listener(
                    move |this, _event: &ClickEvent, _window, cx| {
                        cx.stop_propagation();
                        this.apply_search_suggestion_at(index, cx);
                    },
                ))
            }))
            .children(explanation.map(|text| {
                div()
                    .id("search-explanation")
                    .debug_selector(|| "search-explanation".to_string())
                    .role(Role::Status)
                    .aria_label(text.clone())
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(text)
            }));

        Some(gpui::deferred(panel).with_priority(1).into_any_element())
    }
}

/// Thumbnail size 1–5, smallest to largest, as grid columns.
pub(crate) const SIZE_COLUMNS: [usize; 5] = [8, 6, 4, 3, 2];

/// The ladder column count for a saved one: the old 7-step wheel could
/// save 5 or 7, which the ladder does not have.
pub(crate) fn ladder_columns(columns: usize) -> usize {
    SIZE_COLUMNS[size_level(columns)]
}

/// Ladder level (0-based) closest to a column count.
pub(crate) fn size_level(columns: usize) -> usize {
    SIZE_COLUMNS
        .iter()
        .position(|&c| c <= columns.clamp(2, 8))
        .unwrap_or(SIZE_COLUMNS.len() - 1)
}
