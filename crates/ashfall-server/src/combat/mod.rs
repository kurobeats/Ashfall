//! Combat system — damage calculation + resolution.

/// Fallout 3 damage formula.
///
/// Damage = (base_damage * limb_mult) * (1 - DR) - DT
/// where DR = Damage Resistance (0.0–0.85 capped), DT = Damage Threshold (FNV only).
///
/// Minimum 1 damage if hit connects.
pub struct DamageFormula;

impl DamageFormula {
    /// Calculate final damage.
    /// `base_damage` — raw weapon damage before modifiers.
    /// `limb_mult` — body part multiplier (head=2.0, torso=1.0, limbs=0.5).
    /// `dr` — damage resistance (0.0–0.85).
    /// `dt` — damage threshold (FNV only, 0 for FO3).
    /// `crit_mult` — critical hit multiplier (default 1.0 if not critical).
    pub fn calculate(base_damage: f32, limb_mult: f32, dr: f32, dt: f32, crit_mult: f32) -> f32 {
        let modified = base_damage * limb_mult * crit_mult;
        let dr_capped = dr.clamp(0.0, 0.85);
        let after_dr = modified * (1.0 - dr_capped);
        let final_damage = (after_dr - dt).max(1.0); // minimum 1 damage
        final_damage
    }

    /// Limb multiplier for body part indices.
    pub fn limb_multiplier(limb: u8) -> f32 {
        match limb {
            0 => 1.0, // Torso
            1 => 2.0, // Head
            2 => 0.5, // Left arm
            3 => 0.5, // Right arm
            4 => 0.5, // Left leg
            5 => 0.5, // Right leg
            _ => 1.0,
        }
    }

    /// Compute total DR from armor. Sum of individual DR values, capped.
    pub fn compute_dr(armor_dr: &[f32]) -> f32 {
        armor_dr.iter().sum::<f32>().clamp(0.0, 0.85)
    }

    /// Check if headshot is lethal (head hit + damage >= remaining health).
    pub fn is_headshot_fatal(damage: f32, current_health: f32) -> bool {
        damage >= current_health
    }
}

/// Combat resolution state for an actor.
#[derive(Debug, Clone)]
pub struct CombatState {
    pub is_in_combat: bool,
    pub last_attacker: Option<ashfall_core::id::NetworkID>,
    pub last_hit_time: Option<std::time::Instant>,
    pub hits_landed: u32,
    pub damage_dealt: f32,
    pub damage_taken: f32,
}

impl CombatState {
    pub fn new() -> Self {
        CombatState {
            is_in_combat: false,
            last_attacker: None,
            last_hit_time: None,
            hits_landed: 0,
            damage_dealt: 0.0,
            damage_taken: 0.0,
        }
    }

    pub fn record_hit(
        &mut self,
        attacker: ashfall_core::id::NetworkID,
        damage: f32,
        is_outgoing: bool,
    ) {
        self.is_in_combat = true;
        if !is_outgoing {
            self.last_attacker = Some(attacker);
        }
        self.last_hit_time = Some(std::time::Instant::now());
        if is_outgoing {
            self.hits_landed += 1;
            self.damage_dealt += damage;
        } else {
            self.damage_taken += damage;
        }
    }

    pub fn out_of_combat(&mut self) {
        if let Some(t) = self.last_hit_time {
            if t.elapsed().as_secs() > 10 {
                self.is_in_combat = false;
            }
        }
    }
}

impl Default for CombatState {
    fn default() -> Self {
        Self::new()
    }
}

pub mod resolver;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_minimum_one_damage() {
        // 100% DR + huge DT still lands the 1-damage floor.
        let d = DamageFormula::calculate(50.0, 1.0, 0.85, 999.0, 1.0);
        assert!((d - 1.0).abs() < 1e-6);
    }

    #[test]
    fn formula_applies_dr_then_dt() {
        // 100 dmg, head (2.0), 0.5 DR, 10 DT: (100*2)*(0.5) - 10 = 90
        let d = DamageFormula::calculate(100.0, 2.0, 0.5, 10.0, 1.0);
        assert!((d - 90.0).abs() < 1e-3);
    }

    #[test]
    fn formula_crit_scales_before_dr() {
        // 100 dmg, crit 2.0, 0.25 DR: (100*2)*0.75 = 150
        let d = DamageFormula::calculate(100.0, 1.0, 0.25, 0.0, 2.0);
        assert!((d - 150.0).abs() < 1e-3);
    }

    #[test]
    fn dr_capped_at_85_percent() {
        // DR beyond 0.85 clamps to 0.85.
        let d = DamageFormula::calculate(100.0, 1.0, 10.0, 0.0, 1.0);
        assert!((d - 15.0).abs() < 1e-3);
    }

    #[test]
    fn limb_multiplier_table() {
        assert_eq!(DamageFormula::limb_multiplier(0), 1.0); // torso
        assert_eq!(DamageFormula::limb_multiplier(1), 2.0); // head
        assert_eq!(DamageFormula::limb_multiplier(4), 0.5); // leg
        assert_eq!(DamageFormula::limb_multiplier(99), 1.0); // unknown → torso
    }

    #[test]
    fn compute_dr_sums_and_caps() {
        assert!((DamageFormula::compute_dr(&[0.2, 0.3]) - 0.5).abs() < 1e-6);
        assert!((DamageFormula::compute_dr(&[0.9, 0.9]) - 0.85).abs() < 1e-6);
        assert_eq!(DamageFormula::compute_dr(&[]), 0.0);
    }

    #[test]
    fn headshot_fatal_threshold() {
        assert!(DamageFormula::is_headshot_fatal(100.0, 50.0));
        assert!(!DamageFormula::is_headshot_fatal(50.0, 100.0));
        assert!(DamageFormula::is_headshot_fatal(50.0, 50.0)); // exact
    }
}
