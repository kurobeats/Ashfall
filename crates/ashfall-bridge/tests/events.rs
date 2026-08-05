//! Event sink + pipe event frame tests.

use ashfall_bridge::events::{
    self, TESActivateEvent, TESCellChangeEvent, TESDeathEvent, TESEquipEvent, TESHitEvent,
    TESLoadGameEvent, TESMagicEffectApplyEvent, EVENT_ON_ACTIVATE, EVENT_ON_CELL_CHANGE,
    EVENT_ON_DEATH, EVENT_ON_EQUIP, EVENT_ON_HIT, EVENT_ON_LOAD_GAME, EVENT_ON_MAGIC_EFFECT,
};
use ashfall_bridge::hooks;
use std::ffi::c_void;

// Global counters for extern "C" callbacks — callbacks MUST NOT panic.
static mut CALLBACK_HIT_FIRED: u32 = 0;
static mut CALLBACK_DEATH_FIRED: u32 = 0;
static mut LAST_EVENT_TYPE: u32 = u32::MAX;
static mut LAST_HIT_TARGET: u32 = 0;
static mut LAST_HIT_DAMAGE: f32 = 0.0;

/// Read a mutable static without forming a shared reference
/// (avoids the `static_mut_refs` lint on read-only assertions).
macro_rules! stat {
    ($s:ident) => {
        unsafe { *std::ptr::addr_of!($s) }
    };
}

extern "C" fn hit_sink(event_type: u32, event_data: *const c_void) {
    unsafe {
        CALLBACK_HIT_FIRED += 1;
        LAST_EVENT_TYPE = event_type;
        let ev = &*(event_data as *const TESHitEvent);
        LAST_HIT_TARGET = ev.target;
        LAST_HIT_DAMAGE = ev.damage;
    }
}

extern "C" fn death_sink(_event_type: u32, _event_data: *const c_void) {
    unsafe { CALLBACK_DEATH_FIRED += 1; }
}

fn reset_counters() {
    unsafe {
        CALLBACK_HIT_FIRED = 0;
        CALLBACK_DEATH_FIRED = 0;
        LAST_EVENT_TYPE = u32::MAX;
        LAST_HIT_TARGET = 0;
        LAST_HIT_DAMAGE = 0.0;
    }
}

#[test]
fn test_event_sink_registration() {
    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    assert!(!events::has_event_sinks(EVENT_ON_HIT));

    events::register_event_sink(EVENT_ON_HIT, hit_sink);
    assert!(events::has_event_sinks(EVENT_ON_HIT));

    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    assert!(!events::has_event_sinks(EVENT_ON_HIT));
}

#[test]
fn test_event_sink_dispatch_passes_struct() {
    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    reset_counters();

    events::register_event_sink(EVENT_ON_HIT, hit_sink);

    let ev = TESHitEvent {
        target: 0x1234,
        attacker: 0x5678,
        damage: 25.5,
        weapon: 0x999,
        projectile: 0,
        flags: 0,
    };
    let count = events::dispatch_event(EVENT_ON_HIT, &ev as *const _ as *const c_void);
    assert_eq!(count, 1);
    assert_eq!(stat!(CALLBACK_HIT_FIRED), 1);
    assert_eq!(stat!(LAST_EVENT_TYPE), EVENT_ON_HIT);
    assert_eq!(stat!(LAST_HIT_TARGET), 0x1234);
    assert_eq!(stat!(LAST_HIT_DAMAGE), 25.5);

    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
}

#[test]
fn test_event_sink_multiple_sinks_per_type() {
    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    events::unregister_event_sink(EVENT_ON_HIT, death_sink);
    reset_counters();

    events::register_event_sink(EVENT_ON_HIT, hit_sink);
    events::register_event_sink(EVENT_ON_HIT, death_sink);

    let ev = TESHitEvent { target: 1, attacker: 2, damage: 3.0, weapon: 0, projectile: 0, flags: 0 };
    let count = events::dispatch_event(EVENT_ON_HIT, &ev as *const _ as *const c_void);
    assert_eq!(count, 2);
    assert_eq!(stat!(CALLBACK_HIT_FIRED), 1);
    assert_eq!(stat!(CALLBACK_DEATH_FIRED), 1);

    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    events::unregister_event_sink(EVENT_ON_HIT, death_sink);
}

#[test]
fn test_event_sink_unknown_type_and_empty() {
    // Unknown type: no sinks, dispatch returns 0
    assert!(!events::has_event_sinks(99));
    let count = events::dispatch_event(99, std::ptr::null());
    assert_eq!(count, 0);
}

