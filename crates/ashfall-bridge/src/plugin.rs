//! NVSE/FOSE plugin interface — exports and registration.
//!
//! Called by NVSE/FOSE when the bridge DLL is loaded as a script extender
//! plugin. Fallback: if loaded via Wine DLL override (non-NVSE), DllMain
//! handles init.
//!
//! Layouts and signatures VERIFIED against xNVSE/xFOSE PluginAPI.h
//! (2026-08-06): the interface carries RegisterCommand/SetOpcodeBase/
//! QueryInterface/etc. — NOT SafeWrite helpers (the earlier "simplified
//! layout" with SafeWrite fields was wrong and would have read garbage
//! offsets). PluginInfo.name is a `const char*`, not an inline array.

use std::ffi::{c_char, c_void};
use std::sync::{LazyLock, Mutex};

/// PluginInfo struct — matches FOSE/NVSE PluginInfo layout exactly
/// (xFOSE/xNVSE PluginAPI.h): infoVersion(u32) + name(const char*) + version(u32).
#[repr(C)]
pub struct PluginInfo {
    pub info_version: u32,
    pub name: *const c_char,
    pub version: u32,
}

impl PluginInfo {
    pub fn new(name: &str, version: u32) -> Self {
        // ponytail: leak a static buffer — the engine reads the pointer
        // only during Query, and the process lives as long as the game.
        let cstr = std::ffi::CString::new(name).unwrap_or_default();
        let ptr = cstr.into_raw();
        PluginInfo {
            info_version: 1,
            name: ptr,
            version,
        }
    }

    pub fn name_str(&self) -> String {
        if self.name.is_null() {
            return String::new();
        }
        unsafe { std::ffi::CStr::from_ptr(self.name) }
            .to_string_lossy()
            .into_owned()
    }
}

/// The FOSE/NVSE bootstrap interface (layout per xFOSE FOSEInterface /
/// xNVSE NVSEInterface, both identical for the first 11 fields).
///
/// Only read during `Query`/`Load`; the bridge never calls back into it —
/// the fields are kept so the snapshot matches the engine's layout.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NVSEInterface {
    pub nvse_version: u32,                                           // 0x00
    pub runtime_version: u32,                                        // 0x04
    pub editor_version: u32,                                         // 0x08
    pub is_editor: u32,                                              // 0x0C
    pub register_command: unsafe extern "C" fn(*mut c_void) -> bool, // 0x10
    pub set_opcode_base: unsafe extern "C" fn(u32),                  // 0x14
    pub query_interface: unsafe extern "C" fn(u32) -> *mut c_void,   // 0x18
    pub get_plugin_handle: unsafe extern "C" fn() -> u32,            // 0x1C
    pub register_typed_command: unsafe extern "C" fn(*mut c_void, u32) -> bool, // 0x20
    pub get_runtime_directory: unsafe extern "C" fn() -> *const c_char, // 0x24
    pub is_nogore: u32,                                              // 0x28
}

/// Snapshot of the engine interface passed at load time, if any.
static ENGINE_INTERFACE: LazyLock<Mutex<Option<NVSEInterface>>> =
    LazyLock::new(|| Mutex::new(None));

/// Return the engine interface captured during `NVSEPlugin_Load`, if any.
pub fn nvse_interface() -> Option<NVSEInterface> {
    *ENGINE_INTERFACE.lock().unwrap()
}

/// Plugin interface version constant.
const PLUGIN_INTERFACE_VERSION: u32 = 1;

/// Core Query body — shared by the NVSE- and FOSE-named exports.
unsafe fn plugin_query(nvse: *const NVSEInterface, info: *mut PluginInfo) -> bool {
    // Forward-compatible: accept any engine interface >= our minimum.
    if !nvse.is_null() && (*nvse).nvse_version < PLUGIN_INTERFACE_VERSION {
        return false;
    }
    if !info.is_null() {
        *info = PluginInfo::new("Ashfall Bridge", 1);
    }
    true
}

/// Called by NVSE to query plugin info.
#[no_mangle]
pub unsafe extern "C" fn NVSEPlugin_Query(
    nvse: *const NVSEInterface,
    info: *mut PluginInfo,
) -> bool {
    plugin_query(nvse, info)
}

/// Called by FOSE/xFOSE to query plugin info (FO3 — the same interface).
#[no_mangle]
pub unsafe extern "C" fn FOSEPlugin_Query(
    nvse: *const NVSEInterface,
    info: *mut PluginInfo,
) -> bool {
    plugin_query(nvse, info)
}

/// Core Load body — shared by the NVSE- and FOSE-named exports.
fn plugin_load(nvse: *const NVSEInterface) -> bool {
    // Snapshot the interface (copied — the engine keeps the original alive).
    if !nvse.is_null() {
        unsafe {
            *ENGINE_INTERFACE.lock().unwrap() = Some(*nvse);
        }
    }

    // Mark initialized so DllMain becomes a no-op
    crate::INITIALIZED.store(true, std::sync::atomic::Ordering::SeqCst);

    // Initialize engine hooks
    crate::hooks::install();

    // Start TCP server in background
    std::thread::spawn(|| {
        crate::network::run_server("127.0.0.1:1771");
    });

    // Register default console commands
    crate::console::register_defaults();

    true
}

/// Called by NVSE to load the plugin.
#[no_mangle]
pub extern "C" fn NVSEPlugin_Load(nvse: *const NVSEInterface) -> bool {
    plugin_load(nvse)
}

/// Called by FOSE/xFOSE to load the plugin (FO3).
#[no_mangle]
pub extern "C" fn FOSEPlugin_Load(nvse: *const NVSEInterface) -> bool {
    plugin_load(nvse)
}
