//! Phase 5 integration test — scripting engine, timers, callbacks, freeroam.

use ashfall_server::script::engine::ScriptEngine;
use ashfall_server::script::timer::TimerManager;
use std::path::Path;

// ═══════════════════════════════════════════════════════════════
// Engine tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_script_engine_creation() {
    let engine = ScriptEngine::new();
    assert!(engine.is_ok(), "ScriptEngine::new() should succeed");
    let engine = engine.unwrap();
    assert_eq!(engine.module_count(), 0);
    assert_eq!(engine.instance_count(), 0);
}

#[test]
fn test_load_modules_empty_dir() {
    let mut engine = ScriptEngine::new().unwrap();
    let dir = std::env::temp_dir().join("ashfall_test_empty_scripts");
    let _ = std::fs::create_dir_all(&dir);
    let result = engine.load_modules(&dir);
    assert!(result.is_ok(), "load_modules on empty dir should succeed");
    assert_eq!(engine.module_count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_load_modules_nonexistent_dir() {
    let mut engine = ScriptEngine::new().unwrap();
    let dir = Path::new("/tmp/ashfall_nonexistent_scripts_xyz");
    let result = engine.load_modules(dir);
    assert!(result.is_ok(), "load_modules on nonexistent dir should succeed gracefully");
    assert_eq!(engine.module_count(), 0);
}

// ═══════════════════════════════════════════════════════════════
// Timer tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_timer_create_and_fire() {
    let mut manager = TimerManager::new();
    let id = manager.create_timer(1, "test_callback".to_string(), true);
    assert_eq!(id, 1);
    std::thread::sleep(std::time::Duration::from_millis(5));
    let fired = manager.tick();
    assert!(fired.iter().any(|(_, name)| name == "test_callback"));
}

#[test]
fn test_timer_kill() {
    let mut manager = TimerManager::new();
    let id = manager.create_timer(100_000, "killed".to_string(), true);
    assert!(manager.kill_timer(id));
    let fired = manager.tick();
    assert!(!fired.iter().any(|(_, name)| name == "killed"));
}

#[test]
fn test_timer_kill_nonexistent() {
    let mut manager = TimerManager::new();
    assert!(!manager.kill_timer(999));
}

// ═══════════════════════════════════════════════════════════════
// Callback stubs — verify permissive defaults
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_callback_stubs_auth() {
    use ashfall_server::script::callbacks::CallbackDispatcher;
    assert!(CallbackDispatcher::on_client_authenticate("player", "password"));
    assert!(CallbackDispatcher::on_client_authenticate("", ""));
}

#[test]
fn test_callback_stubs_chat() {
    use ashfall_server::script::callbacks::CallbackDispatcher;
    assert!(CallbackDispatcher::on_player_chat(
        ashfall_core::id::NetworkID::new(1), "hello",
    ));
}

#[test]
fn test_callback_stubs_player() {
    use ashfall_server::script::callbacks::CallbackDispatcher;
    let pid = ashfall_core::id::NetworkID::new(1);
    CallbackDispatcher::on_player_disconnect(pid, 0);
    CallbackDispatcher::on_spawn(pid);
    let cell = CallbackDispatcher::on_player_request_game(pid);
    assert_ne!(cell, 0);
}

#[test]
fn test_callback_stubs_hit() {
    use ashfall_server::script::callbacks::CallbackDispatcher;
    let tid = ashfall_core::id::NetworkID::new(1);
    let aid = ashfall_core::id::NetworkID::new(2);
    assert!(CallbackDispatcher::on_hit(tid, aid, 0, 25.0));
}

#[test]
fn test_callback_stubs_equip() {
    use ashfall_server::script::callbacks::CallbackDispatcher;
    let aid = ashfall_core::id::NetworkID::new(1);
    let iid = ashfall_core::id::NetworkID::new(2);
    CallbackDispatcher::on_equip(aid, iid, true);
}

#[test]
fn test_core_sdk_types() {
    use ashfall_core::id::NetworkID;
    use ashfall_core::types::{ObjectKind, Reason};
    use ashfall_core::form_id::FormID;

    let nid = NetworkID::new(42);
    assert_eq!(nid.as_u64(), 42);
    let fid = FormID::new(0x01001234);
    assert_eq!(fid.mod_index(), 1);
    assert_eq!(Reason::Quit as u8, 4);
    assert_eq!(ObjectKind::Player as u32, 0x40);
}

#[test]
fn test_freeroam_script_exists() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scripts")
        .join("freeroam")
        .join("Cargo.toml");
    assert!(path.exists(), "freeroam Cargo.toml should exist at {}", path.display());
}
