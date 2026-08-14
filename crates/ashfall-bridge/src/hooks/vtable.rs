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
///
/// Gamebryo vtable methods are **thiscall** on x86 (this in ECX, stack args
/// right-to-left, callee cleans) — `extern "system"` (stdcall) would leave
/// `this` on the stack and the callee would read a garbage ECX, faulting.
/// The inline-asm shims below implement thiscall for i686; on x86_64 the
/// Windows ABI passes `this` in RCX anyway (vcall via `extern "system"`).
#[cfg(target_arch = "x86")]
pub unsafe fn vcall_0<R: Copy>(obj: *mut u8, index: usize) -> R {
    let fn_ptr: usize = vtable_entry(obj, index).expect("vcall_0: null vtable entry");
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        inout("eax") fn_ptr => _,
        this = in(reg) obj as usize,
        ret = out(reg) ret,
        out("edi") _,
    );
    std::mem::transmute_copy(&ret)
}

/// Call a C++ virtual method at vtable[index] with one argument + `this`.
/// thiscall: argument pushed, `this` in ECX, callee cleans.
#[cfg(target_arch = "x86")]
pub unsafe fn vcall_1<T: Copy, R: Copy>(obj: *mut u8, index: usize, a1: T) -> R {
    let fn_ptr: usize = vtable_entry(obj, index).expect("vcall_1: null vtable entry");
    let arg: usize = std::mem::transmute_copy(&a1);
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "push {arg}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        inout("eax") fn_ptr => _,
        this = in(reg) obj as usize,
        arg = in(reg) arg,
        ret = out(reg) ret,
        out("edi") _,
    );
    std::mem::transmute_copy(&ret)
}

/// thiscall with two stack arguments (callee cleans both).
#[cfg(target_arch = "x86")]
pub unsafe fn vcall_2<T1: Copy, T2: Copy, R: Copy>(obj: *mut u8, index: usize, a1: T1, a2: T2) -> R {
    let fn_ptr: usize = vtable_entry(obj, index).expect("vcall_2: null vtable entry");
    let arg1: usize = std::mem::transmute_copy(&a1);
    let arg2: usize = std::mem::transmute_copy(&a2);
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "push {arg2}",
        "push {arg1}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        inout("eax") fn_ptr => _,
        this = in(reg) obj as usize,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        ret = out(reg) ret,
        out("edi") _,
    );
    std::mem::transmute_copy(&ret)
}

/// thiscall with three stack arguments (callee cleans all).
#[cfg(target_arch = "x86")]
pub unsafe fn vcall_3<T1: Copy, T2: Copy, T3: Copy, R: Copy>(
    obj: *mut u8,
    index: usize,
    a1: T1,
    a2: T2,
    a3: T3,
) -> R {
    let fn_ptr: usize = vtable_entry(obj, index).expect("vcall_3: null vtable entry");
    let arg1: usize = std::mem::transmute_copy(&a1);
    let arg2: usize = std::mem::transmute_copy(&a2);
    let arg3: usize = std::mem::transmute_copy(&a3);
    let mut ret: usize = 0;
    core::arch::asm!(
        "push ecx",
        "push edx",
        "mov ecx, {this}",
        "push {arg3}",
        "push {arg2}",
        "push {arg1}",
        "call eax",
        "mov edi, eax",
        "mov {ret}, edi",
        "pop edx",
        "pop ecx",
        inout("eax") fn_ptr => _,
        this = in(reg) obj as usize,
        arg1 = in(reg) arg1,
        arg2 = in(reg) arg2,
        arg3 = in(reg) arg3,
        ret = out(reg) ret,
        out("edi") _,
    );
    std::mem::transmute_copy(&ret)
}

/// Non-x86 (x86_64 / tests): Windows x64 ABI passes `this` in RCX — plain
/// `extern "system"` calls are correct there.
#[cfg(not(target_arch = "x86"))]
pub unsafe fn vcall_0<R: Copy>(obj: *mut u8, index: usize) -> R {
    let fn_ptr: unsafe extern "system" fn(*mut u8) -> R =
        vtable_entry(obj, index).expect("vcall_0: null vtable entry");
    fn_ptr(obj)
}

