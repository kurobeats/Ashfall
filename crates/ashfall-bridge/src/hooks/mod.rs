//! Gamebryo engine hooks — VTable patching for Fallout 3 / New Vegas.
//!
//! Hooks intercept engine functions to read/write game state.
//! Pattern: replace vtable entry → call original → read result.
//!
//! ponytail: stubs return zero/default. Real VTable offsets filled in
//! when reverse-engineered from Fallout3.exe + FOSE.

/// Install all hooks. Called from DllMain on DLL_PROCESS_ATTACH.
pub fn install() {
    // TODO: locate TESObjectREFR vtable, patch GetPos/SetPos/etc.
    // For Proton: same VTable layout as Windows — Wine mirrors the binary exactly.
}

/// Uninstall all hooks. Called from DllMain on DLL_PROCESS_DETACH.
pub fn uninstall() {
    // TODO: restore original vtable entries
}

// ── Position ──

/// Get position of a reference by refID.
/// Returns [X, Y, Z] as floats.
pub fn get_pos(ref_id: u32) -> [f32; 3] {
    let _ = ref_id;
    // TODO: call TESObjectREFR::GetPos through vtable
    [0.0, 0.0, 0.0]
}

/// Set position of a reference by refID.
pub fn set_pos(ref_id: u32, pos: [f32; 3]) {
    let _ = (ref_id, pos);
    // TODO: call TESObjectREFR::SetPos through vtable
}

// ── Angle ──

/// Get rotation angles of a reference.
/// Returns [X, Y, Z] in degrees.
pub fn get_angle(ref_id: u32) -> [f32; 3] {
    let _ = ref_id;
    [0.0, 0.0, 0.0]
}

/// Set rotation angles of a reference.
pub fn set_angle(ref_id: u32, angle: [f32; 3]) {
    let _ = (ref_id, angle);
}

// ── Actor State ──

/// Get actor animation state.
pub fn get_actor_state(ref_id: u32) -> (u32, u8, u8, u8, bool, bool) {
    let _ = ref_id;
    // (idle, moving, weapon, flags, alerted, sneaking)
    (0, 0, 0, 0, false, false)
}

/// Get actor value by index (health, skills, SPECIAL, etc.).
pub fn get_actor_value(ref_id: u32, index: u8) -> f32 {
    let _ = (ref_id, index);
    0.0
}

/// Set actor value.
pub fn set_actor_value(ref_id: u32, index: u8, value: f32) {
    let _ = (ref_id, index, value);
}

// ── Controls ──

/// Get player control state (key binding for a control index).
pub fn get_control(ref_id: u32, control: u8) -> u8 {
    let _ = (ref_id, control);
    0
}

/// Enable/disable a player control.
pub fn set_control(ref_id: u32, control: u8, enabled: bool) {
    let _ = (ref_id, control, enabled);
}

// ── Misc ──

/// Get the base ID of a reference.
pub fn get_base(ref_id: u32) -> u32 {
    let _ = ref_id;
    0
}

/// Get the name of a reference.
pub fn get_name(ref_id: u32) -> String {
    let _ = ref_id;
    String::new()
}
