//! VTable access patterns and Gamebryo field offset helpers.
//!
//! Ported from vaultmp-extended vaultmpdll/vaultmp.cpp (inline asm field reads,
//! GetPosAngle, GetActorState) and FOSE community VTable offset knowledge.
//!
//! All functions are unsafe — caller must ensure pointers are valid.
//! VTable approach preferred (version-independent), raw offsets as fallback.

use std::ptr;

// ═══════════════════════════════════════════════════════════════
// Architecture detection
// ═══════════════════════════════════════════════════════════════

/// Size of a vtable entry in bytes: 4 on x86, 8 on x86_64.
#[cfg(target_arch = "x86")]
const VTABLE_ENTRY_SIZE: usize = 4;

#[cfg(target_arch = "x86_64")]
const VTABLE_ENTRY_SIZE: usize = 8;

/// Convert a vtable byte-offset to an entry index.
#[inline(always)]
pub const fn vtable_index(byte_offset: usize) -> usize {
    byte_offset / VTABLE_ENTRY_SIZE
}

// ═══════════════════════════════════════════════════════════════
// VTable access primitives
// ═══════════════════════════════════════════════════════════════

/// Read a VTable entry at `index` (offset / ptr_size).
/// `object` points to a C++ object whose first field is the vtable pointer.
/// Returns None if the vtable slot is null.
pub unsafe fn vtable_entry<T>(object: *mut u8, index: usize) -> Option<T> {
    if object.is_null() {
        return None;
    }
    let vtable = ptr::read(object as *const *const usize);
    if vtable.is_null() {
        return None;
    }
    let entry_ptr = vtable.add(index);
    let entry_value = ptr::read(entry_ptr);
    if entry_value == 0 {
        return None;
    }
    Some(std::mem::transmute_copy(&entry_value))
}

/// Call a C++ virtual method at vtable[index] with no arguments beyond `this`.
/// On Windows ABI (x86 thiscall / x86_64): `this` goes in ECX/RCX.
/// `extern "system"` = Windows ABI.
pub unsafe fn vcall_0<R: Copy>(obj: *mut u8, index: usize) -> R {
    let fn_ptr: unsafe extern "system" fn(*mut u8) -> R =
        vtable_entry(obj, index).expect("vcall_0: null vtable entry");
    fn_ptr(obj)
}

/// Call a C++ virtual method at vtable[index] with one argument + `this`.
pub unsafe fn vcall_1<T: Copy, R: Copy>(obj: *mut u8, index: usize, a1: T) -> R {
    let fn_ptr: unsafe extern "system" fn(*mut u8, T) -> R =
        vtable_entry(obj, index).expect("vcall_1: null vtable entry");
    fn_ptr(obj, a1)
}

// ═══════════════════════════════════════════════════════════════
// Raw field access (offset-based, from vaultmp.cpp known offsets)
// ═══════════════════════════════════════════════════════════════

/// Read a value at `object + offset`. Copied via ptr::read (no alignment req).
pub unsafe fn read_field<T: Copy>(obj: *mut u8, offset: usize) -> T {
    let addr = obj.add(offset) as *const T;
    ptr::read(addr)
}

/// Write a value at `object + offset`.
pub unsafe fn write_field<T>(obj: *mut u8, offset: usize, value: T) {
    let addr = obj.add(offset) as *mut T;
    ptr::write(addr, value);
}

// ═══════════════════════════════════════════════════════════════
// FormID resolution (FOSE LOOKUP_FORM pattern)
// ═══════════════════════════════════════════════════════════════

/// Hardcoded address of `LookupFormByID` in FO3 1.7.0.3 EN.
/// FNV equivalent: different address, detected at runtime.
const LOOKUP_FORM_FO3: usize = 0x00455190;

/// Resolve a FormID to a memory pointer. Returns null if form not loaded.
///
/// On non-Windows targets (tests), always returns null — the hardcoded
/// FO3/FNV addresses are only valid inside the Wine/Proton game process.
pub unsafe fn lookup_form_by_id(form_id: u32) -> *mut u8 {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = form_id;
        return std::ptr::null_mut();
    }

    #[cfg(target_os = "windows")]
    {
        let addr: usize = LOOKUP_FORM_FO3;
        let fn_ptr: unsafe extern "system" fn(u32) -> *mut u8 =
            std::mem::transmute(addr as *const ());
        fn_ptr(form_id)
    }
}

// ═══════════════════════════════════════════════════════════════
// Known VTable offset indices (x86)
// ═══════════════════════════════════════════════════════════════

