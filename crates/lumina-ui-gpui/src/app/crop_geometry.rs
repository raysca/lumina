//! Pure crop geometry in normalized coordinates of the frame as shown:
//! aspect presets, corner drags, arrow moves and the quarter-turn mapping
//! to the catalog's unrotated coordinates. Unit-tested here; used by
//! `app/crop.rs`.

use lumina_core::CropRect;

/// Smallest crop side, as a fraction of the frame (the catalog's minimum).
const MIN_SIDE: f32 = 0.02;
/// A rectangle this close to the whole frame is stored as "no crop".
const FULL_FRAME_EPSILON: f32 = 1e-4;
pub(crate) const FULL_FRAME: CropRect = CropRect {
    x: 0.0,
    y: 0.0,
    w: 1.0,
    h: 1.0,
};

/// Aspect presets, in the order `C` cycles through them. A ratio is in
/// pixels of the frame as shown, long side to short side, turned to match
/// the frame (a portrait photo gets a portrait 4:3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropAspect {
    /// The photo's own aspect, as shown.
    Original,
    Square,
    FourThree,
    ThreeTwo,
    SixteenNine,
    Free,
}

impl CropAspect {
    pub const ALL: [CropAspect; 6] = [
        Self::Original,
        Self::Square,
        Self::FourThree,
        Self::ThreeTwo,
        Self::SixteenNine,
        Self::Free,
    ];

    /// The name in `edit-aspect-{name}` selectors.
    pub fn name(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Square => "square",
            Self::FourThree => "4-3",
            Self::ThreeTwo => "3-2",
            Self::SixteenNine => "16-9",
            Self::Free => "free",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Original",
            Self::Square => "Square",
            Self::FourThree => "4:3",
            Self::ThreeTwo => "3:2",
            Self::SixteenNine => "16:9",
            Self::Free => "Free",
        }
    }

    /// The preset button's tooltip.
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Original => "Crop to the photo's own aspect (C cycles)",
            Self::Square => "Square crop (C cycles)",
            Self::FourThree => "4:3 crop, turned to match the photo (C cycles)",
            Self::ThreeTwo => "3:2 crop, turned to match the photo (C cycles)",
            Self::SixteenNine => "16:9 crop, turned to match the photo (C cycles)",
            Self::Free => "Free crop, any shape (C cycles)",
        }
    }

    pub(crate) fn next(self) -> Self {
        let index = Self::ALL.iter().position(|&a| a == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    /// `h = w * k` in normalized coordinates of a `frame` (pixels, as shown)
    /// for this aspect; `None` for Free.
    fn k(self, frame: (f32, f32)) -> Option<f32> {
        let long_to_short = match self {
            Self::Original => return Some(1.0),
            Self::Free => return None,
            Self::Square => 1.0,
            Self::FourThree => 4.0 / 3.0,
            Self::ThreeTwo => 1.5,
            Self::SixteenNine => 16.0 / 9.0,
        };
        let ratio = if frame.0 >= frame.1 {
            long_to_short
        } else {
            1.0 / long_to_short
        };
        Some(frame.0 / (ratio * frame.1))
    }
}

/// A rectangle corner; the dragged handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Corner; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    /// The name in `crop-handle-{name}` selectors.
    pub fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "tl",
            Self::TopRight => "tr",
            Self::BottomLeft => "bl",
            Self::BottomRight => "br",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Crop top-left corner",
            Self::TopRight => "Crop top-right corner",
            Self::BottomLeft => "Crop bottom-left corner",
            Self::BottomRight => "Crop bottom-right corner",
        }
    }

    /// Direction from the opposite (fixed) corner towards this one.
    fn dir(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (-1.0, -1.0),
            Self::TopRight => (1.0, -1.0),
            Self::BottomLeft => (-1.0, 1.0),
            Self::BottomRight => (1.0, 1.0),
        }
    }

    /// This corner of `r`.
    pub fn point(self, r: CropRect) -> (f32, f32) {
        let (dx, dy) = self.dir();
        (
            if dx > 0.0 { r.x + r.w } else { r.x },
            if dy > 0.0 { r.y + r.h } else { r.y },
        )
    }

    fn opposite(self) -> Self {
        match self {
            Self::TopLeft => Self::BottomRight,
            Self::TopRight => Self::BottomLeft,
            Self::BottomLeft => Self::TopRight,
            Self::BottomRight => Self::TopLeft,
        }
    }
}

