//! The Search screen's header: the token query bar, with
//! the parsed filter tokens as chips, the typed text after them, and the
//! result count. It replaces the page header while the Search screen shows.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::search::{query_bar_parts, query_has_text, remove_query_tokens, FilterChip};
use crate::app::text_input::{FieldEditor, TextTarget};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// Height of the search header (the Narrow by row fills the rest).
const SEARCH_HEADER_HEIGHT: f32 = 131.5;

/// The query bar's height; the suggestions hang 4px under it.
const QUERY_BAR_HEIGHT: f32 = 48.0;

impl LuminaApp {
    /// The decided screen rule: free text in the query,
    /// or a focused search field, shows the Search screen; filter tokens
    /// alone show the Library with its chip row. A header popover (Sort,
    /// Add filter, Downloads) hangs from the page header's buttons, so the
    /// page header comes back while one is open. A click that blurs search
    /// keeps the screen until the mouse comes up (`held_for_click`). The
    /// sort menu hangs from the results head instead while there are
    /// results to head.
    pub(crate) fn search_screen_shown(&self) -> bool {
        let sort_from_page = self.sort_menu_open && self.displayed_count() == 0;
        let popover = sort_from_page || self.filter_menu.is_some() || self.downloads.open;
        let focused = self.search_focused || self.query_bar.held_for_click;
        !popover && (focused || query_has_text(&self.search_query))
    }

    /// Whether the query's last token may still be being typed: the field
    /// is focused and the user has typed since focusing it. The one rule
    /// both the bar's display and the input's edits split the query by.
    pub(crate) fn search_typing(&self) -> bool {
        self.search_focused && self.query_bar.open_token
    }

    /// The query bar's chips and typed text for the current query.
    pub(crate) fn search_bar_parts(&self) -> (Vec<FilterChip>, String) {
        query_bar_parts(&self.search_query, self.search_typing())
    }

    /// Focuses the search field with its suggestions open. Entering it
    /// commits the query's tokens: a last filter shows as a chip (and ⌫
    /// removes it) until the user types again.
    pub(crate) fn enter_search_field(&mut self) {
        if !self.search_focused {
            self.search_focused = true;
            self.query_bar.open_token = false;
            self.scroll_query_bar_to_caret();
        }
        self.reset_search_suggestions();
    }

    /// Scrolls the token row to its end, where the typed text and caret are.
    pub(crate) fn scroll_query_bar_to_caret(&self) {
        let chips = self.search_bar_parts().0.len();
        // Two children per chip (connector, chip), then the text.
        self.query_bar.scroll.scroll_to_item(chips * 2);
    }

    /// The bar's ← selected chip, None when the selection is out of range.
    fn selected_search_chip(&self, count: usize) -> Option<usize> {
        self.query_bar.chip.filter(|&i| i < count)
    }

    /// Selects chip `chip` (None: back to the text) and scrolls to it.
    fn select_search_chip(&mut self, chip: Option<usize>, cx: &mut Context<Self>) {
        self.query_bar.chip = chip;
        match chip {
            Some(i) => self.query_bar.scroll.scroll_to_item(i * 2 + 1),
            None => self.scroll_query_bar_to_caret(),
        }
        cx.notify();
    }

    /// "{n} results" for the bar; None with no query, and while the first
    /// count loads, as the page meta line does.
    pub(crate) fn search_result_label(&self) -> Option<String> {
        let known = !self.view_loading() || self.cached_active_count().is_some();
        (known && !self.search_query.trim().is_empty())
            .then(|| crate::i18n::plural("search.results", self.displayed_count()))
    }

    /// The caret sits at the start of the bar's typed text with chips before
    /// it, or a chip is selected: ← and ⌫ then act on the chips.
    pub(crate) fn search_caret_at_chips(&self, cx: &App) -> bool {
        if !self.search_focused || !self.search_screen_shown() {
            return false;
        }
        let (chips, typed) = self.search_bar_parts();
        if chips.is_empty() {
            return false;
        }
        if self.selected_search_chip(chips.len()).is_some() || typed.is_empty() {
            return true;
        }
        matches!(
            self.field_input_for(TextTarget::Search),
            Some(FieldEditor::Line(state)) if state.read(cx).selected_range() == (0..0)
        )
    }