/// TESObjectREFR::GetPos — returns [f32; 3] or similar (by ref/out param).
/// On x86_64: returned in XMM0/XMM1 or via out pointer — implementation depends on game binary.
/// ponytail: we read raw field offsets as fallback; VTable call for correctness.
const VTBL_REF_GET_POS: usize = vtable_index(0x30);        // index 12 (x86)
const VTBL_ACTOR_GET_VALUE: usize = vtable_index(0x68);    // index 26 (x86)
const VTBL_ACTOR_GET_BASE_VALUE: usize = vtable_index(0x70); // index 28 (x86, estimated)
const VTBL_ACTOR_ANIM_DATA: usize = vtable_index(0x01E4);   // index 121 (x86, vaultmp.cpp GetActorState)

// ═══════════════════════════════════════════════════════════════
// Known raw field offsets (vaultmp.cpp confirmed, FO3 1.7)
// ═══════════════════════════════════════════════════════════════

const OFFSET_REF_ID: usize = 0x0C;
const OFFSET_ANGLE_X: usize = 0x20; // radians
const OFFSET_ANGLE_Y: usize = 0x24; // radians
const OFFSET_ANGLE_Z: usize = 0x28; // radians
const OFFSET_POS_X: usize = 0x2C;
const OFFSET_POS_Y: usize = 0x30;
const OFFSET_POS_Z: usize = 0x34;
const OFFSET_GLOBAL_VALUE: usize = 0x24;

// Anim data struct offsets (from vaultmp.cpp GetActorState: VTable+0x01E4 → struct)
const OFFSET_ANIM_MOVING: usize = 0x4E;
const OFFSET_ANIM_WEAPON: usize = 0x54;
const OFFSET_ANIM_IDLE_PTR: usize = 0x118; // → +0x2C → +0x0C = idle anim BaseForm

// ═══════════════════════════════════════════════════════════════
// Concrete hook implementations
// ═══════════════════════════════════════════════════════════════

/// Read position of a reference.
/// Tries VTable GetPos first, falls back to raw field offsets.
pub unsafe fn get_pos(ref_id: u32) -> [f32; 3] {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return [0.0, 0.0, 0.0];
    }

    // Try VTable call: TESObjectREFR::GetPos()
    // On x86: GetPos([f32;3]* out) — tricky calling convention.
    // ponytail: read raw field offsets directly — same memory, faster.
    // Vaultmp does the same (vaultmp.cpp GetPosAngle reads +0x2C/+0x30/+0x34).
    let x = read_field::<f32>(obj, OFFSET_POS_X);
    let y = read_field::<f32>(obj, OFFSET_POS_Y);
    let z = read_field::<f32>(obj, OFFSET_POS_Z);
    [x, y, z]
}

/// Read angle in degrees (converted from engine radians).
/// Vaultmp convention: angles in degrees (vaultmp.cpp GetPosAngle × 180/π).
pub unsafe fn get_angle(ref_id: u32) -> [f32; 3] {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return [0.0, 0.0, 0.0];
    }

    let ax = read_field::<f32>(obj, OFFSET_ANGLE_X);
    let ay = read_field::<f32>(obj, OFFSET_ANGLE_Y);
    let az = read_field::<f32>(obj, OFFSET_ANGLE_Z);

    // vaultmp.cpp: data[n] * 180 / M_PI
    use std::f32::consts::PI;
    [ax * 180.0 / PI, ay * 180.0 / PI, az * 180.0 / PI]
}

/// Read actor animation state: (idle, moving, weapon, flags, alerted, sneaking).
/// Ported from vaultmp.cpp vaultfunction() GetActorState case.
pub unsafe fn get_actor_state(ref_id: u32) -> (u32, u8, u8, u8, bool, bool) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return (0, 0, 0, 0, false, false);
    }

    // Call VTable[0x01E4] → returns animation data pointer
    let anim_data: *mut u8 = vcall_0(obj, VTBL_ACTOR_ANIM_DATA);
    if anim_data.is_null() {
        return (0, 0, 0, 0, false, false);
    }

    let moving = read_field::<u8>(anim_data, OFFSET_ANIM_MOVING);
    let weapon = read_field::<u8>(anim_data, OFFSET_ANIM_WEAPON);

    // Idle animation: *(anim+0x118) → *(result+0x2C) → *(result+0x0C)
    let idle_ptr: u32 = read_field::<u32>(anim_data, OFFSET_ANIM_IDLE_PTR);
    let idle = if idle_ptr != 0 {
        let p1: u32 = read_field::<u32>(idle_ptr as *mut u8, 0x2C);
        if p1 != 0 {
            read_field::<u32>(p1 as *mut u8, 0x0C)
        } else {
            0
        }
    } else {
        0
    };

    // ponytail: alerted/sneaking need engine function calls (ALERTED_STATE,
    // SNEAKING_STATE from vaultmp.hpp). VTable offsets unknown for these.
    // Return false until RE completes. vaultmp uses hardcoded FO3 1.7 addresses.
    let alerted = false;
    let sneaking = false;

    // flags: diagonal movement detection not implemented (needs GetAsyncKeyState).
    // ponytail: returning 0. vaultmp's diagonal detection is unreliable anyway.
    let flags = 0u8;

    (idle, moving, weapon, flags, alerted, sneaking)
}

