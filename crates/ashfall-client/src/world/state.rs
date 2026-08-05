//! Derived render state — interpolation helpers.
//!
//! ponytail: unused by the stub-mode client; used once position rendering lands.
#![allow(dead_code)]

pub fn interpolate_position(last: [f32; 3], current: [f32; 3], t: f32) -> [f32; 3] {
    [
        last[0] + (current[0] - last[0]) * t,
        last[1] + (current[1] - last[1]) * t,
        last[2] + (current[2] - last[2]) * t,
    ]
}
