//! Remote-actor animation controller.
//!
//! Ports the actor-state → animation dispatch from vaultmp's
//! `Game::net_SetActorState` (MIT) with the PlayGroup numeric anim-group
//! encoding, plus the locomotion grouping from mojave-online's fnvmp
//! `animation.cpp` (MIT) for the FNV client.
//!
//! The bridge runs this per remote actor on every `UpdateActorState` packet
//! and executes the resulting `PlayGroup` opcodes in the game, so remote
//! players/NPCs walk, run, aim, draw and fire with matching animation.
//!
//! Pure state machine — no unsafe, unit-testable on any target.

/// Base anim-group values shared by FO3 (FOSE) and FNV (NVSE).
/// Source: vaultmp `API.hpp` (values match FOSE `AnimGroup` enum).
#[allow(dead_code)]
pub mod anim_group {
    pub const IDLE: u8 = 0x00;
    pub const DYNAMIC_IDLE: u8 = 0x01;
    pub const SPECIAL_IDLE: u8 = 0x02;
    pub const FORWARD: u8 = 0x03;
    pub const BACKWARD: u8 = 0x04;
    pub const LEFT: u8 = 0x05;
    pub const RIGHT: u8 = 0x06;
    pub const FAST_FORWARD: u8 = 0x07;
    pub const FAST_BACKWARD: u8 = 0x08;
    pub const FAST_LEFT: u8 = 0x09;
    pub const FAST_RIGHT: u8 = 0x0A;
    pub const DODGE_FORWARD: u8 = 0x0B;
    pub const DODGE_BACK: u8 = 0x0C;
    pub const DODGE_LEFT: u8 = 0x0D;
    pub const DODGE_RIGHT: u8 = 0x0E;
    pub const TURN_LEFT: u8 = 0x0F;
    pub const TURN_RIGHT: u8 = 0x10;
    pub const AIM: u8 = 0x11;
    pub const AIM_UP: u8 = 0x12;
    pub const AIM_DOWN: u8 = 0x13;
    pub const AIM_IS: u8 = 0x14;
    pub const AIM_IS_UP: u8 = 0x15;
    pub const AIM_IS_DOWN: u8 = 0x16;
    pub const HOLSTER: u8 = 0x17;
    pub const EQUIP: u8 = 0x18;
    pub const UNEQUIP: u8 = 0x19;
    pub const ATTACK_LEFT: u8 = 0x1A;
    pub const ATTACK_RIGHT: u8 = 0x20;
    pub const ATTACK_3: u8 = 0x26;
    pub const ATTACK_LOOP: u8 = 0x4A;
    pub const ATTACK_SPIN: u8 = 0x50;
    pub const ATTACK_SPIN2: u8 = 0x56;
    pub const ATTACK_POWER: u8 = 0x5C;
    pub const ATTACK_FORWARD_POWER: u8 = 0x5D;
    pub const ATTACK_BACK_POWER: u8 = 0x5E;
    pub const ATTACK_LEFT_POWER: u8 = 0x5F;
    pub const ATTACK_RIGHT_POWER: u8 = 0x60;
    pub const BLOCK_IDLE: u8 = 0x8B;
    pub const BLOCK_HIT: u8 = 0x8C;
    pub const RELOAD_A: u8 = 0x8E;
    pub const RELOAD_B: u8 = 0x8F;
    pub const RELOAD_C: u8 = 0x90;
    pub const JUMP_START: u8 = 0xA8;
    pub const JUMP_LOOP: u8 = 0xA9;
    pub const JUMP_LAND: u8 = 0xAA;
    // FNV-only additions (NVSE AnimGroup enum).
    pub const JUMP_LAND_LEFT: u8 = 0xB5;
    pub const JUMP_LAND_RIGHT: u8 = 0xB6;
}

/// What the game should do for one remote actor after a state update.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimAction {
    /// (anim_group, force) — run `PlayGroup anim force`.
    pub play_group: Vec<(u8, bool)>,
    /// Sneak state to force (`SetForceSneak`).
    pub set_sneak: Option<bool>,
    /// Alert state to force (`SetAlert`).
    pub set_alert: Option<bool>,
    /// Z-angle adjustment when strafing (moving_xy 0x01 → -45°, 0x02 → +45°).
    pub yaw_adjust: Option<f32>,
    /// Re-sync position (when movement anim went idle — the actor stopped).
    pub set_pos: bool,
}

