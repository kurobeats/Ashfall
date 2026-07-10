//! Ashfall script SDK — helpers for writing WASM game mode scripts.
//!
//! Provides re-exports of core types and ergonomic macros for
//! defining host imports and callback exports.

pub use ashfall_core::id::NetworkID;
pub use ashfall_core::types::{ObjectKind, Reason};
pub use ashfall_core::protocol;
pub use ashfall_core::constants;
pub use ashfall_core::form_id::FormID;

// ═══════════════════════════════════════════════════════════════
// Type aliases for script authors
// ═══════════════════════════════════════════════════════════════

/// Object ID (maps to NetworkID on host).
pub type ObjectId = u64;

/// Form ID — Gamebryo base/reference ID.
pub type GameFormID = u32;

/// Cell ID — world cell coordinate.
pub type CellID = u32;

/// Quest ID.
pub type QuestID = u32;

/// Actor value index (health=0x14, skills, SPECIAL, etc.).
pub type ActorValueIndex = u8;

/// Limb index for combat (torso=0, head=1, limbs=2-5).
pub type LimbIndex = u8;

/// Faction ID.
pub type FactionID = u32;

// ═══════════════════════════════════════════════════════════════
// Host import declaration macros
// ═══════════════════════════════════════════════════════════════

/// Declare a host function import in a WASM module.
#[macro_export]
macro_rules! host_fn {
    ($vis:vis fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?) => {
        extern "C" {
            $vis fn $name($($arg: $ty),*) $(-> $ret)?;
        }
    };
}

/// Declare multiple host function imports at once.
#[macro_export]
macro_rules! host_fns {
    ($(fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;)+) => {
        extern "C" {
            $(
                fn $name($($arg: $ty),*) $(-> $ret)?;
            )+
        }
    };
}

/// Mark a callback as optional (server tolerates missing export).
#[macro_export]
macro_rules! optional {
    ($item:item) => {
        $item
    };
}

// ═══════════════════════════════════════════════════════════════
// Utility helpers
// ═══════════════════════════════════════════════════════════════

/// Read a string from a raw pointer + length (passed from host).
///
/// # Safety
/// The pointer must be valid for `len` bytes.
pub unsafe fn read_host_string(ptr: *const u8, len: u32) -> Option<String> {
    if ptr.is_null() || len == 0 {
        return None;
    }
    let slice = unsafe { core::slice::from_raw_parts(ptr, len as usize) };
    core::str::from_utf8(slice).ok().map(|s| s.to_string())
}

/// Try to read a host string, returning empty string on failure.
///
/// # Safety
/// The pointer must be valid for `len` bytes.
pub unsafe fn read_host_string_or_empty(ptr: *const u8, len: u32) -> String {
    unsafe { read_host_string(ptr, len) }.unwrap_or_default()
}