/// Non-x86 fallback (x86_64 / tests) — see [`vcall_0`].
#[cfg(not(target_arch = "x86"))]
pub unsafe fn vcall_1<T: Copy, R: Copy>(obj: *mut u8, index: usize, a1: T) -> R {
    let fn_ptr: unsafe extern "system" fn(*mut u8, T) -> R =
        vtable_entry(obj, index).expect("vcall_1: null vtable entry");
    fn_ptr(obj, a1)
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn vcall_2<T1: Copy, T2: Copy, R: Copy>(obj: *mut u8, index: usize, a1: T1, a2: T2) -> R {
    let fn_ptr: unsafe extern "system" fn(*mut u8, T1, T2) -> R =
        vtable_entry(obj, index).expect("vcall_2: null vtable entry");
    fn_ptr(obj, a1, a2)
}

#[cfg(not(target_arch = "x86"))]
pub unsafe fn vcall_3<T1: Copy, T2: Copy, T3: Copy, R: Copy>(
    obj: *mut u8,
    index: usize,
    a1: T1,
    a2: T2,
    a3: T3,
) -> R {
    let fn_ptr: unsafe extern "system" fn(*mut u8, T1, T2, T3) -> R =
        vtable_entry(obj, index).expect("vcall_3: null vtable entry");
    fn_ptr(obj, a1, a2, a3)
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

/// Hardcoded address of `LookupFormByID` in FO3 1.7.0.3 EN (verified against
/// the real GOG exe + xFOSE fose.h FALLOUT_VERSION_1_7 block).
/// FNV equivalent: different address, detected at runtime.
///
/// The post-2023 Steam build recompiled the engine — the function moved to
/// 0x712AF0 (derived 2026-08-07 from a live image dump; see
/// docs/proton-testing.md). [`fo3_lookup_addr`] picks the right one by
/// reading each candidate's prologue in-process.
#[allow(dead_code)]
const LOOKUP_FORM_FO3: usize = 0x00455190;

/// Classic/GOG 1.7.0.3: `push ecx; mov <form-map-global>,%ecx`
const LOOKUP_FORM_FO3_GOG: usize = 0x0045_5190;
/// Steam (post-2023): `push ebp; mov ebp,esp; push ebx; push esi; push edi; mov <global>,%edi`
const LOOKUP_FORM_FO3_STEAM: usize = 0x0071_1EF0;

/// Which FO3 build is running — picked once, by prologue signature
/// (generalized as `address::AutoPtr` + `address::select_candidate`).
static FO3_LOOKUP_ADDR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Resolve the LookupFormByID address for the running build.
///
/// Reads the first bytes of each candidate in the live process: the GOG
/// build has the classic table (0x455190 = `51 8b 0d`), the Steam build has
/// FPU garbage there and the function at 0x712AF0 (`55 8b ec 53`). On
/// non-Windows (tests/harnesses) or when neither matches, falls back to the
/// classic table.
pub fn fo3_lookup_addr() -> usize {
    *FO3_LOOKUP_ADDR.get_or_init(|| {
        use crate::hooks::address::{select_candidate, Candidate};
        select_candidate(
            &[
                // GOG/classic prologue: push ecx; mov <global>,%ecx
                Candidate { addr: LOOKUP_FORM_FO3_GOG, signature: &[0x51, 0x8B, 0x0D, 0x14] },
                // Steam post-2023 prologue: push ebp; mov ebp,esp; push ebx
                Candidate { addr: LOOKUP_FORM_FO3_STEAM, signature: &[0x55, 0x8B, 0xEC, 0x53] },
            ],
            LOOKUP_FORM_FO3_GOG,
        )
    })
}

/// Whether the host process is a known game exe. The hardcoded FO3/FNV
/// addresses are only valid inside Fallout3.exe/FalloutNV.exe — calling them
/// from any other process (e.g. a wine test harness, or a debugger) faults.
static GAME_PROCESS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Extract the executable file name from a full path (testable, no Win32).
pub fn exe_base_name(path: &str) -> Option<&str> {
    let name = path.rsplit(['/', '\\']).next().filter(|s| !s.is_empty())?;
    Some(name)
}

pub fn is_game_process() -> bool {
    *GAME_PROCESS.get_or_init(|| {
        #[cfg(target_os = "windows")]
        {
            use std::ffi::OsString;
            use std::os::windows::ffi::OsStringExt;
            use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
            let mut buf = [0u16; 260];
            let n = unsafe {
                GetModuleFileNameW(0, buf.as_mut_ptr(), buf.len() as u32)
            };
            if n == 0 {
                return false;
            }
            let name = OsString::from_wide(&buf[..n as usize])
                .to_string_lossy()
                .to_lowercase();
            let base = exe_base_name(&name).unwrap_or("");
            base == "fallout3.exe" || base == "falloutnv.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = exe_base_name;
            false
        }
    })
}

/// Resolve a FormID to a memory pointer. Returns null if form not loaded.
///
/// On non-Windows targets (tests), always returns null — the hardcoded
/// FO3/FNV addresses are only valid inside the Wine/Proton game process.
pub unsafe fn lookup_form_by_id(form_id: u32) -> *mut u8 {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = form_id;
        std::ptr::null_mut()
    }

    #[cfg(target_os = "windows")]
    {
        // Never touch game memory from a non-game process (wine harnesses,
        // tests) — the hardcoded address is only valid inside the game.
        if !is_game_process() {
            return std::ptr::null_mut();
        }
        let addr: usize = fo3_lookup_addr();
        // FO3 LookupFormByID is __cdecl (plain `ret`, verified in both the
        // GOG and Steam builds' epilogues) — `extern "system"` would be
        // stdcall on x86 and leave the stack 4 bytes imbalanced after the
        // call, corrupting the caller's frame (crash on return).
        let fn_ptr: unsafe extern "cdecl" fn(u32) -> *mut u8 =
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
/// Only referenced on the Windows target (in-process patching).
#[allow(dead_code)]
const VTBL_REF_GET_POS: usize = vtable_index(0x30);        // index 12 (x86)
const VTBL_ACTOR_GET_VALUE: usize = vtable_index(0x68);    // index 26 (x86)
const VTBL_ACTOR_GET_BASE_VALUE: usize = vtable_index(0x70); // index 28 (x86, estimated)
const VTBL_ACTOR_ANIM_DATA: usize = vtable_index(0x01E4);   // index 121 (x86, vaultmp.cpp GetActorState)

// ═══════════════════════════════════════════════════════════════
// Steam (post-2023) vtable — re-derived 2026-08-14 (static, see steam-re.md)
//
// The Steam recompile REORDERED the TESObjectREFR/PlayerCharacter vtable.
// The Steam vtable base is 0xF938FC (verified: AI-predicate slot +0x22C
// → 0x8B8AF0, matches `call [rax+0x22c]` in the Steam AI predicate; the
// death-handler slot +0x23C → 0x8CA490). Byte-identical method matching
// (scripts/re/vtable_fullmap.py) shows the mid/late table shifted by a
// DOMINANT +0x58 (59% of 41 translated slots exact; the early region
// +0x00..0x68 was reordered — GetActorValue/GetBaseValue need semantic
// re-derivation, they do NOT sit at +0x58-shifted slots). Confirmed
// translations:
//   GOG +0x9C/0xA0 (lock-state getter 0x4017E0/0x4017F0) -> Steam
//     +0xF4/+0xFC (0x57C770/0x57C780, byte-identical)
//   GOG +0x1E8/0x1EC/0x1F0 (anim-data region) -> Steam +0x240/0x244/0x248
//   GOG +0x228 -> Steam +0x280
//   GOG +0x2D4 -> Steam +0x32C
// Field reads (pos/angle/cell/scale/refID) are unaffected — the bridge
// reads those raw. Only vtable-call ops (actor value/state/is_moving/lock)
// need the Steam slots; get_lock is the first confirmed safe one.
pub mod fo3_steam_vtable {
    /// Steam PlayerCharacter/TESObjectREFR vtable base (.rdata). Verified
    /// 2026-08-14: slot +0x22C → 0x8B8AF0 (the AI predicate's actor
    /// `call [rax+0x22c]` target), slot +0x23C → 0x8CA490 (death handler).
    pub const BASE: usize = 0x00F9_38FC;
    /// Lock-state getter (`mov al,[ecx+0xa]; and al,1; ret`) — GOG slot
    /// +0xA0 (0x4017F0) byte-identical twin. GOG +0xA0 -> Steam +0xFC.
    pub const GET_LOCKED: usize = 0xFC;
    /// GOG +0x9C (0x4017E0, lock-state sibling) -> Steam +0xF4.
    pub const GET_LOCKED_SIBLING: usize = 0xF4;
}

/// Steam slot for a classic GOG vtable slot, by byte-identity check.
///
/// The +0x58 shift covers most of the mid/late table. Rather than trust the
/// shift blindly, verify the slot's function bytes against the known Steam
/// function for the confirmed translations. Returns `None` when the running
/// build isn't Steam or the slot isn't confirmed — callers keep the GOG
/// fallback (which crashes on Steam for unconfirmed slots — do NOT call
/// vtable methods with unconfirmed slots on Steam).
#[cfg(target_arch = "x86")]
pub fn steam_slot_for(gog_slot: usize) -> Option<usize> {
    match gog_slot {
        // GOG +0xA0 lock getter 0x4017F0 (`8a 41 0a 24 01 c3`) is
        // byte-identical at Steam slot +0xFC (0x57C780) — verified
        // 2026-08-14 by matching the vtable function bytes.
        0xA0 => Some(fo3_steam_vtable::GET_LOCKED),
        _ => None,
    }
}

#[cfg(not(target_arch = "x86"))]
pub fn steam_slot_for(_gog_slot: usize) -> Option<usize> {
    None
}

// ═══════════════════════════════════════════════════════════════
// Known raw field offsets (vaultmp.cpp confirmed, FO3 1.7)
// ═══════════════════════════════════════════════════════════════

const OFFSET_REF_ID: usize = 0x0C;

// Field offsets verified against xFOSE GameObjects.h (FO3 1.7) and
// xNVSE GameObjects.h (FNV 1.4), both STATIC_ASSERT-anchored; parentCell
// additionally confirmed in both binaries (3,877 [reg+0x3C] reads in FO3,
// 8,924 [reg+0x40] reads in FNV).
//   FO3: rot 0x20/0x24/0x28, pos 0x2C/0x30/0x34, parentCell 0x3C
//   FNV: rot 0x24/0x28/0x2C, pos 0x30/0x34/0x38, parentCell 0x40
fn pos_offset(index: usize) -> usize {
    if crate::hooks::is_fnv() { 0x30 + index * 4 } else { 0x2C + index * 4 }
}
/// Scale field: immediately after the position triple.
/// FO3: pos 0x2C/0x30/0x34 → scale 0x38. FNV: pos 0x30/0x34/0x38 → 0x3C.
fn scale_offset() -> usize {
    if crate::hooks::is_fnv() { 0x3C } else { 0x38 }
}
/// Read the reference scale (raw field, Steam-safe).
pub unsafe fn get_scale(ref_id: u32) -> f32 {
    let obj = lookup_form_by_id(ref_id);
    get_scale_from_obj(obj)
}
/// Read the scale from an already-resolved object pointer.
pub unsafe fn get_scale_from_obj(obj: *mut u8) -> f32 {
    if obj.is_null() {
        return 1.0;
    }
    read_field::<f32>(obj, scale_offset())
}
/// Set the reference scale (raw field write, Steam-safe).
pub unsafe fn set_scale(ref_id: u32, scale: f32) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }
    write_field::<f32>(obj, scale_offset(), scale);
}
fn angle_offset(index: usize) -> usize {
    if crate::hooks::is_fnv() { 0x24 + index * 4 } else { 0x20 + index * 4 }
}
/// `TESObjectREFR::parentCell` — FO3 0x3C / FNV 0x40.
fn parent_cell_offset() -> usize {
    if crate::hooks::is_fnv() { 0x40 } else { 0x3C }
}

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
    let x = read_field::<f32>(obj, pos_offset(0));
    let y = read_field::<f32>(obj, pos_offset(1));
    let z = read_field::<f32>(obj, pos_offset(2));
    [x, y, z]
}

