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
    a.is_finite() && (-360.0..=360.0).contains(&a)
}

/// Check if a 3D position is valid.
#[inline]
pub fn is_valid_pos(pos: [f32; 3]) -> bool {
    is_valid_coordinate(pos[0]) && is_valid_coordinate(pos[1]) && is_valid_coordinate(pos[2])
}

/// Check if a 3D angle is valid.
#[inline]
pub fn is_valid_angle3(angle: [f32; 3]) -> bool {
    is_valid_angle(0, angle[0]) && is_valid_angle(1, angle[1]) && is_valid_angle(2, angle[2])
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_coordinates_accepted() {
        assert!(is_valid_pos([0.0, 0.0, 0.0]));
        assert!(is_valid_pos([299_999.0, -299_999.0, 123.5]));
    }

    #[test]
    fn nan_inf_out_of_range_rejected() {
        assert!(!is_valid_pos([f32::NAN, 0.0, 0.0]));
        assert!(!is_valid_pos([f32::INFINITY, 0.0, 0.0]));
        assert!(!is_valid_pos([300_001.0, 0.0, 0.0]));
        assert!(!is_valid_pos([-300_001.0, 0.0, 0.0]));
    }

    #[test]
    fn angle_bounds_enforced() {
        assert!(is_valid_angle3([0.0, 90.0, 180.0]));
        assert!(is_valid_angle3([-360.0, 360.0, 0.0]));
        assert!(!is_valid_angle3([361.0, 0.0, 0.0]));
        assert!(!is_valid_angle3([f32::NAN, 0.0, 0.0]));
    }

    #[test]
    fn distance_and_near() {
        assert_eq!(distance([0.0, 0.0, 0.0], [3.0, 4.0, 0.0]), 5.0);
        assert!(is_near([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], 2.0));
        assert!(!is_near([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 2.0));
    }

    #[test]
    fn vault_vector_conversions() {
        let v = VaultVector::new(1.0, 2.0, 3.0);
        assert_eq!(v.as_tuple(), (1.0, 2.0, 3.0));
        let p = VaultVector::from_pos([4.0, 5.0, 6.0]);
        assert_eq!(p.as_tuple(), (4.0, 5.0, 6.0));
    }
}
