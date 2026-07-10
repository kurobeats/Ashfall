//! Derived render state — interpolation helpers.
//!
//! ponytail: stubs — full interpolation in Phase 9.

pub fn interpolate_position(last: [f32; 3], current: [f32; 3], t: f32) -> [f32; 3] {
    [
        last[0] + (current[0] - last[0]) * t,
        last[1] + (current[1] - last[1]) * t,
        last[2] + (current[2] - last[2]) * t,
    ]
}