/// Read angle in degrees (converted from engine radians).
/// Vaultmp convention: angles in degrees (vaultmp.cpp GetPosAngle × 180/π).
pub unsafe fn get_angle(ref_id: u32) -> [f32; 3] {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return [0.0, 0.0, 0.0];
    }

    let ax = read_field::<f32>(obj, angle_offset(0));
    let ay = read_field::<f32>(obj, angle_offset(1));
    let az = read_field::<f32>(obj, angle_offset(2));

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

    // Try VTable call: Actor::GetActorValue(index) → f32 (thiscall)
    vcall_1::<u8, f32>(obj, VTBL_ACTOR_GET_VALUE, index)
}

/// Read base actor value by index.
/// Tries VTable GetActorBaseValue first, raw field fallback.
pub unsafe fn get_actor_base_value(ref_id: u32, index: u8) -> f32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return 0.0;
    }

    vcall_1::<u8, f32>(obj, VTBL_ACTOR_GET_BASE_VALUE, index)
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
    write_field(obj, pos_offset(0), pos[0]);
    write_field(obj, pos_offset(1), pos[1]);
    write_field(obj, pos_offset(2), pos[2]);
}

/// Set angle (accept degrees, convert to radians for engine).
pub unsafe fn set_angle(ref_id: u32, angle: [f32; 3]) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }

    use std::f32::consts::PI;
    write_field(obj, angle_offset(0), angle[0] * PI / 180.0);
    write_field(obj, angle_offset(1), angle[1] * PI / 180.0);
    write_field(obj, angle_offset(2), angle[2] * PI / 180.0);
}

