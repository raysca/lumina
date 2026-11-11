//! The Duplicates destination: an "Exact copies | Similar shots" header over
//! the merge view (`views/duplicates.rs`) or the stacks review list.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{Flag, StackKind};

use crate::app::stacks::{DuplicatesTab, ReviewStack, StacksList};
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::marks::{flag_badge, REJECTED_OPACITY};
use crate::LuminaApp;

/// Members drawn per stack; the rest are counted in a "+k more" label.
const MAX_SHOWN: usize = 8;
const THUMB_WIDTH: f32 = 140.0;
const THUMB_HEIGHT: f32 = 100.0;
/// Header, gap, thumbnail and file name, inside the card's padding.
const CARD_HEIGHT: f32 = 196.0;
/// A list row: the card plus the gap above it.
const ROW_HEIGHT: f32 = CARD_HEIGHT + 12.0;

/// Room for the "+k more" label after the drawn members.
const MORE_WIDTH: f32 = 72.0;
/// Gap between members (`gap_3`).
const GAP: f32 = 12.0;

/// How many members fit across a stack card in this window.
#[derive(Clone, Copy)]
struct Fit {
    /// Members that fit with nothing after them.
    all: usize,
    /// Members that fit before the "+k more" label.
    with_more: usize,
}

impl Fit {
    fn shown(self, len: usize) -> usize {
        if len <= self.all {
            len
        } else {
            self.with_more
        }
    }
}

/// The content width is worked out from the window and the panes beside
/// the list (sidebar, inspector, resizers), the list's padding and the
/// card's padding and border, since `uniform_list` gives rows no width.
fn members_that_fit(this: &LuminaApp, window: &Window) -> Fit {
    let pane = crate::views::grid::center_pane_width(
        window.viewport_size().width.into(),
        this.sidebar_width,
        this.sidebar_collapsed,
        false, // no year rail on Duplicates
        this.inspector_open.then_some(this.inspector_width),
    );
    // px_4 on the row, p_3 and a 1 px border on the card.
    let content = pane - 32.0 - 26.0;
    // Zero when not even one fits: then only "+k more" is drawn (k = all).
    let count = |width: f32| {
        (((width + GAP) / (THUMB_WIDTH + GAP)).floor().max(0.0) as usize).min(MAX_SHOWN)
    };
    Fit {
        all: count(content),
        with_more: count(content - MORE_WIDTH - GAP),
    }
}

/// The tab header and the shown tab's body.
pub fn render_duplicates(
    this: &LuminaApp,
    total_count: usize,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let body = match this.stacks.tab {
        DuplicatesTab::Exact => {
            crate::views::duplicates::DuplicatesView::render(this, total_count, cx)
                .into_any_element()
        }
        DuplicatesTab::Similar => render_similar(this, cx),
    };
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(theme::bg_0())
        .child(
            div()
                .id("duplicates-tabs")
                .role(Role::TabList)
                .aria_label(t("duplicates.title"))
                .px_4()
                .pt_3()
                .flex()
                .flex_row()
                .gap_1()
                .child(tab(
                    this,
                    DuplicatesTab::Exact,
                    "exact-copies-tab",
                    "duplicates.tab_exact",
                    "duplicates.tab_exact_hint",
                    cx,
                ))
                .child(tab(
                    this,
                    DuplicatesTab::Similar,
                    "stacks-tab",
                    "duplicates.tab_similar",
                    "duplicates.tab_similar_hint",
                    cx,
                )),
        )
        .child(div().flex_1().min_h_0().flex().flex_row().child(body))
        .into_any_element()
}

fn tab(
    this: &LuminaApp,
    which: DuplicatesTab,
    selector: &'static str,
    key: &str,
    hint: &str,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let label = t(key);
    if this.stacks.tab == which {
        theme::control_on(selector)
    } else {
        theme::control(selector)
    }
    .debug_selector(move || selector.to_string())
    .role(Role::Tab)
    .aria_label(label.clone())
    .aria_selected(this.stacks.tab == which)
    .tooltip(theme::tooltip_label(t(hint)))
    .child(label)
    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
        this.set_duplicates_tab(which, cx);
    }))
}

/// A centered message: loading, error or empty.
fn notice(selector: &'static str, title: String, body: Option<String>) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Status)
        .aria_label(title.clone())
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(title),
        )
        .children(body.map(|b| div().text_xs().text_color(theme::t2()).child(b)))
}

