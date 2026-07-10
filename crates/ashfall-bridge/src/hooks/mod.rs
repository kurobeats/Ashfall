//! Gamebryo engine hooks — VTable patching for Fallout 3 / New Vegas.
//!
//! Hooks intercept engine functions to read/write game state.
//! Pattern: replace vtable entry → call original → read result.
//!
//! Categories:
//! - Position/Angle: TESObjectREFR::GetPos, SetPos, GetAngle, SetAngle
//! - Havok physics: bhkRigidBody velocity, grounded, collision
//! - Combat: hit detection, damage resistance/threshold
//! - AI: combat target, AI package, faction
//! - Quest/Dialogue: quest stages, dialogue flags, dialogue choices
//! - World: doors, terminals, projectiles, explosions
//! - FNV: reputation, hardcore stats (conditional on FNV exe)
//! - NVSE/FOSE: CommandTable, event sinks, opcode interception
//!
//! ponytail: stubs return zero/default. Real VTable offsets filled in
//! when reverse-engineered from Fallout3.exe / FalloutNV.exe + FOSE/NVSE.

use std::sync::atomic::{AtomicBool, Ordering};

static HOOKS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Which game engine is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEngine {
    Fallout3,
    FalloutNV,
    Unknown,
}

static GAME_ENGINE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2); // Unknown

pub fn detect_engine(crc: u32) -> GameEngine {
    // ponytail: CRC-based detection at bridge init
    if crc == 0x00E59528 { // FALLOUT3_EN_VER17
        GAME_ENGINE.store(0, Ordering::SeqCst);
        GameEngine::Fallout3
    } else if crc == 0x0206FEC7 { // FNV_EN_VER14
        GAME_ENGINE.store(1, Ordering::SeqCst);
        GameEngine::FalloutNV
    } else {
        GameEngine::Unknown
    }
}

pub fn is_fnv() -> bool {
    GAME_ENGINE.load(Ordering::SeqCst) == 1
}

/// Install all hooks. Called from DllMain on DLL_PROCESS_ATTACH.
pub fn install() {
    HOOKS_INSTALLED.store(true, Ordering::SeqCst);
    // TODO: locate TESObjectREFR vtable, patch all hooks.
    // For Proton: same VTable layout as Windows — Wine mirrors the binary exactly.
}

/// Uninstall all hooks. Called from DllMain on DLL_PROCESS_DETACH.
pub fn uninstall() {
    HOOKS_INSTALLED.store(false, Ordering::SeqCst);
    // TODO: restore original vtable entries
}

// ═══════════════════════════════════════════════════════════════
// Position & Angle
// ═══════════════════════════════════════════════════════════════

/// Get position of a reference by refID.
pub fn get_pos(ref_id: u32) -> [f32; 3] {
    let _ = ref_id;
    // TODO: TESObjectREFR + 0x30 → TESObjectREFR::GetPos()
    [0.0, 0.0, 0.0]
}

pub fn set_pos(ref_id: u32, pos: [f32; 3]) {
    let _ = (ref_id, pos);
    // TODO: TESObjectREFR + 0x34 → TESObjectREFR::SetPos()
}

pub fn get_angle(ref_id: u32) -> [f32; 3] {
    let _ = ref_id;
    [0.0, 0.0, 0.0]
}

pub fn set_angle(ref_id: u32, angle: [f32; 3]) {
    let _ = (ref_id, angle);
}

pub fn get_scale(ref_id: u32) -> f32 {
    let _ = ref_id;
    1.0
}

pub fn set_scale(ref_id: u32, scale: f32) {
    let _ = (ref_id, scale);
}

// ═══════════════════════════════════════════════════════════════
// Havok Physics
// ═══════════════════════════════════════════════════════════════

/// Get velocity of the bhkRigidBody attached to this reference.
pub fn get_velocity(ref_id: u32) -> [f32; 3] {
    let _ = ref_id;
    // TODO: TESObjectREFR → bhkRigidBody → hkRigidBody::getLinearVelocity()
    [0.0, 0.0, 0.0]
}