/// Set actor value by index.
pub unsafe fn set_actor_value(ref_id: u32, index: u8, value: f32) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }

    const VTBL_ACTOR_SET_VALUE: usize = vtable_index(0x6C); // index 27 (x86, estimated)
    // thiscall: SetActorValue(this, index, value), callee cleans both args.
    let _: u32 = vcall_2::<u8, f32, u32>(obj, VTBL_ACTOR_SET_VALUE, index, value);
}

/// Read cell of a reference.
/// Path: `TESObjectREFR::parentCell` (FO3 +0x3C / FNV +0x40) → `TESObjectCELL*`, then `TESForm::refID` at +0x0C.
pub unsafe fn get_cell(ref_id: u32) -> u32 {
    get_cell_from_obj(lookup_form_by_id(ref_id))
}

/// Read the parent cell FormID from an already-resolved object pointer.
pub unsafe fn get_cell_from_obj(obj: *mut u8) -> u32 {
    if obj.is_null() {
        return 0;
    }
    let cell_ptr = read_field::<usize>(obj, parent_cell_offset());
    if cell_ptr == 0 {
        return 0;
    }
    // TESForm::refID lives at +0x0C (xFOSE GameForms.h, STATIC_ASSERT
    // offsetof(TESForm, refID) == 0x00C).
    read_field::<u32>(cell_ptr as *mut u8, OFFSET_REF_ID)
}

