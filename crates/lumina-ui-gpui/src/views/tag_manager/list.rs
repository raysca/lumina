//! Left column of the Tags page: the keyword table (TAG / USAGE / `…`),
//! its row menu, and the bulk bar.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::keywords::KeywordRow;
use crate::app::types::TagRowAction;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::{popover_under, text_marker};
use crate::views::sidebar::tag_color;
use crate::LuminaApp;

/// What the virtualised rows read, built once per frame from cached state.
struct Table {
    rows: Vec<KeywordRow>,
    /// Aliases per keyword, from `keywords.aliases`.
    aliases: HashMap<String, usize>,
    max_count: usize,
    usage: Usage,
}

/// How much of the usage column fits beside a name of at least
/// [`MIN_NAME`] (Review Focus: the track goes first, then the count).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Usage {
    Track,
    Count,
    Hidden,
}

impl Usage {
    /// From the list column's width (the centre pane less the detail panel),
    /// not a per-row measurement, so every row and the header agree. The
    /// deepest shown indent counts, so the column never moves between rows.
    fn fit(list_width: f32, max_depth: usize) -> Usage {
        // Border, the table's 28 + 24 padding and the row's 8 + 8.
        let inner = list_width - 1.0 - 52.0 - 16.0;
        // Checkbox, dot and indent, act button, and the gaps between them.
        let base = 16.0 + 9.0 + indent(max_depth) + ACT_WIDTH + 3.0 * GAP;
        let name = |usage: f32| inner - base - usage - GAP;
        if name(USAGE_WIDTH) >= MIN_NAME {
            Usage::Track
        } else if name(USAGE_WIDTH - USAGE_BAR - GAP) >= MIN_NAME {
            Usage::Count
        } else {
            Usage::Hidden
        }
    }

    fn width(self) -> Option<f32> {
        match self {
            Usage::Track => Some(USAGE_WIDTH),
            Usage::Count => Some(USAGE_WIDTH - USAGE_BAR - GAP),
            Usage::Hidden => None,
        }
    }
}

fn indent(depth: usize) -> f32 {
    INDENT * depth.min(MAX_INDENT_LEVELS) as f32
}

/// The keyword table. `list_width` is the column's width this frame.
pub fn render_list(
    app: &LuminaApp,
    rows: Vec<KeywordRow>,
    list_width: f32,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let mut aliases: HashMap<String, usize> = HashMap::new();
    for (_, keyword) in &app.keywords.aliases {
        *aliases.entry(keyword.clone()).or_default() += 1;
    }
    let max_depth = rows.iter().map(|row| row.depth).max().unwrap_or(0);
    let table = Rc::new(Table {
        max_count: rows.iter().filter_map(|row| row.count).max().unwrap_or(0),
        usage: Usage::fit(list_width, max_depth),
        aliases,
        rows,
    });
    let rows = &table.rows;
    let selected = &app.tag_manager.selected;
    let shown_checked = rows
        .iter()
        .filter(|row| selected.contains(&row.path))
        .count();
    let all_state = match shown_checked {
        0 => Check::Off,
        n if n == rows.len() => Check::On,
        _ => Check::Mixed,
    };

    div()
        .id("tag-manager-list")
        .debug_selector(|| "tag-manager-list".to_string())
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .pt(px(14.0))
        .pr(px(24.0))
        .pl(px(28.0))
        .border_r_1()
        .border_color(theme::line())
        // Column header with select-all. Inter 9.5/600 t3; the design's
        // 0.9 letter spacing has no GPUI equivalent.
        .child(
            div()
                .h(px(29.5))
                .flex_shrink_0()
                .px(px(8.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(GAP))
                .border_b_1()
                .border_color(theme::line())
                .text_size(theme::TEXT_CAPTION)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t3())
                .child(
                    checkbox("tag-manager-select-all", all_state)
                        .debug_selector(|| "tag-manager-select-all".to_string())
                        .tooltip(theme::tooltip_label(if all_state == Check::On {
                            "Deselect all shown (⌘A)"
                        } else {
                            "Select all shown (⌘A)"
                        }))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.toggle_all_tag_manager_rows(cx);
                        })),
                )
                .child(div().flex_1().child(t("tags.col_tag")))
                .when_some(table.usage.width(), |header, width| {
                    header.child(div().w(px(width)).child(t("tags.col_usage")))
                })
                .child(div().w(px(ACT_WIDTH))),
        )
        .map(|col| {
            if rows.is_empty() {
                col.child(
                    div()
                        .p_4()
                        .text_size(theme::TEXT_SMALL)
                        .text_color(theme::t2())
                        .child(if app.library_tag_counts.is_empty() {
                            "No tags yet. Add tags from the inspector."
                        } else {
                            "No tags match the filter."
                        }),
                )
            } else {
                // Virtualised: thousands of keywords build only the rows on
                // screen, all ROW_HEIGHT tall. The rows come from cached
                // counts and aliases, never a query.
                col.child(
                    uniform_list("tag-manager-rows", rows.len(), {
                        let table = table.clone();
                        cx.processor(move |this, range: std::ops::Range<usize>, _w, cx| {
                            range
                                .filter_map(|i| {
                                    Some(render_row(this, &table, table.rows.get(i)?, cx))
                                })
                                .collect()
                        })
                    })
                    .track_scroll(&app.tag_manager.list_scroll)
                    .flex_1()
                    .min_h_0(),
                )
            }
        })
        // The bar is for acting on several tags; one tag uses the detail.
        .when(selected.len() >= 2, |col| {
            col.child(super::bulk_bar::render_bulk_bar(app, cx))
        })
}

