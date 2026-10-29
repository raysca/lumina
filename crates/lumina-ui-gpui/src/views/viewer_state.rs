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
    /// Both controls in top-bar and context-menu order.
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    /// Signed quarter-turn delta applied to the stored 0..=3 value.
    pub fn delta(self) -> i32 {
        match self {
            Self::Left => -1,
            Self::Right => 1,
        }
    }

    /// Menu and tooltip label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Rotate Left",
            Self::Right => "Rotate Right",
        }
    }

    /// Tooltip text; the display-only hint keeps the non-destructive promise
    /// visible next to the control.
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Left => "Rotate Left 90° (display only)",
            Self::Right => "Rotate Right 90° (display only)",
        }
    }

    /// Stable automation selector for this control.
    pub fn debug_selector(self) -> &'static str {
        match self {
            Self::Left => "quick-edit-rotate-left",
            Self::Right => "quick-edit-rotate-right",
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
}

/// Minimum viewer zoom. 1.0 means "fit", not pixel-accurate 100%.
pub const ZOOM_MIN_SCALE: f32 = 1.0;
/// Maximum viewer zoom multiple relative to fit.
pub const ZOOM_MAX_SCALE: f32 = 5.0;
/// Wheel and +/- keyboard step.
pub const ZOOM_STEP: f32 = 0.25;

/// Explicit zoom presets surfaced by the viewer HUD and number keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewerZoomPreset {
    /// Fit the media to the viewport (1.0x).
    Fit,
    /// Twice the fit scale.
    TwoX,
    /// Four times the fit scale.
    FourX,
}

impl ViewerZoomPreset {
    /// Every preset in HUD order.
    pub const ALL: [Self; 3] = [Self::Fit, Self::TwoX, Self::FourX];

    /// Scale multiple applied by this preset.
    pub fn scale(self) -> f32 {
        match self {
            Self::Fit => ZOOM_MIN_SCALE,
            Self::TwoX => 2.0,
            Self::FourX => 4.0,
        }
    }

    /// Compact HUD label for the preset itself.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fit => "Fit",
            Self::TwoX => "2x",
            Self::FourX => "4x",
        }
    }

    /// Single-key viewer shortcut for this preset. `0` remains reset.
    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Fit => "1",
            Self::TwoX => "2",
            Self::FourX => "4",
        }
    }

    /// Tooltip text pairing the label with its shortcut.
    pub fn tooltip(self) -> String {
        match self {
            Self::Fit => format!("Fit ({})", self.shortcut()),
            Self::TwoX => format!("2x zoom ({})", self.shortcut()),
            Self::FourX => format!("4x zoom ({})", self.shortcut()),
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

    pub fn toggle_zoom(&mut self) {
        if self.zoom_scale > 1.05 {
            self.reset();
        } else {
            self.set_zoom(2.0);
        }
    }

    /// Applies an explicit zoom preset (HUD buttons and 1/2/4 shortcuts).
    /// Fit clears pan through `set_zoom`, matching the existing reset-on-fit rule.
    pub fn apply_preset(&mut self, preset: ViewerZoomPreset) {
        self.set_zoom(preset.scale());
    }

    /// True when the current zoom sits on the given preset.
    pub fn matches_preset(&self, preset: ViewerZoomPreset) -> bool {
        (self.zoom_scale - preset.scale()).abs() < 0.005
    }

    /// HUD label: "Fit" at fit, otherwise the percentage relative to fit.
    ///
    /// 1.0 is not a pixel-accurate 100%, so percentages are explicitly relative
    /// to the fitted media size.
    pub fn zoom_label(&self) -> String {
        let scale = if self.zoom_scale.is_finite() {
            self.zoom_scale
        } else {
            ZOOM_MIN_SCALE
        };
        if (scale - ZOOM_MIN_SCALE).abs() < 0.005 {
            "Fit".to_string()
        } else {
            format!("{}%", (scale * 100.0).round() as i32)
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

    #[cfg(test)]
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        if self.zoom_scale > ZOOM_MIN_SCALE {
            self.pan_offset.0 += dx;
            self.pan_offset.1 += dy;
            self.clamp_pan();
        }
    }

    fn clamp_pan(&mut self) {
        let max_pan = (self.zoom_scale - ZOOM_MIN_SCALE) * 800.0;
        self.pan_offset.0 = self.pan_offset.0.clamp(-max_pan, max_pan);
        self.pan_offset.1 = self.pan_offset.1.clamp(-max_pan, max_pan);
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

        state.set_zoom(10.0);
        assert_eq!(state.zoom_scale, 5.0);

        state.zoom_in();
        assert_eq!(state.zoom_scale, 5.0);
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
        assert!(state.matches_preset(ViewerZoomPreset::TwoX));
        assert!(!state.matches_preset(ViewerZoomPreset::FourX));

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

        // Zooming out past Fit clamps to 1.0 and recenters, like the HUD reset.
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
            ViewerRotation::ALL
                .into_iter()
                .map(ViewerRotation::debug_selector)
                .collect::<Vec<_>>(),
            vec!["quick-edit-rotate-left", "quick-edit-rotate-right"],
            "the automation contract must stay stable"
        );
        assert_eq!(
            ViewerRotation::Left.delta() + ViewerRotation::Right.delta(),
            0,
            "left and right must be inverse directions"
        );
    }
}