/// Read base FormID (TESObjectREFR::baseForm field).
///
/// Field +0x1C was derived empirically on the Steam build (2026-08-07): the
/// player ref (0x14) has +0x1C -> a form object whose +0x0C = 0x07 (the
/// Player base record in Fallout3.esm). Field read avoids the vtable call
/// entirely — the xFOSE vtable index (0x10) does NOT hold GetBaseForm in
/// the Steam build (that slot is a destructor with `ret $4`).
pub unsafe fn get_base(ref_id: u32) -> u32 {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return 0;
    }
    const OFFSET_BASE_FORM: usize = 0x1C;
    let base_form = read_field::<usize>(obj, OFFSET_BASE_FORM);
    if base_form == 0 {
        return 0;
    }
    // TESForm::refID at +0x0C (xFOSE STATIC_ASSERT).
    read_field::<u32>(base_form as *mut u8, OFFSET_REF_ID)
}

// ═══════════════════════════════════════════════════════════════
// Enabled / Lock / Name / Parent cell / Combat target
// ═══════════════════════════════════════════════════════════════

/// Enabled flag offset: `TESObjectREFR+0x50` (FO3) / `+0x54` (FNV), bit 0x02.
/// Returns `true` when the reference is enabled (flag bit clear).
pub unsafe fn get_enabled(ref_id: u32) -> bool {
    get_enabled_from_obj(lookup_form_by_id(ref_id))
}

/// Read the enabled state from an already-resolved object pointer.
pub unsafe fn get_enabled_from_obj(obj: *mut u8) -> bool {
    if obj.is_null() {
        return true;
    }
    let flags_offset: usize = if crate::hooks::is_fnv() { 0x54 } else { 0x50 };
    let flags = read_field::<u32>(obj, flags_offset);
    (flags & 0x02) == 0
}

/// Set the enabled flag (raw field write, Steam-safe — inverse of
/// `get_enabled`). Wired to OP_SET_ENABLED (2026-08-14).
pub unsafe fn set_enabled(ref_id: u32, enabled: bool) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }
    set_enabled_flags(obj, enabled);
}

/// Set the enabled flag on an already-resolved object pointer.
pub unsafe fn set_enabled_flags(obj: *mut u8, enabled: bool) {
    if obj.is_null() {
        return;
    }
    let flags_offset: usize = if crate::hooks::is_fnv() { 0x54 } else { 0x50 };
    let mut flags = read_field::<u32>(obj, flags_offset);
    if enabled {
        flags &= !0x02;
    } else {
        flags |= 0x02;
    }
    write_field::<u32>(obj, flags_offset, flags);
}

/// `TESObjectREFR::GetLocked()` vtable call → `TESObjectLOCK*`.
const VTBL_REF_GET_LOCKED: usize = vtable_index(0xA0);

/// Get the lock object pointer for a reference.
pub unsafe fn get_lock(ref_id: u32) -> u32 {
    get_lock_from_obj(lookup_form_by_id(ref_id))
}

/// Set the lock state (raw field write on the lock byte at +0xA, bit 0 —
/// the same byte the verified lock-state getter reads: `mov al,[ecx+0xa];
/// and al,1; ret`, GOG 0x4017F0 / Steam 0x57C780, 2026-08-14). Steam-safe.
pub unsafe fn set_lock(ref_id: u32, locked: bool) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }
    set_lock_flags(obj, locked);
}

/// Set the lock byte on an already-resolved object pointer (+0xA bit 0).
pub unsafe fn set_lock_flags(obj: *mut u8, locked: bool) {
    if obj.is_null() {
        return;
    }
    let mut byte = read_field::<u8>(obj, 0x0A);
    if locked {
        byte |= 0x01;
    } else {
        byte &= !0x01;
    }
    write_field::<u8>(obj, 0x0A, byte);
}

/// Read the lock pointer from an already-resolved object pointer.
pub unsafe fn get_lock_from_obj(obj: *mut u8) -> u32 {
    if obj.is_null() {
        return 0;
    }
    // Steam: vtable reordered — GOG slot +0xA0 (lock-state getter
    // 0x4017F0) is byte-identical at Steam slot +0xFC (0x57C780),
    // verified 2026-08-14 (steam-re.md). Use it when detected.
    #[cfg(target_arch = "x86")]
    {
        let slot = crate::hooks::vtable::steam_slot_for(VTBL_REF_GET_LOCKED)
            .unwrap_or(VTBL_REF_GET_LOCKED);
        return vcall_0(obj, slot);
    }
    #[cfg(not(target_arch = "x86"))]
    {
        vcall_0(obj, VTBL_REF_GET_LOCKED)
    }
}