/// Row height; `uniform_list` needs every row the same.
const ROW_HEIGHT: f32 = 44.0;
/// Indent per tree level, up to MAX_INDENT_LEVELS so a deep path keeps the
/// usage column at its width (the name truncates instead).
const INDENT: f32 = 16.0;
const MAX_INDENT_LEVELS: usize = 4;
/// Usage column: a 190 px track scaled to the most used shown tag, then the
/// count. The `…` button column after it.
const USAGE_BAR: f32 = 190.0;
const USAGE_WIDTH: f32 = 280.0;
const ACT_WIDTH: f32 = 32.0;
const GAP: f32 = 14.0;
/// The narrowest name the usage column leaves (Review Focus: ~160 px).
const MIN_NAME: f32 = 160.0;
/// Fixed selectors under the row menu contain `//`, which no keyword path
/// has (empty segments are dropped), so a tag-derived selector such as
/// `tag-row-menu-<tag>` can never equal one of them.
const MENU_CARD: &str = "tag-row-menu//card";

fn render_row(
    app: &LuminaApp,
    table: &Table,
    row: &KeywordRow,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let max_count = table.max_count;
    let alias_count = table.aliases.get(&row.path).copied().unwrap_or(0);
    let tag = row.path.as_str();
    // An ancestor with no items of its own shows no count; the detail loads
    // its subtree count when it is focused.
    let count = row.count.unwrap_or(0);
    let depth = row.depth;
    let path_selector = format!("tag-row-{tag}");
    let indent_selector = format!("tag-row-indent-{depth}");
    let focused = app.tag_manager.focused.as_deref() == Some(tag);
    let checked = app.tag_manager.selected.contains(tag);
    let (row_tag, check_tag, menu_tag) = (tag.to_string(), tag.to_string(), tag.to_string());
    let row_selector = format!("tag-manager-row-{tag}");
    let focus_selector = format!("tag-row-focus-{tag}");
    let check_selector = format!("tag-manager-checkbox-{tag}");
    let usage_selector = format!("tag-manager-usage-{tag}");
    let fill_selector = format!("tag-manager-usage-fill-{tag}");
    let aliases_selector = format!("tag-row-aliases-{tag}");
    let name_selector = format!("tag-manager-name-{tag}");
    let count_selector = format!("tag-manager-count-{tag}");
    let menu_selector = format!("tag-row-menu-{tag}");
    let menu_label = t("tags.row_menu").replace("{tag}", tag);
    let menu_highlight = app
        .tag_manager
        .row_menu
        .as_ref()
        .filter(|(menu_tag, _)| menu_tag == tag)
        .map(|(_, highlight)| *highlight);
    let fill = if max_count == 0 {
        0.0
    } else {
        USAGE_BAR * count as f32 / max_count as f32
    };
    let aliases = match alias_count {
        0 => None,
        1 => Some(t("tags.aliases_one")),
        n => Some(t("tags.aliases_other").replace("{n}", &n.to_string())),
    };

    div()
        .id(ElementId::Name(format!("modal-tag-row-{tag}").into()))
        .debug_selector(move || row_selector.clone())
        .relative()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(GAP))
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .map(|r| {
            if focused {
                r.bg(theme::bg_2())
            } else {
                r.hover(|s| s.bg(theme::bg_2()))
            }
        })
        .when(focused, |r| {
            r.child(
                div()
                    .debug_selector(move || focus_selector.clone())
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.0))
                    .bg(theme::accent()),
            )
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.focus_tag_manager_tag(row_tag.clone(), cx);
        }))
        .child(
            checkbox(
                ElementId::Name(format!("modal-tag-cb-{tag}").into()),
                if checked { Check::On } else { Check::Off },
            )
            .debug_selector(move || check_selector.clone())
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                // The row would otherwise also take the click and move focus.
                cx.stop_propagation();
                this.toggle_tag_manager_selection(check_tag.clone(), cx);
            })),
        )
        // The indent and the dot share one flex slot: no gap at depth 0.
        .child(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .child(
                    div()
                        .debug_selector(move || indent_selector.clone())
                        .w(px(indent(depth))),
                )
                .child(div().size(px(9.0)).rounded_full().bg(tag_color(tag))),
        )
        .child(
            div()
                .id(ElementId::Name(format!("tag-row-name-{tag}").into()))
                .debug_selector(move || name_selector.clone())
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                // The row may show only the leaf, truncated: the full path.
                .tooltip(theme::tooltip_label(tag.to_string()))
                .child(
                    div()
                        .debug_selector(move || path_selector.clone())
                        .min_w_0()
                        .truncate()
                        .text_size(theme::TEXT_BODY)
                        .text_color(if row.count.is_none() {
                            theme::t2()
                        } else {
                            theme::t1()
                        })
                        .when(focused, |name| name.font_weight(FontWeight::MEDIUM))
                        .child(row.label().to_string()),
                )
                .when_some(aliases, |name, aliases| {
                    let marker: SharedString = format!("{tag} {aliases}").into();
                    name.child(
                        div()
                            .debug_selector(move || aliases_selector.clone())
                            .flex_none()
                            .text_size(theme::TEXT_CAPTION)
                            .text_color(theme::t3())
                            .children(text_marker("tag-row-aliases", &marker))
                            .child(aliases),
                    )
                }),
        )
        .when_some(table.usage.width(), |r, width| {
            r.child(
                div()
                    .w(px(width))
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(GAP))
                    .when(table.usage == Usage::Track, |usage| {
                        usage.child(
                            div()
                                .id(ElementId::Name(format!("tag-usage-{tag}").into()))
                                .debug_selector(move || usage_selector.clone())
                                .w(px(USAGE_BAR))
                                .h(px(6.0))
                                .flex_none()
                                .rounded_full()
                                .bg(theme::bg_3())
                                .child(
                                    div()
                                        .id(ElementId::Name(format!("tag-usage-fill-{tag}").into()))
                                        .debug_selector(move || fill_selector.clone())
                                        .w(px(fill))
                                        .h_full()
                                        .rounded_full()
                                        .bg(if focused {
                                            theme::accent()
                                        } else {
                                            theme::chart_grey()
                                        }),
                                ),
                        )
                    })
                    .child(
                        div()
                            .debug_selector(move || count_selector.clone())
                            .flex_1()
                            .flex()
                            .justify_end()
                            .font_family(theme::FONT_SANS)
                            .font_features(theme::tabular_figures())
                            .text_size(theme::TEXT_CAPTION)
                            .text_color(theme::t2())
                            .child(row.count.map_or(String::new(), |n| n.to_string())),
                    ),
            )
        })
        .child(
            div()
                .id(ElementId::Name(menu_selector.clone().into()))
                .debug_selector(move || menu_selector.clone())
                .role(Role::Button)
                .aria_label(menu_label.clone())
                .tooltip(theme::tooltip_label(format!("{menu_label} (⇧F10)")))
                .relative()
                .w(px(ACT_WIDTH))
                .h(px(28.0))
                .flex_none()
                .rounded(theme::RADIUS_MD)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(theme::bg_3()))
                .child(render_icon(Icon::Ellipsis, px(14.0), theme::t3()))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.open_tag_row_menu(menu_tag.clone(), cx);
                }))
                .children(menu_highlight.map(|highlight| {
                    popover_under(Anchor::TopRight, row_menu(tag, highlight, cx))
                })),
        )
        .into_any_element()
}

