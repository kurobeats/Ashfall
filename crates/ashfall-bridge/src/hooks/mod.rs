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

pub mod address;
pub mod animation;
pub mod detour;
pub mod discovery;
pub mod memory;
pub mod opcode;
pub mod vaultmp;
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

// ═══════════════════════════════════════════════════════════════
// Engine detection
//
// Verdict from real-binary analysis (a Linux analysis host, GOG
// Fallout3.exe 1.7.0.3, md5 7691d7180f225ee8e876358d170ecc93): the CRC
// constants below match NO computable hash of the real exe (whole-file
// CRC32 = 0x425A8C16; no chunk/CRC-variant matches). FOSE/NVSE never use
// CRC detection — they compile per-version with `#if FALLOUT_VERSION`.
// The `crc` parameter is therefore ignored until a real signature scheme
// (e.g. reading the exe's VS_VERSION_INFO at runtime) replaces it.
pub fn detect_engine(_crc: u32) -> GameEngine {
    GameEngine::Unknown
}

// ═══════════════════════════════════════════════════════════════
// Verified FO3 1.7.0.3 address table (xFOSE fose.h FALLOUT_VERSION_1_7
// block, cross-checked against the real GOG exe — 0x455190 confirmed as
// the hot form-lookup with 884 direct call sites)
pub mod fo3_17 {
    /// `TESForm* LookupFormByID(u32)` — verified.
    pub const LOOKUP_FORM_BY_ID: usize = 0x0045_5190;
    /// Script-VM argument extraction (opcode interception entry).
    pub const EXTRACT_ARGS: usize = 0x0051_7950;
    /// `void* CreateFormInstance(...)`.
    pub const CREATE_FORM_INSTANCE: usize = 0x0043_CDA0;
    /// `ConsoleManager* ConsoleManager_GetSingleton(bool)` — console hooks.
    pub const CONSOLE_MANAGER_GET_SINGLETON: usize = 0x0062_B5D0;
    /// `TESForm::FormHeap_Allocate/Free`.
    pub const FORM_HEAP_ALLOCATE: usize = 0x0040_1000;
    pub const FORM_HEAP_FREE: usize = 0x0040_1010;
    /// `DataHandler**` global.
    pub const DATA_HANDLER: usize = 0x0106_CDCC;
    /// `TESObjectREFR::SetPos(x,y,z)` — the engine position setter
    /// (verified via the Anniversary-Patcher catalog 2026-08-14; the
    /// bridge uses raw field writes as the Steam-safe path instead).
    pub const SET_POS: usize = 0x006F_2050;
    /// `bool Actor::GetAlertedState()` — `[this+0x60]` obj vtable +0x450.
    /// Verified via the Anniversary-Patcher catalog + GOG binary.
    pub const ACTOR_GET_ALERTED: usize = 0x006F_6C70;
    /// `bool Actor::GetSneakingState()` — `[this+0x184]` obj vtable +0x20.
    pub const ACTOR_GET_SNEAKING: usize = 0x006F_58B0;
    /// `QueueUIMessage` (HUD message queue).
    pub const QUEUE_UI_MESSAGE: usize = 0x0061_B850;
}

// ═══════════════════════════════════════════════════════════════
// Steam FO3 (post-2023 update) — re-derived 2026-08-07 from a live image
// dump (bridge OP_DUMP_IMAGE on the running Steam build, md5 8a3adab8).
// The 2023 update recompiled the engine: .text grew 10MB → 11.6MB and
// function addresses shifted (0x455190 now holds FPU garbage). Addresses
// were located by call-count fingerprinting + structural disassembly
// against the GOG binary (see docs/proton-testing.md).
pub mod fo3_steam_17 {
    /// `TESForm* LookupFormByID(u32)` — high confidence: 880+ call sites
    /// (GOG: 880), identical structure (loads form-map global 0x1224B84,
    /// tests, calls a helper). Selected automatically by
    /// `vtable::fo3_lookup_addr()`.
    pub const LOOKUP_FORM_BY_ID: usize = 0x0071_1EF0;
    /// Form-map global used by the Steam lookup (NiTPointerMap<TESForm>**).
    pub const FORM_MAP: usize = 0x0122_4B84;
    /// Script-VM argument extraction — medium confidence: ~431 call sites
    /// (GOG: 434), matching structure (arg null-check + early exit).
    pub const EXTRACT_ARGS: usize = 0x0078_7530;
    /// Console manager — medium confidence: 33 call sites (GOG: 33).
    pub const CONSOLE_MANAGER_GET_SINGLETON: usize = 0x0078_8B30;
}

