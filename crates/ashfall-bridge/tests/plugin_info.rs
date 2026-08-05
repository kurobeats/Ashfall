//! PluginInfo + NVSEInterface tests — NVSE/FOSE plugin identity.

use ashfall_bridge::plugin::{EventListener, NVSEInterface, NVSEPlugin_Load, NVSEPlugin_Query, PluginInfo};

#[test]
fn test_plugin_info_struct_size() {
    // PluginInfo = info_version(u32=4) + name([u8; 256]=256) + version(u32=4) = 264 bytes
    assert_eq!(std::mem::size_of::<PluginInfo>(), 264);
}

#[test]
fn test_plugin_info_name_truncation() {
    // Name longer than 255 chars gets truncated
    let long_name = "A".repeat(300);
    let info = PluginInfo::new(&long_name, 1);

    let name_str = info.name_str();
    assert_eq!(name_str.len(), 255);

    // Verify truncation (no 300-byte name leaked)
    assert!(name_str.len() < 300);
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
fn test_plugin_info_exact_max() {
    // 255 chars exactly
    let name = "X".repeat(255);
    let info = PluginInfo::new(&name, 1);
    assert_eq!(info.name_str().len(), 255);
}

#[test]
fn test_plugin_info_null_terminated() {
    let info = PluginInfo::new("Test", 1);
    // name_str should stop at null byte
    let raw_name = &info.name;
    assert_eq!(raw_name[4], 0); // null terminator at position 4
    assert_eq!(raw_name[5], 0); // rest zeros
}

#[test]
fn test_nvse_plugin_query_ok() {
    let mut info = PluginInfo::new("", 0);
    let ok = NVSEPlugin_Query(1, &mut info, std::ptr::null_mut());
    assert!(ok);
    assert_eq!(info.name_str(), "Ashfall Bridge");
    assert_eq!(info.version, 1);
}

#[test]
fn test_nvse_plugin_query_wrong_version() {
    // Version 0 predates the plugin interface → false, info untouched
    let mut info = PluginInfo::new("unchanged", 0);
    let ok = NVSEPlugin_Query(0, &mut info, std::ptr::null_mut());
    assert!(!ok);
    assert_eq!(info.name_str(), "unchanged");
}

#[test]
fn test_nvse_plugin_query_forward_compat() {
    // Newer interface versions are accepted (xNVSE v6+ bumps the version)
    let mut info = PluginInfo::new("", 0);
    let ok = NVSEPlugin_Query(2, &mut info, std::ptr::null_mut());
    assert!(ok);
    assert_eq!(info.name_str(), "Ashfall Bridge");
}

// ── NVSEInterface layout + NVSEPlugin_Load snapshot ──

#[test]
fn test_nvse_interface_layout() {
    // Flat repr(C) struct: u32 header + function pointers.
    // On x86_64 fn pointers are 8 bytes, so the first fn pointer sits at
    // offset 8 (after 4-byte header + 4 padding). Alignment matches usize.
    assert_eq!(std::mem::align_of::<NVSEInterface>(), std::mem::align_of::<usize>());
    let ptr_size = std::mem::size_of::<usize>();
    assert_eq!(
        std::mem::offset_of!(NVSEInterface, get_plugin_info),
        if ptr_size == 8 { 8 } else { 4 }
    );
    assert!(std::mem::size_of::<NVSEInterface>() >= 2 * ptr_size);
}

unsafe extern "C" fn fake_plugin_info() -> *mut PluginInfo {
    std::ptr::null_mut()
}
unsafe extern "C" fn fake_query(_id: u32) -> *mut std::ffi::c_void {
    std::ptr::null_mut()
}
unsafe extern "C" fn fake_listener(
    _iface: *mut NVSEInterface,
    _name: *const u8,
    _listener: EventListener,
) {
}
unsafe extern "C" fn fake_dispatch(
    _iface: *mut NVSEInterface,
    _a: *const u8,
    _b: *const u8,
    _c: *mut u8,
    _d: u32,
    _e: *const u8,
) -> bool {
    true
}
unsafe extern "C" fn fake_safe_write8(_addr: u32, _value: u32) {}
unsafe extern "C" fn fake_safe_write16(_addr: u32, _value: u32) {}
unsafe extern "C" fn fake_safe_write32(_addr: u32, _value: u32) {}
unsafe extern "C" fn fake_safe_write_buf(_addr: u32, _data: *mut u8, _len: u32) {}
unsafe extern "C" fn fake_write_rel_jump(_from: u32, _to: u32) -> *mut u8 {
    std::ptr::null_mut()
}
unsafe extern "C" fn fake_write_rel_call(_from: u32, _to: u32) -> *mut u8 {
    std::ptr::null_mut()
}

#[test]
fn test_nvse_plugin_load_snapshots_interface() {
    let iface = NVSEInterface {
        interface_version: 1,
        get_plugin_info: fake_plugin_info,
        query_interface: fake_query,
        register_listener: fake_listener,
        dispatch_message: fake_dispatch,
        safe_write8: fake_safe_write8,
        safe_write16: fake_safe_write16,
        safe_write32: fake_safe_write32,
        safe_write_buf: fake_safe_write_buf,
        write_rel_jump: fake_write_rel_jump,
        write_rel_call: fake_write_rel_call,
    };

    assert!(NVSEPlugin_Load(&iface));

    let stored = ashfall_bridge::plugin::nvse_interface().expect("interface should be snapshotted");
    assert_eq!(stored.interface_version, 1);
    assert_eq!(stored.safe_write8 as usize, fake_safe_write8 as usize);
    assert_eq!(stored.write_rel_call as usize, fake_write_rel_call as usize);
}
