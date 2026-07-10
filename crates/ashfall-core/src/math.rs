//! Math utilities — coordinates, vectors, validation.

/// 3D vector used for positions and angles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VaultVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl VaultVector {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        VaultVector { x, y, z }
    }

    pub fn as_tuple(&self) -> (f32, f32, f32) {
        (self.x, self.y, self.z)
    }

    pub fn from_pos(pos: [f32; 3]) -> Self {
        VaultVector::new(pos[0], pos[1], pos[2])
    }
}

/// Check if a single coordinate is valid (not NaN, not infinite, in valid range).
#[inline]
pub fn is_valid_coordinate(c: f32) -> bool {
    c.is_finite() && c > -300_000.0 && c < 300_000.0
}

/// Check if an axis angle is valid.
#[inline]
pub fn is_valid_angle(_axis: u8, a: f32) -> bool {
    a.is_finite() && a >= -360.0 && a <= 360.0
}

/// Check if a 3D position is valid.
#[inline]
pub fn is_valid_pos(pos: [f32; 3]) -> bool {
    is_valid_coordinate(pos[0])
        && is_valid_coordinate(pos[1])
        && is_valid_coordinate(pos[2])
}

/// Check if a 3D angle is valid.
#[inline]
pub fn is_valid_angle3(angle: [f32; 3]) -> bool {
    is_valid_angle(0, angle[0])
        && is_valid_angle(1, angle[1])
        && is_valid_angle(2, angle[2])
}

/// Distance between two 3D points.
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Check if a is near b within radius r.
#[inline]
pub fn is_near(a: [f32; 3], b: [f32; 3], r: f32) -> bool {
    distance(a, b) <= r
}