fn render_similar(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let stacks = match &this.stacks.list {
        StacksList::Loading => {
            return notice("stacks-loading", t("stacks.loading"), None).into_any_element();
        }
        StacksList::Failed => {
            let retry = t("stacks.retry");
            return notice(
                "stacks-error",
                t("stacks.error_title"),
                Some(t("stacks.error_body")),
            )
            .child(
                theme::control("stacks-retry")
                    .debug_selector(|| "stacks-retry".to_string())
                    .role(Role::Button)
                    .aria_label(retry.clone())
                    .aria_keyshortcuts("Enter")
                    .tooltip(theme::tooltip_label(t_with(
                        "stacks.with_shortcut",
                        &[&retry, "Enter"],
                    )))
                    .child(retry)
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.load_stacks(cx);
                    })),
            )
            .into_any_element();
        }
        StacksList::Loaded(stacks) if stacks.is_empty() => {
            return notice(
                "stacks-empty",
                t("stacks.empty_title"),
                Some(t("stacks.empty_body")),
            )
            .into_any_element();
        }
        StacksList::Loaded(stacks) => stacks.len(),
    };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
            div()
                .px_4()
                .pt_2()
                .text_xs()
                .text_color(theme::t2())
                .child(t("stacks.hint")),
        )
        .child(
            div()
                .id("stacks-list")
                .debug_selector(|| "stacks-list".to_string())
                .role(Role::List)
                .aria_label(t("duplicates.tab_similar"))
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(
                    // Virtualized: only the visible rows are built (and queue
                    // thumbnails). Every row is ROW_HEIGHT tall, so a stack
                    // shows at most MAX_SHOWN members.
                    uniform_list(
                        "stacks-rows",
                        stacks,
                        cx.processor(|this, range: std::ops::Range<usize>, window, cx| {
                            let fit = members_that_fit(this, window);
                            let StacksList::Loaded(stacks) = &this.stacks.list else {
                                return Vec::new();
                            };
                            range
                                .filter_map(|index| {
                                    let stack = stacks.get(index)?;
                                    Some(
                                        div()
                                            .h(px(ROW_HEIGHT))
                                            .px_4()
                                            .pt_3()
                                            .child(render_stack(this, index, stack, fit, cx))
                                            .into_any_element(),
                                    )
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(&this.stacks.scroll)
                    .flex_1()
                    .min_h_0(),
                ),
        )
        .into_any_element()
}

fn action(
    selector: String,
    key: &str,
    shortcut: &str,
    index: usize,
    run: fn(&mut LuminaApp, usize, &mut Context<LuminaApp>),
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let label = t(key);
    theme::control(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_keyshortcuts(shortcut.to_string())
        .tooltip(theme::tooltip_label(t_with(
            "stacks.with_shortcut",
            &[&label, shortcut],
        )))
        .child(label)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.focus_stack(index, cx);
            run(this, index, cx);
        }))
}

fn render_stack(
    this: &LuminaApp,
    index: usize,
    stack: &ReviewStack,
    fit: Fit,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let shown = fit.shown(stack.items.len());
    let focused = this.stacks.focused == index;
    let kind = match stack.kind {
        StackKind::RawJpeg => t("stacks.kind_pair"),
        StackKind::Burst => t("stacks.kind_burst"),
    };
    let title = t_with("stacks.title", &[&kind, &stack.items.len().to_string()]);
    div()
        .id(("stack", index as u64))
        .debug_selector(move || format!("stack-{index}"))
        .role(Role::ListItem)
        .aria_label(title.clone())
        .aria_selected(focused)
        .relative()
        .rounded(theme::RADIUS_LG)
        .bg(theme::bg_1())
        .border_1()
        .border_color(if focused {
            theme::focus_ring()
        } else {
            theme::line()
        })
        .h(px(CARD_HEIGHT))
        .overflow_hidden()
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.focus_stack(index, cx);
        }))
        .when(focused, |d| {
            d.child(
                div()
                    .debug_selector(move || format!("stack-focus-{index}"))
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(3.0))
                    .bg(theme::focus_ring()),
            )
        })
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .debug_selector(move || format!("stack-{index}-kind"))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t1())
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_1()
                        .child(action(
                            format!("stack-keep-picks-{index}"),
                            "stacks.keep_picks",
                            "Enter",
                            index,
                            LuminaApp::stack_keep_picks,
                            cx,
                        ))
                        .child(action(
                            format!("stack-reject-others-{index}"),
                            "stacks.reject_others",
                            "Shift+Enter",
                            index,
                            LuminaApp::stack_reject_others,
                            cx,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .overflow_hidden()
                .children(
                    stack
                        .items
                        .iter()
                        .take(shown)
                        .map(|item| render_member(this, index, item)),
                )
                .when(stack.items.len() > shown, |row| {
                    let more = (stack.items.len() - shown).to_string();
                    row.child(
                        div()
                            .debug_selector(move || format!("stack-{index}-more"))
                            .w(px(MORE_WIDTH))
                            .flex_shrink_0()
                            .h(px(THUMB_HEIGHT))
                            .flex()
                            .items_center()
                            .text_xs()
                            .text_color(theme::t2())
                            .child(t_with("stacks.more", &[&more])),
                    )
                }),
        )
        .into_any_element()
}

fn render_member(this: &LuminaApp, index: usize, item: &lumina_core::MediaItem) -> AnyElement {
    let id = item.id.0;
    let flag = this.item_flag(item.id);
    let texture: Option<Arc<RenderImage>> = this
        .grid_state
        .as_ref()
        .and_then(|state| state.texture_cache.lock().get(&item.id).cloned());
    if texture.is_none() {
        this.queue_thumbnail_request_item(item);
    }
    div()
        .id(("stack-item", id))
        .debug_selector(move || format!("stack-{index}-item-{id}"))
        .role(Role::Image)
        .aria_label(item.file_name.clone())
        .w(px(THUMB_WIDTH))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap_1()
        .when(flag == Flag::Reject, |d| d.opacity(REJECTED_OPACITY))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(THUMB_HEIGHT))
                .rounded(theme::RADIUS_MD)
                .overflow_hidden()
                .bg(theme::bg_2())
                .flex()
                .items_center()
                .justify_center()
                .child(match texture {
                    Some(tex) => img(tex).size_full().into_any_element(),
                    None => render_icon(Icon::Image, px(24.0), theme::t2()).into_any_element(),
                })
                .child(
                    div()
                        .absolute()
                        .top_1()
                        .right_1()
                        .child(flag_badge(id as usize, flag)),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::t1())
                .truncate()
                .child(item.file_name.clone()),
        )
        .into_any_element()
}
