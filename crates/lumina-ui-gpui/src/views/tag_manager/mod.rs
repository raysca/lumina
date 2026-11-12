//! The Tags page (frame `L3F8Cs`): the Keywords tab with the filter field
//! and sort menu, then the keyword list on the left and the focused
//! keyword's details on the right, in the main column under the page header
//! while `AlbumSelection::Tags` shows. State lives in `app/tags.rs`, keys in
//! `app/keymap/tags_page.rs`.

pub mod detail;
pub mod list;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::text_input::TextTarget;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::{page_tabs, popover_under, text_marker, PageTab};
use crate::{LuminaApp, TagSortOrder};

/// The header meta: "2 keywords · 67% of library tagged". Reads the counts
/// the counts refresh caches (`library_tag_counts`, `untagged_count`,
/// `total_library_count`); the keyword count alone until those land.
pub(crate) fn meta(app: &LuminaApp) -> String {
    let keywords = app.library_tag_counts.len();
    let total = app.total_library_count;
    if keywords > 0 && total == 0 {
        return crate::i18n::keywords(keywords);
    }
    let tagged = total.saturating_sub(app.untagged_count);
    // Never 0% or 100% unless it is exactly that.
    let percent = match (tagged, total) {
        (0, _) => 0,
        (t, n) if t >= n => 100,
        (t, n) => ((t * 100 + n / 2) / n).clamp(1, 99),
    };
    t("tags.meta")
        .replace("{keywords}", &crate::i18n::keywords(keywords))
        .replace("{percent}", &percent.to_string())
}

/// The page body under the header: the tab row, then list and detail.
pub(crate) fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let rows = app.tag_manager_rows();
    let tabs = [PageTab {
        id: "keywords",
        label: t("tags.tab_keywords").into(),
        count: app.library_tag_counts.len(),
        active: true,
    }];
    div()
        .id("tags-page")
        .debug_selector(|| "tags-page".to_string())
        .relative()
        .size_full()
        .bg(theme::bg_0())
        .flex()
        .flex_col()
        .child(
            // One tab, so a click has nothing to switch.
            page_tabs(t("tags.title"), &tabs, false, |_, _, _| {})
                .child(div().flex_1())
                .child(render_search(app, cx))
                .child(render_sort(app, cx)),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_row()
                .child(list::render_list(app, &rows, cx))
                .child(detail::render_detail(app, cx)),
        )
        // The open menu's dismiss backdrop; the menu paints above it.
        .when(app.tag_manager.sort_menu.is_some(), |page| {
            page.child(div().absolute().inset_0().occlude().on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.tag_manager.sort_menu = None;
                    cx.notify();
                }),
            ))
        })
        .into_any_element()
}

/// The filter: 200×32, bg-1, 8px radius, `search` 13 and "Filter tags".
/// Typing filters while the page shows (unless the sidebar search has
/// focus); the focus border shows the field is the keyboard target.
fn render_search(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let search = &app.tag_manager.search;
    let placeholder: SharedString = t("tags.filter_placeholder").into();
    div()
        .id("tag-manager-search")
        .debug_selector(|| "tag-manager-search".to_string())
        .w(px(200.0))
        .h(px(32.0))
        .flex_shrink_0()
        .px(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .rounded(theme::RADIUS_LG)
        .border_1()
        .border_color(if app.tag_manager.search_focused && !app.search_focused {
            theme::focus_ring()
        } else {
            theme::line()
        })
        .bg(theme::bg_1())
        .cursor_text()
        .text_size(px(12.0))
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.tag_manager.search_focused = true;
            this.search_focused = false;
            cx.notify();
        }))
        .child(render_icon(Icon::Search, px(13.0), theme::t3()))
        .when(search.is_empty(), |field| {
            field.children(text_marker("placeholder", &placeholder))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(if search.is_empty() {
                    theme::t3()
                } else {
                    theme::t1()
                })
                .child(match app.field_input_for(TextTarget::TagManagerSearch) {
                    Some(input) => input.into_any_element(),
                    None if search.is_empty() => placeholder.into_any_element(),
                    None => search.clone().into_any_element(),
                }),
        )
}

/// The text sort menu "Most used ▾" (Inter 11.5, t2) and its popover.
fn render_sort(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let label: SharedString = format!("{} ▾", app.tag_manager.sort.label()).into();
    div()
        .id("tag-sort-menu")
        .debug_selector(|| "tag-sort-menu".to_string())
        .role(Role::Button)
        .aria_label(t("tags.sort_menu"))
        .relative()
        .flex_shrink_0()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(theme::RADIUS_SM)
        .cursor_pointer()
        .hover(|s| s.bg(theme::bg_2()))
        .text_size(px(11.5))
        .text_color(theme::t2())
        .children(text_marker("tag-sort", &label))
        .child(label)
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.toggle_tag_sort_menu(cx);
        }))
        .children(
            app.tag_manager
                .sort_menu
                .map(|highlight| popover_under(Anchor::TopRight, sort_menu(app, highlight, cx))),
        )
}

fn sort_menu(app: &LuminaApp, highlight: usize, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    theme::menu("tag-sort-menu-card")
        .debug_selector(|| "tag-sort-menu-card".to_string())
        .role(Role::Menu)
        .aria_label(t("tags.sort_menu"))
        .w(px(160.0))
        .occlude()
        .children(TagSortOrder::ALL.iter().enumerate().map(|(i, &order)| {
            let selector = match order {
                TagSortOrder::CountDesc => "tag-sort-most-used",
                TagSortOrder::Alphabetical => "tag-sort-az",
            };
            let current = app.tag_manager.sort == order;
            theme::menu_item(selector, i == highlight)
                .debug_selector(move || selector.to_string())
                .role(Role::MenuItemRadio)
                .aria_label(order.label())
                .aria_selected(current)
                .child(order.label())
                .child(if current { "✓" } else { "" })
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_tag_sort_order(order, cx);
                }))
        }))
}
