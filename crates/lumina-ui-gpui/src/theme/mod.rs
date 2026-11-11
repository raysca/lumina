//! Lumina theme tokens, named after the `lumina.pen` design variables.
//!
//! Rules: solid fills + 1px `line` borders, depth from the bg-0..bg-3 steps
//! (GPUI has no backdrop blur, so translucent design surfaces are drawn
//! opaque), shadows only on floating panels. Amber is the single accent and
//! means selection or the primary action; keyboard focus is the neutral
//! `focus_ring`.
use gpui::{div, prelude::*, px, rgb, rgba, Div, Pixels, Rgba, SharedString};

mod controls;
pub use controls::*;

/// Body text: sidebar rows, fields and menus (v3.1 type scale).
pub const TEXT_BODY: Pixels = px(13.0);

// Corner radii (v3.1). Photo thumbnails stay square wherever they appear;
// icon tiles keep their own shape.
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

/// Like `tokens!`, for colours with alpha (`0xRRGGBBAA`).
macro_rules! tokens_alpha {
    ($($(#[$m:meta])* $name:ident = $hex:literal;)*) => {
        $($(#[$m])* #[inline] pub fn $name() -> Rgba { rgba($hex) })*
    };
}

// The `lumina.pen` variables.
tokens! {
    /// Main canvas and inset fields.
    bg_0 = 0x0A0A0C;
    /// Sidebar, cards, panels and dialogs.
    bg_1 = 0x121216;
    /// Buttons, fields, chips, badges and hover.
    bg_2 = 0x1A1A20;
    /// Selected row or segment, control hover, tracks.
    bg_3 = 0x24242C;
    /// 1px outlines.
    line = 0x26262E;
    /// Stronger outlines.
    line_2 = 0x35353F;
    /// Primary text.
    t1 = 0xF2F2F5;
    /// Secondary text, idle row labels and icons.
    t2 = 0x93939E;
    /// Section labels, placeholders and meta lines. Lifted from the design's
    /// #5E5E6A, which is 2.4-3.1:1, so essential text stays 4.5:1 on bg-0/bg-1.
    t3 = 0x7C7C88;
    accent = 0xE9A23B;
    danger = 0xE8584F;
    positive = 0x5BC27A;
}

tokens_alpha! {
    /// Accent wash: selected cards, drag marquee.
    accent_soft = 0xE9A23B26;
    /// Danger wash: error banners and panels.
    danger_soft = 0xE8584F1F;
}

// Colours the app needs that the design has no variable for.
tokens! {
    /// Letterbox behind media.
    bg_media = 0x000000;
    /// Viewer and player stage.
    bg_stage = 0x08080A;
    /// Menus, popovers, tooltips and HUDs.
    bg_float = 0x16161C;
    /// Text and icons on an `accent` fill.
    on_accent = 0x1A1206;
    /// Text and icons over media and `media_scrim`.
    on_media = 0xFFFFFF;
    /// Hovered `accent` fill (the design has no hover state).
    accent_hover = 0xECB058;
    /// Shortcut hints on an `accent` fill (a hovered menu row).
    accent_hint = 0x5A4214;
    /// Keyboard focus on fields and controls. Neutral: amber means selection.
    focus_ring = 0xAAAAA8;
    /// Border of a danger panel or button. Invalid-value outlines use `danger`.
    danger_border = 0x45211E;

    // Metadata marks. Grid cards draw them white on a scrim over the photo.
    rating = 0xE9A23B;
    /// Empty star outlines: dimmer than text, still 3:1 on every surface.
    rating_unset = 0x77797F;
    label_red = 0xE5484D;
    label_orange = 0xF28C38;
    label_yellow = 0xF5D90A;
    label_green = 0x46A758;
    label_blue = 0x3E8BF5;
    label_purple = 0x8E4EC6;
}

tokens_alpha! {
    /// Hovered `danger_soft` button.
    danger_bg_hover = 0xE8584F33;
    /// Dialog scrim.
    scrim = 0x07070BD9;
    /// Library dim behind modals and sheets: the grid stays readable.
    modal_dim = 0x00000066;
    /// Badges and buttons drawn over media.
    media_scrim = 0x000000A6;
    /// Library card surface. Cards are transparent by request: thumbnails sit
    /// directly on the library background; selection shows as an outline.
    card_surface = 0x00000000;
}

/// Section label (LIBRARY, TAGS, ...): 10px semibold `t3`. Callers add
/// spacing; sidebar and inspector sections sit 16px below the previous.
pub fn section_label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(t3())
        .child(text.into())
}

/// Body and UI text.
pub const FONT_SANS: &str = "Inter";
/// Counts, sizes, durations and shortcut hints.
pub const FONT_MONO: &str = "JetBrains Mono";

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

    fn hex(c: Rgba) -> u32 {
        let b = |v: f32| (v * 255.0).round() as u32;
        b(c.r) << 24 | b(c.g) << 16 | b(c.b) << 8 | b(c.a)
    }

    #[core::prelude::v1::test]
    fn pen_tokens_have_design_values() {
        for (name, token, want) in [
            ("bg-0", bg_0(), 0x0A0A0CFF),
            ("bg-1", bg_1(), 0x121216FF),
            ("bg-2", bg_2(), 0x1A1A20FF),
            ("bg-3", bg_3(), 0x24242CFF),
            ("line", line(), 0x26262EFF),
            ("line-2", line_2(), 0x35353FFF),
            ("t1", t1(), 0xF2F2F5FF),
            ("t2", t2(), 0x93939EFF),
            ("accent", accent(), 0xE9A23BFF),
            ("accent-soft", accent_soft(), 0xE9A23B26),
            ("danger", danger(), 0xE8584FFF),
            ("danger-soft", danger_soft(), 0xE8584F1F),
            ("positive", positive(), 0x5BC27AFF),
            ("on_accent", on_accent(), 0x1A1206FF),
            ("bg_float", bg_float(), 0x16161CFF),
            ("bg_stage", bg_stage(), 0x08080AFF),
            ("scrim", scrim(), 0x07070BD9),
            ("media_scrim", media_scrim(), 0x000000A6),
            ("danger_border", danger_border(), 0x45211EFF),
        ] {
            assert_eq!(hex(token), want, "{name} must equal its lumina.pen value");
        }
        assert_eq!(
            hex(t3()),
            0x7C7C88FF,
            "t3 is lifted from the design's #5E5E6A for legibility (D1)"
        );
    }

    #[core::prelude::v1::test]
    fn card_surface_is_transparent() {
        assert_eq!(
            card_surface().a,
            0.0,
            "grid cards must not paint a surface behind thumbnails"
        );
    }

    #[core::prelude::v1::test]
    fn focus_ring_is_neutral_and_visible() {
        let f = focus_ring();
        assert!(
            (f.r - f.b).abs() < 0.02 && (f.g - f.b).abs() < 0.02,
            "focus is grey, not amber: amber is reserved for selection"
        );
        for bg in [bg_stage(), bg_0(), bg_1(), bg_2(), bg_3(), bg_float()] {
            assert!(
                contrast(f, bg) >= 3.0,
                "the focus border needs 3:1 against the field (WCAG 1.4.11)"
            );
        }
    }

    #[core::prelude::v1::test]
    fn pen_text_pairs_meet_aa() {
        for bg in [bg_stage(), bg_0(), bg_1(), bg_2(), bg_3(), bg_float()] {
            assert!(contrast(t1(), bg) >= 4.5, "primary text is AA everywhere");
            assert!(contrast(t2(), bg) >= 4.5, "secondary text is AA everywhere");
            assert!(
                contrast(accent(), bg) >= 4.5,
                "accent text is AA everywhere"
            );
            assert!(
                contrast(rating_unset(), bg) >= 3.0,
                "unset stars are a control and need 3:1 (WCAG 1.4.11)"
            );
        }
        for bg in [bg_0(), bg_1(), bg_2(), bg_float()] {
            assert!(contrast(danger(), bg) >= 4.5, "error text is AA on panels");
        }
        for bg in [bg_0(), bg_1()] {
            assert!(
                contrast(t3(), bg) >= 4.5,
                "section labels and counts stay AA on the canvas and sidebar"
            );
        }
        assert!(contrast(on_accent(), accent()) >= 4.5);
        assert!(
            contrast(bg_0(), danger()) >= 4.5,
            "text on a danger fill is drawn in bg_0"
        );
    }
}
