//! The Library's filter chip row and the Add filter menu
//! that both its "Add filter" chip and the header's Filter button open.

use gpui::prelude::FluentBuilder;
use gpui::*;

use lumina_core::StructuredQuery;

use crate::app::search::{
    filter_chips, filter_label, query_has_filters, query_tokens_closed, remove_query_tokens,
    FilterChip,
};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme::{self, ChipKind};
use crate::views::page_header::popover_under;

/// The open Add filter menu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterMenu {
    /// The keyboard highlight; ↓ from none picks the first row.
    pub highlight: Option<usize>,
    /// Opened from the row's "Add filter" chip (else the header button),
    /// so the card hangs under the control that opened it.
    pub from_chip: bool,
}

/// One Add filter row.
#[derive(Clone)]
enum MenuRow {
    /// A filter in the query, checked: choosing it removes its chip. The
    /// keyboard path for chips no toggle row covers (place, camera, date).
    Remove(FilterChip),
    /// Toggles this quick-filter token, as the palette's quick filters do.
    Toggle(&'static str),
    /// Starts this field's token in the search field for the value to be
    /// typed, with the field's own suggestions. (prefix, label key, icon)
    Field(&'static str, &'static str, Icon),
}

/// The quick-filter families of `docs/search.md`, then the typed fields.
const FIXED_ROWS: &[MenuRow] = &[
    MenuRow::Toggle("is:video"),
    MenuRow::Toggle("is:photo"),
    MenuRow::Toggle("is:raw"),
    MenuRow::Toggle("is:favorite"),
    MenuRow::Toggle("is:pick"),
    MenuRow::Toggle("rating:>=3"),
    MenuRow::Toggle("is:unrated"),
    MenuRow::Toggle("has:location"),
    MenuRow::Field("camera:", "filter.camera", Icon::Camera),
    MenuRow::Field("city:", "filter.place", Icon::MapPin),
    MenuRow::Field("date:", "filter.date", Icon::Calendar),
];

/// The menu for `query`: its chips that no toggle row stands for, then the
/// fixed rows.
fn menu_rows(query: &str) -> Vec<MenuRow> {
    let toggles: Vec<StructuredQuery> = FIXED_ROWS
        .iter()
        .filter_map(|row| match row {
            MenuRow::Toggle(token) => Some(StructuredQuery::parse(token)),
            _ => None,
        })
        .collect();
    let tokens = lumina_core::search::tokenize_query(query);
    filter_chips(query)
        .into_iter()
        .filter(|chip| {
            !matches!(chip.tokens[..], [i] if toggles.contains(&StructuredQuery::parse(&tokens[i])))
        })
        .map(MenuRow::Remove)
        .chain(FIXED_ROWS.iter().cloned())
        .collect()
}

impl MenuRow {
    fn label_and_icon(&self) -> (String, Icon) {
        match self {
            MenuRow::Remove(chip) => (chip.label.clone(), chip.icon),
            MenuRow::Toggle(token) => match filter_label(token) {
                Some((label, icon, _)) => (label, icon),
                // A static row the label table does not know still gets a row;
                // showing the raw token beats panicking the render path.
                None => (token.to_string(), Icon::Sliders),
            },
            MenuRow::Field(_, key, icon) => (t(key), *icon),
        }
    }
}

impl LuminaApp {
    /// Opens the Add filter menu, or closes it when it is already open.
    pub(crate) fn toggle_filter_menu(&mut self, from_chip: bool, cx: &mut Context<Self>) {
        if self.filter_menu.is_some() {
            self.close_filter_menu(cx);
        } else {
            self.open_filter_menu(from_chip, cx);
        }
    }

    pub(crate) fn open_filter_menu(&mut self, from_chip: bool, cx: &mut Context<Self>) {
        // Keys go to the menu, not to the search field behind it.
        self.blur_search(cx);
        self.sort_menu_open = false;
        self.filter_menu = Some(FilterMenu {
            highlight: None,
            from_chip,
        });
        cx.notify();
    }

    pub(crate) fn close_filter_menu(&mut self, cx: &mut Context<Self>) {
        if self.filter_menu.take().is_some() {
            cx.notify();
        }
    }

    /// ↑ / ↓ in the menu, clamped to its rows.
    pub(crate) fn move_filter_menu_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let last = menu_rows(&self.search_query).len() as isize - 1;
        let Some(menu) = self.filter_menu.as_mut() else {
            return;
        };
        menu.highlight = Some(match menu.highlight {
            None if delta > 0 => 0,
            None => last as usize,
            Some(i) => (i as isize + delta).clamp(0, last) as usize,
        });
        cx.notify();
    }