    /// ←, → and ⌫ on the query bar's chips (rerouted from the input by
    /// `field_input_actions`), and Esc to drop a chip selection before the
    /// Esc layers below. True when the key was used.
    pub(crate) fn search_bar_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let count = self.search_bar_parts().0.len();
        let selected = self.selected_search_chip(count);
        match key {
            "backspace" if self.search_caret_at_chips(cx) => {
                self.remove_search_chip(selected.unwrap_or(count - 1), cx);
            }
            "left" if self.search_caret_at_chips(cx) => {
                let chip = selected.map_or(count - 1, |i| i.saturating_sub(1));
                self.select_search_chip(Some(chip), cx);
            }
            "right" if selected.is_some() => {
                let chip = selected.map(|i| i + 1).filter(|&i| i < count);
                self.select_search_chip(chip, cx);
            }
            "escape" if selected.is_some() => self.select_search_chip(None, cx),
            _ => return false,
        }
        true
    }

    /// Drops the tokens behind the bar's chip `index`.
    pub(crate) fn remove_search_chip(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(chip) = self.search_bar_parts().0.into_iter().nth(index) else {
            return;
        };
        self.query_bar.chip = None;
        let query = remove_query_tokens(&self.search_query, &chip.tokens);
        self.set_search_query(&query, cx);
        self.scroll_query_bar_to_caret();
        cx.notify();
    }

    /// The search header: 131.5 high, the query bar 22 from its top.
    pub(crate) fn render_search_header(&self, cx: &mut Context<Self>) -> Div {
        div()
            .debug_selector(|| "search-header".to_string())
            .w_full()
            .h(px(SEARCH_HEADER_HEIGHT))
            .flex_shrink_0()
            .px(px(28.0))
            .pt(px(22.0))
            .bg(theme::bg_0())
            .border_b_1()
            .border_color(theme::line())
            .flex()
            .flex_col()
            .gap(px(13.0))
            // The traffic lights reach into the header past a collapsed
            // sidebar, as in the page header.
            .when(
                cfg!(target_os = "macos") && self.sidebar_collapsed,
                |header| header.pl(px(84.0 - crate::views::grid::SIDEBAR_COLLAPSED_WIDTH)),
            )
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    // The frame has no sidebar button; with the sidebar
                    // collapsed, mouse users still need a way back to it.
                    .when(self.sidebar_collapsed, |row| {
                        row.child(self.render_sidebar_toggle(cx))
                    })
                    .child(div().flex_1().min_w_0().child(self.render_query_bar(cx))),
            )
            .child(self.render_narrow_row(cx))
    }

    /// The bar, in a `relative` wrapper the suggestions hang from.
    fn render_query_bar(&self, cx: &mut Context<Self>) -> Div {
        let focused = self.search_focused;
        let (chips, typed) = self.search_bar_parts();
        let selected = self.selected_search_chip(chips.len()).filter(|_| focused);
        let input = self.field_input_for(TextTarget::Search);
        let tokens = div()
            .id("search-tokens")
            .debug_selector(|| "search-tokens".to_string())
            .track_scroll(&self.query_bar.scroll)
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_x_scroll()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(7.0))
            .children(chips.into_iter().enumerate().flat_map(|(i, chip)| {
                let connector = chip.family.connector().map(|word| {
                    div()
                        .debug_selector(move || format!("search-token-connector-{i}"))
                        .flex_shrink_0()
                        .text_size(px(13.5))
                        .text_color(theme::t3())
                        .child(word)
                        .into_any_element()
                });
                let chip = token_chip(i, chip, selected == Some(i), cx).into_any_element();
                // Two children per chip: `scroll_query_bar_to_caret` counts them.
                [connector.unwrap_or_else(|| Empty.into_any_element()), chip]
            }))
            .child(match input {
                Some(input) => div()
                    .debug_selector(|| "search-typed-input".to_string())
                    .flex_grow(1.0)
                    .flex_shrink_0()
                    .min_w(px(120.0))
                    .text_size(px(13.5))
                    .text_color(theme::t1())
                    .child(input)
                    .into_any_element(),
                None => div()
                    .debug_selector(|| "search-typed-text".to_string())
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_size(px(13.5))
                    .text_color(theme::t1())
                    .child(typed)
                    .into_any_element(),
            });
        let bar = div()
            .id("search-query-bar")
            .debug_selector(|| "search-query-bar".to_string())
            .role(Role::Search)
            .aria_label(SharedString::from(t("search.header")))
            .w_full()
            .h(px(QUERY_BAR_HEIGHT))
            .px(px(14.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(9.0))
            .bg(theme::bg_1())
            .rounded(px(11.0))
            .border_1()
            .border_color(theme::line_2())
            .cursor_text()
            // The app root blurs search on mouse down; the bar keeps it, so
            // a click here cannot flip the screen out from under itself.
            .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.enter_search_field();
                cx.notify();
            }))
            .child(render_icon(Icon::Search, px(16.0), theme::t3()).flex_none())
            .child(tokens)
            .children(self.search_result_label().map(|label| {
                div()
                    .id("search-result-count")
                    .debug_selector(|| "search-result-count".to_string())
                    .role(Role::Status)
                    .aria_label(SharedString::from(label.clone()))
                    .flex_shrink_0()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(10.5))
                    .text_color(theme::t3())
                    .child(label)
            }))
            .child(self.render_save_button(cx))
            .when(!self.search_query.is_empty(), |bar| {
                bar.child(
                    div()
                        .id("search-bar-clear")
                        .debug_selector(|| "search-bar-clear".to_string())
                        .role(Role::Button)
                        .aria_label(SharedString::from(t("search.clear")))
                        .tooltip(theme::tooltip_label(t("search.clear")))
                        .size(px(20.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::bg_3()))
                        .child(render_icon(Icon::X, px(12.0), theme::t3()))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            cx.stop_propagation();
                            this.clear_search(cx);
                        })),
                )
            });
        div()
            .debug_selector(|| "search-query-slot".to_string())
            .relative()
            .w_full()
            .child(bar)
            .children(crate::views::search_bar::SearchBarView::render_suggestions(
                self,
                px(QUERY_BAR_HEIGHT + 4.0),
                cx,
            ))
    }
}

