//! Derived render state — interpolation helpers.
//!
//! Remote object positions are interpolated between the last two received
//! updates (100ms blend window) so rendering is smooth despite the 30Hz
//! update rate.

/// Blend window: how long a position update takes to interpolate across.
pub const INTERP_WINDOW_MS: f32 = 100.0;

/// Linear interpolation factor for `t` in [0, 1].
pub fn interpolate_position(last: [f32; 3], current: [f32; 3], t: f32) -> [f32; 3] {
    [
        last[0] + (current[0] - last[0]) * t,
        last[1] + (current[1] - last[1]) * t,
        last[2] + (current[2] - last[2]) * t,
    ]
}

/// Interpolation alpha for an update received `elapsed_ms` ago.
/// 0 → exactly at the old position, 1 → exactly at the new one; clamps
/// past the window (hold the latest position, no extrapolation).
pub fn interpolation_alpha(elapsed_ms: f32) -> f32 {
    (elapsed_ms / INTERP_WINDOW_MS).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lerp_midpoint() {
        let p = interpolate_position([0.0, 0.0, 0.0], [100.0, 200.0, 0.0], 0.5);
        assert_eq!(p, [50.0, 100.0, 0.0]);
    }

    #[test]
    fn test_lerp_endpoints() {
        let a = [1.0, 2.0, 3.0];
        let b = [9.0, 9.0, 9.0];
        assert_eq!(interpolate_position(a, b, 0.0), a);
        assert_eq!(interpolate_position(a, b, 1.0), b);
    }

    #[test]
    fn test_alpha_clamps() {
        assert_eq!(interpolation_alpha(-5.0), 0.0);
        assert_eq!(interpolation_alpha(0.0), 0.0);
        assert_eq!(interpolation_alpha(50.0), 0.5);
        assert_eq!(interpolation_alpha(150.0), 1.0, "clamped, no extrapolation");
    }
}