/// Read actor value by index (health=0x14, AP=0x15, DR=0x29, DT=0x2A for FNV).
/// Tries VTable GetActorValue first, raw field fallback.
pub unsafe fn get_actor_value(ref_id: u32, index: u8) -> f32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return 0.0;
    }

    // Try VTable call: Actor::GetActorValue(index) → f32
    match vtable_entry::<unsafe extern "system" fn(*mut u8, u8) -> f32>(obj, VTBL_ACTOR_GET_VALUE) {
        Some(get_av) => get_av(obj, index),
        None => 0.0, // ponytail: raw offset unknown for actor values, VTable needed
    }
}

/// Read base actor value by index.
/// Tries VTable GetActorBaseValue first, raw field fallback.
pub unsafe fn get_actor_base_value(ref_id: u32, index: u8) -> f32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return 0.0;
    }

    match vtable_entry::<unsafe extern "system" fn(*mut u8, u8) -> f32>(obj, VTBL_ACTOR_GET_BASE_VALUE) {
        Some(get_bav) => get_bav(obj, index),
        None => 0.0,
    }
}

/// Read the refID (FormID) of a TESObjectREFR.
/// Offset +0x0C confirmed from vaultmp.cpp GetActivate.
pub unsafe fn get_ref_id(obj: *mut u8) -> u32 {
    read_field::<u32>(obj, OFFSET_REF_ID)
}

/// Write position. Tries VTable SetPos first, raw field fallback.
pub unsafe fn set_pos(ref_id: u32, pos: [f32; 3]) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }

    // Try VTable call first
    #[cfg(target_arch = "x86_64")]
    {
        const VTBL_REF_SET_POS: usize = vtable_index(0x38); // x86_64 index 7
        if let Some(set_pos_fn) = vtable_entry::<unsafe extern "system" fn(*mut u8, f32, f32, f32)>(obj, VTBL_REF_SET_POS) {
            set_pos_fn(obj, pos[0], pos[1], pos[2]);
            return;
        }
    }

    // Fallback: raw field write (vaultmp.cpp approach via SETPOS engine function)
    write_field(obj, OFFSET_POS_X, pos[0]);
    write_field(obj, OFFSET_POS_Y, pos[1]);
    write_field(obj, OFFSET_POS_Z, pos[2]);
}

/// Set angle (accept degrees, convert to radians for engine).
pub unsafe fn set_angle(ref_id: u32, angle: [f32; 3]) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }

    use std::f32::consts::PI;
    write_field(obj, OFFSET_ANGLE_X, angle[0] * PI / 180.0);
    write_field(obj, OFFSET_ANGLE_Y, angle[1] * PI / 180.0);
    write_field(obj, OFFSET_ANGLE_Z, angle[2] * PI / 180.0);
}

/// Set actor value by index.
pub unsafe fn set_actor_value(ref_id: u32, index: u8, value: f32) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }

    const VTBL_ACTOR_SET_VALUE: usize = vtable_index(0x6C); // index 27 (x86, estimated)
    if let Some(set_av) = vtable_entry::<unsafe extern "system" fn(*mut u8, u8, f32)>(obj, VTBL_ACTOR_SET_VALUE) {
        set_av(obj, index, value);
    }
}

/// Read cell of a reference.
pub unsafe fn get_cell(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return 0;
    }
    // ponytail: cell field offset unknown. Returns 0 until RE.
    0
}

/// Read base FormID (TESObjectREFR::GetBaseForm → TESForm::GetFormID).
pub unsafe fn get_base(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return 0;
    }

    // VTable GetBaseForm (index 4 on x86) → returns TESForm*
    const VTBL_REF_GET_BASE_FORM: usize = vtable_index(0x10);
    let base_form: u32 = vcall_0(obj, VTBL_REF_GET_BASE_FORM);
    if base_form == 0 {
        return 0;
    }

    // VTable GetFormID (index 1) on the base form
    const VTBL_FORM_GET_FORM_ID: usize = vtable_index(0x04);
    vcall_0(base_form as *mut u8, VTBL_FORM_GET_FORM_ID)
}

