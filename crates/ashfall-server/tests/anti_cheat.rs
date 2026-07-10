//! Anti-cheat integration tests — verify server rejects malicious input.

use ashfall_server::anti_cheat::AntiCheat;
use ashfall_core::constants::{MAX_SPEED, MAX_TELEPORT_DISTANCE, MAX_ITEM_STACK};
use std::time::Duration;

#[test]
fn test_teleport_rejected() {
    let result = AntiCheat::validate_position(
        [MAX_TELEPORT_DISTANCE + 1.0, 0.0, 0.0],
        Some([0.0, 0.0, 0.0]),
        Duration::from_millis(100),
    );
    assert!(!result, "teleport beyond MAX_TELEPORT_DISTANCE should be rejected");
}

#[test]
fn test_speed_hack_rejected() {
    // 3000 units in 1ms = 3M u/s > MAX_SPEED (5000 = safe, so use 1000ms worth in 1ms)
    let result = AntiCheat::validate_position(
        [6000.0, 0.0, 0.0], // 6000 units
        Some([0.0, 0.0, 0.0]),
        Duration::from_millis(1), // in 1ms => 6M u/s
    );
    assert!(!result, "movement exceeding MAX_SPEED should be rejected");
}

#[test]
fn test_valid_position_accepted() {
    let result = AntiCheat::validate_position(
        [100.0, 0.0, 0.0],
        Some([0.0, 0.0, 0.0]),
        Duration::from_millis(100),
    );
    assert!(result, "normal movement should be accepted");
}

#[test]
fn test_first_position_accepted() {
    let result = AntiCheat::validate_position(
        [5000.0, 0.0, 0.0],
        None, // no previous position
        Duration::from_millis(100),
    );
    assert!(result, "first position should always be accepted");
}

#[test]
fn test_nan_position_rejected() {
    let result = AntiCheat::validate_position(
        [f32::NAN, 0.0, 0.0],
        Some([0.0, 0.0, 0.0]),
        Duration::from_millis(100),
    );
    assert!(!result, "NaN position should be rejected");
}

#[test]
fn test_item_count_at_limit_accepted() {
    assert!(AntiCheat::validate_item_count(MAX_ITEM_STACK));
}

#[test]
fn test_item_count_over_limit_rejected() {
    assert!(!AntiCheat::validate_item_count(MAX_ITEM_STACK + 1));
}

#[test]
fn test_item_count_zero_accepted() {
    assert!(AntiCheat::validate_item_count(0));
}

#[test]
fn test_damage_zero_rejected() {
    assert!(!AntiCheat::validate_damage(0.0));
}

#[test]
fn test_damage_negative_rejected() {
    assert!(!AntiCheat::validate_damage(-5.0));
}

#[test]
fn test_damage_normal_accepted() {
    assert!(AntiCheat::validate_damage(25.0));
}

#[test]
fn test_damage_at_boundary_rejected() {
    assert!(!AntiCheat::validate_damage(10000.0));
    assert!(AntiCheat::validate_damage(9999.0));
}

#[test]
fn test_sequence_valid_progression() {
    assert!(AntiCheat::validate_sequence(5, Some(4)));
    assert!(AntiCheat::validate_sequence(10, Some(5)));
}

#[test]
fn test_sequence_first_packet_accepted() {
    assert!(AntiCheat::validate_sequence(0, None));
}

#[test]
fn test_sequence_duplicate_rejected() {
    assert!(!AntiCheat::validate_sequence(3, Some(5)));
}

#[test]
fn test_sequence_replay_rejected() {
    assert!(!AntiCheat::validate_sequence(4, Some(5)));
}

#[test]
fn test_sequence_wrapping_accepted() {
    assert!(AntiCheat::validate_sequence(0, Some(u16::MAX)));
    assert!(AntiCheat::validate_sequence(1, Some(u16::MAX)));
}

#[test]
fn test_sequence_far_future_rejected() {
    // Gap >32767 should be rejected
    assert!(!AntiCheat::validate_sequence(32768, Some(0)));
}

#[test]
fn test_form_id_spoof_zero_rejected() {
    assert!(!AntiCheat::validate_form_id(0x00000000));
}

#[test]
fn test_form_id_spoof_max_rejected() {
    assert!(!AntiCheat::validate_form_id(0xFFFFFFFF));
}

#[test]
fn test_form_id_valid_accepted() {
    assert!(AntiCheat::validate_form_id(0x00000001));
    assert!(AntiCheat::validate_form_id(0x12345678));
    assert!(AntiCheat::validate_form_id(0x00ABCDEF));
}

#[test]
fn test_velocity_at_max_speed_accepted() {
    assert!(AntiCheat::validate_velocity([MAX_SPEED - 1.0, 0.0, 0.0]));
}

#[test]
fn test_velocity_over_max_rejected() {
    assert!(!AntiCheat::validate_velocity([MAX_SPEED, 0.0, 0.0]));
}

#[test]
fn test_scale_valid_accepted() {
    assert!(AntiCheat::validate_scale(1.0));
    assert!(AntiCheat::validate_scale(0.1));
    assert!(AntiCheat::validate_scale(10.0));
}

#[test]
fn test_scale_out_of_bounds_rejected() {
    assert!(!AntiCheat::validate_scale(0.05));
    assert!(!AntiCheat::validate_scale(100.0));
}
