use crate::theme;
use crate::LuminaApp;
use gpui::prelude::FluentBuilder;
use gpui::*;

pub struct SearchBarView;

impl SearchBarView {
    /// Search field in the sidebar's search slot: idle on
    /// `bg_0` with a `line` outline, focused on `bg_3` with an accent one.
    pub fn render_field(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let active = !this.search_query.is_empty();
        let focused = this.search_focused;
        // One search input: the query bar holds it while the Search screen
        // shows (always, once focused), and this field mirrors the query.
        let in_bar = this.search_screen_shown();
        let input = (!in_bar)
            .then(|| this.field_input_for(crate::app::text_input::TextTarget::Search))
            .flatten();

        let field = div()
            .id("search-input-box")
            .debug_selector(|| "search-input".to_string())
            // With the input inside, it carries the role; one field, not two.
            .when(input.is_none(), |d| {
                d.role(Role::TextInput)
                    .aria_label(crate::i18n::t("search.placeholder"))
            })
            .w_full()
            .h(px(34.0))
            .px(px(10.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .rounded(theme::RADIUS_LG)
            .border_1()
            .border_color(if focused {
                theme::accent()
            } else {
                theme::line()
            })
            .bg(if focused {
                theme::bg_3()
            } else {
                theme::bg_0()
            })
            .cursor_text()
            .text_size(theme::TEXT_SMALL)
            // Keeps the app root from blurring search under the click (the
            // Search screen would shrink the header between down and up).
            .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.enter_search_field();
                cx.notify();
            }))
            .child(
                crate::icons::render_icon(crate::icons::Icon::Search, theme::ICON_SM, theme::t3())
                    .flex_none(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(theme::t1())
                    .child(match input {
                        Some(input) => input.into_any_element(),
                        None if active => this.search_query.clone().into_any_element(),
                        None => div()
                            .debug_selector(|| "search-placeholder".to_string())
                            .truncate()
                            .text_color(theme::t3())
                            .child(crate::i18n::t("search.placeholder"))
                            .into_any_element(),
                    }),
            )
            .when(!active, |field| {
                field.child(
                    div()
                        .debug_selector(|| "search-shortcut".to_string())
                        .flex_none()
                        .font_family(theme::FONT_MONO)
                        .text_size(theme::TEXT_CAPTION)
                        .text_color(theme::t3())
                        .child(crate::i18n::t("search.shortcut")),
                )
            })
            .children(active.then(|| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .font_family(theme::FONT_SANS)
                            .font_features(theme::tabular_figures())
                            .text_size(theme::TEXT_CAPTION)
                            .text_color(theme::t3())
                            .child(crate::i18n::count(this.displayed_count())),
                    )
                    .child(
                        div()
                            .id("clear-search-btn")
                            .debug_selector(|| "clear-search".to_string())
                            .tooltip(theme::tooltip_label(crate::i18n::t("search.clear")))
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_full()
                            .bg(theme::bg_2())
                            .hover(|s| s.bg(theme::bg_3()))
                            .flex()
                            .justify_center()
                            .items_center()
                            .cursor_pointer()
                            .text_color(theme::t2())
                            .child(crate::icons::render_icon(
                                crate::icons::Icon::X,
                                theme::ICON_XS,
                                theme::t2(),
                            ))
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                cx.stop_propagation();
                                this.clear_search(cx);
                            })),
                    )
            }));

        // Relative wrapper so the suggestion dropdown can anchor under the
        // field and paint above the sidebar rows.
        div()
            .debug_selector(|| "sidebar-search-slot".to_string())
            .relative()
            .flex_none()
            .w_full()
            .child(field)
            .children(
                (!in_bar)
                    .then(|| Self::render_suggestions(this, px(38.0), cx))
                    .flatten(),
            )
    }

    /// Compact suggestion dropdown anchored `top` below the top of the
    /// focused field's `relative` wrapper: the sidebar field, or the query
    /// bar while the Search screen shows.
    ///
    /// The panel is deferred so it paints over the rows that follow the
    /// field; the occluding card keeps pointer events off them.
    pub(crate) fn render_suggestions(
        this: &LuminaApp,
        top: Pixels,
        cx: &mut Context<LuminaApp>,
    ) -> Option<AnyElement> {
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
            .top(top)
            .left_0()
            .right_0()
            .p(px(4.0))
            .bg(theme::bg_1())
            .border_1()
            .border_color(theme::line())
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
                    .text_size(theme::TEXT_SMALL)
                    .cursor_pointer();
                let row = if active {
                    row.bg(theme::bg_3()).text_color(theme::t1())
                } else {
                    // A lighter step than the keyboard row, so Enter's target stays clear.
                    row.text_color(theme::t1())
                        .hover(|style| style.bg(theme::bg_2()))
                };
                let row = row.gap_1p5().children(recent[index].then(|| {
                    // ponytail: no clock icon in the set; rotate-ccw reads as history.
                    div()
                        .debug_selector(move || format!("search-suggestion-recent-{index}"))
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::RotateCcw,
                            theme::ICON_XS,
                            theme::t2(),
                        ))
                }));
                // A long entry ends in an ellipsis in the narrow sidebar.
                row.child(div().flex_1().min_w_0().truncate().child(suggestion))
                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                        cx.stop_propagation();
                        this.apply_search_suggestion_at(index, cx);
                    }))
            }))
            .children(explanation.map(|text| {
                div()
                    .id("search-explanation")
                    .debug_selector(|| "search-explanation".to_string())
                    .role(Role::Status)
                    .aria_label(text.clone())
                    .px_2()
                    .py_1()
                    .text_size(theme::TEXT_SMALL)
                    .text_color(theme::t2())
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