/// Set velocity of the bhkRigidBody attached to this reference.
pub fn set_velocity(ref_id: u32, vel: [f32; 3]) {
    let _ = (ref_id, vel);
}

/// Check if actor is on the ground (bhkRigidBody ground contact).
pub fn is_on_ground(ref_id: u32) -> bool {
    let _ = ref_id;
    false
}

/// Get rigid body collision layer flags.
pub fn get_collision_flags(ref_id: u32) -> u32 {
    let _ = ref_id;
    0
}

// ═══════════════════════════════════════════════════════════════
// Combat
// ═══════════════════════════════════════════════════════════════

/// Get limb hit by the most recent attack. Returns limb index.
pub fn get_hit_limb(ref_id: u32) -> u8 {
    let _ = ref_id;
    0 // LIMB_TORSO
}

/// Get damage resistance for an actor.
pub fn get_damage_resistance(actor_id: u32) -> f32 {
    let _ = actor_id;
    // TODO: Actor::GetDamageResistance() → AV_DamageResistance
    0.0
}

/// Get damage threshold for an actor (FNV only).
pub fn get_damage_threshold(actor_id: u32) -> f32 {
    let _ = actor_id;
    // TODO: Actor::GetDamageThreshold() → AV_DamageThreshold (FNV only)
    0.0
}

/// Get base damage for a weapon FormID.
pub fn get_weapon_base_damage(weapon_base_id: u32) -> f32 {
    let _ = weapon_base_id;
    // TODO: TESObjectWEAP::GetAttackDamage()
    0.0
}

/// Get weapon critical damage multiplier.
pub fn get_weapon_crit_mult(weapon_base_id: u32) -> f32 {
    let _ = weapon_base_id;
    1.0
}

/// Get weapon critical chance bonus.
pub fn get_weapon_crit_chance(weapon_base_id: u32) -> f32 {
    let _ = weapon_base_id;
    0.0
}

// ═══════════════════════════════════════════════════════════════
// Actor State
// ═══════════════════════════════════════════════════════════════

pub fn get_actor_state(ref_id: u32) -> (u32, u8, u8, u8, bool, bool) {
    let _ = ref_id;
    // (idle, moving, weapon, flags, alerted, sneaking)
    (0, 0, 0, 0, false, false)
}

pub fn get_actor_value(ref_id: u32, index: u8) -> f32 {
    let _ = (ref_id, index);
    0.0
}

pub fn set_actor_value(ref_id: u32, index: u8, value: f32) {
    let _ = (ref_id, index, value);
}

pub fn get_actor_base_value(ref_id: u32, index: u8) -> f32 {
    let _ = (ref_id, index);
    0.0
}

// ═══════════════════════════════════════════════════════════════
// NPC AI
// ═══════════════════════════════════════════════════════════════

/// Get current combat target FormID for an NPC.
pub fn get_combat_target(ref_id: u32) -> u32 {
    let _ = ref_id;
    // TODO: Actor::GetCombatTarget() → TESObjectREFR*
    0
}

/// Get current AI package ID for an NPC.
pub fn get_ai_package(ref_id: u32) -> (u32, u8) {
    let _ = ref_id;
    // TODO: Actor::GetCurrentAIPackage() → (package_type, flags)
    (0, 0)
}

/// Get NPC faction FormID and rank.
pub fn get_faction(ref_id: u32) -> Vec<(u32, i8)> {
    let _ = ref_id;
    // TODO: Actor::GetFactionList()
    vec![]
}

/// Check if two factions are hostile.
pub fn is_faction_hostile(faction_a: u32, faction_b: u32) -> bool {
    let _ = (faction_a, faction_b);
    false
}

// ═══════════════════════════════════════════════════════════════
// Controls
// ═══════════════════════════════════════════════════════════════

pub fn get_control(ref_id: u32, control: u8) -> u8 {
    let _ = (ref_id, control);
    0
}

pub fn set_control(ref_id: u32, control: u8, enabled: bool) {
    let _ = (ref_id, control, enabled);
}

// ═══════════════════════════════════════════════════════════════
// World Objects (Doors, Terminals)
// ═══════════════════════════════════════════════════════════════

