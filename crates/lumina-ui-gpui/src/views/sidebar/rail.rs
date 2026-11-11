//! The collapsed sidebar: a 48 px rail with one icon button per Library
//! collection, rendered from the same `nav_entries` as the expanded rows.

use gpui::*;

use super::nav::{self, nav_entries, NavEntry};
use super::AlbumSelection;
use crate::i18n::t;
use crate::icons::render_icon;
use crate::theme;
use crate::views::grid::SIDEBAR_COLLAPSED_WIDTH;
use crate::LuminaApp;

pub(super) fn render_rail(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .id("sidebar-rail")
        .debug_selector(|| "sidebar-rail".to_string())
        .w(px(SIDEBAR_COLLAPSED_WIDTH))
        .flex_none()
        .h_full()
        .bg(theme::bg_1())
        .border_r_1()
        .border_color(theme::line())
        .flex()
        .flex_col()
        .items_center()
        // macOS draws the traffic lights over the top of the rail.
        .pt(px(if cfg!(target_os = "macos") {
            52.0
        } else {
            16.0
        }))
        .pb_4()
        .gap_1()
        // Scrolls rather than clips if a window is ever shorter than the list.
        .overflow_y_scroll()
        .children(
            nav_entries(this)
                .into_iter()
                // Untagged sits under Smart Albums, which the rail leaves out.
                .filter(|entry| entry.selection != AlbumSelection::Untagged)
                .map(|entry| render_rail_button(this, entry, cx)),
        )
}

fn render_rail_button(
    this: &LuminaApp,
    entry: NavEntry,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let is_active = this.selection == entry.selection;
    let key = entry.key;
    let tooltip = t("sidebar.rail_tooltip")
        .replace("{label}", entry.label)
        .replace("{n}", &entry.count.to_string());
    let icon_color = if is_active { theme::t1() } else { theme::t2() };
    let pill = div()
        .size_full()
        .rounded(theme::RADIUS_MD)
        .flex()
        .justify_center()
        .items_center()
        .child(render_icon(entry.icon, px(16.0), icon_color));
    // The pill carries its own selector so tests can see which entry is active.
    let pill = if is_active {
        pill.debug_selector(|| "sidebar-rail-active".to_string())
    } else {
        pill
    };
    let selection = entry.selection;
    // The active fill sits on the button itself so its hover can replace it.
    let button = if is_active {
        div().bg(theme::bg_3())
    } else {
        div()
    };
    button
        .id(ElementId::Name(format!("sidebar-rail-{key}").into()))
        .debug_selector(move || format!("sidebar-rail-{key}"))
        .role(Role::Button)
        .aria_label(crate::i18n::nav_aria_label(entry.label, entry.count))
        .tooltip(theme::tooltip_label(tooltip))
        .flex_none()
        .size(px(32.0))
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .hover(move |s| {
            if is_active {
                s.bg(theme::bg_3())
            } else {
                s.bg(theme::bg_2())
            }
        })
        .child(pill)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            nav::open_entry(this, &selection, cx);
        }))
        .into_any_element()
}
