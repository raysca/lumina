//! Lumina v3 "graphite" theme tokens (see the v3 token sheet in the design canvas).
//!
//! Rules: solid fills + 1px borders, depth from brightness steps (no blur/translucency),
//! shadows only on floating panels, amber is the single accent and always means selection.
#![allow(dead_code)]
use gpui::{
    div, prelude::*, px, rgb, rgba, AnyView, App, Context, Div, ElementId, Pixels, Render, Rgba,
    SharedString, Stateful, Window,
};

/// Body text: sidebar rows, fields and menus (v3.1 type scale).
pub const TEXT_BODY: Pixels = px(13.0);

// Corner radii (v3.1). Grid thumbnails stay square; icon tiles keep their
// own shape.
/// Chips, badges, swatches, and rows nested in an 8px menu.
pub const RADIUS_SM: Pixels = px(4.0);
/// Buttons, fields, rows and segmented controls.
pub const RADIUS_MD: Pixels = px(6.0);
/// Floating surfaces: menus, popovers, toasts, dialogs, the palette.
pub const RADIUS_LG: Pixels = px(8.0);

macro_rules! tokens {
    ($($(#[$m:meta])* $name:ident = $hex:literal;)*) => {
        $($(#[$m])* #[inline] pub fn $name() -> Rgba { rgb($hex) })*
    };
}

tokens! {
    // Surfaces, darkest to lightest.
    bg_media = 0x000000;
    bg_viewer = 0x141517;
    bg_app = 0x1C1D20;
    bg_pane = 0x202125;
    /// Confirmation dialogs (Delete).
    bg_dialog = 0x232428;
    bg_raised = 0x26272B;
    bg_hud = 0x2A2C30;
    bg_control = 0x2C2E32;
    bg_control_hover = 0x34363B;

    // Lines.
    border_subtle = 0x2E3034;
    border = 0x36383D;
    border_strong = 0x44464C;
    /// Keyboard focus on fields and controls. Neutral: amber means selection.
    focus_ring = 0xAAAAA8;

    // Text.
    text = 0xE7E6E3;
    text_muted = 0xA0A2A7;
    text_subtle = 0x909298;
    text_disabled = 0x5A5D63;
    /// Idle list/sidebar row text.
    text_row = 0xC9CACD;
    /// Section labels (LIBRARY, TAGS, ...).
    text_label = 0x8E9096;

    // Accent: selection and primary actions (focus is `focus_ring`).
    accent = 0xF0B54B;
    accent_hover = 0xF4C46A;
    /// Text/icons drawn on an `accent` fill.
    accent_fg = 0x1A1206;

    // Selection (rows and toggles that are "on").
    selection_bg = 0x3A3426;
    selection_bg_hover = 0x453D2B;
    selection_border = 0x6B5A33;
    selection_fg = 0xF5DDA8;
    /// Counts and hints inside a selected row.
    selection_count = 0xC9AE76;
    /// Shortcut hints on an `accent` fill (a hovered menu row).
    accent_hint = 0x5A4214;

    // Metadata marks. Grid cards draw them white on a scrim over the photo.
    rating = 0xF0B54B;
    /// Empty star outlines: dimmer than text, still 3:1 on every surface.
    rating_unset = 0x77797F;
    flag = 0xF28C38;
    label_red = 0xE5484D;
    label_orange = 0xF28C38;
    label_yellow = 0xF5D90A;
    label_green = 0x46A758;
    label_blue = 0x3E8BF5;
    label_purple = 0x8E4EC6;

    // Status.
    danger = 0xC4343A;
    danger_text = 0xF08A7A;
    danger_bg = 0x3A2224;
    danger_bg_hover = 0x4A2A2C;
    danger_border = 0x7A3A3C;
    success = 0x2E7D46;
    success_text = 0x7FCDA8;
    success_bg = 0x1F3328;
}

// Shared control shapes from the token sheet. Callers add content and handlers.

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
        .border_color(border())
        .bg(bg_control())
        .hover(|s| s.bg(bg_control_hover()))
        .text_color(text())
}

/// Borderless toolbar control: no fill until hover (v3 chrome). The 1px
/// border is transparent so it sizes like `control_on`.
pub fn control_ghost(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .border_color(rgba(0x00000000))
        .hover(|s| s.bg(bg_control_hover()))
        .text_color(text_row())
}

/// `control` in the "on" state (a toggle that is active).
pub fn control_on(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .bg(selection_bg())
        .border_color(selection_border())
        .text_color(selection_fg())
        .hover(|s| s.bg(selection_bg_hover()))
}

/// The one primary (amber) action in a bar.
pub fn control_primary(id: impl Into<ElementId>) -> Stateful<Div> {
    control_base(id)
        .bg(accent())
        .border_color(accent())
        .text_color(accent_fg())
        .hover(|s| s.bg(accent_hover()))
}

/// Semi-transparent dark overlay for modals and viewports.
#[inline]
pub fn scrim() -> Rgba {
    rgba(0x000000cc)
}

/// Library dim behind modals and sheets: the grid stays readable, as in
/// the designs (0.35–0.4).
#[inline]
pub fn modal_dim() -> Rgba {
    rgba(0x00000066)
}

/// Section label (LIBRARY, TAGS, ...): 11px semibold #8E9096. Callers
/// add spacing; sidebar and inspector sections sit 16px below the previous.
pub fn section_label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(text_label())
        .child(text.into())
}

/// Marquee fill: the accent at 12%, so drag-selection stays amber.
#[inline]
pub fn selection_wash() -> Rgba {
    rgba(0xF0B54B1F)
}

/// Library card surface.
///
/// Cards are transparent by request: thumbnails sit directly on the library
/// background instead of a raised panel. Selection is still shown by the
/// accent outline and the card border.
#[inline]
pub fn card_surface() -> Rgba {
    rgba(0x00000000)
}

pub const FONT_SANS: &str = "IBM Plex Sans";
pub const FONT_MONO: &str = "IBM Plex Mono";

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
        .border_color(border())
        .bg(bg_control())
        .text_size(TEXT_BODY)
        .text_color(text())
        .cursor_text()
}

/// Floating menu container (context menus, dropdowns).
pub fn menu(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .bg(bg_hud())
        .border_1()
        .border_color(border())
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
        base.bg(accent()).text_color(accent_fg())
    } else {
        base.text_color(text()).hover(|s| s.bg(bg_control_hover()))
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
        base.bg(selection_bg())
            .text_color(selection_fg())
            .hover(|s| s.bg(selection_bg_hover()))
    } else {
        base.text_color(text_row())
            .hover(|s| s.bg(bg_control_hover()))
    }
}

