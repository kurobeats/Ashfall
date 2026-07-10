//! Gamebryo engine hooks — VTable patching for Fallout 3 / New Vegas.
//!
//! Sub-modules:
//! - `memory`: SafeWrite8/16/32/Buf, WriteRelJump/Call, MemoryProtect, Patch
//! - `vtable`: VTable entry lookup, field access, concrete hook implementations
//! - `detour`: Trampoline pattern for function hooking
//! - `opcode`: OpcodeHandler table, BethesdaDelegator interception
//!
//! Known offsets (from xSE community):
//!   TESObjectREFR::GetPos   = VTable+0x30 (FO3 1.7)
//!   TESObjectREFR::SetPos   = VTable+0x34
//!   Actor::GetActorValue    = VTable+0x68
//!   PlayerCharacter::GetControl = VTable+0x90
//!
//! Resource: https://github.com/ianpatt/fose/blob/master/common/GameAPI.cpp

pub mod detour;
pub mod memory;
pub mod opcode;
pub mod vtable;

use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

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
/// Delegates to vtable::get_pos (VTable call or raw field read).
#[inline]
pub fn get_pos(ref_id: u32) -> [f32; 3] {
    unsafe { vtable::get_pos(ref_id) }
}

/// Set position via VTable or raw field write.
#[inline]
pub fn set_pos(ref_id: u32, pos: [f32; 3]) {
    unsafe { vtable::set_pos(ref_id, pos) }
}

/// Get angle in degrees (converted from engine radians).
#[inline]
pub fn get_angle(ref_id: u32) -> [f32; 3] {
    unsafe { vtable::get_angle(ref_id) }
}

