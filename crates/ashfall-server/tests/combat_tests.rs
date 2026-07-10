//! Combat integration tests — damage formula, critical hits, death transitions.

use ashfall_server::combat::DamageFormula;

#[test]
fn test_damage_formula_basic() {
    // base=10, limb_mult=1.0 (torso), dr=0, dt=0, crit=1.0
    let dmg = DamageFormula::calculate(10.0, 1.0, 0.0, 0.0, 1.0);
    assert_eq!(dmg, 10.0, "unmodified damage should equal base");
}

#[test]
fn test_damage_formula_headshot() {
    // base=10, limb_mult=2.0, dr=0, dt=0, crit=1.0
    let dmg = DamageFormula::calculate(10.0, 2.0, 0.0, 0.0, 1.0);
    assert_eq!(dmg, 20.0, "headshot should double damage");
}

#[test]
fn test_damage_formula_limb() {
    // base=100, limb_mult=0.5 (arm/leg), dr=0, dt=0, crit=1.0
    let dmg = DamageFormula::calculate(100.0, 0.5, 0.0, 0.0, 1.0);
    assert_eq!(dmg, 50.0, "limb hit should halve damage");
}

#[test]
fn test_damage_formula_dr() {
    // 50% DR
    let dmg = DamageFormula::calculate(100.0, 1.0, 0.5, 0.0, 1.0);
    assert_eq!(dmg, 50.0, "50% DR should halve damage");
}

#[test]
fn test_damage_formula_dr_capped_at_85() {
    // DR = 1.0 (100%) should be capped at 0.85
    let dmg = DamageFormula::calculate(100.0, 1.0, 1.0, 0.0, 1.0);
    assert!((dmg - 15.0).abs() < 0.01, "capped DR should leave ~15% damage, got {dmg}");
}

#[test]
fn test_damage_formula_dt() {
    // DT = 5, should subtract after DR
    let dmg = DamageFormula::calculate(20.0, 1.0, 0.0, 5.0, 1.0);
    assert_eq!(dmg, 15.0, "DT should subtract from damage");
}

#[test]
fn test_damage_formula_dt_non_zero_minimum() {
    // DT exceeds post-DR damage → minimum 1 damage
    let dmg = DamageFormula::calculate(5.0, 1.0, 0.0, 100.0, 1.0);
    assert_eq!(dmg, 1.0, "minimum 1 damage always applied");
}

#[test]
fn test_damage_formula_critical() {
    // crit_mult = 2.0
    let dmg = DamageFormula::calculate(10.0, 1.0, 0.0, 0.0, 2.0);
    assert_eq!(dmg, 20.0, "crit should double base damage before DR/DT");
}

#[test]
fn test_damage_formula_full_pipeline() {
    // base=50, headshot(2.0), DR=0.3, DT=5, crit=1.5
    let dmg = DamageFormula::calculate(50.0, 2.0, 0.3, 5.0, 1.5);
    // modified = 50 * 2.0 * 1.5 = 150
    // after DR = 150 * 0.7 = 105
    // after DT = 105 - 5 = 100
    assert_eq!(dmg, 100.0, "full pipeline calculation");
}

#[test]
fn test_limb_multiplier_indices() {
    assert_eq!(DamageFormula::limb_multiplier(0), 1.0);  // Torso
    assert_eq!(DamageFormula::limb_multiplier(1), 2.0);  // Head
    assert_eq!(DamageFormula::limb_multiplier(2), 0.5);  // Left arm
    assert_eq!(DamageFormula::limb_multiplier(3), 0.5);  // Right arm
    assert_eq!(DamageFormula::limb_multiplier(4), 0.5);  // Left leg
    assert_eq!(DamageFormula::limb_multiplier(5), 0.5);  // Right leg
}

#[test]
fn test_is_headshot_fatal() {
    assert!(DamageFormula::is_headshot_fatal(100.0, 100.0));
    assert!(DamageFormula::is_headshot_fatal(50.0, 49.0));
    assert!(!DamageFormula::is_headshot_fatal(49.0, 50.0));
}

#[test]
fn test_compute_dr_empty() {
    assert_eq!(DamageFormula::compute_dr(&[]), 0.0);
}

#[test]
fn test_compute_dr_multiple() {
    assert_eq!(DamageFormula::compute_dr(&[0.2, 0.3]), 0.5);
}

#[test]
fn test_compute_dr_capped() {
    assert_eq!(DamageFormula::compute_dr(&[0.5, 0.5]), 0.85); // capped
}