/// Small metadata badge (e.g. `R+J`, `0:42` with a play icon, stack count).
pub fn badge(label: impl Into<SharedString>) -> Div {
    div()
        .px(px(5.0))
        .py(px(1.5))
        .rounded(RADIUS_SM)
        .bg(scrim())
        .border_1()
        .border_color(border_subtle())
        .text_xs()
        .font_family(FONT_MONO)
        .text_color(text())
        .child(label.into())
}

/// 8px circular color label dot.
pub fn label_dot(color: Rgba) -> Div {
    div().size(px(8.0)).rounded_full().bg(color)
}

/// Floating HUD / panel container with subtle border and elevation.
pub fn hud_panel(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .bg(bg_hud())
        .border_1()
        .border_color(border())
        .rounded(RADIUS_LG)
        .shadow_md()
}

/// Small keyboard shortcut tag (e.g. `⌘I`, `Space`).
pub fn kbd(label: impl Into<SharedString>) -> Div {
    div()
        .px(px(4.0))
        .py(px(1.0))
        .rounded(RADIUS_SM)
        .bg(bg_control())
        .border_1()
        .border_color(border())
        .text_xs()
        .font_family(FONT_MONO)
        .text_color(text_muted())
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
        .bg(bg_control())
        .border_1()
        .border_color(border())
}

/// Segmented control item.
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
        base.bg(selection_bg())
            .text_color(selection_fg())
            .border_1()
            .border_color(selection_border())
    } else {
        base.text_color(text_muted())
            .hover(|s| s.text_color(text()).bg(bg_control_hover()))
    }
}

/// Small floating label used for hover tooltips.
struct TooltipLabel {
    text: SharedString,
}

impl Render for TooltipLabel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .bg(bg_hud())
            .border_1()
            .border_color(border())
            .rounded(RADIUS_MD)
            .text_xs()
            .text_color(text())
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
        None => swatch.border_1().border_color(text_muted()),
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
        base.border_2().border_color(text())
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // WCAG relative luminance contrast, so palette tweaks can't silently break legibility.
    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let l = |c: Rgba| {
            let f = |v: f32| {
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
        };
        let (x, y) = (l(a), l(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn card_surface_is_transparent() {
        assert_eq!(
            card_surface().a,
            0.0,
            "grid cards must not paint a surface behind thumbnails"
        );
    }

    #[test]
    fn focus_ring_is_neutral_and_visible() {
        let f = focus_ring();
        assert!(
            (f.r - f.b).abs() < 0.02 && (f.g - f.b).abs() < 0.02,
            "focus is grey, not amber: amber is reserved for selection"
        );
        for bg in [
            bg_viewer(),
            bg_app(),
            bg_pane(),
            bg_dialog(),
            bg_raised(),
            bg_control(),
        ] {
            assert!(
                contrast(f, bg) >= 3.0,
                "the focus border needs 3:1 against the field (WCAG 1.4.11)"
            );
        }
    }

    #[test]
    fn text_pairs_meet_aa() {
        for bg in [
            bg_viewer(),
            bg_app(),
            bg_pane(),
            bg_dialog(),
            bg_raised(),
            bg_hud(),
            bg_control(),
        ] {
            assert!(contrast(text(), bg) >= 4.5);
            assert!(contrast(text_muted(), bg) >= 4.5);
            assert!(
                contrast(text_subtle(), bg) >= 4.0,
                "hints and counts stay readable on every surface"
            );
            assert!(
                contrast(rating_unset(), bg) >= 3.0,
                "unset stars are a control and need 3:1 (WCAG 1.4.11)"
            );
        }
        assert!(contrast(accent_fg(), accent()) >= 4.5);
        assert!(contrast(selection_fg(), selection_bg()) >= 4.5);
        assert!(contrast(danger_text(), bg_app()) >= 4.5);
    }
}
