//! Physics validator — server-authoritative velocity + position bounds.

use ashfall_core::constants::{MAX_SCALE, MAX_SPEED, MAX_TELEPORT_DISTANCE, MIN_SCALE};
use ashfall_core::math::is_valid_pos;

/// Validate physics updates from clients.
pub struct PhysicsValidator;

impl PhysicsValidator {
    /// Check if a position update is valid (no NaN, within world bounds, no teleport).
    pub fn validate_position(pos: [f32; 3]) -> bool {
        is_valid_pos(pos)
    }

    /// Check if a velocity is within sane bounds (anti-speed-hack).
    pub fn validate_velocity(vel: [f32; 3]) -> bool {
        let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
        if !speed.is_finite() {
            return false;
        }
        speed < MAX_SPEED
    }

    /// Check if a position delta is within teleport bounds.
    pub fn validate_delta(prev: [f32; 3], next: [f32; 3]) -> bool {
        let dx = next[0] - prev[0];
        let dy = next[1] - prev[1];
        let dz = next[2] - prev[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        dist < MAX_TELEPORT_DISTANCE
    }

    /// Check if scale is within valid bounds.
    pub fn validate_scale(scale: f32) -> bool {
        scale.is_finite() && (MIN_SCALE..=MAX_SCALE).contains(&scale)
    }

    /// Validate a full physics state update.
    pub fn validate_all(pos: [f32; 3], vel: [f32; 3], prev_pos: Option<[f32; 3]>) -> bool {
        if !Self::validate_position(pos) || !Self::validate_velocity(vel) {
            return false;
        }
        if let Some(prev) = prev_pos {
            if !Self::validate_delta(prev, pos) {
                tracing::warn!(
                    "Physics: teleport rejected — {prev:?} → {pos:?} distance exceeds limit"
                );
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_bounds() {
        assert!(PhysicsValidator::validate_velocity([0.0, 0.0, 0.0]));
        assert!(PhysicsValidator::validate_velocity([500.0, 0.0, 0.0]));
        assert!(!PhysicsValidator::validate_velocity([MAX_SPEED, 0.0, 0.0]));
        assert!(!PhysicsValidator::validate_velocity([f32::NAN, 0.0, 0.0]));
        assert!(!PhysicsValidator::validate_velocity([f32::INFINITY; 3]));
    }

    #[test]
    fn teleport_delta_bounds() {
        assert!(PhysicsValidator::validate_delta(
            [0.0; 3],
            [100.0, 0.0, 0.0]
        ));
        assert!(!PhysicsValidator::validate_delta(
            [0.0; 3],
            [MAX_TELEPORT_DISTANCE, 0.0, 0.0]
        ));
    }

    #[test]
    fn scale_bounds() {
        assert!(PhysicsValidator::validate_scale(1.0));
        assert!(PhysicsValidator::validate_scale(MIN_SCALE));
        assert!(!PhysicsValidator::validate_scale(MIN_SCALE - 0.01));
        assert!(!PhysicsValidator::validate_scale(MAX_SCALE + 1.0));
        assert!(!PhysicsValidator::validate_scale(f32::NAN));
    }

    #[test]
    fn validate_all_combines() {
        // good pos + good vel + small delta
        assert!(PhysicsValidator::validate_all(
            [10.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            Some([0.0; 3])
        ));
        // bad velocity fails
        assert!(!PhysicsValidator::validate_all(
            [10.0, 0.0, 0.0],
            [MAX_SPEED; 3],
            Some([0.0; 3])
        ));
        // teleport delta fails
        assert!(!PhysicsValidator::validate_all(
            [MAX_TELEPORT_DISTANCE + 1.0, 0.0, 0.0],
            [0.0; 3],
            Some([0.0; 3])
        ));
        // no previous position → delta skipped
        assert!(PhysicsValidator::validate_all(
            [10.0, 0.0, 0.0],
            [0.0; 3],
            None
        ));
    }
}
