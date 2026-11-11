//! Shared design components from `lumina.pen`: NavItem (`S9a9Iv`) and Chip
//! (`aisTo`). Callers add handlers and trailing children.
use super::*;
use crate::icons::{render_icon, Icon};
use gpui::{div, px, rgba, ElementId, FontWeight, Role, Stateful};

/// Sidebar navigation row (NavItem): 32px, 10px padding, 8px radius, icon 16
/// and label 13 in t2, or bg-3 with a t1/500 label and an accent icon while
/// active. A non-zero `count` sits on the right in mono 10 t3 (t2 on the
/// active row, where t3 would fall below 4.5:1). `selector` names the row;
/// its icon and count are `{selector}-icon` and `{selector}-count`.
pub fn nav_item(
    selector: String,
    icon: Icon,
    label: &str,
    count: usize,
    active: bool,
) -> Stateful<Div> {
    let (icon_selector, count_selector) = (format!("{selector}-icon"), format!("{selector}-count"));
    let aria = crate::i18n::nav_aria_label(label, count);
    div()
        .id(ElementId::Name(selector.clone().into()))
        .debug_selector(move || selector)
        .role(Role::Button)
        .aria_label(aria)
        .h(px(32.0))
        .px(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .rounded(RADIUS_LG)
        .cursor_pointer()
        .map(|row| {
            if active {
                row.bg(bg_3())
            } else {
                row.hover(|s| s.bg(bg_2()))
            }
        })
        .child(
            render_icon(icon, px(16.0), if active { accent() } else { t2() })
                .flex_none()
                .debug_selector(move || icon_selector),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(TEXT_BODY)
                .font_weight(if active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if active { t1() } else { t2() })
                .child(label.to_string()),
        )
        .when(count > 0, |row| {
            row.child(
                div()
                    .debug_selector(move || count_selector)
                    .flex_none()
                    .text_size(px(10.0))
                    .font_family(FONT_MONO)
                    .text_color(if active { t2() } else { t3() })
                    .child(crate::i18n::count(count)),
            )
        })
}

/// Chip variants. `Ghost` (transparent, `line` outline, t2) also draws
/// partial batch tags, with a dashed outline added by the caller.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipKind {
    Normal,
    Ghost,
}

/// Pill chip (Chip): 28px, 11px padding, 6px gap, label 12. Normal is bg-2
/// with a `line` outline and t1 text.
pub fn chip(id: impl Into<ElementId>, kind: ChipKind) -> Stateful<Div> {
    let base = div()
        .id(id)
        .h(px(28.0))
        .px(px(11.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .rounded_full()
        .border_1()
        .border_color(line())
        .text_size(px(12.0));
    match kind {
        ChipKind::Normal => base.bg(bg_2()).text_color(t1()),
        ChipKind::Ghost => base.bg(rgba(0x00000000)).text_color(t2()),
    }
}
