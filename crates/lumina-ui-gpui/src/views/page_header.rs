//! The main column's page header: one title bar row with the title and its
//! meta on the left, an empty drag area, then the page's actions.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;

/// Height of the page header: the title bar band plus its 1px bottom border,
/// so its contents centre on the window controls' line. Sheets and popovers
/// hang from its bottom edge.
pub(crate) const PAGE_HEADER_HEIGHT: f32 = theme::TITLEBAR_HEIGHT + 1.0;

const TITLE_SIZE: f32 = 20.0;
const META_SIZE: f32 = 10.5;
/// GPUI lays text out with no baseline, so `items_baseline` lines up the
/// boxes' bottom edges, and the smaller meta lands below the title's
/// baseline. Its baseline sits 0.365 of the font size above its box's
/// bottom (Inter and the mono alike, at the default line height), so the
/// meta is lifted by the difference to share the title's baseline.
pub(crate) const META_BASELINE_LIFT: f32 = 0.365 * (TITLE_SIZE - META_SIZE);

/// The header row: `leading` (the sidebar toggle while the sidebar is
/// collapsed), the title with its meta after it on the same baseline,
/// a drag area (it moves a frameless window when `draggable`), and `actions`.
/// A long title truncates before the actions move.
pub(crate) fn page_header(
    leading: Option<AnyElement>,
    title: impl Into<SharedString>,
    meta: Option<SharedString>,
    actions: impl IntoElement,
    draggable: bool,
) -> Div {
    page_header_with_meta_font(leading, title, meta, theme::FONT_MONO, actions, draggable)
}

/// [`page_header`] with the meta line in `meta_font`: Settings' subtitle is
/// sans, the other pages' mono.
pub(crate) fn page_header_with_meta_font(
    leading: Option<AnyElement>,
    title: impl Into<SharedString>,
    meta: Option<SharedString>,
    meta_font: &'static str,
    actions: impl IntoElement,
    draggable: bool,
) -> Div {
    let title: SharedString = title.into();
    div()
        .debug_selector(|| "page-header".to_string())
        .w_full()
        .h(px(PAGE_HEADER_HEIGHT))
        .flex_shrink_0()
        .px(px(28.0))
        .bg(theme::bg_0())
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .children(leading)
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(10.0))
                .child(
                    // ponytail: the design's -0.3 tracking is skipped; GPUI
                    // 0.3.11 has no letter-spacing style.
                    div()
                        .debug_selector(|| "page-title".to_string())
                        .min_w_0()
                        .truncate()
                        .font_family(theme::FONT_DISPLAY)
                        .text_size(px(TITLE_SIZE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t1())
                        .children(text_marker("page-title", &title))
                        .child(title),
                )
                .children(meta.map(|meta| {
                    let marker = text_marker("page-meta", &meta);
                    div()
                        .debug_selector(|| "page-meta".to_string())
                        .min_w_0()
                        .truncate()
                        .font_family(meta_font)
                        .text_size(px(META_SIZE))
                        .mb(px(META_BASELINE_LIFT))
                        .text_color(theme::t3())
                        .children(marker)
                        .children(text_marker("page-meta-font", &meta_font.into()))
                        .child(meta)
                })),
        )
        // Empty header space moves a frameless window; double-click maximizes.
        .child(
            div()
                .id("toolbar-drag-area")
                .debug_selector(|| "toolbar-drag-area".to_string())
                .flex_1()
                // Some empty header always remains: the meta truncates first.
                .min_w(px(24.0))
                .h_full()
                .when(draggable, |area| {
                    area.on_mouse_down(MouseButton::Left, |event, window, _| {
                        if event.click_count >= 2 {
                            window.zoom_window();
                        } else {
                            window.start_window_move();
                        }
                    })
                }),
        )
        .child(
            div()
                .debug_selector(|| "page-actions".to_string())
                .flex_shrink_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .child(actions),
        )
}

/// One tab of [`page_tabs`]: `id` makes the `page-tab-{id}` selector.
pub(crate) struct PageTab {
    pub id: &'static str,
    pub label: SharedString,
    pub count: usize,
    pub active: bool,
}