// ═══════════════════════════════════════════════════════════════
// Tests — operate on a local buffer to verify primitives
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a fake C++ object: [vtable_ptr][field0: u32][field1: f32][field2: f32][field3: f32]
    unsafe fn make_fake_obj(vtable_addr: *const usize) -> (Vec<u8>, *mut u8) {
        let mut buf = vec![0u8; 64];
        let ptr = buf.as_mut_ptr();
        // Write vtable pointer at offset 0
        ptr::write(ptr as *mut *const usize, vtable_addr);
        (buf, ptr)
    }

    #[test]
    fn test_read_field_u32() {
        let mut buf = vec![0u8; 32];
        let ptr = buf.as_mut_ptr();
        unsafe {
            write_field::<u32>(ptr, 0x0C, 0xDEADBEEF);
            let val: u32 = read_field(ptr, 0x0C);
            assert_eq!(val, 0xDEADBEEF);
        }
    }

    #[test]
    fn test_read_field_f32_chain() {
        let mut buf = vec![0u8; 64];
        let ptr = buf.as_mut_ptr();
        unsafe {
            // Write pos at known offsets
            write_field::<f32>(ptr, OFFSET_POS_X, 1.0f32);
            write_field::<f32>(ptr, OFFSET_POS_Y, 2.0f32);
            write_field::<f32>(ptr, OFFSET_POS_Z, 3.0f32);

            let x: f32 = read_field(ptr, OFFSET_POS_X);
            let y: f32 = read_field(ptr, OFFSET_POS_Y);
            let z: f32 = read_field(ptr, OFFSET_POS_Z);
            assert_eq!((x, y, z), (1.0, 2.0, 3.0));
        }
    }

    #[test]
    fn test_write_and_read_angle_conversion() {
        let mut buf = vec![0u8; 64];
        let ptr = buf.as_mut_ptr();
        unsafe {
            // Write angles in radians
            write_field::<f32>(ptr, OFFSET_ANGLE_X, std::f32::consts::PI); // 180°
            write_field::<f32>(ptr, OFFSET_ANGLE_Y, std::f32::consts::FRAC_PI_2); // 90°
            write_field::<f32>(ptr, OFFSET_ANGLE_Z, 0.0);

            let ax: f32 = read_field(ptr, OFFSET_ANGLE_X);
            let ay: f32 = read_field(ptr, OFFSET_ANGLE_Y);
            let az: f32 = read_field(ptr, OFFSET_ANGLE_Z);

            // Convert to degrees (vaultmp convention)
            let dx = ax * 180.0 / std::f32::consts::PI;
            let dy = ay * 180.0 / std::f32::consts::PI;
            let dz = az * 180.0 / std::f32::consts::PI;

            assert!((dx - 180.0).abs() < 0.001, "dx={dx}");
            assert!((dy - 90.0).abs() < 0.001, "dy={dy}");
            assert!(dz.abs() < 0.001, "dz={dz}");
        }
    }

    #[test]
    fn test_vtable_entry_null_object() {
        unsafe {
            let result: Option<usize> = vtable_entry(std::ptr::null_mut(), 0);
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_vtable_entry_null_vtable() {
        let mut buf = vec![0u8; 16];
        let ptr = buf.as_mut_ptr();
        unsafe {
            // Write null vtable pointer
            ptr::write::<usize>(ptr as *mut usize, 0);
            let result: Option<usize> = vtable_entry(ptr, 0);
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_lookup_form_by_id_null_on_bogus() {
        unsafe {
            let ptr = lookup_form_by_id(0);
            // On non-Windows: always null. On Windows: 0 is bogus FormID → likely null.
            assert!(ptr.is_null());
        }
    }

    #[test]
    fn test_write_field_u8() {
        let mut buf = vec![0u8; 16];
        let ptr = buf.as_mut_ptr();
        unsafe {
            write_field::<u8>(ptr, 4, 0x7B);
            write_field::<u8>(ptr, 5, 0x42);
            assert_eq!(read_field::<u8>(ptr, 4), 0x7B);
            assert_eq!(read_field::<u8>(ptr, 5), 0x42);
        }
    }

    #[test]
    fn test_vtable_index_x86() {
        // x86: entry_size=4. VTable offset 0x30 → index 12.
        #[cfg(target_arch = "x86")]
        {
            assert_eq!(VTABLE_ENTRY_SIZE, 4);
            assert_eq!(vtable_index(0x30), 12);
            assert_eq!(vtable_index(0x68), 26);
            assert_eq!(vtable_index(0x01E4), 121);
        }
        // x86_64: entry_size=8. VTable offset 0x30 → index 6.
        #[cfg(target_arch = "x86_64")]
        {
            assert_eq!(VTABLE_ENTRY_SIZE, 8);
            assert_eq!(vtable_index(0x30), 6);
            assert_eq!(vtable_index(0x68), 13);
        }
    }
}
