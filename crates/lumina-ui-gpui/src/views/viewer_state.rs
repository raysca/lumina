use lumina_core::normalize_quarter_turns;

/// Viewer quick-edit rotation direction.
///
/// Rotation is a display-only catalog edit: it changes how the still is
/// presented and never rewrites the original file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerRotation {
    /// Counter-clockwise 90° (decrements the stored quarter turn).
    Left,
    /// Clockwise 90° (increments the stored quarter turn).
    Right,
}

impl ViewerRotation {
    /// Signed quarter-turn delta applied to the stored 0..=3 value.
    pub fn delta(self) -> i32 {
        match self {
            Self::Left => -1,
            Self::Right => 1,
        }
    }

    /// Applies this direction to a stored quarter-turn value, wrapping 0..=3.
    pub fn apply_to(self, quarters: u8) -> u8 {
        normalize_quarter_turns(quarters as i32 + self.delta())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewerState {
    pub zoom_scale: f32,
    pub pan_offset: (f32, f32),
    pub is_dragging: bool,
    pub drag_start_mouse: (f32, f32),
    pub drag_start_pan: (f32, f32),
    /// Laid-out media canvas, captured at prepaint. `None` until painted.
    pub canvas: Option<ViewerCanvas>,
}

/// Media canvas geometry the zoom math needs, in window points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewerCanvas {
    /// Canvas size; the zoom anchor is its center.
    pub size: (f32, f32),
    pub center: (f32, f32),
    /// Media size at Fit (contained in the canvas).
    pub fitted: (f32, f32),
    /// Zoom scale at which one source pixel is one physical screen pixel.
    pub actual_scale: f32,
}

impl ViewerCanvas {
    /// Geometry for media of `media_aspect` (w / h) whose long edge has
    /// `source_long_px` pixels, shown in a canvas of `size` points.
    pub fn new(
        size: (f32, f32),
        center: (f32, f32),
        media_aspect: f32,
        source_long_px: f32,
        scale_factor: f32,
    ) -> Option<Self> {
        let valid = |v: f32| v.is_finite() && v > 0.0;
        if !(valid(size.0) && valid(size.1) && valid(media_aspect) && valid(scale_factor)) {
            return None;
        }
        let fitted = if size.0 / size.1 > media_aspect {
            (size.1 * media_aspect, size.1)
        } else {
            (size.0, size.0 / media_aspect)
        };
        let fitted_long = fitted.0.max(fitted.1);
        let actual_scale = if valid(source_long_px) {
            source_long_px / scale_factor / fitted_long
        } else {
            ZOOM_MIN_SCALE
        };
        Some(Self {
            size,
            center,
            fitted,
            actual_scale,
        })
    }
}

/// Minimum viewer zoom. 1.0 means "fit", not pixel-accurate 100%.
pub const ZOOM_MIN_SCALE: f32 = 1.0;
/// Safety cap for viewer zoom, relative to fit.
///
/// The cap is deliberately high so trackpad scrolling feels unbounded; the
/// fitted image is already 1.0, and 16x leaves headroom for pixel inspection.
pub const ZOOM_MAX_SCALE: f32 = 16.0;
/// Wheel and +/- keyboard step.
pub const ZOOM_STEP: f32 = 0.25;

/// Explicit zoom presets on the 0/1/2/4 keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerZoomPreset {
    /// Fit the media to the viewport (1.0x).
    Fit,
    /// Actual pixels: one source pixel per physical screen pixel.
    Actual,
    /// 200% of actual pixels (Lightroom's 2:1).
    TwoX,
    /// 400% of actual pixels (Lightroom's 4:1).
    FourX,
}