    /// ↵: runs the highlighted row.
    pub(crate) fn activate_filter_menu_highlight(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.filter_menu.as_ref().and_then(|menu| menu.highlight) {
            self.choose_filter_menu_row(index, cx);
        }
    }

    /// Runs row `index` and closes the menu. The query (and so the rows)
    /// cannot change while the menu is open: it takes every key.
    fn choose_filter_menu_row(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(row) = menu_rows(&self.search_query).into_iter().nth(index) else {
            return;
        };
        self.filter_menu = None;
        match row {
            MenuRow::Remove(chip) => self.remove_filter_chip(&chip.tokens, cx),
            MenuRow::Toggle(token) => self.toggle_quick_filter(token, cx),
            MenuRow::Field(prefix, ..) => {
                let mut tokens = query_tokens_closed(&self.search_query);
                tokens.push(prefix.to_string());
                self.set_search_query(&tokens.join(" "), cx);
                self.focus_search_field(cx);
            }
        }
    }

    /// Drops the tokens behind one chip.
    fn remove_filter_chip(&mut self, tokens: &[usize], cx: &mut Context<Self>) {
        let query = remove_query_tokens(&self.search_query, tokens);
        self.set_search_query(&query, cx);
    }

    /// "{n} matches" for the row: the latest fetched count, kept while the
    /// next one loads so the row does not jump. None before any fetch.
    pub(crate) fn filter_match_label(&self) -> Option<String> {
        self.last_fetched_count()
            .map(|n| crate::i18n::plural("filter.matches", n))
    }

    /// Whether the header's Filter button carries the menu: opened there,
    /// or opened from a chip row that has since gone.
    pub(crate) fn filter_menu_under_header(&self) -> bool {
        self.filter_menu
            .as_ref()
            .is_some_and(|menu| !menu.from_chip || !query_has_filters(&self.search_query))
    }

    /// The chip row: one chip per filter token, the Add filter chip, then
    /// the match count and Clear all. None without filter tokens.
    pub(crate) fn render_filter_bar(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        // On the Search screen the query bar shows the chips.
        if self.search_screen_shown() {
            return None;
        }
        if self.videos_view() {
            return Some(self.render_videos_filter_bar(cx));
        }
        if !query_has_filters(&self.search_query) {
            return None;
        }
        let menu_here = self.filter_menu.is_some() && !self.filter_menu_under_header();
        let chips = filter_chips(&self.search_query);
        let add = theme::chip("filter-add", ChipKind::Ghost)
            .debug_selector(|| "filter-add".to_string())
            .role(Role::Button)
            .aria_label(SharedString::from(t("filter.add")))
            .flex_shrink_0()
            .relative()
            .border_color(theme::line_2())
            .text_color(theme::t3())
            .cursor_pointer()
            .hover(|s| s.bg(theme::bg_2()))
            .child(render_icon(Icon::Plus, px(13.0), theme::t3()))
            .child(t("filter.add"))
            .when(menu_here, |chip| {
                chip.child(popover_under(Anchor::TopLeft, filter_menu_card(self, cx)))
            })
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.toggle_filter_menu(true, cx);
            }));
        Some(
            div()
                .id("filter-bar")
                .debug_selector(|| "filter-bar".to_string())
                .role(Role::Toolbar)
                .aria_label(SharedString::from(t("header.filter")))
                .w_full()
                .h(px(51.5))
                .flex_shrink_0()
                .px(px(28.0))
                .border_b_1()
                .border_color(theme::line())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(
                    // Scrolls sideways rather than wrapping in a narrow window.
                    div()
                        .id("filter-chips")
                        .debug_selector(|| "filter-chips".to_string())
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_x_scroll()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .children(chips.into_iter().enumerate().map(|(i, chip)| {
                            let remove_label = t("filter.remove").replace("{label}", &chip.label);
                            let tokens = chip.tokens;
                            theme::chip(("filter-chip", i), ChipKind::Normal)
                                .debug_selector(move || format!("filter-chip-{i}"))
                                .role(Role::Group)
                                .aria_label(SharedString::from(chip.label.clone()))
                                .flex_shrink_0()
                                .child(render_icon(chip.icon, px(13.0), theme::t2()))
                                .child(chip.label)
                                .child(
                                    div()
                                        .id(("filter-chip-remove", i))
                                        .debug_selector(move || format!("filter-chip-remove-{i}"))
                                        .role(Role::Button)
                                        .aria_label(SharedString::from(remove_label.clone()))
                                        .tooltip(theme::tooltip_label(remove_label))
                                        .size(px(16.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme::bg_3()))
                                        .child(render_icon(Icon::X, px(11.0), theme::t3()))
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.remove_filter_chip(&tokens, cx);
                                            },
                                        )),
                                )
                        }))
                        .child(add),
                )
                .children(self.filter_match_label().map(|label| {
                    div()
                        .id("filter-match-count")
                        .debug_selector(|| "filter-match-count".to_string())
                        .role(Role::Status)
                        .aria_label(SharedString::from(label.clone()))
                        .flex_shrink_0()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(10.5))
                        .text_color(theme::t3())
                        .child(label)
                }))
                .child(
                    div()
                        .id("filter-clear-all")
                        .debug_selector(|| "filter-clear-all".to_string())
                        .role(Role::Button)
                        .aria_label(SharedString::from(t("filter.clear_all")))
                        .tooltip(theme::tooltip_label(t("command.filter_clear")))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .text_size(px(11.5))
                        .text_color(theme::accent())
                        .hover(|s| s.text_color(theme::accent_hover()))
                        .child(t("filter.clear_all"))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.clear_quick_filters(cx);
                        })),
                ),
        )
    }
}

