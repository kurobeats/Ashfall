//! Top-down world view — projects world (x, z) onto a 2D canvas.
//!
//! The game world is the XZ plane (Y is up); the view maps X → right,
//! Z → up, centered on the local player (or the origin before spawn).
//! Pure math — unit-testable without a display; the egui painter in
//! `ui/app.rs` renders through this.

/// Screen-space projection for a world view.
#[derive(Debug, Clone, Copy)]
pub struct WorldView {
    /// World (x, z) at the viewport center.
    pub center: [f32; 2],
    /// Pixels (points) per world unit.
    pub scale: f32,
    /// Viewport size in points.
    pub size: [f32; 2],
}

impl WorldView {
    /// Center on a world position, keeping the current scale/size.
    pub fn centered_on(center: [f32; 2], scale: f32, size: [f32; 2]) -> Self {
        WorldView {
            center,
            scale,
            size,
        }
    }

    /// Viewport center in screen points.
    pub fn screen_center(&self) -> [f32; 2] {
        [self.size[0] / 2.0, self.size[1] / 2.0]
    }

    /// Project a world (x, z) to screen points.
    ///
    /// X maps right (+x screen), Z maps up (−y screen, since screen y grows
    /// downward).
    pub fn world_to_screen(&self, x: f32, z: f32) -> [f32; 2] {
        let [cx, cz] = self.center;
        let [sx, sy] = self.screen_center();
        [sx + (x - cx) * self.scale, sy - (z - cz) * self.scale]
    }

    /// World (x, z) at a screen point (inverse projection).
    /// ponytail: kept for click-to-teleport interactivity, wired when the
    /// view gets mouse input.
    #[allow(dead_code)]
    pub fn screen_to_world(&self, px: f32, py: f32) -> [f32; 2] {
        let [cx, cz] = self.center;
        let [sx, sy] = self.screen_center();
        [cx + (px - sx) / self.scale, cz + (sy - py) / self.scale]
    }

    /// Zoom the view by `factor` (>1 zoom in) keeping the center fixed.
    /// ponytail: kept for scroll-wheel zoom, wired when the view gets input.
    #[allow(dead_code)]
    pub fn zoom(&mut self, factor: f32) {
        self.scale = (self.scale * factor).clamp(0.01, 1000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_projects_to_screen_center() {
        let v = WorldView::centered_on([100.0, -50.0], 2.0, [800.0, 600.0]);
        assert_eq!(v.world_to_screen(100.0, -50.0), [400.0, 300.0]);
    }

    #[test]
    fn positive_x_maps_right() {
        let v = WorldView::centered_on([0.0, 0.0], 1.0, [100.0, 100.0]);
        // World +x → right of center.
        let p = v.world_to_screen(10.0, 0.0);
        assert_eq!(p, [60.0, 50.0]);
    }

    #[test]
    fn positive_z_maps_up() {
        let v = WorldView::centered_on([0.0, 0.0], 1.0, [100.0, 100.0]);
        // World +z → up = smaller screen y.
        let p = v.world_to_screen(0.0, 10.0);
        assert_eq!(p, [50.0, 40.0]);
    }

    #[test]
    fn scale_scales_distances() {
        let v = WorldView::centered_on([0.0, 0.0], 2.0, [100.0, 100.0]);
        let a = v.world_to_screen(0.0, 0.0);
        let b = v.world_to_screen(5.0, 0.0);
        assert_eq!(b[0] - a[0], 10.0, "5 world units * scale 2 = 10 px");
    }

    #[test]
    fn inverse_round_trip() {
        let v = WorldView::centered_on([123.0, -456.0], 3.5, [640.0, 480.0]);
        let w = v.screen_to_world(100.0, 200.0);
        let back = v.world_to_screen(w[0], w[1]);
        assert!((back[0] - 100.0).abs() < 1e-4);
        assert!((back[1] - 200.0).abs() < 1e-4);
    }

    #[test]
    fn zoom_keeps_center() {
        let mut v = WorldView::centered_on([50.0, 50.0], 1.0, [200.0, 200.0]);
        let center = v.world_to_screen(50.0, 50.0);
        v.zoom(2.0);
        assert_eq!(v.world_to_screen(50.0, 50.0), center);
        assert_eq!(v.scale, 2.0);
        v.zoom(0.001);
        assert!(v.scale >= 0.01, "scale clamped");
    }
}