impl ViewerZoomPreset {
    /// Scale multiple (of Fit) applied by this preset. The non-Fit presets are
    /// multiples of actual pixels, so they agree with the zoom readout; until
    /// the canvas has painted they fall back to multiples of Fit.
    ///
    /// Zoom never goes below Fit, so for media smaller than the canvas 100%
    /// shows Fit and 200%/400% fall back to 2x/4x of Fit (the readout
    /// still shows the true percentage). The viewer decodes stills at most
    /// 2048px, so 100% of a larger photo is an enlarged decode, not a true 1:1.
    pub fn scale(self, canvas: Option<ViewerCanvas>) -> f32 {
        let actual = canvas.map_or(ZOOM_MIN_SCALE, |c| c.actual_scale);
        let of_actual = |multiple: f32| {
            if multiple * actual > ZOOM_MIN_SCALE + 0.05 {
                multiple * actual
            } else {
                multiple
            }
        };
        match self {
            Self::Fit => ZOOM_MIN_SCALE,
            Self::Actual => actual,
            Self::TwoX => of_actual(2.0),
            Self::FourX => of_actual(4.0),
        }
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            zoom_scale: ZOOM_MIN_SCALE,
            pan_offset: (0.0, 0.0),
            is_dragging: false,
            drag_start_mouse: (0.0, 0.0),
            drag_start_pan: (0.0, 0.0),
            canvas: None,
        }
    }
}

impl ViewerState {
    pub fn reset(&mut self) {
        self.zoom_scale = ZOOM_MIN_SCALE;
        self.pan_offset = (0.0, 0.0);
        self.is_dragging = false;
        self.drag_start_mouse = (0.0, 0.0);
        self.drag_start_pan = (0.0, 0.0);
    }

