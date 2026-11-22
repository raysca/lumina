//! The empty-state shell's building blocks (glyph, headline, body, button),
//! shared with First Run, the unreachable library and the viewer's states.

use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

/// The shell's round glyph: `bg_2` fill, `line` outline and a 20 px icon.
/// Callers may restyle the fill and outline (the unreachable library's danger glyph).
pub(crate) fn empty_glyph(size: Pixels, icon: Icon, ink: Rgba) -> Div {
    div()
        .size(size)
        .flex_none()
        .rounded_full()
        .bg(theme::bg_2())
        .border_1()
        .border_color(theme::line())
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(icon, theme::ICON_LG, ink))
}

/// The First Run glyph: a 150 px box holding an 84 px
/// `bg_1` ring with an accent outline, a soft accent glow and a 36 px accent
/// icon.
pub(crate) fn hero_glyph(icon: Icon) -> Div {
    div()
        .size(px(150.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(84.0))
                .rounded_full()
                .bg(theme::bg_1())
                .border_1()
                .border_color(theme::accent_outline())
                // ponytail: the frame's radial glow as a soft shadow, by eye.
                .shadow(vec![BoxShadow {
                    color: theme::accent_soft().into(),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(48.0),
                    spread_radius: px(12.0),
                    inset: false,
                }])
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(icon, theme::ICON_XL, theme::accent())),
        )
}

/// The shell's headline: Inter Tight 18/600, centred, 14 px under the glyph.
pub(crate) fn empty_headline(selector: &'static str, title: SharedString) -> Div {
    div()
        .debug_selector(move || selector.to_string())
        .mt(px(14.0))
        .font_family(theme::FONT_DISPLAY)
        .text_size(theme::TEXT_TITLE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::t1())
        .text_center()
        .children(text_marker(selector, &title))
        .child(title)
}

/// The shell's body: 12.5/19 `t2`, centred.
pub(crate) fn empty_body(selector: &'static str, body: SharedString) -> Div {
    div()
        .debug_selector(move || selector.to_string())
        .mt(px(8.0))
        .text_size(theme::TEXT_SMALL)
        .line_height(px(19.0))
        .text_color(theme::t2())
        .text_center()
        .children(text_marker(selector, &body))
        .child(body)
}

/// The shell's button: accent primary or `line_2` secondary, 12.5 text, an
/// optional leading icon (`{id}-icon`; 14 px, or 15 px on a 40 px button).
/// `id` is also the selector; callers add the click.
pub(crate) fn empty_button(
    id: &'static str,
    primary: bool,
    label: SharedString,
    icon: Option<Icon>,
    height: Pixels,
) -> Stateful<Div> {
    let (button, ink) = if primary {
        let button = theme::control_primary(id).font_weight(FontWeight::SEMIBOLD);
        (button, theme::on_accent())
    } else {
        (
            theme::control(id).border_color(theme::line_2()),
            theme::t1(),
        )
    };
    button
        .debug_selector(move || id.to_string())
        .h(height)
        .px(px(14.0))
        .gap(px(7.0))
        .rounded(theme::RADIUS_LG)
        .text_size(theme::TEXT_SMALL)
        .role(Role::Button)
        .aria_label(label.clone())
        .children(icon.map(|icon| {
            // The 40 px Hero buttons (First Run) carry a 15 px icon.
            let size = if height >= px(40.0) {
                px(15.0)
            } else {
                px(14.0)
            };
            render_icon(icon, size, ink).debug_selector(move || format!("{id}-icon"))
        }))
        .children(text_marker(id, &label))
        .child(label)
}
