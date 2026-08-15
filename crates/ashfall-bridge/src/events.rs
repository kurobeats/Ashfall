//! NVSE EventSink types and registration.
//!
//! Gamebryo engine dispatches events via `BSTEventSink<T>` virtual classes.
//! Bridge registers callbacks that fire when engine events occur, encoding
//! them as pipe frames to the native client (see `hooks::encode_event_frame`).
//!
//! This is the authoritative event registry. `hooks::register_event_sink` was
//! removed — hooks/mod.rs bridges these events to pipe frames instead.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// TESHitEvent — dispatched when an actor takes damage.
#[repr(C)]
pub struct TESHitEvent {
    pub target: u32,   // RefID
    pub attacker: u32, // RefID
    pub damage: f32,
    pub weapon: u32,     // FormID
    pub projectile: u32, // FormID
    pub flags: u32,
}

/// TESActivateEvent — dispatched when an object is activated.
#[repr(C)]
pub struct TESActivateEvent {
    pub activator: u32, // RefID
    pub target: u32,    // RefID
}

/// TESEquipEvent — dispatched when equipment is equipped/unequipped.
#[repr(C)]
pub struct TESEquipEvent {
    pub actor: u32,    // RefID
    pub base_obj: u32, // FormID
    pub equip_slot: u32,
    pub equipped: bool,
}

/// TESCellChangeEvent — dispatched when a reference changes cell.
#[repr(C)]
pub struct TESCellChangeEvent {
    pub reference: u32, // RefID
    pub old_cell: u32,
    pub new_cell: u32,
}

/// TESDeathEvent — dispatched when an actor dies.
#[repr(C)]
pub struct TESDeathEvent {
    pub actor: u32,  // RefID
    pub killer: u32, // RefID
    pub limbs: u16,
    pub cause: i8,
}

/// TESLoadGameEvent — dispatched when a save game is loaded.
#[repr(C)]
pub struct TESLoadGameEvent {
    pub loaded: bool,
}

/// TESMagicEffectApplyEvent — dispatched when a magic effect applies.
#[repr(C)]
pub struct TESMagicEffectApplyEvent {
    pub caster: u32,      // RefID
    pub target: u32,      // RefID
    pub effect_code: u32, // Magic effect FormID
    pub magnitude: f32,
}

/// Event type identifiers for sink registration.
pub const EVENT_ON_HIT: u32 = 0;
pub const EVENT_ON_ACTIVATE: u32 = 1;
pub const EVENT_ON_EQUIP: u32 = 2;
pub const EVENT_ON_CELL_CHANGE: u32 = 3;
pub const EVENT_ON_DEATH: u32 = 4;
pub const EVENT_ON_LOAD_GAME: u32 = 5;
pub const EVENT_ON_MAGIC_EFFECT: u32 = 6;

/// Callback type for event handlers.
/// - `event_type`: one of EVENT_ON_* constants
/// - `event_data`: pointer to the event struct for that type
pub type EventCallback = extern "C" fn(event_type: u32, event_data: *const std::ffi::c_void);

/// Event sink registry — multiple sinks per event type.
static EVENT_SINKS: LazyLock<Mutex<HashMap<u32, Vec<EventCallback>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Register an event callback. Called during plugin init.
/// Multiple callbacks may register for the same event type.
pub fn register_event_sink(event_type: u32, callback: EventCallback) {
    EVENT_SINKS
        .lock()
        .unwrap()
        .entry(event_type)
        .or_default()
        .push(callback);
}

/// Unregister an event callback (by function pointer identity).
pub fn unregister_event_sink(event_type: u32, callback: EventCallback) {
    let mut sinks = EVENT_SINKS.lock().unwrap();
    if let Some(list) = sinks.get_mut(&event_type) {
        list.retain(|&cb| cb as usize != callback as usize);
    }
}

/// Check whether any sinks are registered for an event type.
pub fn has_event_sinks(event_type: u32) -> bool {
    EVENT_SINKS
        .lock()
        .unwrap()
        .get(&event_type)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// Dispatch an event to the registered sinks (called from VTable hooks).
/// Returns the number of callbacks fired.
pub fn dispatch_event(event_type: u32, event_data: *const std::ffi::c_void) -> usize {
    let sinks = EVENT_SINKS.lock().unwrap();
    let callbacks = match sinks.get(&event_type) {
        Some(list) if !list.is_empty() => list.clone(),
        _ => return 0,
    };
    drop(sinks);
    for cb in &callbacks {
        cb(event_type, event_data);
    }
    callbacks.len()
}