/// Crop mode's state inside the edit session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropSession {
    pub aspect: CropAspect,
    /// Normalized coordinates of the frame as shown (rotated).
    pub rect: CropRect,
    /// The handle a mouse drag holds.
    pub dragging: Option<CropDrag>,
}

/// A handle drag: the corner, and the pointer (window points) and rect
/// when it started, so the corner moves by the pointer's travel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropDrag {
    pub corner: Corner,
    pub start: (f32, f32),
    pub rect: CropRect,
}

/// `r` in a frame turned `quarters` clockwise (the same turn as
/// `DecoderPipeline::rotate_frame`). Unrotated to shown is `quarters`;
/// shown to unrotated is `4 - quarters`. The result is kept inside the
/// frame, so float error never fails the catalog's validation.
pub fn rotate_rect(r: CropRect, quarters: u8) -> CropRect {
    let CropRect { x, y, w, h } = r;
    let turned = match quarters % 4 {
        0 => r,
        // Clockwise: the left column becomes the top row.
        1 => CropRect {
            x: 1.0 - (y + h),
            y: x,
            w: h,
            h: w,
        },
        2 => CropRect {
            x: 1.0 - (x + w),
            y: 1.0 - (y + h),
            w,
            h,
        },
        _ => CropRect {
            x: y,
            y: 1.0 - (x + w),
            w: h,
            h: w,
        },
    };
    inside(turned)
}

fn inside(r: CropRect) -> CropRect {
    // The catalog refuses `x + w > 1`; float rounding must never cause it.
    let side = |start: f32, len: f32| {
        let len = len.clamp(MIN_SIDE, 1.0);
        let mut start = start.clamp(0.0, 1.0 - len);
        while start > 0.0 && start + len > 1.0 {
            start = f32::from_bits(start.to_bits() - 1);
        }
        (start, len)
    };
    let (x, w) = side(r.x, r.w);
    let (y, h) = side(r.y, r.h);
    CropRect { x, y, w, h }
}

/// The largest centred rect of `aspect` in a `frame` (pixels, as shown).
/// Free keeps `current`.
pub fn preset_rect(aspect: CropAspect, frame: (f32, f32), current: CropRect) -> CropRect {
    let Some(k) = aspect.k(frame) else {
        return current;
    };
    // On a frame thinner than 50:1 the aspect's short side would be under
    // the minimum; the minimum wins (the rect is valid, not exact).
    let w = 1f32.min(1.0 / k).max(MIN_SIDE);
    let h = (w * k).clamp(MIN_SIDE, 1.0);
    inside(CropRect {
        x: (1.0 - w) / 2.0,
        y: (1.0 - h) / 2.0,
        w,
        h,
    })
}

/// A rect grown from the fixed corner `anchor` in direction `dir` with
/// width `w` (and height `h` when Free; with an aspect `h = w * k`), held
/// inside the frame and above the minimum side.
fn sized_from(anchor: (f32, f32), dir: (f32, f32), (w, h): (f32, f32), k: Option<f32>) -> CropRect {
    let room_w = if dir.0 > 0.0 {
        1.0 - anchor.0
    } else {
        anchor.0
    };
    let room_h = if dir.1 > 0.0 {
        1.0 - anchor.1
    } else {
        anchor.1
    };
    let (w, h) = match k {
        Some(k) => {
            let w = w
                .max(MIN_SIDE)
                .max(MIN_SIDE / k)
                .min(room_w)
                .min(room_h / k);
            (w, w * k)
        }
        // `room` can be under the minimum at the edge: min after max, never
        // `clamp` (it panics when min > max); `inside` restores the minimum.
        None => (w.max(MIN_SIDE).min(room_w), h.max(MIN_SIDE).min(room_h)),
    };
    inside(CropRect {
        x: if dir.0 > 0.0 { anchor.0 } else { anchor.0 - w },
        y: if dir.1 > 0.0 { anchor.1 } else { anchor.1 - h },
        w,
        h,
    })
}

/// `corner` dragged to `point` (normalized, may be outside the frame); the
/// opposite corner stays put. With an aspect the rect covers the pointer.
pub fn drag_corner(
    r: CropRect,
    corner: Corner,
    point: (f32, f32),
    aspect: CropAspect,
    frame: (f32, f32),
) -> CropRect {
    let anchor = corner.opposite().point(r);
    let dir = corner.dir();
    let w = (point.0 - anchor.0) * dir.0;
    let h = (point.1 - anchor.1) * dir.1;
    let k = aspect.k(frame);
    let w = k.map_or(w, |k| w.max(h / k));
    sized_from(anchor, dir, (w, h), k)
}

