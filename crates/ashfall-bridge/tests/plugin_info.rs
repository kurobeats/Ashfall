//! PluginInfo + NVSEInterface tests — NVSE/FOSE plugin identity.
//!
//! Layout verified against xFOSE/xNVSE PluginAPI.h (2026-08-06):
//!   PluginInfo = infoVersion(u32) + name(const char*) + version(u32) = 12B
//!   NVSEInterface/FOSEInterface = 11 fields (no SafeWrite helpers).

use ashfall_bridge::plugin::{NVSEPlugin_Load, NVSEPlugin_Query, PluginInfo};
use std::ffi::c_char;

#[test]
fn test_plugin_info_struct_size() {
    // infoVersion(u32) + name(const char*) + version(u32). On the i686
    // Windows target (the real DLL): 4 + 4 + 4 = 12 bytes. Host tests run
    // 64-bit where the pointer widens the struct.
    let ptr = std::mem::size_of::<usize>();
    let expected = if ptr == 4 { 12 } else { 24 }; // 4 + pad + ptr + 4, align 8
    assert_eq!(std::mem::size_of::<PluginInfo>(), expected);
    // name is a POINTER (matches the engine's PluginInfo), not an array
    assert_eq!(std::mem::size_of::<*const c_char>(), ptr);
}

#[test]
fn test_plugin_info_default_name() {
    let info = PluginInfo::new("Ashfall Bridge", 1);
    assert_eq!(info.name_str(), "Ashfall Bridge");
    assert_eq!(info.info_version, 1);
    assert_eq!(info.version, 1);
}

#[test]
fn test_plugin_info_empty_name() {
    let info = PluginInfo::new("", 1);
    assert_eq!(info.name_str(), "");
}

#[test]
fn test_plugin_info_single_char() {
    let info = PluginInfo::new("A", 1);
    assert_eq!(info.name_str(), "A");
}

#[test]
fn test_plugin_info_long_name() {
    // Long names are fine — the engine only reads up to the NUL.
    let name = "X".repeat(300);
    let info = PluginInfo::new(&name, 1);
    assert_eq!(info.name_str().len(), 300);
}

/// Minimal engine interface (FOSE/NVSE layout) for Query/Load tests.
fn fake_interface(version: u32) -> ashfall_bridge::plugin::NVSEInterface {
    unsafe extern "C" fn noop_command(_: *mut std::ffi::c_void) -> bool {
        true
    }
    unsafe extern "C" fn noop_set(_: u32) {}
    unsafe extern "C" fn noop_query(_: u32) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
    unsafe extern "C" fn noop_handle() -> u32 {
        0
    }
    unsafe extern "C" fn noop_typed(_: *mut std::ffi::c_void, _: u32) -> bool {
        true
    }
    unsafe extern "C" fn noop_dir() -> *const std::ffi::c_char {
        std::ptr::null()
    }
    ashfall_bridge::plugin::NVSEInterface {
        nvse_version: version,
        runtime_version: 0x01070030, // 1.7.0.3
        editor_version: 0,
        is_editor: 0,
        register_command: noop_command,
        set_opcode_base: noop_set,
        query_interface: noop_query,
        get_plugin_handle: noop_handle,
        register_typed_command: noop_typed,
        get_runtime_directory: noop_dir,
        is_nogore: 0,
    }
}

#[test]
fn test_nvse_plugin_query_ok() {
    let mut info = PluginInfo::new("", 0);
    let ok = unsafe { NVSEPlugin_Query(&fake_interface(1), &mut info) };
    assert!(ok);
    assert_eq!(info.name_str(), "Ashfall Bridge");
    assert_eq!(info.version, 1);
    assert_eq!(info.info_version, 1);
}

#[test]
fn test_nvse_plugin_query_null_interface() {
    // Loaded without the engine (plain DLL override) — still answers.
    let mut info = PluginInfo::new("", 0);
    let ok = unsafe { NVSEPlugin_Query(std::ptr::null(), &mut info) };
    assert!(ok);
    assert_eq!(info.name_str(), "Ashfall Bridge");
}

#[test]
fn test_nvse_plugin_query_null_info() {
    // Query with NULL info must not fault.
    assert!(unsafe { NVSEPlugin_Query(&fake_interface(1), std::ptr::null_mut()) });
}

#[test]
fn test_nvse_plugin_load_null_interface() {
    // Null interface (Wine DLL override path) — init must still happen.
    assert!(NVSEPlugin_Load(std::ptr::null()));
}