impl AnimAction {
    fn none() -> Self {
        AnimAction {
            play_group: Vec::new(),
            set_sneak: None,
            set_alert: None,
            yaw_adjust: None,
            set_pos: false,
        }
    }
}

/// Per-actor animation state machine. One instance per remote actor.
///
/// Mirrors `Game::net_SetActorState`: only changed fields produce actions,
/// and weapon-anim changes are suppressed while firing / unequipped /
/// holstered, with the Aim↔AimIS transition emitting the proper down/up
/// sequence.
#[derive(Debug, Clone, Default)]
pub struct ActorAnimController {
    prev_moving: u8,
    prev_moving_xy: u8,
    prev_weapon: u8,
    prev_alerted: bool,
    prev_sneaking: bool,
}

impl ActorAnimController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one actor-state update; returns the animations/state to apply.
    ///
    /// `moving` = locomotion anim group (0x00 idle … 0x0A fast-right),
    /// `moving_xy` = direction bitmask (0x01/0x02 = diagonal strafe flags),
    /// `weapon` = weapon anim group, `alerted`/`sneaking` = state flags,
    /// `firing` = suppress weapon animation while a shot is in progress.
    pub fn update(
        &mut self,
        moving: u8,
        moving_xy: u8,
        weapon: u8,
        alerted: bool,
        sneaking: bool,
        firing: bool,
    ) -> AnimAction {
        let mut action = AnimAction::none();

        // Strafing: adjust yaw so diagonal movement follows the facing.
        if moving_xy != self.prev_moving_xy {
            match moving_xy {
                0x01 => action.yaw_adjust = Some(-45.0),
                0x02 => action.yaw_adjust = Some(45.0),
                _ => action.yaw_adjust = Some(0.0),
            }
        }
        self.prev_moving_xy = moving_xy;

        if alerted != self.prev_alerted {
            action.set_alert = Some(alerted);
            self.prev_alerted = alerted;
        }

        if sneaking != self.prev_sneaking {
            action.set_sneak = Some(sneaking);
            self.prev_sneaking = sneaking;
        }

        if moving != self.prev_moving {
            action.play_group.push((moving, true));
            if moving == anim_group::IDLE {
                // Stopped: re-sync position so clients converge exactly.
                action.set_pos = true;
            }
            self.prev_moving = moving;
        }

        // Weapon animation — the vaultmp guard: skip while firing, and skip
        // the equip/unequip/holster groups (the engine animates those itself
        // on the local actor; only the resulting state matters remotely).
        let is_equip_cycle = matches!(
            weapon,
            anim_group::IDLE | anim_group::EQUIP | anim_group::UNEQUIP | anim_group::HOLSTER
        );
        if weapon != self.prev_weapon && !firing && alerted && !is_equip_cycle {
            if weapon == anim_group::AIM && self.prev_weapon == anim_group::AIM_IS {
                action.play_group.push((anim_group::AIM_DOWN, true));
                action.play_group.push((anim_group::AIM_UP, true));
            }
            action.play_group.push((weapon, true));
            if weapon == anim_group::AIM_IS {
                action.play_group.push((anim_group::AIM_IS_DOWN, true));
                action.play_group.push((anim_group::AIM_IS_UP, true));
            }
        }
        self.prev_weapon = weapon;

        action
    }

    /// Reset to a clean slate (actor despawned or re-spawned).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Map a locomotion anim group to its PlayGroup name string