// ═══════════════════════════════════════════════════════════════
// Verified FNV 1.4.0.525 address table (xNVSE GameAPI.cpp RUNTIME block,
// cross-checked against the real GOG FalloutNV.exe — 32-bit PE,
// md5 0f374bae0d6c34b754d3a487d49486ba, crc32 0x881FDAF8)
pub mod fnv_14 {
    /// FNV has NO direct LookupFormByID function — xNVSE wraps the form-map
    /// global: `*(NiTPointerMap<TESForm>**)0x11C54C0` then `->Lookup(id)`.
    pub const FORM_MAP: usize = 0x011C_54C0;
    /// Script-VM argument extraction.
    pub const EXTRACT_ARGS: usize = 0x005A_CCB0;
    /// `void* CreateFormInstance(...)`.
    pub const CREATE_FORM_INSTANCE: usize = 0x0046_5110;
    /// `ConsoleManager* ConsoleManager_GetSingleton(bool)`.
    pub const CONSOLE_MANAGER_GET_SINGLETON: usize = 0x0071_B160;
    /// `TESForm::FormHeap_Allocate/Free`.
    pub const FORM_HEAP_ALLOCATE: usize = 0x0040_1000;
    pub const FORM_HEAP_FREE: usize = 0x0040_1030;
    /// `bool* bEchoConsole` global.
    pub const ECHO_CONSOLE: usize = 0x011F_158C;
    /// `TESForm* GetFormByID(...)` — LookupFormByID's neighbor.
    pub const GET_FORM_BY_ID: usize = 0x0048_3A00;
}

pub fn is_fnv() -> bool {
    GAME_ENGINE.load(Ordering::SeqCst) == 1
}

/// Install all hooks. Called from DllMain on DLL_PROCESS_ATTACH.
pub fn install() {
    HOOKS_INSTALLED.store(true, Ordering::SeqCst);
    // Steam respawn disable (sites verified 2026-08-08, docs/steam-re.md).
    // Byte-guarded inside — no-op on non-matching builds (GOG/classic, launcher).
    unsafe {
        vaultmp::apply_steam_respawn();
    }
    // Full vaultmp behavior-patch set (respawn, fire relay, activate,
    // PlaceAtMe, anim/idle forwarding) on the classic/GOG build. Byte-
    // guarded — no-op on Steam/Anniversary (sites differ).
    unsafe {
        vaultmp::apply_classic_vaultmp();
    }
    // Actor-discovery detour (classic FO3: 0x6FAE90 AI predicate, verified
    // 2026-08-13 — the engine's per-actor processing gate). Byte-guarded —
    // no-op on Steam until the Steam AI-pause address is re-derived
    // (steam-re.md remaining site groups).
    vaultmp::apply_actor_discovery();
    // FNV per-frame player-state hook (0x86B386 main-loop call, NVSE anchor).
    // Byte-guarded — no-op on FO3.
    vaultmp::apply_fnv_frame_hook();
    // FO3 classic per-frame player-state hook (0x6EEB2F frame-body call).
    // Byte-guarded — no-op on the Steam/Anniversary build (downgrade path).
    vaultmp::apply_fo3_frame_hook();
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
    unsafe { vtable::get_scale(ref_id) }
}