    pub fn set_zoom(&mut self, scale: f32) {
        self.zoom_scale = scale.clamp(ZOOM_MIN_SCALE, ZOOM_MAX_SCALE);
        if (self.zoom_scale - ZOOM_MIN_SCALE).abs() < 0.01 {
            self.zoom_scale = ZOOM_MIN_SCALE;
            self.pan_offset = (0.0, 0.0);
        } else {
            self.clamp_pan();
        }
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_scale + ZOOM_STEP);
    }

    /// Zooms by `delta` around the content point under the cursor.
    ///
    /// `cursor` is the wheel position and `canvas_center` is the center of the
    /// viewer canvas, both in window-content pixels. The renderer applies a
    /// uniform scale about the canvas center plus `pan_offset`, so the content
    /// point `anchor = cursor - canvas_center` stays under the cursor when
    ///
    /// ```text
    /// pan' = anchor + (pan - anchor) * (scale' / scale)
    /// ```
    ///
    /// Non-finite input is ignored rather than poisoning zoom/pan. Returning to
    /// Fit keeps the existing recenter rule, and any resulting pan is clamped to
    /// the same range as dragging.
    pub fn zoom_at(&mut self, delta: f32, cursor: (f32, f32), canvas_center: (f32, f32)) {
        if !delta.is_finite()
            || !cursor.0.is_finite()
            || !cursor.1.is_finite()
            || !canvas_center.0.is_finite()
            || !canvas_center.1.is_finite()
        {
            return;
        }

        // A corrupt scale should recover instead of propagating NaN into pan.
        if !self.zoom_scale.is_finite() {
            self.zoom_scale = ZOOM_MIN_SCALE;
            self.pan_offset = (0.0, 0.0);
        }

        let scale_before = self.zoom_scale;
        let scale_after = (scale_before + delta).clamp(ZOOM_MIN_SCALE, ZOOM_MAX_SCALE);
        if !scale_after.is_finite() || (scale_after - scale_before).abs() < f32::EPSILON {
            return;
        }

        let ratio = scale_after / scale_before;
        let anchor = (cursor.0 - canvas_center.0, cursor.1 - canvas_center.1);
        let pan = (
            anchor.0 + (self.pan_offset.0 - anchor.0) * ratio,
            anchor.1 + (self.pan_offset.1 - anchor.1) * ratio,
        );
        if !pan.0.is_finite() || !pan.1.is_finite() {
            return;
        }

        self.zoom_scale = scale_after;
        if (scale_after - ZOOM_MIN_SCALE).abs() < 0.01 {
            // Fit always recenters, matching `set_zoom`.
            self.zoom_scale = ZOOM_MIN_SCALE;
            self.pan_offset = (0.0, 0.0);
        } else {
            self.pan_offset = pan;
            self.clamp_pan();
        }
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_scale - ZOOM_STEP);
    }

    /// Z / double-click: Fit <-> actual pixels, like Lightroom's loupe.
    /// Media already at or below 1:1 at Fit falls back to 2x.
    pub fn toggle_zoom(&mut self) {
        if self.zoom_scale > 1.05 {
            self.reset();
        } else {
            let actual = ViewerZoomPreset::Actual.scale(self.canvas);
            self.set_zoom(if actual > 1.05 { actual } else { 2.0 });
        }
    }

    /// Applies an explicit zoom preset (the 0/1/2/4 shortcuts).
    /// Fit clears pan through `set_zoom`, matching the existing reset-on-fit rule.
    pub fn apply_preset(&mut self, preset: ViewerZoomPreset) {
        self.set_zoom(preset.scale(self.canvas));
    }

    /// Navigator readout: "Fit" at fit, otherwise the percentage of actual pixels.
    /// Before the canvas has painted, percentages are relative to fit.
    pub fn zoom_label(&self) -> String {
        let scale = if self.zoom_scale.is_finite() {
            self.zoom_scale
        } else {
            ZOOM_MIN_SCALE
        };
        if (scale - ZOOM_MIN_SCALE).abs() < 0.005 {
            "Fit".to_string()
        } else {
            let actual = self.canvas.map_or(ZOOM_MIN_SCALE, |c| c.actual_scale);
            format!("{}%", (scale / actual * 100.0).round() as i32)
        }
    }

    pub fn start_drag(&mut self, mouse_x: f32, mouse_y: f32) {
        if self.zoom_scale > ZOOM_MIN_SCALE {
            self.is_dragging = true;
            self.drag_start_mouse = (mouse_x, mouse_y);
            self.drag_start_pan = self.pan_offset;
        }
    }

    pub fn drag_to(&mut self, mouse_x: f32, mouse_y: f32) {
        if self.is_dragging && self.zoom_scale > ZOOM_MIN_SCALE {
            let dx = mouse_x - self.drag_start_mouse.0;
            let dy = mouse_y - self.drag_start_mouse.1;
            self.pan_offset = (self.drag_start_pan.0 + dx, self.drag_start_pan.1 + dy);
            self.clamp_pan();
        }
    }

    pub fn end_drag(&mut self) {
        self.is_dragging = false;
    }

    /// Pans by a delta (scroll-pan), clamped like drag-pan. No-op at Fit.
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        if self.zoom_scale > ZOOM_MIN_SCALE {
            self.pan_offset.0 += dx;
            self.pan_offset.1 += dy;
            self.clamp_pan();
        }
    }

    /// True when `point` (window points) lands on the visible media rather
    /// than the letterbox around it. Without geometry every point counts.
    pub fn hits_media(&self, point: gpui::Point<gpui::Pixels>) -> bool {
        let Some(c) = self.canvas else {
            return true;
        };
        let center = (
            c.center.0 + self.pan_offset.0,
            c.center.1 + self.pan_offset.1,
        );
        let half = (
            c.fitted.0 * self.zoom_scale * 0.5,
            c.fitted.1 * self.zoom_scale * 0.5,
        );
        (f32::from(point.x) - center.0).abs() <= half.0
            && (f32::from(point.y) - center.1).abs() <= half.1
    }

    /// Visible part of the media as `(x, y, w, h)` fractions of the whole
    /// picture, for the zoom navigator. `None` before the canvas has painted.
    pub fn visible_region(&self) -> Option<(f32, f32, f32, f32)> {
        let c = self.canvas?;
        let axis = |fitted: f32, size: f32, pan: f32| {
            let shown = fitted * self.zoom_scale;
            let start = ((shown - size) * 0.5 - pan) / shown;
            let end = ((shown + size) * 0.5 - pan) / shown;
            let (start, end) = (start.clamp(0.0, 1.0), end.clamp(0.0, 1.0));
            (start, end - start)
        };
        let (x, w) = axis(c.fitted.0, c.size.0, self.pan_offset.0);
        let (y, h) = axis(c.fitted.1, c.size.1, self.pan_offset.1);
        Some((x, y, w, h))
    }

    /// Keeps the zoomed media covering the canvas: pan never exceeds half the
    /// overflow on each axis. Without geometry it uses a generous fallback.
    fn clamp_pan(&mut self) {
        let (max_x, max_y) = match self.canvas {
            Some(c) => (
                ((c.fitted.0 * self.zoom_scale - c.size.0) * 0.5).max(0.0),
                ((c.fitted.1 * self.zoom_scale - c.size.1) * 0.5).max(0.0),
            ),
            None => {
                let max_pan = (self.zoom_scale - ZOOM_MIN_SCALE) * 800.0;
                (max_pan, max_pan)
            }
        };
        self.pan_offset.0 = self.pan_offset.0.clamp(-max_x, max_x);
        self.pan_offset.1 = self.pan_offset.1.clamp(-max_y, max_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewer_state_defaults_and_reset() {
        let mut state = ViewerState::default();
        assert_eq!(state.zoom_scale, 1.0);
        assert_eq!(state.pan_offset, (0.0, 0.0));
        assert!(!state.is_dragging);

        state.set_zoom(2.5);
        state.pan_by(50.0, -30.0);
        assert_eq!(state.zoom_scale, 2.5);
        assert_eq!(state.pan_offset, (50.0, -30.0));

        state.reset();
        assert_eq!(state.zoom_scale, 1.0);
        assert_eq!(state.pan_offset, (0.0, 0.0));
        assert!(!state.is_dragging);
    }

    #[test]
    fn test_viewer_zoom_stepping_and_clamping() {
        let mut state = ViewerState::default();

        state.zoom_in();
        assert_eq!(state.zoom_scale, 1.25);

        state.zoom_out();
        assert_eq!(state.zoom_scale, 1.0);

        state.zoom_out();
        assert_eq!(state.zoom_scale, 1.0);

        state.set_zoom(100.0);
        assert_eq!(state.zoom_scale, ZOOM_MAX_SCALE);

        state.zoom_in();
        assert_eq!(state.zoom_scale, ZOOM_MAX_SCALE);
    }

    #[test]
    fn test_viewer_toggle_zoom() {
        let mut state = ViewerState::default();
        assert_eq!(state.zoom_scale, 1.0);

        state.toggle_zoom();
        assert_eq!(state.zoom_scale, 2.0);

        state.toggle_zoom();
        assert_eq!(state.zoom_scale, 1.0);
        assert_eq!(state.pan_offset, (0.0, 0.0));
    }

    #[test]
    fn test_viewer_drag_and_pan() {
        let mut state = ViewerState::default();

        state.start_drag(100.0, 100.0);
        assert!(!state.is_dragging);

        state.set_zoom(2.0);
        state.start_drag(100.0, 100.0);
        assert!(state.is_dragging);

        state.drag_to(150.0, 120.0);
        assert_eq!(state.pan_offset, (50.0, 20.0));

        state.end_drag();
        assert!(!state.is_dragging);

        state.pan_by(-10.0, 5.0);
        assert_eq!(state.pan_offset, (40.0, 25.0));

        state.set_zoom(1.0);
        assert_eq!(state.pan_offset, (0.0, 0.0));
    }

    #[test]
    fn test_viewer_zoom_presets_apply_and_clamp() {
        let mut state = ViewerState::default();

        state.apply_preset(ViewerZoomPreset::TwoX);
        assert_eq!(state.zoom_scale, 2.0);

        state.apply_preset(ViewerZoomPreset::FourX);
        assert_eq!(state.zoom_scale, 4.0);
        state.pan_by(100.0, 100.0);
        assert_ne!(state.pan_offset, (0.0, 0.0));

        state.apply_preset(ViewerZoomPreset::Fit);
        assert_eq!(state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(state.pan_offset, (0.0, 0.0), "Fit must clear pan");

        // Presets cannot escape the supported range.
        state.set_zoom(f32::INFINITY);
        assert_eq!(state.zoom_scale, ZOOM_MAX_SCALE);
        state.set_zoom(f32::NEG_INFINITY);
        assert_eq!(state.zoom_scale, ZOOM_MIN_SCALE);
    }

    #[test]
    fn test_viewer_zoom_hud_label_formatting() {
        let mut state = ViewerState::default();
        assert_eq!(state.zoom_label(), "Fit");

        state.set_zoom(2.0);
        assert_eq!(state.zoom_label(), "200%");
        state.set_zoom(4.0);
        assert_eq!(state.zoom_label(), "400%");
        state.set_zoom(1.25);
        assert_eq!(state.zoom_label(), "125%");
        state.set_zoom(2.5);
        assert_eq!(state.zoom_label(), "250%");
        state.set_zoom(5.0);
        assert_eq!(state.zoom_label(), "500%");

        // A corrupt/non-finite scale must not render "NaN%" or "0%".
        state.zoom_scale = f32::NAN;
        assert_eq!(state.zoom_label(), "Fit");
    }

    /// Content-space coordinate currently sitting under `anchor` (cursor minus
    /// canvas center), derived from the same transform the renderer applies.
    fn anchored_content_point(state: &ViewerState, anchor: (f32, f32)) -> (f32, f32) {
        (
            (anchor.0 - state.pan_offset.0) / state.zoom_scale,
            (anchor.1 - state.pan_offset.1) / state.zoom_scale,
        )
    }

    #[test]
    fn test_viewer_zoom_at_cursor_moves_pan_toward_cursor() {
        let mut state = ViewerState::default();
        let canvas_center = (400.0, 300.0);
        // Cursor right of and above the canvas center.
        let cursor = (640.0, 180.0);
        let anchor = (cursor.0 - canvas_center.0, cursor.1 - canvas_center.1);
        let content_before = anchored_content_point(&state, anchor);

        state.zoom_at(ZOOM_STEP, cursor, canvas_center);

        assert_eq!(state.zoom_scale, 1.25);
        assert!(
            state.pan_offset.0 < 0.0,
            "zooming in with the cursor right of center must shift content left, got {:?}",
            state.pan_offset
        );
        assert!(
            state.pan_offset.1 > 0.0,
            "zooming in with the cursor above center must shift content down, got {:?}",
            state.pan_offset
        );

        let content_after = anchored_content_point(&state, anchor);
        assert!(
            (content_after.0 - content_before.0).abs() < 0.01
                && (content_after.1 - content_before.1).abs() < 0.01,
            "the content point under the cursor must stay anchored: before={content_before:?} after={content_after:?}"
        );
    }

    #[test]
    fn test_viewer_zoom_out_preserves_cursor_anchor() {
        let mut state = ViewerState::default();
        let canvas_center = (400.0, 300.0);
        let cursor = (640.0, 180.0);
        let anchor = (cursor.0 - canvas_center.0, cursor.1 - canvas_center.1);

        state.set_zoom(2.0);
        state.zoom_at(ZOOM_STEP, cursor, canvas_center);
        assert_eq!(state.zoom_scale, 2.25);
        let content_before = anchored_content_point(&state, anchor);

        state.zoom_at(-ZOOM_STEP, cursor, canvas_center);
        assert_eq!(state.zoom_scale, 2.0, "zoom out must return to 2x");
        let content_after = anchored_content_point(&state, anchor);
        assert!(
            (content_after.0 - content_before.0).abs() < 0.01
                && (content_after.1 - content_before.1).abs() < 0.01,
            "zooming out must keep the same anchor: before={content_before:?} after={content_after:?}"
        );

        // Stepping back in at the same cursor restores the previous pan exactly.
        let pan_at_two = state.pan_offset;
        state.zoom_at(ZOOM_STEP, cursor, canvas_center);
        state.zoom_at(-ZOOM_STEP, cursor, canvas_center);
        assert!(
            (state.pan_offset.0 - pan_at_two.0).abs() < 0.01
                && (state.pan_offset.1 - pan_at_two.1).abs() < 0.01,
            "in/out at one cursor must be reversible: expected {pan_at_two:?}, got {:?}",
            state.pan_offset
        );
    }

    #[test]
    fn test_viewer_zoom_at_fit_resets_pan() {
        let mut state = ViewerState::default();
        let canvas_center = (400.0, 300.0);
        let cursor = (700.0, 200.0);

        state.zoom_at(ZOOM_STEP, cursor, canvas_center);
        state.zoom_at(ZOOM_STEP, cursor, canvas_center);
        assert_eq!(state.zoom_scale, 1.5);
        assert_ne!(state.pan_offset, (0.0, 0.0), "zooming in must move pan");

        // Zooming out past Fit clamps to 1.0 and recenters, like Fit to window.
        state.zoom_at(-10.0, cursor, canvas_center);
        assert_eq!(state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(state.pan_offset, (0.0, 0.0), "Fit must clear pan");

        // Starting the next zoom from Fit is still anchored to the cursor.
        state.zoom_at(ZOOM_STEP, cursor, canvas_center);
        assert_eq!(state.zoom_scale, 1.25);
        assert!(state.pan_offset.0 < 0.0 && state.pan_offset.1 > 0.0);
    }

    #[test]
    fn test_viewer_zoom_at_clamping_stays_finite() {
        let mut state = ViewerState::default();
        let canvas_center = (400.0, 300.0);

        // A cursor far outside the canvas anchors a huge pan; the existing pan
        // clamp must bound it without producing NaN or infinity.
        state.zoom_at(ZOOM_STEP, (10_000.0, -10_000.0), canvas_center);
        let max_pan = (state.zoom_scale - ZOOM_MIN_SCALE) * 800.0;
        assert!(state.pan_offset.0.is_finite() && state.pan_offset.1.is_finite());
        assert!(state.pan_offset.0 >= -max_pan && state.pan_offset.0 <= max_pan);
        assert!(state.pan_offset.1 >= -max_pan && state.pan_offset.1 <= max_pan);

        // Zooming in at the ceiling is a no-op, and non-finite input is ignored.
        state.set_zoom(ZOOM_MAX_SCALE);
        let pan_at_max = state.pan_offset;
        state.zoom_at(ZOOM_STEP, (900.0, 700.0), canvas_center);
        assert_eq!(state.zoom_scale, ZOOM_MAX_SCALE);
        assert_eq!(state.pan_offset, pan_at_max);
        state.zoom_at(f32::NAN, (900.0, 700.0), canvas_center);
        state.zoom_at(ZOOM_STEP, (f32::INFINITY, 0.0), canvas_center);
        state.zoom_at(ZOOM_STEP, (0.0, 0.0), (f32::NAN, 0.0));
        assert_eq!(state.zoom_scale, ZOOM_MAX_SCALE);
        assert_eq!(state.pan_offset, pan_at_max);

        // A corrupt scale is repaired by the next zoomed event.
        state.zoom_scale = f32::NAN;
        state.zoom_at(-6.0, (500.0, 300.0), canvas_center);
        assert_eq!(state.zoom_scale, ZOOM_MIN_SCALE);
        assert!(state.pan_offset.0.is_finite() && state.pan_offset.1.is_finite());
    }

    #[test]
    fn test_viewer_rotation_wraps_quarter_turns() {
        assert_eq!(ViewerRotation::Right.apply_to(0), 1);
        assert_eq!(ViewerRotation::Right.apply_to(1), 2);
        assert_eq!(ViewerRotation::Right.apply_to(2), 3);
        assert_eq!(
            ViewerRotation::Right.apply_to(3),
            0,
            "Rotate Right from 3 must wrap to 0"
        );
        assert_eq!(
            ViewerRotation::Left.apply_to(0),
            3,
            "Rotate Left from 0 must wrap to 3"
        );
        assert_eq!(ViewerRotation::Left.apply_to(3), 2);
        assert_eq!(ViewerRotation::Left.apply_to(1), 0);

        let mut quarters = 0;
        for _ in 0..4 {
            quarters = ViewerRotation::Right.apply_to(quarters);
        }
        assert_eq!(quarters, 0, "four right turns are the identity");

        assert_eq!(
            ViewerRotation::Left.delta() + ViewerRotation::Right.delta(),
            0,
            "left and right must be inverse directions"
        );
    }

    #[test]
    fn test_visible_region_tracks_zoom_and_pan() {
        let mut state = ViewerState::default();
        assert_eq!(state.visible_region(), None, "no region before paint");
        // 3:2 media in 1200x600: fitted 900x600.
        state.canvas = ViewerCanvas::new((1200.0, 600.0), (600.0, 300.0), 1.5, 3000.0, 2.0);
        assert_eq!(
            state.visible_region(),
            Some((0.0, 0.0, 1.0, 1.0)),
            "at Fit the whole picture is visible"
        );
        state.set_zoom(2.0);
        let (x, y, w, h) = state.visible_region().unwrap();
        assert!((x - 1.0 / 6.0).abs() < 1e-4 && (w - 2.0 / 3.0).abs() < 1e-4);
        assert!((y - 0.25).abs() < 1e-4 && (h - 0.5).abs() < 1e-4);
        // Panning right (content moves right) reveals the left edge.
        state.pan_offset = (300.0, 0.0);
        let (x, _, w, _) = state.visible_region().unwrap();
        assert!(x.abs() < 1e-4 && (w - 2.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn test_viewer_canvas_contains_media_and_finds_actual_pixels() {
        // 3:2 media in a 1200x600 canvas is height-bound: 900x600.
        let canvas = ViewerCanvas::new((1200.0, 600.0), (600.0, 300.0), 1.5, 3000.0, 2.0)
            .expect("valid geometry");
        assert_eq!(canvas.fitted, (900.0, 600.0));
        // 3000 source px on a 2x screen is 1500pt, over a 900pt fit.
        assert!((canvas.actual_scale - 1500.0 / 900.0).abs() < 1e-4);
        assert!(ViewerCanvas::new((0.0, 600.0), (0.0, 0.0), 1.5, 3000.0, 2.0).is_none());
    }

    #[test]
    fn test_viewer_actual_preset_toggle_and_pan_clamp_use_canvas() {
        let mut state = ViewerState {
            canvas: ViewerCanvas::new((1200.0, 600.0), (600.0, 300.0), 1.5, 3000.0, 2.0),
            ..ViewerState::default()
        };
        let actual = state.canvas.unwrap().actual_scale;

        state.toggle_zoom();
        assert!(
            (state.zoom_scale - actual).abs() < 1e-4,
            "Z must toggle Fit to actual pixels"
        );
        assert_eq!(state.zoom_label(), "100%");

        state.pan_by(10_000.0, 10_000.0);
        // 900pt * 1.667 = 1500pt wide over a 1200pt canvas: 150pt each side.
        assert!((state.pan_offset.0 - 150.0).abs() < 0.1);
        assert_eq!(state.pan_offset.1, (600.0 * actual - 600.0) * 0.5);

        state.toggle_zoom();
        assert_eq!(state.zoom_scale, ZOOM_MIN_SCALE, "Z returns to Fit");
        assert!(
            state.hits_media(gpui::point(gpui::px(600.0), gpui::px(300.0))),
            "the canvas center is on the media"
        );
        assert!(
            !state.hits_media(gpui::point(gpui::px(20.0), gpui::px(300.0))),
            "the pillarbox beside height-bound media is not media"
        );
    }
}