/// The dismiss backdrop behind the menu card, which paints above it.
pub fn render_filter_menu_overlay(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if app.filter_menu.is_none() {
        return Empty.into_any_element();
    }
    div()
        .absolute()
        .inset_0()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.close_filter_menu(cx);
            }),
        )
        .into_any_element()
}

/// The Add filter card, 220 wide: filters in the query show checked, and
/// choosing a row closes the menu.
pub fn filter_menu_card(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let highlight = app.filter_menu.as_ref().and_then(|menu| menu.highlight);
    let active: Vec<StructuredQuery> = lumina_core::search::tokenize_query(&app.search_query)
        .iter()
        .map(|token| StructuredQuery::parse(token))
        .collect();
    div()
        .id("filter-add-menu")
        .debug_selector(|| "filter-add-menu".to_string())
        .role(Role::Menu)
        .aria_label(SharedString::from(t("filter.add")))
        .w(px(220.0))
        .p(px(6.0))
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line())
        .rounded(theme::RADIUS_LG)
        .shadow_2xl()
        .occlude()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .children(
            menu_rows(&app.search_query)
                .into_iter()
                .enumerate()
                .map(|(i, row)| {
                    let (label, icon) = row.label_and_icon();
                    // None for a row that is not a toggle (the typed fields).
                    let checked = match &row {
                        MenuRow::Remove(_) => Some(true),
                        MenuRow::Toggle(token) => {
                            Some(active.contains(&StructuredQuery::parse(token)))
                        }
                        MenuRow::Field(..) => None,
                    };
                    let on = checked == Some(true);
                    div()
                        .id(("filter-menu-row", i))
                        .debug_selector(move || format!("filter-menu-row-{i}"))
                        .role(if checked.is_some() {
                            Role::MenuItemCheckBox
                        } else {
                            Role::MenuItem
                        })
                        .when_some(checked, |row, on| {
                            row.aria_toggled(if on { Toggled::True } else { Toggled::False })
                        })
                        .aria_label(SharedString::from(label.clone()))
                        .h(px(28.0))
                        .px(px(8.0))
                        .rounded(theme::RADIUS_MD)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .text_color(theme::t1())
                        .when(highlight == Some(i), |row| row.bg(theme::bg_3()))
                        .hover(|s| s.bg(theme::bg_3()))
                        .child(render_icon(icon, px(13.0), theme::t2()))
                        .child(div().flex_1().child(label))
                        .when(on, |row| {
                            row.child(render_icon(Icon::Check, px(13.0), theme::accent()))
                        })
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.choose_filter_menu_row(i, cx);
                        }))
                }),
        )
}
#[cfg(test)]
mod tests {
    use super::{filter_label, MenuRow, FIXED_ROWS};

    #[test]
    fn every_fixed_menu_row_has_a_label() {
        for row in FIXED_ROWS {
            if let MenuRow::Toggle(token) = row {
                assert!(
                    filter_label(token).is_some(),
                    "FIXED_ROWS has {token} but filter_label does not know it"
                );
            }
        }
    }
}