/// Get door open state.
pub fn get_door_state(ref_id: u32) -> bool {
    let _ = ref_id;
    // TODO: TESObjectDOOR::GetOpenState()
    false
}

/// Set door open state.
pub fn set_door_state(ref_id: u32, open: bool) {
    let _ = (ref_id, open);
}

/// Get terminal locked state.
pub fn get_terminal_locked(ref_id: u32) -> bool {
    let _ = ref_id;
    // TODO: TESObjectREFR::GetLocked() for TERM form type
    false
}

/// Set terminal locked state.
pub fn set_terminal_locked(ref_id: u32, locked: bool) {
    let _ = (ref_id, locked);
}

// ═══════════════════════════════════════════════════════════════
// Quest & Dialogue
// ═══════════════════════════════════════════════════════════════

/// Get quest stage.
pub fn get_quest_stage(quest_id: u32) -> u16 {
    let _ = quest_id;
    // TODO: TESQuest::GetCurrentStageID()
    0
}

/// Set quest stage.
pub fn set_quest_stage(quest_id: u32, stage: u16) {
    let _ = (quest_id, stage);
    // TODO: TESQuest::SetStage()
}

/// Get dialogue flag value (used in result scripts).
pub fn get_dialogue_flag(flag_id: u32) -> bool {
    let _ = flag_id;
    false
}

/// Set dialogue flag value.
pub fn set_dialogue_flag(flag_id: u32, value: bool) {
    let _ = (flag_id, value);
}

// ═══════════════════════════════════════════════════════════════
// FNV-Specific (only called when is_fnv() == true)
// ═══════════════════════════════════════════════════════════════

/// Get reputation with a faction (FNV only).
pub fn get_reputation(_faction: u32) -> i32 {
    // Guard: only valid for FNV
    if !is_fnv() { return 0; }
    // TODO: PlayerCharacter::GetReputation()
    0
}

/// Set reputation with a faction (FNV only).
pub fn set_reputation(_faction: u32, _value: i32) {
    if !is_fnv() { return; }
}

/// Get hardcore stat values (FNV only).
/// Returns (hunger, thirst, sleep).
pub fn get_hardcore_stats() -> (f32, f32, f32) {
    if !is_fnv() { return (0.0, 0.0, 0.0); }
    // TODO: PlayerCharacter::GetHardcoreStats()
    (0.0, 0.0, 0.0)
}

/// Set hardcore stat values (FNV only).
pub fn set_hardcore_stats(_hunger: f32, _thirst: f32, _sleep: f32) {
    if !is_fnv() { return; }
}

// ═══════════════════════════════════════════════════════════════
// Misc
// ═══════════════════════════════════════════════════════════════

pub fn get_base(ref_id: u32) -> u32 {
    let _ = ref_id;
    0
}

pub fn get_name(ref_id: u32) -> String {
    let _ = ref_id;
    String::new()
}

// ═══════════════════════════════════════════════════════════════
// NVSE/FOSE Integration
// ═══════════════════════════════════════════════════════════════

/// Event sink callback type: invoked by the engine when events fire.
pub type EventSinkCallback = extern "C" fn(event_type: u32, arg0: u32, arg1: u32, arg2: u32);

/// Register an event sink with the NVSE/FOSE event dispatcher.
pub fn register_event_sink(event_type: u32, callback: EventSinkCallback) {
    // TODO: RegisterEventSink via NVSE CommandTable
    let _ = (event_type, callback);
}

/// Unregister an event sink.
pub fn unregister_event_sink(event_type: u32, callback: EventSinkCallback) {
    let _ = (event_type, callback);
}

/// Hook a console command — intercept before engine processes it.
pub fn hook_console_command(command: &str) -> bool {
    // TODO: ConsoleManager::HookCommand
    let _ = command;
    false
}

/// Intercept a script opcode — validate before execution.
pub fn intercept_opcode(opcode: u16, args: &[u32]) -> bool {
    // TODO: ScriptRunner::InterceptOpcode
    let _ = (opcode, args);
    true // allow by default
}