/// (mojave-online fnvmp console-command form, for the FNV client path).
pub fn locomotion_name(moving: u8) -> &'static str {
    use anim_group::*;
    match moving {
        FORWARD => "Forward",
        BACKWARD => "Backward",
        LEFT => "Left",
        RIGHT => "Right",
        FAST_FORWARD => "FastForward",
        FAST_BACKWARD => "FastBackward",
        FAST_LEFT => "FastLeft",
        FAST_RIGHT => "FastRight",
        _ => "Idle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anim_group::*;

    #[test]
    fn idle_to_forward_plays_group() {
        let mut c = ActorAnimController::new();
        // Actor idle at first.
        let a = c.update(IDLE, 0, IDLE, false, false, false);
        assert!(a.play_group.is_empty(), "no-op on first idle sample");

        let a = c.update(FORWARD, 0, IDLE, false, false, false);
        assert_eq!(a.play_group, vec![(FORWARD, true)]);
        assert!(!a.set_pos);
    }

    #[test]
    fn stopping_resyncs_position() {
        let mut c = ActorAnimController::new();
        c.update(FORWARD, 0, IDLE, false, false, false);
        let a = c.update(IDLE, 0, IDLE, false, false, false);
        assert_eq!(a.play_group, vec![(IDLE, true)]);
        assert!(a.set_pos, "idle after moving must re-sync position");
    }

    #[test]
    fn weapon_draw_requires_alert() {
        let mut c = ActorAnimController::new();
        // Not alerted → weapon change suppressed.
        let a = c.update(IDLE, 0, AIM, false, false, false);
        assert!(a.play_group.is_empty(), "no weapon anim while unalerted");
        // Same weapon, now alerted → still nothing (weapon unchanged).
        let a = c.update(IDLE, 0, AIM, true, false, false);
        assert!(a.play_group.is_empty());
        assert_eq!(a.set_alert, Some(true));
        // New weapon while alerted → plays.
        let a = c.update(IDLE, 0, ATTACK_3, true, false, false);
        assert_eq!(a.play_group, vec![(ATTACK_3, true)]);
    }

    #[test]
    fn firing_suppresses_weapon_anim() {
        let mut c = ActorAnimController::new();
        c.update(IDLE, 0, IDLE, true, false, false);
        let a = c.update(IDLE, 0, ATTACK_3, true, false, true);
        assert!(a.play_group.is_empty(), "firing suppresses weapon anim");
    }

    #[test]
    fn aim_is_transition_emits_sequence() {
        let mut c = ActorAnimController::new();
        c.update(IDLE, 0, AIM_IS, true, false, false);
        // Leaving iron sights → Aim: down then up.
        let a = c.update(IDLE, 0, AIM, true, false, false);
        assert_eq!(
            a.play_group,
            vec![(AIM_DOWN, true), (AIM_UP, true), (AIM, true)]
        );
        // Into iron sights → AimIS then IS down/up.
        let a = c.update(IDLE, 0, AIM_IS, true, false, false);
        assert_eq!(
            a.play_group,
            vec![(AIM_IS, true), (AIM_IS_DOWN, true), (AIM_IS_UP, true)]
        );
    }

    #[test]
    fn equip_cycle_is_skipped() {
        let mut c = ActorAnimController::new();
        c.update(IDLE, 0, IDLE, true, false, false);
        let a = c.update(IDLE, 0, EQUIP, true, false, false);
        assert!(a.play_group.is_empty(), "equip/unequip/holster skipped");
    }

    #[test]
    fn strafing_yaw_adjust() {
        let mut c = ActorAnimController::new();
        let a = c.update(FORWARD, 0x01, IDLE, false, false, false);
        assert_eq!(a.yaw_adjust, Some(-45.0));
        let a = c.update(FORWARD, 0x02, IDLE, false, false, false);
        assert_eq!(a.yaw_adjust, Some(45.0));
        let a = c.update(FORWARD, 0x00, IDLE, false, false, false);
        assert_eq!(a.yaw_adjust, Some(0.0));
    }

    #[test]
    fn state_flags_emit() {
        let mut c = ActorAnimController::new();
        let a = c.update(IDLE, 0, IDLE, true, true, false);
        assert_eq!(a.set_alert, Some(true));
        assert_eq!(a.set_sneak, Some(true));
        // Unchanged second sample → nothing.
        let a = c.update(IDLE, 0, IDLE, true, true, false);
        assert_eq!(a, AnimAction::none());
    }

    #[test]
    fn locomotion_names() {
        assert_eq!(locomotion_name(FORWARD), "Forward");
        assert_eq!(locomotion_name(FAST_RIGHT), "FastRight");
        assert_eq!(locomotion_name(IDLE), "Idle");
        assert_eq!(locomotion_name(0xFF), "Idle");
    }

    #[test]
    fn reset_clears_history() {
        let mut c = ActorAnimController::new();
        c.update(FORWARD, 0, AIM, true, false, false);
        c.reset();
        // After reset the next update diffs against a clean slate: everything
        // that differs from the defaults is re-emitted.
        let a = c.update(FORWARD, 0, AIM, true, false, false);
        assert!(
            !a.play_group.is_empty(),
            "reset should re-emit current state"
        );
        // And the update after that is a no-op again.
        let a = c.update(FORWARD, 0, AIM, true, false, false);
        assert_eq!(a, AnimAction::none());
    }
}