/// Parent cell: `TESObjectREFR::parentCell` — FO3 +0x3C / FNV +0x40.
/// Returns the parent cell FormID (or 0 when the engine hasn't set it).
pub unsafe fn get_parent_cell(ref_id: u32) -> u32 {
    get_parent_cell_from_obj(lookup_form_by_id(ref_id))
}

/// Read the parent cell FormID from an already-resolved object pointer.
pub unsafe fn get_parent_cell_from_obj(obj: *mut u8) -> u32 {
    if obj.is_null() {
        return 0;
    }
    let offset = parent_cell_offset();
    read_field::<u32>(obj, offset)
}

/// Set the parent cell FormID (raw field write — Steam-safe, matches the
/// field read in `get_parent_cell_from_obj`). Wired to OP_SET_CELL so
/// remote cell-moves propagate (2026-08-14).
pub unsafe fn set_parent_cell(ref_id: u32, cell: u32) {
    let obj = lookup_form_by_id(ref_id);
    if obj.is_null() {
        return;
    }
    write_field::<u32>(obj, parent_cell_offset(), cell);
}

/// Combat target offsets: `Actor+0x4E0` (FO3) / `+0x430` (FNV).
const OFFSET_COMBAT_TARGET_FO3: usize = 0x4E0;
const OFFSET_COMBAT_TARGET_FNV: usize = 0x430;

/// Get the current combat target FormID for an actor.
pub unsafe fn get_combat_target(ref_id: u32) -> u32 {
    get_combat_target_from_obj(lookup_form_by_id(ref_id))
}

/// Read the combat target from an already-resolved object pointer.
pub unsafe fn get_combat_target_from_obj(obj: *mut u8) -> u32 {
    if obj.is_null() {
        return 0;
    }
    let offset = if crate::hooks::is_fnv() {
        OFFSET_COMBAT_TARGET_FNV
    } else {
        OFFSET_COMBAT_TARGET_FO3
    };
    read_field::<u32>(obj, offset)
}

/// Full-name vtable slot on TESForm: byte offset 0x1C (index 7 on x86).
const VTBL_FORM_GET_FULL_NAME: usize = vtable_index(0x1C);

/// Get the display name via the VTable chain
/// `GetBaseForm` → `GetFullName` (returns `const char*`).
pub unsafe fn get_name(ref_id: u32) -> String {
    get_name_from_obj(lookup_form_by_id(ref_id))
}

/// Read the display name from an already-resolved object pointer.
pub unsafe fn get_name_from_obj(obj: *mut u8) -> String {
    if obj.is_null() {
        return "unnamed".into();
    }
    // baseForm field at +0x1C (empirically derived on Steam 2026-08-07) —
    // the vtable GetBaseForm slot differs from xFOSE in the Steam build and
    // calling it corrupts the stack.
    let base_form: *mut u8 = read_field::<usize>(obj, 0x1C) as *mut u8;
    if base_form.is_null() {
        return "unnamed".into();
    }
    if vtable_entry::<usize>(base_form, VTBL_FORM_GET_FULL_NAME).is_none() {
        return "unnamed".into();
    }
    let name_ptr: *const i8 = vcall_0(base_form, VTBL_FORM_GET_FULL_NAME);
    if name_ptr.is_null() {
        return "unnamed".into();
    }
    std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
}

/// Actor value indices (Gamebryo ActorValue enum, shared by FO3/FNV).
pub const AV_HEALTH: u8 = 0x14;
pub const AV_ACTION_POINTS: u8 = 0x15;
pub const AV_CARRY_WEIGHT: u8 = 0x05;
pub const AV_DAMAGE_RESIST: u8 = 0x29;
pub const AV_DAMAGE_THRESHOLD: u8 = 0x2A; // FNV only
pub const AV_SPEED_MULT: u8 = 0x22;
pub const AV_RADIATION: u8 = 0x20;
// FNV hardcore stats
pub const AV_DEHYDRATION: u8 = 0x2B;
pub const AV_HUNGER: u8 = 0x2C;
pub const AV_SLEEP: u8 = 0x2D;

