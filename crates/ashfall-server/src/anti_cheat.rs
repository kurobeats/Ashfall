//! Anti-cheat validator — server-side checks on all client input.
//!
//! Each validation is a pure function — no side effects, just boolean checks.
//! Rejected inputs are logged and dropped silently.
//!
//! ponytail: this exists because clients run untrusted game binaries.
//! Every single position, velocity, item count, and damage value goes through
//! these checks before the server acts on it.

use ashfall_core::constants::{MAX_SPEED, MAX_TELEPORT_DISTANCE, MAX_ITEM_STACK, MIN_SCALE, MAX_SCALE};
use ashfall_core::math::{distance, is_valid_pos};
use std::time::Duration;

pub struct AntiCheat;

impl AntiCheat {
    // ── Position ──

    /// Validate position update — reject NaN, infinite, teleport, speed hack.
    pub fn validate_position(pos: [f32; 3], prev_pos: Option<[f32; 3]>, delta_time: Duration) -> bool {
        if !is_valid_pos(pos) {
            return false;
        }
        if let Some(prev) = prev_pos {
            let dist = distance(pos, prev);
            // Teleport check
            if dist > MAX_TELEPORT_DISTANCE {
                tracing::warn!("AntiCheat: teleport rejected — {dist:.0} units");
                return false;
            }
            // Speed hack check (dist / time > MAX_SPEED)
            let dt = delta_time.as_secs_f32().max(0.001);
            let speed = dist / dt;
            if speed > MAX_SPEED {
                tracing::warn!("AntiCheat: speed hack rejected — {speed:.0} u/s (dist={dist:.0}, dt={dt:.3}s)");
                return false;
            }
        }
        true
    }

    // ── Velocity ──

    /// Validate velocity — no NaN, no insane speeds.
    pub fn validate_velocity(vel: [f32; 3]) -> bool {
        let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
        speed.is_finite() && speed < MAX_SPEED
    }

    // ── Items ──

    /// Validate item count — reject negative or insanely high stacks.
    pub fn validate_item_count(count: u32) -> bool {
        count <= MAX_ITEM_STACK
    }

    // ── Scale ──

    /// Validate scale — within sane bounds.
    pub fn validate_scale(scale: f32) -> bool {
        scale.is_finite() && scale >= MIN_SCALE && scale <= MAX_SCALE
    }

    // ── Damage ──

    /// Validate damage bounds — no negative, zero, or absurdly large hits.
    pub fn validate_damage(damage: f32) -> bool {
        damage > 0.0 && damage < 10000.0
    }

    // ── Sequence / Anti-replay ──

    /// Validate packet sequence number — anti-replay check.
    /// Uses wrapping comparison for u16 to handle overflow.
    pub fn validate_sequence(seq: u16, last_seq: Option<u16>) -> bool {
        match last_seq {
            Some(last) => {
                // Wrapping: seq is newer if (seq - last) in 1..32768
                seq.wrapping_sub(last) > 0 && seq.wrapping_sub(last) < 32768
            }
            None => true,
        }
    }

    /// Track the last seen sequence for a session.
    pub fn update_sequence(last_seq: &mut Option<u16>, seq: u16) {
        *last_seq = Some(seq);
    }

    // ── FormID ──

    /// Validate FormID — reject obvious spoofs (0x00000000, all-FF).
    pub fn validate_form_id(form_id: u32) -> bool {
        form_id != 0 && form_id != 0xFFFFFFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_position_normal() {
        assert!(AntiCheat::validate_position([100.0, 0.0, 0.0], Some([0.0, 0.0, 0.0]), Duration::from_millis(100)));
    }

    #[test]
    fn test_validate_position_teleport_rejected() {
        assert!(!AntiCheat::validate_position([20000.0, 0.0, 0.0], Some([0.0, 0.0, 0.0]), Duration::from_millis(100)));
    }

    #[test]
    fn test_validate_position_speed_hack() {
        // 10000 units in 1ms = 10M u/s
        assert!(!AntiCheat::validate_position([10000.0, 0.0, 0.0], Some([0.0, 0.0, 0.0]), Duration::from_millis(1)));
    }

    #[test]
    fn test_validate_position_nan_rejected() {
        assert!(!AntiCheat::validate_position([f32::NAN, 0.0, 0.0], None, Duration::from_millis(100)));
    }

    #[test]
    fn test_validate_velocity_normal() {
        assert!(AntiCheat::validate_velocity([100.0, 0.0, 0.0]));
    }

    #[test]
    fn test_validate_velocity_speed_hack() {
        assert!(!AntiCheat::validate_velocity([10000.0, 0.0, 0.0]));
    }

    #[test]
    fn test_validate_velocity_nan() {
        assert!(!AntiCheat::validate_velocity([f32::NAN, 0.0, 0.0]));
    }

    #[test]
    fn test_validate_item_count_ok() {
        assert!(AntiCheat::validate_item_count(100));
        assert!(AntiCheat::validate_item_count(MAX_ITEM_STACK));
    }

    #[test]
    fn test_validate_item_count_rejected() {
        assert!(!AntiCheat::validate_item_count(MAX_ITEM_STACK + 1));
    }

    #[test]
    fn test_validate_scale_ok() {
        assert!(AntiCheat::validate_scale(1.0));
        assert!(AntiCheat::validate_scale(0.1));
        assert!(AntiCheat::validate_scale(10.0));
    }

    #[test]
    fn test_validate_scale_rejected() {
        assert!(!AntiCheat::validate_scale(0.05));
        assert!(!AntiCheat::validate_scale(11.0));
        assert!(!AntiCheat::validate_scale(f32::NAN));
    }

    #[test]
    fn test_validate_damage_ok() {
        assert!(AntiCheat::validate_damage(10.0));
        assert!(AntiCheat::validate_damage(9999.0));
    }

    #[test]
    fn test_validate_damage_rejected() {
        assert!(!AntiCheat::validate_damage(0.0));
        assert!(!AntiCheat::validate_damage(-1.0));
        assert!(!AntiCheat::validate_damage(10000.0));
    }

    #[test]
    fn test_validate_sequence_normal() {
        assert!(AntiCheat::validate_sequence(5, Some(4)));
        assert!(AntiCheat::validate_sequence(1, Some(0)));
        assert!(AntiCheat::validate_sequence(0, None)); // first packet OK
    }

    #[test]
    fn test_validate_sequence_duplicate_rejected() {
        assert!(!AntiCheat::validate_sequence(4, Some(5))); // old seq
    }

    #[test]
    fn test_validate_sequence_wrapping() {
        assert!(AntiCheat::validate_sequence(0, Some(u16::MAX)));
    }

    #[test]
    fn test_validate_form_id_ok() {
        assert!(AntiCheat::validate_form_id(0x12345678));
        assert!(AntiCheat::validate_form_id(0x00000001));
    }

    #[test]
    fn test_validate_form_id_spoof() {
        assert!(!AntiCheat::validate_form_id(0));
        assert!(!AntiCheat::validate_form_id(0xFFFFFFFF));
    }
}
