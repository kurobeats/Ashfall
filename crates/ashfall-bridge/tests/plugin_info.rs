//! PluginInfo struct tests — NVSE/FOSE plugin identity.

use ashfall_bridge::hooks::PluginInfo;

#[test]
fn test_plugin_info_struct_size() {
    // PluginInfo = info_version(u32=4) + name([u8; 256]=256) = 260 bytes
    assert_eq!(std::mem::size_of::<PluginInfo>(), 260);
}

#[test]
fn test_plugin_info_name_truncation() {
    // Name longer than 255 chars gets truncated
    let long_name = "A".repeat(300);
    let info = PluginInfo::new(&long_name);

    let name_str = info.name_str();
    assert_eq!(name_str.len(), 255);

    // Verify truncation (no 300-byte name leaked)
    assert!(name_str.len() < 300);
}

#[test]
fn test_plugin_info_default_name() {
    let info = PluginInfo::new("Ashfall Bridge");
    assert_eq!(info.name_str(), "Ashfall Bridge");
    assert_eq!(info.info_version, 1);
}

#[test]
fn test_plugin_info_empty_name() {
    let info = PluginInfo::new("");
    assert_eq!(info.name_str(), "");
}

#[test]
fn test_plugin_info_single_char() {
    let info = PluginInfo::new("A");
    assert_eq!(info.name_str(), "A");
}

#[test]
fn test_plugin_info_exact_max() {
    // 255 chars exactly
    let name = "X".repeat(255);
    let info = PluginInfo::new(&name);
    assert_eq!(info.name_str().len(), 255);
}

#[test]
fn test_plugin_info_null_terminated() {
    let info = PluginInfo::new("Test");
    // name_str should stop at null byte
    let raw_name = &info.name;
    assert_eq!(raw_name[4], 0); // null terminator at position 4
    assert_eq!(raw_name[5], 0); // rest zeros
}