/// One token chip: 26 high, accent wash and outline, icon, label (truncated
/// at 180 with the full value in the tooltip) and its remove button. A
/// chip ← selected shows filled, for ⌫ to remove.
fn token_chip(
    i: usize,
    chip: FilterChip,
    selected: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let label = chip.bar_label();
    let name = match chip.family.connector() {
        Some(word) => format!("{word} {label}"),
        None => label.clone(),
    };
    let remove_label = t("filter.remove").replace("{label}", &label);
    let (fill, ink) = if selected {
        (theme::accent(), theme::on_accent())
    } else {
        (theme::accent_soft(), theme::accent())
    };
    div()
        .id(("search-token", i))
        .debug_selector(move || format!("search-token-{i}"))
        .role(Role::Group)
        .aria_label(SharedString::from(name))
        .aria_selected(selected)
        .tooltip(theme::tooltip_label(label.clone()))
        .h(px(26.0))
        .flex_shrink_0()
        .px(px(9.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(theme::accent())
        .bg(fill)
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink)
        .child(render_icon(chip.icon, px(12.0), ink).flex_none())
        .child(
            div()
                .when(selected, |d| {
                    d.debug_selector(move || format!("search-token-{i}-selected"))
                })
                .max_w(px(180.0))
                .truncate()
                .child(label),
        )
        .child(
            div()
                .id(("search-token-remove", i))
                .debug_selector(move || format!("search-token-remove-{i}"))
                .role(Role::Button)
                .aria_label(SharedString::from(remove_label.clone()))
                .tooltip(theme::tooltip_label(remove_label))
                .size(px(14.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .cursor_pointer()
                .hover(|s| s.bg(theme::bg_3()))
                .child(render_icon(Icon::X, px(11.0), ink))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.remove_search_chip(i, cx);
                })),
        )
}
