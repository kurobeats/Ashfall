//! NVSE EventSink types and registration stubs.
//!
//! Gamebryo engine dispatches events via `BSTEventSink<T>` virtual classes.
//! Bridge registers callbacks that fire when engine events occur,
//! encoding them as pipe commands to the native client.

/// TESHitEvent — dispatched when an actor takes damage.
#[repr(C)]
pub struct TESHitEvent {
    pub target: u32,      // RefID
    pub attacker: u32,    // RefID
    pub damage: f32,
    pub weapon: u32,      // FormID
    pub projectile: u32,  // FormID
    pub flags: u32,
}

/// TESActivateEvent — dispatched when an object is activated.
#[repr(C)]
pub struct TESActivateEvent {
    pub activator: u32,   // RefID
    pub target: u32,      // RefID
}

/// TESEquipEvent — dispatched when equipment is equipped/unequipped.
#[repr(C)]
pub struct TESEquipEvent {
    pub actor: u32,       // RefID
    pub base_obj: u32,    // FormID
    pub equip_slot: u32,
    pub equipped: bool,
}

/// TESCellChangeEvent — dispatched when a reference changes cell.
#[repr(C)]
pub struct TESCellChangeEvent {
    pub reference: u32,   // RefID
    pub old_cell: u32,
    pub new_cell: u32,
}

/// TESDeathEvent — dispatched when an actor dies.
#[repr(C)]
pub struct TESDeathEvent {
    pub actor: u32,       // RefID
    pub killer: u32,      // RefID
    pub limbs: u16,
    pub cause: i8,
}

/// Event type identifiers for sink registration.
pub const EVENT_ON_HIT: u32 = 0;
pub const EVENT_ON_ACTIVATE: u32 = 1;
pub const EVENT_ON_EQUIP: u32 = 2;
pub const EVENT_ON_CELL_CHANGE: u32 = 3;
pub const EVENT_ON_DEATH: u32 = 4;

/// Maximum number of event sinks.
const MAX_SINKS: usize = 5;

/// Callback type for event handlers.
/// - `event_type`: one of EVENT_ON_* constants
/// - `event_data`: pointer to the event struct (cast to appropriate type)
pub type EventCallback = extern "C" fn(event_type: u32, event_data: *const std::ffi::c_void);

/// Event sink registry — static array of optional callbacks.
static mut EVENT_SINKS: Option<[Option<EventCallback>; MAX_SINKS]> = None;

/// Register an event callback. Called during plugin init.
/// Only one callback per event type.
pub fn register_event_sink(event_type: u32, callback: EventCallback) {
    unsafe {
        if EVENT_SINKS.is_none() {
            EVENT_SINKS = Some([None, None, None, None, None]);
        }
        if let Some(sinks) = &mut EVENT_SINKS {
            let idx = event_type as usize;
            if idx < MAX_SINKS {
                sinks[idx] = Some(callback);
            }
        }
    }
}

/// Dispatch an event to the registered sink (called from VTable hooks).
pub fn dispatch_event(event_type: u32, event_data: *const std::ffi::c_void) {
    unsafe {
        if let Some(sinks) = &EVENT_SINKS {
            let idx = event_type as usize;
            if idx < MAX_SINKS {
                if let Some(callback) = sinks[idx] {
                    callback(event_type, event_data);
                }
            }
        }
    }
}