/// Set angle (degrees → radians → engine).
#[inline]
pub fn set_angle(ref_id: u32, angle: [f32; 3]) {
    unsafe { vtable::set_angle(ref_id, angle) }
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

/// Read actor animation state: (idle, moving, weapon, flags, alerted, sneaking).
#[inline]
pub fn get_actor_state(ref_id: u32) -> (u32, u8, u8, u8, bool, bool) {
    unsafe { vtable::get_actor_state(ref_id) }
}

/// Read actor value by index (health=0x14, AP=0x15, DR=0x29, DT=0x2A).
#[inline]
pub fn get_actor_value(ref_id: u32, index: u8) -> f32 {
    unsafe { vtable::get_actor_value(ref_id, index) }
}

/// Write actor value via VTable.
#[inline]
pub fn set_actor_value(ref_id: u32, index: u8, value: f32) {
    unsafe { vtable::set_actor_value(ref_id, index, value) }
}

/// Read base actor value.
#[inline]
pub fn get_actor_base_value(ref_id: u32, index: u8) -> f32 {
    unsafe { vtable::get_actor_base_value(ref_id, index) }
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

// ═══════════════════════════════════════════════════════════════
// Misc getters (stubs)
// ═══════════════════════════════════════════════════════════════

/// Read cell of a reference. ponytail: returns 0 until RE completes.
#[inline]
pub fn get_cell(ref_id: u32) -> u32 {
    unsafe { vtable::get_cell(ref_id) }
}

pub fn get_activate(ref_id: u32) -> u32 {
    let _ = ref_id;
    0
}

pub fn get_enabled(ref_id: u32) -> bool {
    let _ = ref_id;
    true
}

pub fn get_lock(ref_id: u32) -> u32 {
    let _ = ref_id;
    0
}

/// Get base FormID via VTable chain: GetBaseForm → GetFormID.
#[inline]
pub fn get_base(ref_id: u32) -> u32 {
    unsafe { vtable::get_base(ref_id) }
}

pub fn get_name(ref_id: u32) -> String {
    let _ = ref_id;
    "unnamed".to_string()
}

// ═══════════════════════════════════════════════════════════════
// NVSE/FOSE Integration
// ═══════════════════════════════════════════════════════════════
//
// ponytail: in-memory registries for testing. Real implementation
// replaces these with NVSE CommandTable + BSTEventSink subclass.

/// PluginInfo struct matching NVSE/FOSE plugin signature.
/// Size: 4 + 256 = 260 bytes.
#[repr(C)]
pub struct PluginInfo {
    pub info_version: u32,
    pub name: [u8; 256],
}

impl PluginInfo {
    pub fn new(name: &str) -> Self {
        let mut info = PluginInfo {
            info_version: 1,
            name: [0u8; 256],
        };
        let bytes = name.as_bytes();
        let len = bytes.len().min(255);
        info.name[..len].copy_from_slice(&bytes[..len]);
        info.name[len] = 0;
        info
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(256);
        std::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

/// Event sink callback type: invoked by the engine when events fire.
pub type EventSinkCallback = extern "C" fn(event_type: u32, arg0: u32, arg1: u32, arg2: u32);

static EVENT_SINKS: LazyLock<Mutex<HashMap<u32, Vec<EventSinkCallback>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CONSOLE_COMMANDS: LazyLock<Mutex<HashMap<String, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register an event sink with the NVSE/FOSE event dispatcher.
pub fn register_event_sink(event_type: u32, callback: EventSinkCallback) {
    let mut sinks = EVENT_SINKS.lock().unwrap();
    sinks.entry(event_type).or_default().push(callback);
}

/// Unregister an event sink.
pub fn unregister_event_sink(event_type: u32, callback: EventSinkCallback) {
    let mut sinks = EVENT_SINKS.lock().unwrap();
    if let Some(list) = sinks.get_mut(&event_type) {
        list.retain(|&cb| cb as usize != callback as usize);
    }
}

/// Dispatch an event to all registered sinks. Returns count of callbacks fired.
pub fn dispatch_event(event_type: u32, arg0: u32, arg1: u32, arg2: u32) -> usize {
    let sinks = EVENT_SINKS.lock().unwrap();
    if let Some(list) = sinks.get(&event_type) {
        let callbacks: Vec<_> = list.clone();
        drop(sinks);
        for cb in &callbacks {
            cb(event_type, arg0, arg1, arg2);
        }
        callbacks.len()
    } else {
        0
    }
}

/// Check if any sinks are registered for an event type.
pub fn has_event_sinks(event_type: u32) -> bool {
    EVENT_SINKS.lock().unwrap().get(&event_type).map(|v| !v.is_empty()).unwrap_or(false)
}

/// Hook a console command — intercept before engine processes it.
pub fn hook_console_command(command: &str) -> bool {
    let cmds = CONSOLE_COMMANDS.lock().unwrap();
    cmds.contains_key(command)
}

/// Register a console command handler.
pub fn register_console_command(command: &str) {
    CONSOLE_COMMANDS.lock().unwrap().insert(command.to_string(), true);
}

/// Unregister a console command.
pub fn unregister_console_command(command: &str) {
    CONSOLE_COMMANDS.lock().unwrap().remove(command);
}

/// Check if a console command is registered.
pub fn has_console_command(command: &str) -> bool {
    CONSOLE_COMMANDS.lock().unwrap().contains_key(command)
}

/// Intercept a script opcode — validate before execution.
pub fn intercept_opcode(opcode: u16, args: &[u32]) -> bool {
    // TODO: ScriptRunner::InterceptOpcode
    let _ = (opcode, args);
    true // allow by default
}

// ═══════════════════════════════════════════════════════════════
// Tier 1-4 Hook Stubs (for extended commands.rs)
// ponytail: these return zero/default. Real impl in vtable.rs.
// ═══════════════════════════════════════════════════════════════

/// Check if actor is dead.
pub fn is_dead(ref_id: u32) -> bool {
    let _ = ref_id;
    // TODO: Actor::GetDead()
    false
}

/// Get the cell FormID this reference currently occupies.
pub fn get_parent_cell(ref_id: u32) -> u32 {
    let _ = ref_id;
    // TODO: TESObjectREFR::GetParentCell()
    0
}

/// Equip an item on an actor.
pub fn equip_item(ref_id: u32, item_id: u32, equip_slot: u32, prevent_removal: u8) {
    let _ = (ref_id, item_id, equip_slot, prevent_removal);
    // TODO: Actor::EquipItem() via GECK opcode
}

/// Unequip an item from an actor.
pub fn unequip_item(ref_id: u32, item_id: u32, equip_slot: u32, prevent_removal: u8) {
    let _ = (ref_id, item_id, equip_slot, prevent_removal);
    // TODO: Actor::UnequipItem() via GECK opcode
}

/// Add items to inventory.
pub fn add_item(ref_id: u32, item_id: u32, count: u32, silent: u8) {
    let _ = (ref_id, item_id, count, silent);
    // TODO: Actor::AddItem() via GECK opcode
}

/// Remove items from inventory.
pub fn remove_item(ref_id: u32, item_id: u32, count: u32, silent: u8) {
    let _ = (ref_id, item_id, count, silent);
    // TODO: Actor::RemoveItem() via GECK opcode
}

/// Remove all items, optionally transferring to another container.
pub fn remove_all_items(ref_id: u32, transfer_to: u32) {
    let _ = (ref_id, transfer_to);
    // TODO: Actor::RemoveAllItems() via GECK opcode
}

/// Get reference count for an inventory item form.
pub fn get_ref_count(ref_id: u32) -> u32 {
    let _ = ref_id;
    // TODO: Actor::GetRefCount() via FOSE/NVSE
    0
}

/// Kill an actor (direct death, bypasses damage).
pub fn kill_actor(ref_id: u32, killer_id: u32, limb: i8, cause: i8) {
    let _ = (ref_id, killer_id, limb, cause);
    // TODO: Actor::Kill() via GECK opcode
}

/// Apply damage to an actor value.
pub fn damage_actor_value(ref_id: u32, index: u8, damage: f32) {
    let _ = (ref_id, index, damage);
    // TODO: Actor::DamageActorValue() via GECK opcode
}

/// Restore an actor value (heal, repair).
pub fn restore_actor_value(ref_id: u32, index: u8, amount: f32) {
    let _ = (ref_id, index, amount);
    // TODO: Actor::RestoreActorValue() via GECK opcode
}

/// Force-set an actor value (bypasses modifiers, sets base+current).
pub fn force_actor_value(ref_id: u32, index: u8, value: f32) {
    let _ = (ref_id, index, value);
    // TODO: Actor::ForceActorValue() via GECK opcode
}

/// Play an animation group on an actor.
pub fn play_group(ref_id: u32, group_id: u32, flags: u32) {
    let _ = (ref_id, group_id, flags);
    // TODO: Actor::PlayGroup() via GECK opcode
}

/// Force weather state globally.
pub fn force_weather(weather_id: u32) {
    let _ = weather_id;
    // TODO: Weather::ForceWeather() via GECK opcode
}

/// Restrain/unrestrain an actor (prevents movement/combat).
pub fn set_restrained(ref_id: u32, restrained: u8) {
    let _ = (ref_id, restrained);
    // TODO: Actor::SetRestrained() via GECK opcode
}