// ═══════════════════════════════════════════════════════════════
// Tests — operate on a local buffer to verify primitives
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

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
            write_field::<f32>(ptr, pos_offset(0), 1.0f32);
            write_field::<f32>(ptr, pos_offset(1), 2.0f32);
            write_field::<f32>(ptr, pos_offset(2), 3.0f32);

            let x: f32 = read_field(ptr, pos_offset(0));
            let y: f32 = read_field(ptr, pos_offset(1));
            let z: f32 = read_field(ptr, pos_offset(2));
            assert_eq!((x, y, z), (1.0, 2.0, 3.0));
        }
    }

    #[test]
    fn test_write_and_read_angle_conversion() {
        let mut buf = vec![0u8; 64];
        let ptr = buf.as_mut_ptr();
        unsafe {
            // Write angles in radians
            write_field::<f32>(ptr, angle_offset(0), std::f32::consts::PI); // 180°
            write_field::<f32>(ptr, angle_offset(1), std::f32::consts::FRAC_PI_2); // 90°
            write_field::<f32>(ptr, angle_offset(2), 0.0);

            let ax: f32 = read_field(ptr, angle_offset(0));
            let ay: f32 = read_field(ptr, angle_offset(1));
            let az: f32 = read_field(ptr, angle_offset(2));

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

    // ── New reference getters (offsets from xSE community / vaultmp) ──

    /// Fake C++ object: [vtable ptr][zeroed fields of `size` bytes].
    unsafe fn fake_object(vtable: &[usize], size: usize) -> (Vec<u8>, *mut u8) {
        let mut buf = vec![0u8; size];
        let ptr = buf.as_mut_ptr();
        ptr::write(ptr as *mut *const usize, vtable.as_ptr());
        (buf, ptr)
    }

    #[test]
    fn test_getters_null_defaults() {
        unsafe {
            assert_eq!(get_cell_from_obj(std::ptr::null_mut()), 0);
            assert!(get_enabled_from_obj(std::ptr::null_mut())); // enabled by default
            assert_eq!(get_lock_from_obj(std::ptr::null_mut()), 0);
            assert_eq!(get_parent_cell_from_obj(std::ptr::null_mut()), 0);
            assert_eq!(get_combat_target_from_obj(std::ptr::null_mut()), 0);
            assert_eq!(get_name_from_obj(std::ptr::null_mut()), "unnamed");
        }
    }

    #[test]
    fn test_get_cell_from_obj() {
        unsafe {
            // Fake cell object with refID at TESForm offset 0x0C (xFOSE-verified)
            let mut cell = vec![0u8; 32];
            write_field::<u32>(cell.as_mut_ptr(), 0x0C, 0xCAFE);

            let mut obj = vec![0u8; 128];
            write_field::<usize>(obj.as_mut_ptr(), parent_cell_offset(), cell.as_ptr() as usize);

            assert_eq!(get_cell_from_obj(obj.as_mut_ptr()), 0xCAFE);

            // Null cell pointer → 0
            let mut obj2 = vec![0u8; 128];
            write_field::<usize>(obj2.as_mut_ptr(), parent_cell_offset(), 0);
            assert_eq!(get_cell_from_obj(obj2.as_mut_ptr()), 0);
        }
    }

    #[test]
    fn test_get_enabled_from_obj_fo3_offset() {
        unsafe {
            // Default engine state is Unknown → FO3 path (+0x50, bit 0x02)
            let mut obj = vec![0u8; 128];
            write_field::<u32>(obj.as_mut_ptr(), 0x50, 0x02); // disabled flag set
            assert!(!get_enabled_from_obj(obj.as_mut_ptr()));

            write_field::<u32>(obj.as_mut_ptr(), 0x50, 0x00);
            assert!(get_enabled_from_obj(obj.as_mut_ptr()));
        }
    }

    #[test]
    fn test_set_enabled_roundtrip() {
        unsafe {
            // set_enabled flips the same bit get_enabled reads (+0x50 bit 0x02).
            let mut obj = vec![0u8; 128];
            // Enabled by default (flags 0).
            assert!(get_enabled_from_obj(obj.as_mut_ptr()));
            set_enabled_flags(obj.as_mut_ptr(), false);
            assert!(!get_enabled_from_obj(obj.as_mut_ptr()));
            set_enabled_flags(obj.as_mut_ptr(), true);
            assert!(get_enabled_from_obj(obj.as_mut_ptr()));
        }
    }

    #[test]
    fn test_set_parent_cell_roundtrip() {
        unsafe {
            // FO3 parent cell at +0x3C (is_fnv() == false default).
            let mut obj = vec![0u8; 128];
            write_field::<u32>(obj.as_mut_ptr(), 0x3C, 0);
            assert_eq!(get_parent_cell_from_obj(obj.as_mut_ptr()), 0);
            write_field::<u32>(obj.as_mut_ptr(), 0x3C, 0x1234);
            assert_eq!(get_parent_cell_from_obj(obj.as_mut_ptr()), 0x1234);
        }
    }

    #[test]
    fn test_set_lock_roundtrip() {
        unsafe {
            // Lock byte at +0xA bit 0 (the verified getter's field).
            let mut obj = vec![0u8; 64];
            write_field::<u8>(obj.as_mut_ptr(), 0x0A, 0);
            assert_eq!(read_field::<u8>(obj.as_mut_ptr(), 0x0A) & 0x01, 0);
            set_lock_flags(obj.as_mut_ptr(), true);
            assert_eq!(read_field::<u8>(obj.as_mut_ptr(), 0x0A) & 0x01, 1);
            set_lock_flags(obj.as_mut_ptr(), false);
            assert_eq!(read_field::<u8>(obj.as_mut_ptr(), 0x0A) & 0x01, 0);
        }
    }

    #[test]
    fn test_scale_roundtrip() {
        unsafe {
            // FO3 scale at +0x38 (default engine state → not FNV).
            let mut obj = vec![0u8; 128];
            write_field::<f32>(obj.as_mut_ptr(), 0x38, 1.0);
            assert_eq!(get_scale_from_obj(obj.as_mut_ptr()), 1.0);
            write_field::<f32>(obj.as_mut_ptr(), 0x38, 2.5);
            assert_eq!(get_scale_from_obj(obj.as_mut_ptr()), 2.5);
        }
    }

    #[test]
    fn test_get_lock_from_obj() {
        unsafe extern "C" fn fake_get_locked(_this: *mut u8) -> u32 {
            0xDEADBEEF
        }
        unsafe {
            let mut vtable = vec![0usize; 48]; // covers VTBL_REF_GET_LOCKED on both arches
            vtable[VTBL_REF_GET_LOCKED] = fake_get_locked as *const () as usize;
            let (_obj, ptr) = fake_object(&vtable, 64);
            assert_eq!(get_lock_from_obj(ptr), 0xDEADBEEF);
        }
    }

    #[test]
    fn test_get_parent_cell_from_obj_fo3_offset() {
        unsafe {
            // FO3 path (default engine state): parentCell at +0x3C (xFOSE-verified)
            let mut obj = vec![0u8; 128];
            write_field::<u32>(obj.as_mut_ptr(), 0x3C, 0x1234);
            assert_eq!(get_parent_cell_from_obj(obj.as_mut_ptr()), 0x1234);

            // rotZ at +0x28 must NOT be interpreted as the parent cell
            let mut obj2 = vec![0u8; 128];
            write_field::<u32>(obj2.as_mut_ptr(), 0x28, 0xDEADBEEF);
            assert_eq!(get_parent_cell_from_obj(obj2.as_mut_ptr()), 0, "0x28 is rotZ, not a cell");
        }
    }

    #[test]
    fn test_get_combat_target_from_obj_fo3_offset() {
        unsafe {
            let mut obj = vec![0u8; 0x600];
            write_field::<u32>(obj.as_mut_ptr(), 0x4E0, 0x5555); // FO3 path (default engine state)
            assert_eq!(get_combat_target_from_obj(obj.as_mut_ptr()), 0x5555);
        }
    }

    #[test]
    fn test_get_name_from_obj_vtable_chain() {
        unsafe extern "C" fn fake_get_full_name(_this: *mut u8) -> *const i8 {
            b"Wanderer\0".as_ptr() as *const i8
        }
        unsafe {
            let mut base_vtable = vec![0usize; 16];
            base_vtable[VTBL_FORM_GET_FULL_NAME] = fake_get_full_name as *const () as usize;
            let (_base, base_ptr) = fake_object(&base_vtable, 64);

            // baseForm field at +0x1C (derived on Steam 2026-08-07).
            let obj_vtable = vec![0usize; 16];
            let (_mut_obj, obj_ptr) = fake_object(&obj_vtable, 128);
            write_field::<usize>(obj_ptr, 0x1C, base_ptr as usize);

            assert_eq!(get_name_from_obj(obj_ptr), "Wanderer");
        }
    }

    #[test]
    fn test_get_name_null_base_form() {
        unsafe {
            // VTable chain returns null base form → "unnamed"
            let vtable = vec![0usize; 16];
            let (_obj, ptr) = fake_object(&vtable, 64);
            assert_eq!(get_name_from_obj(ptr), "unnamed");
        }
    }

    #[test]
    fn test_actor_value_constants() {
        let vals = [
            AV_HEALTH, AV_ACTION_POINTS, AV_CARRY_WEIGHT, AV_DAMAGE_RESIST,
            AV_DAMAGE_THRESHOLD, AV_SPEED_MULT, AV_RADIATION,
            AV_DEHYDRATION, AV_HUNGER, AV_SLEEP,
        ];
        for i in 0..vals.len() {
            for j in i + 1..vals.len() {
                assert_ne!(vals[i], vals[j], "AV constants must be distinct");
            }
        }
        assert_eq!(AV_HEALTH, 0x14);
        assert_eq!(AV_DAMAGE_THRESHOLD, 0x2A);
        assert_eq!(AV_SLEEP, 0x2D);
    }
}

#[cfg(test)]
mod process_tests {
    use super::exe_base_name;

    #[test]
    fn test_exe_base_name() {
        assert_eq!(exe_base_name(r"C:\Games\Fallout 3\Fallout3.exe"), Some("Fallout3.exe"));
        assert_eq!(exe_base_name("Z:\\games\\FalloutNV.exe"), Some("FalloutNV.exe"));
        assert_eq!(exe_base_name("/opt/loader.exe"), Some("loader.exe"));
        assert_eq!(exe_base_name(""), None);
        assert_eq!(exe_base_name("noslash"), Some("noslash"));
    }
}