pub fn set_scale(ref_id: u32, scale: f32) {
    unsafe { vtable::set_scale(ref_id, scale) }
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
#[inline]
pub fn get_combat_target(ref_id: u32) -> u32 {
    unsafe { vtable::get_combat_target(ref_id) }
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
    // TODO: PlayerCharacter::SetReputation()
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
    // TODO: PlayerCharacter::SetHardcoreStats()
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
    // Remote activation confirmation: return the ref if it resolves to a
    // live object (safe field lookup — no vtable call, Steam-safe). The
    // engine-side activation is intercepted by the vaultmp get_activate
    // hook (EVENT_ACTIVATE emission); this getter confirms the target
    // exists so the client sees the propagation succeeded.
    #[cfg(target_arch = "x86")]
    unsafe {
        let obj = crate::hooks::vtable::lookup_form_by_id(ref_id);
        if obj.is_null() {
            return 0;
        }
        ref_id
    }
    #[cfg(not(target_arch = "x86"))]
    {
        let _ = ref_id;
        0
    }
}

/// Apply a remote fire: call the engine's weapon-fire routine on the
/// shooter (thiscall). Resolves the per-build fire routine (classic GOG
/// 0x4BE1A0 / Steam 0x770880 — both E8-verified 2026-08-14) and runs it.
/// Returns 1 on success (the ref resolved), 0 when the shooter is invalid.
///
/// ponytail: the engine fire routine is called with only `this`; the
/// weapon ref is informational (the engine re-reads the actor's equipped
/// weapon). Live verification needed for exact argument shape.
pub fn fire_weapon(shooter_ref: u32, _weapon: u32) -> u32 {
    #[cfg(target_arch = "x86")]
    unsafe {
        let obj = crate::hooks::vtable::lookup_form_by_id(shooter_ref);
        if obj.is_null() {
            return 0;
        }
        // Pick the fire routine: classic GOG by prologue, Steam by table.
        let addr = crate::hooks::vaultmp::fire_routine_addr();
        if addr == 0 {
            return 0;
        }
        let _: u32 = crate::hooks::address::call_thiscall_0(addr, obj);
        1
    }
    #[cfg(not(target_arch = "x86"))]
    {
        let _ = shooter_ref;
        0
    }
}

/// Read the enabled state of a reference (VTable/field read, FO3/FNV aware).
#[inline]
pub fn get_enabled(ref_id: u32) -> bool {
    unsafe { vtable::get_enabled(ref_id) }
}

/// Get the lock object pointer for a reference (GetLocked vtable call).
#[inline]
pub fn get_lock(ref_id: u32) -> u32 {
    unsafe { vtable::get_lock(ref_id) }
}

/// Get base FormID via VTable chain: GetBaseForm → GetFormID.
#[inline]
pub fn get_base(ref_id: u32) -> u32 {
    unsafe { vtable::get_base(ref_id) }
}

/// Get display name via the VTable chain GetBaseForm → GetFullName.
#[inline]
pub fn get_name(ref_id: u32) -> String {
    unsafe { vtable::get_name(ref_id) }
}

// ═══════════════════════════════════════════════════════════════
// NVSE/FOSE Integration
// ═══════════════════════════════════════════════════════════════
//
// Event sinks live in `crate::events` (BSTEventSink pattern, struct-pointer
// callbacks). This module bridges engine events into pipe frames for the
// native client (`encode_event_frame`). Console commands stay here as an
// in-memory registry for testing; the real implementation hooks the engine's
// console command table.
//
// ponytail: in-memory registries for testing. Real implementation
// replaces these with NVSE CommandTable + BSTEventSink subclass.

static CONSOLE_COMMANDS: LazyLock<Mutex<HashMap<String, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Encode an engine event as a pipe frame for the native client:
/// `[PIPE_OP_EVENT][event_type:4B LE][event struct bytes...]`.
///
/// The event struct is interpreted per `event_type` (see `crate::events`).
/// Returns `None` for unknown event types or a null event pointer.
pub fn encode_event_frame(event_type: u32, event_data: *const std::ffi::c_void) -> Option<Vec<u8>> {
    use crate::events::{
        TESActivateEvent, TESCellChangeEvent, TESDeathEvent, TESEquipEvent, TESHitEvent,
        TESLoadGameEvent, TESMagicEffectApplyEvent, EVENT_ON_ACTIVATE, EVENT_ON_CELL_CHANGE,
        EVENT_ON_DEATH, EVENT_ON_EQUIP, EVENT_ON_HIT, EVENT_ON_LOAD_GAME, EVENT_ON_MAGIC_EFFECT,
    };

    if event_data.is_null() {
        return None;
    }
    let payload = unsafe {
        let bytes = |ptr: *const u8, len: usize| std::slice::from_raw_parts(ptr, len);
        match event_type {
            EVENT_ON_HIT => bytes(event_data as *const u8, std::mem::size_of::<TESHitEvent>()),
            EVENT_ON_ACTIVATE => {
                bytes(event_data as *const u8, std::mem::size_of::<TESActivateEvent>())
            }
            EVENT_ON_EQUIP => bytes(event_data as *const u8, std::mem::size_of::<TESEquipEvent>()),
            EVENT_ON_CELL_CHANGE => {
                bytes(event_data as *const u8, std::mem::size_of::<TESCellChangeEvent>())
            }
            EVENT_ON_DEATH => bytes(event_data as *const u8, std::mem::size_of::<TESDeathEvent>()),
            EVENT_ON_LOAD_GAME => {
                bytes(event_data as *const u8, std::mem::size_of::<TESLoadGameEvent>())
            }
            EVENT_ON_MAGIC_EFFECT => {
                bytes(event_data as *const u8, std::mem::size_of::<TESMagicEffectApplyEvent>())
            }
            _ => return None,
        }
    };
    let mut frame = Vec::with_capacity(1 + 4 + payload.len());
    frame.push(crate::network::PIPE_OP_EVENT);
    frame.extend_from_slice(&event_type.to_le_bytes());
    frame.extend_from_slice(payload);
    Some(frame)
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
///
/// Reads the death-state field `Actor+0xFC` (survived the Steam recompile —
/// the respawn handler does `cmp eax,2; je` there, and the AI predicate
/// checks cmp 5/3; non-zero = dead/death flow active). Field read, not a
/// vtable call — Steam-safe (2026-08-14).
pub fn is_dead(ref_id: u32) -> bool {
    #[cfg(target_arch = "x86")]
    unsafe {
        let obj = crate::hooks::vtable::lookup_form_by_id(ref_id);
        if obj.is_null() {
            return false;
        }
        crate::hooks::vtable::read_field::<u32>(obj, 0xFC) != 0
    }
    #[cfg(not(target_arch = "x86"))]
    {
        let _ = ref_id;
        false
    }
}

/// Server-authoritative respawn gate. The vaultmp respawn_detour hook calls
/// this: when false (the default for a running server), the SP respawn path
/// stays disabled so players remain dead until the server revives them.
/// The Steam respawn-disable patch (apply_steam_respawn) is the byte-level
/// enforcement; this flag is the hook-level toggle for the classic build.
pub fn set_respawn_allowed(_allowed: bool) {
    // The respawn-disable byte patches (classic + Steam) already prevent
    // the SP auto-respawn at the engine level. This hook is the semantic
    // gate for the classic ToggleRespawn path — kept as a no-op toggle
    // until the server revive op lands (ponytail: the byte patches are the
    // enforcement; this is the wiring point for OP_REVIVE).
}

/// Get the cell FormID this reference currently occupies.
#[inline]
pub fn get_parent_cell(ref_id: u32) -> u32 {
    unsafe { vtable::get_parent_cell(ref_id) }
}

/// Set the parent cell FormID (raw field write, Steam-safe).
#[inline]
pub fn set_parent_cell(ref_id: u32, cell: u32) {
    unsafe { vtable::set_parent_cell(ref_id, cell) }
}

/// Set the enabled flag (raw field write, Steam-safe).
#[inline]
pub fn set_enabled(ref_id: u32, enabled: bool) {
    unsafe { vtable::set_enabled(ref_id, enabled) }
}

/// Set the lock state (raw byte write at +0xA bit 0, Steam-safe — matches
/// the verified lock-state getter).
#[inline]
pub fn set_lock(ref_id: u32, locked: bool) {
    unsafe { vtable::set_lock(ref_id, locked) }
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

// ═══════════════════════════════════════════════════════════════
// Save-dir probe (debug)
// ═══════════════════════════════════════════════════════════════

/// Temporary debug opcode: resolve the save dir the way the GAME does
/// (SHGetFolderPath + ini SLocalSavePath) and count .fos files.
/// Returns: <personal>\0<save_dir>\0<fos_count:4 LE><dir_exists:1>
#[cfg(target_os = "windows")]
pub fn probe_saves() -> Vec<u8> {
    use windows_sys::Win32::Storage::FileSystem::{
        FindClose, FindFirstFileA, FindNextFileA, WIN32_FIND_DATAA,
    };
    use windows_sys::Win32::UI::Shell::{SHGetFolderPathA, CSIDL_PERSONAL};

    let mut out = Vec::new();
    let mut personal = [0u8; 264];
    let hr = unsafe { SHGetFolderPathA(0, CSIDL_PERSONAL as i32, 0, 0, personal.as_mut_ptr()) };
    let personal_str = if hr == 0 {
        let len = personal.iter().position(|&b| b == 0).unwrap_or(personal.len());
        String::from_utf8_lossy(&personal[..len]).into_owned()
    } else {
        format!("SHGetFolderPath hr={hr:#x}")
    };
    out.extend_from_slice(personal_str.as_bytes());
    out.push(0);

    let save_dir = format!(r"{}\My Games\Fallout3\Saves", personal_str);
    let pattern = format!(r"{}\*", save_dir);
    let mut fd: WIN32_FIND_DATAA = unsafe { std::mem::zeroed() };
    let pattern_c: Vec<u8> = pattern.bytes().chain(std::iter::once(0)).collect();
    let handle = unsafe { FindFirstFileA(pattern_c.as_ptr(), &mut fd) };
    let mut count = 0u32;
    let mut exists = 0u8;
    if handle != -1 {
        exists = 1;
        loop {
            let name = &fd.cFileName;
            let name_str = String::from_utf8_lossy(name);
            if name_str.to_lowercase().ends_with(".fos") {
                count += 1;
            }
            if unsafe { FindNextFileA(handle, &mut fd) } == 0 {
                break;
            }
        }
        unsafe { FindClose(handle) };
    }
    out.extend_from_slice(save_dir.as_bytes());
    out.push(0);
    out.extend_from_slice(&count.to_le_bytes());
    out.push(exists);
    out
}

#[cfg(not(target_os = "windows"))]
pub fn probe_saves() -> Vec<u8> {
    vec![]
}

// ═══════════════════════════════════════════════════════════════
// Image dump + code probe (debug — SteamStub unpacked image analysis)
// ═══════════════════════════════════════════════════════════════

/// Read `len` bytes at an absolute address in this process. No execution.
#[cfg(target_os = "windows")]
pub fn read_bytes(addr: usize, len: usize) -> Vec<u8> {
    if addr == 0 || len == 0 || len > 64 {
        return vec![];
    }
    let slice = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
    slice.to_vec()
}

#[cfg(not(target_os = "windows"))]
pub fn read_bytes(_addr: usize, _len: usize) -> Vec<u8> {
    vec![]
}

/// Stream this process's unpacked image. Reads the PE header directly
/// (image base 0x400000 → e_lfanew → SizeOfImage) and copies the whole
/// image — no /proc/self/maps dependency (that fails inside Proton's
/// pressure-vessel container). Works for SteamStub-unpacked images because
/// the unpacker restores the PE in place.
/// Response: [PIPE_OP_RETURN_BIG][size:4][bytes] or [0x05][err:u32].
#[cfg(target_os = "windows")]
pub fn dump_image() -> Vec<u8> {
    const BASE: usize = 0x0040_0000;
    const CAP: usize = 0x2000_0000; // 512MB ceiling

    unsafe fn rd32(addr: usize) -> u32 {
        *(addr as *const u32)
    }
    unsafe fn rd16(addr: usize) -> u16 {
        *(addr as *const u16)
    }

    let e_lfanew = unsafe { rd32(BASE + 0x3C) } as usize;
    // Sanity: "PE\0\0" signature at BASE + e_lfanew.
    if e_lfanew == 0 || e_lfanew > 0x1000 || unsafe { rd16(BASE + e_lfanew) } != 0x4550 {
        return vec![0x05, 1, 0, 0, 0]; // no valid PE header
    }
    let size_of_image = unsafe { rd32(BASE + e_lfanew + 0x50) } as usize;
    if size_of_image == 0 || size_of_image > CAP {
        return vec![0x05, 2, 0, 0, 0]; // absurd SizeOfImage
    }
    let src = unsafe { std::slice::from_raw_parts(BASE as *const u8, size_of_image) };
    let mut out = Vec::with_capacity(5 + size_of_image);
    out.push(0x04); // PIPE_OP_RETURN_BIG
    out.extend_from_slice(&(size_of_image as u32).to_le_bytes());
    out.extend_from_slice(src);
    out
}

#[cfg(not(target_os = "windows"))]
pub fn dump_image() -> Vec<u8> {
    vec![0x05]
}
