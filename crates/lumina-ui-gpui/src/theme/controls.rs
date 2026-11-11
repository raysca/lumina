//! Shared control shapes: buttons, fields, menus, rows, badges, segmented
//! controls, tooltips and label swatches. Callers add content and handlers.
use super::*;
use gpui::{
    div, px, rgba, AnyView, App, Context, Div, ElementId, Render, Rgba, SharedString, Stateful,
    Window,
};

fn control_base(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.0))
        .px(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .rounded(RADIUS_MD)
        .border_1()
        .cursor_pointer()
        .text_xs()
}

/// Toolbar button: 28px tall, 6px radius, 1px border.
pub fn control(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .border_color(line())
        .bg(bg_2())
        .hover(|s| s.bg(bg_3()))
        .text_color(t1())
}

/// Borderless toolbar control: no fill until hover. The 1px
/// border is transparent so it sizes like `control_on`.
pub fn control_ghost(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .border_color(rgba(0x00000000))
        .hover(|s| s.bg(bg_3()))
        .text_color(t2())
}

/// `control` in the "on" state (a toggle that is active): bg-3 and t1, as a
/// selected segment in the design. The border matches the fill so it sizes
/// like `control`.
pub fn control_on(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .bg(bg_3())
        .border_color(bg_3())
        .text_color(t1())
}

/// The one primary (amber) action in a bar.
pub fn control_primary(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .bg(accent())
        .border_color(accent())
        .text_color(on_accent())
        .hover(|s| s.bg(accent_hover()))
}

/// Text input field: 28px tall, 6px radius, 1px border.
pub fn field(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.0))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .rounded(RADIUS_MD)
        .border_1()
        .border_color(line())
        .bg(bg_2())
        .text_size(TEXT_BODY)
        .text_color(t1())
        .cursor_text()
}

/// Floating menu container (context menus, dropdowns).
pub fn menu(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .bg(bg_float())
        .border_1()
        .border_color(line())
        .rounded(RADIUS_LG)
        .shadow_lg()
        .p(px(4.0))
        .flex()
        .flex_col()
        .gap(px(1.0))
}

/// Menu item row inside a menu.
pub fn menu_item(id: impl Into<ElementId>, active: bool) -> Stateful<Div> {
    let base = div()
        .id(id)
        .h(px(26.0))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .rounded(RADIUS_SM)
        .text_size(TEXT_BODY)
        .cursor_pointer();

    // The highlighted row is amber with dark text. Mouse hover can only
    // change the fill (GPUI fixes text colour at layout), so it stays neutral.
    if active {
        base.bg(accent()).text_color(on_accent())
    } else {
        base.text_color(t1()).hover(|s| s.bg(bg_3()))
    }
}

/// Standard 26px list/sidebar row.
pub fn row(id: impl Into<ElementId>, active: bool) -> Stateful<Div> {
    let base = div()
        .id(id)
        .h(px(26.0))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .rounded(RADIUS_MD)
        .text_size(TEXT_BODY)
        .cursor_pointer();

    if active {
        base.bg(bg_3()).text_color(t1())
    } else {
        base.text_color(t2()).hover(|s| s.bg(bg_3()))
    }
}

/// 8px circular color label dot.
pub fn label_dot(color: Rgba) -> Div {
    div().size(px(8.0)).rounded_full().bg(color)
}

/// Floating HUD / panel container with subtle border and elevation.
pub fn hud_panel(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .bg(bg_float())
        .border_1()
        .border_color(line())
        .rounded(RADIUS_LG)
        .shadow_md()
}

/// Small keyboard shortcut tag (e.g. `⌘I`, `Space`).
pub fn kbd(label: impl Into<SharedString>) -> Div {
    div()
        .px(px(4.0))
        .py(px(1.0))
        .rounded(RADIUS_SM)
        .bg(bg_2())
        .border_1()
        .border_color(line())
        .text_xs()
        .font_family(FONT_MONO)
        .text_color(t2())
        .child(label.into())
}

/// Segmented control container.
pub fn segmented(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.0))
        .p(px(2.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .rounded(RADIUS_MD)
        .bg(bg_2())
        .border_1()
        .border_color(line())
}

/// Segmented control item. Hover lifts only the text: bg-3 marks the
/// selected segment, so an idle one never takes it.
pub fn segmented_item(id: impl Into<ElementId>, active: bool) -> Stateful<Div> {
    let base = div()
        .id(id)
        .h(px(24.0))
        .px(px(8.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap_1()
        .rounded(RADIUS_SM)
        .text_xs()
        .cursor_pointer();

    if active {
        base.bg(bg_3()).text_color(t1())
    } else {
        base.text_color(t2()).hover(|s| s.text_color(t1()))
    }
}

/// Small floating label used for hover tooltips.
struct TooltipLabel {
    text: SharedString,
}

impl Render for TooltipLabel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.text.clone();
        div()
            .debug_selector(move || format!("tooltip-{label}"))
            .px_2()
            .py_1()
            .bg(bg_float())
            .border_1()
            .border_color(line())
            .rounded(RADIUS_MD)
            .text_xs()
            .text_color(t1())
            .child(self.text.clone())
    }
}

/// Builds a tooltip closure for `.tooltip(...)`.
pub fn tooltip_label(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_window, cx| cx.new(|_cx| TooltipLabel { text: text.clone() }).into()
}

/// Color labels in menu order: a hollow "No label", then the colors.
pub fn label_options() -> [(u8, Option<Rgba>, &'static str); 7] {
    [
        (0, None, "No label"),
        (1, Some(label_red()), "Red"),
        (2, Some(label_orange()), "Orange"),
        (3, Some(label_yellow()), "Yellow"),
        (4, Some(label_green()), "Green"),
        (5, Some(label_blue()), "Blue"),
        (6, Some(label_purple()), "Purple"),
    ]
}

/// A 14px label swatch in a 22px hit target; the chosen one gets a 2px ring
/// with a 2px gap. `prefix` names the selector, e.g. `context-label-3`.
pub fn label_swatch(
    prefix: &'static str,
    (idx, color, name): (u8, Option<Rgba>, &'static str),
    selected: bool,
) -> Stateful<Div> {
    let swatch = div().size(px(14.0)).rounded_full();
    let swatch = match color {
        Some(c) => swatch.bg(c),
        None => swatch.border_1().border_color(t2()),
    };
    let base = div()
        .id(ElementId::NamedInteger(prefix.into(), idx as u64))
        .debug_selector(move || format!("{prefix}-{idx}"))
        .tooltip(tooltip_label(name))
        .size(px(22.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(swatch);
    if selected {
        base.border_2().border_color(t1())
    } else {
        base
    }
}