/// Arrow keys: moves `r` by `(dx, dy)`, inside the frame.
pub fn nudge(r: CropRect, (dx, dy): (f32, f32)) -> CropRect {
    CropRect {
        x: (r.x + dx).min(1.0 - r.w).max(0.0),
        y: (r.y + dy).min(1.0 - r.h).max(0.0),
        ..r
    }
}

/// ⌥ + arrows: moves the right edge by `dw` or the bottom edge by `dh`,
/// the top-left corner fixed; an aspect resizes the other side to match.
pub fn resize(
    r: CropRect,
    (dw, dh): (f32, f32),
    aspect: CropAspect,
    frame: (f32, f32),
) -> CropRect {
    let k = aspect.k(frame);
    let w = match k {
        Some(k) if dh != 0.0 => (r.h + dh) / k,
        _ => r.w + dw,
    };
    sized_from((r.x, r.y), (1.0, 1.0), (w, r.h + dh), k)
}

pub(crate) fn is_full_frame(r: CropRect) -> bool {
    [r.x, r.y, 1.0 - r.w, 1.0 - r.h]
        .iter()
        .all(|v| v.abs() < FULL_FRAME_EPSILON)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> CropRect {
        CropRect { x, y, w, h }
    }

    fn close(a: CropRect, b: CropRect) -> bool {
        [a.x - b.x, a.y - b.y, a.w - b.w, a.h - b.h]
            .iter()
            .all(|d| d.abs() < 1e-5)
    }

    #[core::prelude::v1::test]
    fn rotate_rect_matches_each_quarter_turn() {
        // The unrotated top-left quarter-strip: x 0.1..0.4, y 0.2..0.7.
        let r = rect(0.1, 0.2, 0.3, 0.5);
        let expected = [
            r,
            // Clockwise: the top edge goes to the right.
            rect(0.3, 0.1, 0.5, 0.3),
            rect(0.6, 0.3, 0.3, 0.5),
            // Counter-clockwise: the top edge goes to the left.
            rect(0.2, 0.6, 0.5, 0.3),
        ];
        for (quarters, want) in expected.into_iter().enumerate() {
            let got = rotate_rect(r, quarters as u8);
            assert!(close(got, want), "turn {quarters}: {got:?} != {want:?}");
        }
    }

    #[core::prelude::v1::test]
    fn rotate_rect_round_trips_for_each_quarter_turn() {
        let rects = [
            rect(0.1, 0.2, 0.3, 0.5),
            rect(0.0, 0.0, 1.0, 1.0),
            rect(0.98, 0.0, 0.02, 1.0),
            rect(0.25, 0.5, 0.5, 0.5),
        ];
        for r in rects {
            for quarters in 0..4u8 {
                let shown = rotate_rect(r, quarters);
                let back = rotate_rect(shown, (4 - quarters) % 4);
                assert!(close(back, r), "turn {quarters} must round-trip {r:?}");
                assert!(
                    lumina_core::CropRect::validate(&shown).is_ok(),
                    "turned rect {shown:?} must stay valid"
                );
            }
        }
    }

    #[core::prelude::v1::test]
    fn shown_left_half_of_a_clockwise_turn_is_the_unrotated_bottom_half() {
        let stored = rotate_rect(rect(0.0, 0.0, 0.5, 1.0), 3);
        assert!(close(stored, rect(0.0, 0.5, 1.0, 0.5)), "{stored:?}");
    }

    #[core::prelude::v1::test]
    fn presets_centre_the_largest_rect_in_pixels() {
        let frame = (600.0, 400.0);
        let square = preset_rect(CropAspect::Square, frame, FULL_FRAME);
        assert!(
            (square.w * 600.0 - square.h * 400.0).abs() < 1e-3,
            "square in pixels"
        );
        assert!((square.h - 1.0).abs() < 1e-6, "as tall as the frame");
        assert!((square.x - (1.0 - square.w) / 2.0).abs() < 1e-6, "centred");
        let wide = preset_rect(CropAspect::SixteenNine, frame, FULL_FRAME);
        assert!((wide.w * 600.0 / (wide.h * 400.0) - 16.0 / 9.0).abs() < 1e-3);
        assert!((wide.w - 1.0).abs() < 1e-6, "as wide as the frame");
        // A portrait frame gets a portrait 4:3.
        let tall = preset_rect(CropAspect::FourThree, (400.0, 600.0), FULL_FRAME);
        assert!((tall.h * 600.0 / (tall.w * 400.0) - 4.0 / 3.0).abs() < 1e-3);
        assert!(close(
            preset_rect(CropAspect::Original, frame, square),
            FULL_FRAME
        ));
        assert!(close(preset_rect(CropAspect::Free, frame, square), square));
    }

    #[core::prelude::v1::test]
    fn drag_keeps_aspect_frame_and_minimum() {
        let frame = (600.0, 400.0);
        let square = preset_rect(CropAspect::Square, frame, FULL_FRAME);
        let dragged = drag_corner(
            square,
            Corner::BottomRight,
            (0.5, 0.6),
            CropAspect::Square,
            frame,
        );
        assert!(
            (dragged.w * 600.0 - dragged.h * 400.0).abs() < 1e-3,
            "still square"
        );
        assert!(close(
            rect(dragged.x, dragged.y, 0.0, 0.0),
            rect(square.x, square.y, 0.0, 0.0)
        ));
        let out = drag_corner(
            FULL_FRAME,
            Corner::TopLeft,
            (-3.0, -3.0),
            CropAspect::Free,
            frame,
        );
        assert!(close(out, FULL_FRAME), "a drag past the edge stops at it");
        let crossed = drag_corner(
            FULL_FRAME,
            Corner::BottomRight,
            (-1.0, -1.0),
            CropAspect::Free,
            frame,
        );
        assert!(
            close(crossed, rect(0.0, 0.0, MIN_SIDE, MIN_SIDE)),
            "{crossed:?}"
        );
    }

    #[core::prelude::v1::test]
    fn nudge_and_resize_stay_inside_the_frame() {
        let r = rect(0.5, 0.5, 0.4, 0.4);
        assert!(close(nudge(r, (0.5, -0.9)), rect(0.6, 0.0, 0.4, 0.4)));
        let grown = resize(r, (0.5, 0.0), CropAspect::Free, (100.0, 100.0));
        assert!(close(grown, rect(0.5, 0.5, 0.5, 0.4)));
        let shrunk = resize(r, (-1.0, 0.0), CropAspect::Free, (100.0, 100.0));
        assert!(close(shrunk, rect(0.5, 0.5, MIN_SIDE, 0.4)));
        let square = resize(r, (0.0, -0.1), CropAspect::Square, (100.0, 100.0));
        assert!(close(square, rect(0.5, 0.5, 0.3, 0.3)), "{square:?}");
    }

    /// Regression: a rect at the edge with less than the minimum side of
    /// room left made `f32::clamp` panic (min > max) on the UI thread.
    #[core::prelude::v1::test]
    fn edge_rects_never_panic_and_stay_valid() {
        let frame = (600.0, 400.0);
        let cases = [
            resize(
                rect(0.98, 0.0, 0.02, 1.0),
                (0.01, 0.0),
                CropAspect::Free,
                frame,
            ),
            resize(
                rect(0.0, 0.98, 1.0, 0.02),
                (0.0, 0.01),
                CropAspect::Free,
                frame,
            ),
            drag_corner(
                rect(0.98, 0.98, 0.02, 0.02),
                Corner::BottomRight,
                (2.0, 2.0),
                CropAspect::Free,
                frame,
            ),
            drag_corner(
                rect(0.0, 0.0, 0.02, 0.02),
                Corner::TopLeft,
                (-2.0, -2.0),
                CropAspect::Free,
                frame,
            ),
            nudge(rect(0.0, 0.0, 1.0, 1.0), (0.5, 0.5)),
        ];
        for r in cases {
            assert!(r.validate().is_ok(), "{r:?} must stay a valid crop");
        }
    }

    /// A panorama's largest Square is under the 2 % minimum width: the
    /// minimum wins, so the rect is valid but wider than square.
    #[core::prelude::v1::test]
    fn panorama_square_keeps_the_minimum_side() {
        let r = preset_rect(CropAspect::Square, (10_000.0, 100.0), FULL_FRAME);
        assert!(r.validate().is_ok(), "{r:?}");
        assert!(close(r, rect(0.49, 0.0, MIN_SIDE, 1.0)), "{r:?}");
    }
}
