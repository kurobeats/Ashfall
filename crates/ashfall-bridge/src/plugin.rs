//! NVSE/FOSE plugin interface — exports and registration.
//!
//! Called by NVSE/FOSE when the bridge DLL is loaded as a script extender plugin.
//! Fallback: if loaded via Wine DLL override (non-NVSE), DllMain handles init.

use std::sync::{LazyLock, Mutex};

/// PluginInfo struct — matches NVSE/FOSE PluginInfo layout.
/// Size: info_version(u32=4) + name([u8;256]=256) + version(u32=4) = 264 bytes.
#[repr(C)]
pub struct PluginInfo {
    pub info_version: u32,
    pub name: [u8; 256],
    pub version: u32,
}

impl PluginInfo {
    pub fn new(name: &str, version: u32) -> Self {
        let mut info = PluginInfo {
            info_version: 1,
            name: [0u8; 256],
            version,
        };
        let bytes = name.as_bytes();
        let len = bytes.len().min(255);
        info.name[..len].copy_from_slice(&bytes[..len]);
        if len < 256 {
            info.name[len] = 0;
        }
        info
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(256);
        std::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

/// Event listener callback type used by `NVSEInterface::register_listener`.
pub type EventListener = unsafe extern "C" fn(event_type: u32, event_data: *const std::ffi::c_void) -> u32;

/// NVSE bootstrap interface passed to `NVSEPlugin_Load`.
///
/// Real NVSE provides SafeWrite/trampoline helpers for patching. We snapshot
/// the interface at load time so hooks can use the engine's own helpers
/// instead of reimplementing VirtualProtect logic.
/// ponytail: simplified layout — verify against xNVSE NVSEInterface.h when
/// Proton testing begins.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NVSEInterface {
    pub interface_version: u32,
    pub get_plugin_info: unsafe extern "C" fn() -> *mut PluginInfo,
    pub query_interface: unsafe extern "C" fn(id: u32) -> *mut std::ffi::c_void,
    pub register_listener: unsafe extern "C" fn(*mut NVSEInterface, *const u8, EventListener),
    pub dispatch_message: unsafe extern "C" fn(
        *mut NVSEInterface,
        *const u8,
        *const u8,
        *mut u8,
        u32,
        *const u8,
    ) -> bool,
    pub safe_write8: unsafe extern "C" fn(u32, u32),
    pub safe_write16: unsafe extern "C" fn(u32, u32),
    pub safe_write32: unsafe extern "C" fn(u32, u32),
    pub safe_write_buf: unsafe extern "C" fn(u32, *mut u8, u32),
    pub write_rel_jump: unsafe extern "C" fn(u32, u32) -> *mut u8,
    pub write_rel_call: unsafe extern "C" fn(u32, u32) -> *mut u8,
}

/// Snapshot of the NVSE interface passed at load time, if any.
static NVSE_INTERFACE: LazyLock<Mutex<Option<NVSEInterface>>> = LazyLock::new(|| Mutex::new(None));

/// Return the NVSE interface captured during `NVSEPlugin_Load`, if any.
pub fn nvse_interface() -> Option<NVSEInterface> {
    *NVSE_INTERFACE.lock().unwrap()
}

/// Plugin interface version constant.
const PLUGIN_INTERFACE_VERSION: u32 = 1;

/// Called by NVSE/FOSE to query plugin info.
/// Returns true if this plugin supports the requested interface version.
#[no_mangle]
pub extern "C" fn NVSEPlugin_Query(
    interface_version: u32,
    info: *mut PluginInfo,
    _message: *mut u8,
) -> bool {
    if interface_version != PLUGIN_INTERFACE_VERSION {
        return false;
    }
    if !info.is_null() {
        unsafe {
            *info = PluginInfo::new("Ashfall Bridge", 1);
        }
    }
    true
}

/// Called by NVSE/FOSE to load the plugin.
/// `nvse_interface` carries SafeWrite/trampoline helpers (null when loaded
/// without NVSE, e.g. plain Wine DLL override).
/// Returns true on success.
#[no_mangle]
pub extern "C" fn NVSEPlugin_Load(nvse_interface: *const NVSEInterface) -> bool {
    // Snapshot the interface (copied — NVSE keeps the original alive).
    if !nvse_interface.is_null() {
        unsafe {
            *NVSE_INTERFACE.lock().unwrap() = Some(*nvse_interface);
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
