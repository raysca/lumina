#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewerState {
    pub zoom_scale: f32,
    pub pan_offset: (f32, f32),
    pub is_dragging: bool,
    pub drag_start_mouse: (f32, f32),
    pub drag_start_pan: (f32, f32),
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            zoom_scale: 1.0,
            pan_offset: (0.0, 0.0),
            is_dragging: false,
            drag_start_mouse: (0.0, 0.0),
            drag_start_pan: (0.0, 0.0),
        }
    }
}

impl ViewerState {
    pub fn reset(&mut self) {
        self.zoom_scale = 1.0;
        self.pan_offset = (0.0, 0.0);
        self.is_dragging = false;
        self.drag_start_mouse = (0.0, 0.0);
        self.drag_start_pan = (0.0, 0.0);
    }

    pub fn set_zoom(&mut self, scale: f32) {
        self.zoom_scale = scale.clamp(1.0, 5.0);
        if (self.zoom_scale - 1.0).abs() < 0.01 {
            self.zoom_scale = 1.0;
            self.pan_offset = (0.0, 0.0);
        } else {
            self.clamp_pan();
        }
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_scale + 0.25);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_scale - 0.25);
    }

    pub fn toggle_zoom(&mut self) {
        if self.zoom_scale > 1.05 {
            self.reset();
        } else {
            self.set_zoom(2.0);
        }
    }

    pub fn start_drag(&mut self, mouse_x: f32, mouse_y: f32) {
        if self.zoom_scale > 1.0 {
            self.is_dragging = true;
            self.drag_start_mouse = (mouse_x, mouse_y);
            self.drag_start_pan = self.pan_offset;
        }
    }

    pub fn drag_to(&mut self, mouse_x: f32, mouse_y: f32) {
        if self.is_dragging && self.zoom_scale > 1.0 {
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
        if self.zoom_scale > 1.0 {
            self.pan_offset.0 += dx;
            self.pan_offset.1 += dy;
            self.clamp_pan();
        }
    }

    fn clamp_pan(&mut self) {
        let max_pan = (self.zoom_scale - 1.0) * 800.0;
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
}