/// The tab bar under the page header: 51.5 high,
/// tabs 34 high with a mono count. The active tab sits on bg-2; it takes
/// the focus ring while the bar has keyboard focus (`focused`). A click
/// calls `on_select` with the tab's id.
pub(crate) fn page_tabs(
    label: impl Into<SharedString>,
    tabs: &[PageTab],
    focused: bool,
    on_select: impl Fn(&&'static str, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on_select = std::rc::Rc::new(on_select);
    div()
        .id("page-tabs")
        .debug_selector(|| "page-tabs".to_string())
        .role(Role::TabList)
        .aria_label(label.into())
        .w_full()
        .h(px(51.5))
        .flex_shrink_0()
        .px(px(28.0))
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .children(tabs.iter().map(|tab| {
            let id = tab.id;
            let on_select = on_select.clone();
            let bg = if tab.active {
                theme::bg_2()
            } else {
                theme::bg_0()
            };
            div()
                .id(id)
                .debug_selector(move || format!("page-tab-{id}"))
                .role(Role::Tab)
                .aria_label(tab.label.clone())
                .aria_selected(tab.active)
                .h(px(34.0))
                .px(px(12.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(7.0))
                .rounded(theme::RADIUS_LG)
                .bg(bg)
                // Always a border, so the focus ring never shifts the tabs.
                .border_1()
                .border_color(if focused && tab.active {
                    theme::focus_ring()
                } else {
                    bg
                })
                .hover(|s| s.bg(theme::bg_2()))
                .cursor_pointer()
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(if tab.active {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .text_color(if tab.active { theme::t1() } else { theme::t2() })
                        .child(tab.label.clone()),
                )
                .children(text_marker(
                    "page-tab-count",
                    &format!("{id} {}", tab.count).into(),
                ))
                .child(
                    div()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(9.5))
                        .text_color(if tab.active {
                            theme::accent()
                        } else {
                            theme::t3()
                        })
                        .child(crate::i18n::count(tab.count)),
                )
                .on_click(move |_, window, cx| on_select(&id, window, cx))
        }))
}

/// Tests only: a zero-size marker whose `{kind}-text:{text}` selector
/// carries the rendered text, since GPUI's test window exposes neither
/// painted text nor an accessibility tree. Absolute, so it takes no space.
pub(crate) fn text_marker(kind: &'static str, text: &SharedString) -> Option<Div> {
    cfg!(test).then(|| {
        let text = text.clone();
        div()
            .absolute()
            .size_0()
            .debug_selector(move || format!("{kind}-text:{text}"))
    })
}

/// 32×32 header button: bg-2, 8px radius, `line` outline, 15px icon; bg-3
/// and t1 while `on`. `selector` doubles as the element id. `label` is the
/// accessible name; the tooltip adds the `shortcut`.
pub(crate) fn header_icon_button(
    selector: &'static str,
    icon: Icon,
    label: impl Into<SharedString>,
    shortcut: Option<&str>,
    on: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let tooltip = match shortcut {
        Some(keys) => format!("{label} ({keys})"),
        None => label.to_string(),
    };
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label)
        .tooltip(theme::tooltip_label(tooltip))
        .size(px(32.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_LG)
        .border_1()
        .border_color(theme::line())
        .bg(if on { theme::bg_3() } else { theme::bg_2() })
        .hover(|s| s.bg(theme::bg_3()))
        .cursor_pointer()
        .child(render_icon(
            icon,
            px(15.0),
            if on { theme::t1() } else { theme::t2() },
        ))
}

/// The header's primary action: accent, 8px radius, 32 high, icon 14 and a
/// 12.5/600 label.
pub(crate) fn header_primary_button(
    selector: &'static str,
    icon: Icon,
    label: impl Into<SharedString>,
    tooltip: impl Into<SharedString>,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(tooltip))
        .h(px(32.0))
        .px(px(13.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(7.0))
        .rounded(theme::RADIUS_LG)
        .bg(theme::accent())
        .hover(|s| s.bg(theme::accent_hover()))
        .cursor_pointer()
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::on_accent())
        .child(render_icon(icon, px(14.0), theme::on_accent()))
        .child(label)
}

/// Hangs `card` under the `relative` control it is a child of, 6px below
/// and kept inside the window: right edges aligned for `Anchor::TopRight`
/// (header buttons), left edges for `Anchor::TopLeft` (the chip row's Add
/// filter). Deferred, so it paints (and takes clicks) above the rest of the
/// window, its dismiss backdrop included.
pub(crate) fn popover_under(anchor: Anchor, card: impl IntoElement) -> Div {
    // A zero-size point at the control's bottom corner, past its 1px border.
    let point_at = div().absolute().bottom(px(-1.0)).size_0();
    match anchor {
        Anchor::TopLeft => point_at.left(px(-1.0)),
        _ => point_at.right(px(-1.0)),
    }
    .child(deferred(
        anchored()
            .anchor(anchor)
            .offset(point(px(0.0), px(6.0)))
            .snap_to_window_with_margin(px(8.0))
            .child(card),
    ))
}
