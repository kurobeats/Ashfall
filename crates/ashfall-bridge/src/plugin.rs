//! NVSE/FOSE plugin interface — exports and registration.
//!
//! Called by NVSE/FOSE when the bridge DLL is loaded as a script extender plugin.
//! Fallback: if loaded via Wine DLL override (non-NVSE), DllMain handles init.

/// PluginInfo struct — matches NVSE/FOSE PluginInfo layout.
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
        info
    }
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
/// Returns true on success.
#[no_mangle]
pub extern "C" fn NVSEPlugin_Load(_nvse_interface: *const std::ffi::c_void) -> bool {
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