/// The row `…` menu: Rename, Move to…, Delete, with their page keys.
fn row_menu(tag: &str, highlight: usize, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let marker: SharedString = tag.to_string().into();
    theme::menu(MENU_CARD)
        .debug_selector(|| MENU_CARD.to_string())
        .role(Role::Menu)
        .aria_label(t("tags.row_menu").replace("{tag}", tag))
        .w(px(180.0))
        .occlude()
        .children(text_marker(MENU_CARD, &marker))
        .children(TagRowAction::ALL.iter().enumerate().map(|(i, &action)| {
            let selector = format!("tag-row-menu//{}", action.id());
            let active = i == highlight;
            let tag = tag.to_string();
            theme::menu_item(ElementId::Name(selector.clone().into()), active)
                .debug_selector(move || selector.clone())
                .role(Role::MenuItem)
                .aria_label(action.label())
                .when(action == TagRowAction::Delete && !active, |item| {
                    item.text_color(theme::danger())
                })
                .child(action.label())
                .child(
                    div()
                        .text_color(if active {
                            theme::accent_hint()
                        } else {
                            theme::t3()
                        })
                        .child(action.shortcut()),
                )
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.run_tag_row_action(tag.clone(), action, cx);
                }))
        }))
}

#[derive(Clone, Copy, PartialEq)]
enum Check {
    Off,
    On,
    /// Some of the shown rows are checked ("–").
    Mixed,
}

fn checkbox(id: impl Into<ElementId>, state: Check) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(16.0))
        .flex_shrink_0()
        .rounded(theme::RADIUS_SM)
        .border_1()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .map(|b| match state {
            Check::On => b
                .bg(theme::accent())
                .border_color(theme::accent())
                .child(render_icon(Icon::Check, px(11.0), theme::on_accent())),
            Check::Mixed => b
                .bg(theme::accent())
                .border_color(theme::accent())
                .child(div().w(px(8.0)).h(px(2.0)).bg(theme::on_accent())),
            Check::Off => b.border_color(theme::line_2()),
        })
}