#[test]
fn test_event_sink_multiple_types() {
    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    events::unregister_event_sink(EVENT_ON_DEATH, death_sink);

    events::register_event_sink(EVENT_ON_HIT, hit_sink);
    events::register_event_sink(EVENT_ON_DEATH, death_sink);

    assert!(events::has_event_sinks(EVENT_ON_HIT));
    assert!(events::has_event_sinks(EVENT_ON_DEATH));

    reset_counters();
    let ev = TESDeathEvent { actor: 7, killer: 8, limbs: 2, cause: 1 };
    let count = events::dispatch_event(EVENT_ON_DEATH, &ev as *const _ as *const c_void);
    assert_eq!(count, 1);
    assert_eq!(stat!(CALLBACK_DEATH_FIRED), 1);

    events::unregister_event_sink(EVENT_ON_HIT, hit_sink);
    events::unregister_event_sink(EVENT_ON_DEATH, death_sink);
}

// ── Pipe event frames (hooks::encode_event_frame) ──

fn frame_event_type(frame: &[u8]) -> u32 {
    u32::from_le_bytes([frame[1], frame[2], frame[3], frame[4]])
}

#[test]
fn test_event_frame_hit() {
    let ev = TESHitEvent {
        target: 0x1234,
        attacker: 0x5678,
        damage: 25.5,
        weapon: 0x999,
        projectile: 0xAA,
        flags: 1,
    };
    let frame = hooks::encode_event_frame(EVENT_ON_HIT, &ev as *const _ as *const c_void).unwrap();

    assert_eq!(frame[0], ashfall_bridge::network::PIPE_OP_EVENT);
    assert_eq!(frame_event_type(&frame), EVENT_ON_HIT);
    assert_eq!(frame.len(), 1 + 4 + std::mem::size_of::<TESHitEvent>());

    // Spot-check payload fields at their struct offsets
    assert_eq!(u32::from_le_bytes([frame[5], frame[6], frame[7], frame[8]]), 0x1234);
    assert_eq!(u32::from_le_bytes([frame[9], frame[10], frame[11], frame[12]]), 0x5678);
    let damage = f32::from_le_bytes([frame[13], frame[14], frame[15], frame[16]]);
    assert_eq!(damage, 25.5);
    assert_eq!(u32::from_le_bytes([frame[17], frame[18], frame[19], frame[20]]), 0x999);
}

#[test]
fn test_event_frame_equip_roundtrip_payload() {
    let ev = TESEquipEvent { actor: 1, base_obj: 2, equip_slot: 3, equipped: true };
    let frame = hooks::encode_event_frame(EVENT_ON_EQUIP, &ev as *const _ as *const c_void).unwrap();
    assert_eq!(frame[0], ashfall_bridge::network::PIPE_OP_EVENT);
    assert_eq!(frame_event_type(&frame), EVENT_ON_EQUIP);
    assert_eq!(frame.len(), 1 + 4 + std::mem::size_of::<TESEquipEvent>());
    // equipped bool is the last struct field (payload offset 12 = frame offset 17)
    assert_eq!(frame[17], 1);
}

#[test]
fn test_event_frame_all_types_len() {
    let hit = TESHitEvent { target: 0, attacker: 0, damage: 0.0, weapon: 0, projectile: 0, flags: 0 };
    let act = TESActivateEvent { activator: 0, target: 0 };
    let equ = TESEquipEvent { actor: 0, base_obj: 0, equip_slot: 0, equipped: false };
    let cell = TESCellChangeEvent { reference: 0, old_cell: 0, new_cell: 0 };
    let death = TESDeathEvent { actor: 0, killer: 0, limbs: 0, cause: 0 };
    let load = TESLoadGameEvent { loaded: true };
    let magic = TESMagicEffectApplyEvent { caster: 0, target: 0, effect_code: 0, magnitude: 0.0 };

    let cases = [
        (EVENT_ON_HIT, &hit as *const _ as *const c_void, std::mem::size_of::<TESHitEvent>()),
        (EVENT_ON_ACTIVATE, &act as *const _ as *const c_void, std::mem::size_of::<TESActivateEvent>()),
        (EVENT_ON_EQUIP, &equ as *const _ as *const c_void, std::mem::size_of::<TESEquipEvent>()),
        (EVENT_ON_CELL_CHANGE, &cell as *const _ as *const c_void, std::mem::size_of::<TESCellChangeEvent>()),
        (EVENT_ON_DEATH, &death as *const _ as *const c_void, std::mem::size_of::<TESDeathEvent>()),
        (EVENT_ON_LOAD_GAME, &load as *const _ as *const c_void, std::mem::size_of::<TESLoadGameEvent>()),
        (EVENT_ON_MAGIC_EFFECT, &magic as *const _ as *const c_void, std::mem::size_of::<TESMagicEffectApplyEvent>()),
    ];
    for (event_type, ptr, size) in cases {
        let frame = hooks::encode_event_frame(event_type, ptr).unwrap();
        assert_eq!(frame.len(), 1 + 4 + size, "event type {event_type}");
        assert_eq!(frame_event_type(&frame), event_type);
    }
}

#[test]
fn test_event_frame_unknown_and_null() {
    // Unknown event type → None
    assert!(hooks::encode_event_frame(99, &0u32 as *const _ as *const c_void).is_none());
    // Null event pointer → None
    assert!(hooks::encode_event_frame(EVENT_ON_HIT, std::ptr::null()).is_none());
}
