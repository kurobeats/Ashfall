//! Script runtime integration tests — real WASM execution through wasmtime.
//!
//! The test "game mode" is written in WAT (compiled at test time via the
//! `wat` crate — no wasm32 toolchain required) and exercises:
//!   - real host functions (weather, game time, quests, chat, timers)
//!   - callback dispatch into WASM (auth, chat, spawn cell, spawn, death)
//!   - timer callback routing (create_timer → exported fn)
//!   - the script effect queue (chat_message → ScriptEffect::PrivateChat)

use ashfall_server::ai::factions::FactionMatrix;
use ashfall_server::quest::QuestManager;
use ashfall_server::script::engine::{ScriptEngine, ScriptState};
use ashfall_server::script::state::{GameTime, ScriptEffect};
use ashfall_server::world::globals::GlobalState;
use ashfall_server::world::registry::ObjectRegistry;
use ashfall_server::world::weather::WeatherState;
use ashfall_core::id::NetworkID;
use std::sync::atomic::Ordering;
use std::sync::Arc;

const TEST_MODE: &str = r#"
(module
  (import "env" "set_game_weather" (func $set_weather (param i32)))
  (import "env" "set_game_time" (func $set_time (param i32 i32 i32 i32)))
  (import "env" "set_quest_stage" (func $set_quest (param i32 i32)))
  (import "env" "set_dialogue_flag" (func $set_flag (param i32 i32)))
  (import "env" "chat_message" (func $chat (param i64 i32 i32)))
  (import "env" "create_timer" (func $create_timer (param i32 i32 i32 i32) (result i32)))
  (import "env" "get_current_players" (func $get_players (result i32)))
  (import "env" "create_item" (func $create_item (param i32 i32 i64) (result i64)))
  (import "env" "add_item" (func $add_item (param i64 i64)))
  (import "env" "remove_item" (func $remove_item (param i64)))
  (import "env" "equip_item" (func $equip_item (param i64 i64)))
  (import "env" "get_item_count" (func $get_item_count (param i64) (result i32)))
  (import "env" "set_time_scale" (func $set_scale (param f32)))
  (import "env" "get_damage_resistance" (func $get_dr (param i64) (result f32)))
  (import "env" "get_damage_threshold" (func $get_dt (param i64) (result f32)))
  (import "env" "set_server_name" (func $set_name (param i32 i32)))
  (import "env" "get_config_int" (func $get_cfg (param i32 i32) (result i32)))
  (import "env" "create_window" (func $create_window (param i64 i32 i32) (result i64)))
  (import "env" "set_window_text" (func $set_win_text (param i64 i32 i32)))
  (import "env" "create_object" (func $create_object (param i32 i32 i32) (result i64)))
  (import "env" "create_actor" (func $create_actor (param i32 i32 i32) (result i64)))
  (import "env" "set_pos" (func $set_pos (param i64 f32 f32 f32)))
  (import "env" "get_pos_x" (func $get_pos_x (param i64) (result f32)))
  (import "env" "set_actor_value" (func $set_av (param i64 i32 f32)))
  (import "env" "get_actor_value" (func $get_av (param i64 i32) (result f32)))
  (import "env" "kill_actor" (func $kill (param i64)))

  (memory (export "memory") 1)
  (global $players (mut i32) (i32.const 0))
  (global $obj_id (mut i64) (i64.const 0))
  (global $actor_id (mut i64) (i64.const 0))
  (global $item_id (mut i64) (i64.const 0))
  (global $cfg_max (mut i32) (i32.const 0))
  (data (i32.const 2048) "tick_cb")
  (data (i32.const 4096) "Hello from script!")
  (data (i32.const 8192) "My Server")
  (data (i32.const 8208) "max_players")
  (data (i32.const 8224) "Test Window")

  ;; Boot: set weather + clock, arm a repeating 5ms timer
  (func (export "on_server_init")
    (call $set_weather (i32.const 0x00012345))
    (call $set_time (i32.const 2277) (i32.const 1) (i32.const 2) (i32.const 3))
    (drop (call $create_timer (i32.const 5) (i32.const 2048) (i32.const 7) (i32.const 1))))

  ;; Auth: deny names of length 3 starting with byte 'b'
  (func (export "on_client_authenticate")
    (param $nptr i32) (param $nlen i32) (param $pptr i32) (param $plen i32) (result i32)
    (if (i32.and (i32.eq (local.get $nlen) (i32.const 3))
                 (i32.eq (i32.load8_u (local.get $nptr)) (i32.const 98)))
      (then (return (i32.const 0))))
    (i32.const 1))

  ;; Chat: block messages starting with '!'
  (func (export "on_player_chat")
    (param $pid i64) (param $mptr i32) (param $mlen i32) (result i32)
    (if (i32.and (i32.gt_u (local.get $mlen) (i32.const 0))
                 (i32.eq (i32.load8_u (local.get $mptr)) (i32.const 33)))
      (then (return (i32.const 0))))
    (i32.const 1))

  ;; Custom spawn cell
  (func (export "on_player_request_game") (param $pid i64) (result i32)
    (i32.const 0x0000CAFE))

  ;; on_spawn: private-chat the player
  (func (export "on_spawn") (param $pid i64)
    (call $chat (local.get $pid) (i32.const 4096) (i32.const 18)))

  ;; on_actor_death: advance a quest stage as a side effect
  (func (export "on_actor_death") (param $a i64) (param $k i64) (param $limbs i32) (param $cause i32)
    (call $set_quest (i32.const 0x1000) (i32.const 10)))

  ;; on_quest_stage: mirror every stage change into a dialogue flag
  (func (export "on_quest_stage") (param $quest i32) (param $stage i32)
    (call $set_flag (i32.const 55) (i32.const 1)))

  ;; Timer callback: swap the weather
  (func (export "tick_cb") (param $id i32)
    (call $set_weather (i32.const 0x00007777)))

  ;; Exported test helpers
  (func (export "quest_work")
    (call $set_quest (i32.const 0x1000) (i32.const 5))
    (call $set_flag (i32.const 7) (i32.const 1)))

  (func (export "say_hello")
    (call $chat (i64.const 42) (i32.const 4096) (i32.const 18)))

  (func (export "update_player_count")
    (global.set $players (call $get_players)))

  (func (export "get_player_count") (result i32)
    (global.get $players))

  ;; Object/actor CRUD round-trip
  (func (export "spawn_work")
    (global.set $obj_id
      (call $create_object (i32.const 0x100) (i32.const 0x200) (i32.const 0x300)))
    (call $set_pos (global.get $obj_id) (f32.const 10) (f32.const 20) (f32.const 30))
    (global.set $actor_id
      (call $create_actor (i32.const 0x400) (i32.const 0x500) (i32.const 0x300)))
    (call $set_av (global.get $actor_id) (i32.const 0x14) (f32.const 75.5))
    (call $kill (global.get $actor_id)))

  (func (export "get_obj_id") (result i64)
    (global.get $obj_id))

  (func (export "get_actor_id") (result i64)
    (global.get $actor_id))

  (func (export "get_obj_pos_x") (result f32)
    (call $get_pos_x (global.get $obj_id)))

  (func (export "get_actor_hp") (result f32)
    (call $get_av (global.get $actor_id) (i32.const 0x14)))

  ;; Item + inventory ops
  (func (export "item_work")
    (global.set $item_id
      (call $create_item (i32.const 0x10) (i32.const 0x999) (global.get $actor_id)))
    (call $add_item (global.get $item_id) (global.get $actor_id))
    (call $equip_item (global.get $actor_id) (global.get $item_id)))

  (func (export "get_item_id") (result i64)
    (global.get $item_id))

  (func (export "item_count") (result i32)
    (call $get_item_count (global.get $item_id)))

  (func (export "drop_item")
    (call $remove_item (global.get $item_id)))

  ;; Combat values
  (func (export "get_dr") (result f32)
    (call $get_dr (global.get $actor_id)))
  (func (export "get_dt") (result f32)
    (call $get_dt (global.get $actor_id)))

  ;; Server meta
  (func (export "name_server")
    (call $set_name (i32.const 8192) (i32.const 9)))
  (func (export "read_max_players")
    (global.set $cfg_max (call $get_cfg (i32.const 8208) (i32.const 11))))
  (func (export "get_cfg_max") (result i32)
    (global.get $cfg_max))
  (func (export "speed_up_time")
    (call $set_scale (f32.const 100)))

  ;; GUI
  (func (export "make_window")
    (global.set $obj_id (call $create_window (i64.const 0) (i32.const 8224) (i32.const 11)))
    (call $set_win_text (global.get $obj_id) (i32.const 8224) (i32.const 11)))
)
"#;

/// Fresh ScriptState with test defaults.
fn new_state() -> ScriptState {
    ScriptState::new(
        Arc::new(ObjectRegistry::default()),
        WeatherState::default(),
        GlobalState::new(),
        QuestManager::new(),
        FactionMatrix::default(),
        "test".into(),
        String::new(),
        4,
    )
}

/// Load + instantiate the test game mode against `state` (shared by clone).
fn boot_with(state: ScriptState) -> ScriptEngine {
    let mut engine = ScriptEngine::new().expect("engine init");
    let bytes = wat::parse_str(TEST_MODE).expect("valid WAT");
    engine.load_module_bytes("test-mode", &bytes).expect("module loads");
    engine.instantiate_all(state).expect("instantiation");
    engine
}

// ═══════════════════════════════════════════════════════════════
// Real host functions — scripts drive server state
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_on_server_init_host_calls_are_real() {
    let state = new_state();
    let _engine = boot_with(state.clone());
    assert_eq!(state.weather.get(), 0x00012345, "on_server_init set weather");
    assert_eq!(
        state.game_time.get(),
        GameTime { year: 2277, month: 1, day: 2, hour: 3 },
        "on_server_init set game time"
    );
}

#[test]
fn test_quest_host_functions_are_real() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("quest_work", &[]), "quest_work runs");
    assert_eq!(state.quests.get_stage(0x1000), 5, "set_quest_stage is real");
    assert!(state.quests.get_flag(7), "set_dialogue_flag is real");
}

// ═══════════════════════════════════════════════════════════════
// Callback dispatch — server events reach WASM
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_auth_dispatch() {
    let state = new_state();
    let mut engine = boot_with(state);
    assert!(engine.dispatch_auth("alice", "pw"), "normal name allowed");
    assert!(!engine.dispatch_auth("bob", "pw"), "deny rule honored");
    assert!(engine.dispatch_auth("carol", "pw"), "other names allowed");
}

#[test]
fn test_chat_dispatch() {
    let state = new_state();
    let mut engine = boot_with(state);
    assert!(engine.dispatch_chat(1, "hello world"), "normal chat allowed");
    assert!(!engine.dispatch_chat(1, "!kickme"), "block rule honored");
}

#[test]
fn test_spawn_cell_dispatch() {
    let state = new_state();
    let mut engine = boot_with(state);
    assert_eq!(engine.dispatch_spawn_cell(1), 0x0000CAFE);
}

#[test]
fn test_notify_callbacks() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    engine.notify_spawn(42);
    assert_eq!(
        engine.drain_effects(),
        vec![ScriptEffect::PrivateChat { player_id: 42, message: "Hello from script!".into() }],
        "on_spawn private-chat effect queued"
    );

    engine.notify_actor_death(100, 200, 2, 1);
    assert_eq!(state.quests.get_stage(0x1000), 10, "on_actor_death set quest stage");
}

// ═══════════════════════════════════════════════════════════════
// Timers + player count + effect queue
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_timer_routing_calls_wasm_callback() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    // on_server_init armed a 5ms repeating "tick_cb" timer
    std::thread::sleep(std::time::Duration::from_millis(15));
    let fired = engine.tick_timers();
    assert!(
        fired.iter().any(|(_, cb)| cb == "tick_cb"),
        "timer fired with callback name: {fired:?}"
    );
    for (id, cb) in fired {
        engine.dispatch_timer(id, &cb);
    }
    assert_eq!(state.weather.get(), 0x00007777, "tick_cb changed the weather");
}

#[test]
fn test_player_count_host_function() {
    let state = new_state();
    let mut engine = boot_with(state);
    // Simulate the server maintaining the live player count
    let arc = engine.player_count.clone().expect("handle set at instantiation");
    arc.store(3, Ordering::Relaxed);
    assert!(engine.call_export_void("update_player_count", &[]));
    assert_eq!(engine.call_export_i32("get_player_count", &[]), Some(3));
}

#[test]
fn test_effect_queue_private_chat() {
    let state = new_state();
    let mut engine = boot_with(state);
    assert!(engine.call_export_void("say_hello", &[]));
    assert_eq!(
        engine.drain_effects(),
        vec![ScriptEffect::PrivateChat { player_id: 42, message: "Hello from script!".into() }]
    );
    assert!(engine.drain_effects().is_empty(), "queue drained");
}

#[test]
fn test_quest_stage_notification() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    engine.notify_quest_stage(0x999, 3);
    assert!(state.quests.get_flag(55), "on_quest_stage callback fired");
}

// ═══════════════════════════════════════════════════════════════
// Object / actor CRUD host functions
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_object_actor_crud_host_functions() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("spawn_work", &[]), "spawn_work runs");

    let obj_id = NetworkID::new(engine.call_export_i64("get_obj_id", &[]).unwrap() as u64);
    assert_eq!(engine.call_export_f32("get_obj_pos_x", &[]), Some(10.0), "set/get_pos round-trip");
    assert_eq!(engine.call_export_f32("get_actor_hp", &[]), Some(75.5), "set/get_actor_value round-trip");

    // Host-side visibility: the registry holds the spawned object + actor.
    let arc = state.registry.get(obj_id).expect("object in registry");
    let guard = arc.read();
    let obj = guard.as_any().downcast_ref::<ashfall_server::world::objects::Object>()
        .expect("spawned object is an Object");
    assert_eq!(obj.game_pos, [10.0, 20.0, 30.0], "set_pos mutated authoritative state");

    let actor_id = NetworkID::new(engine.call_export_i64("get_actor_id", &[]).unwrap() as u64);
    drop(guard);
    let arc = state.registry.get(actor_id).expect("actor in registry");
    let guard = arc.read();
    let actor = guard.as_any().downcast_ref::<ashfall_server::world::objects::Actor>()
        .expect("spawned actor is an Actor");
    assert!(actor.dead, "kill_actor marked the actor dead");
    assert_eq!(actor.values.get(&0x14), Some(&75.5), "set_actor_value stored");
}

// ═══════════════════════════════════════════════════════════════
// Real item / combat / server-meta / GUI host functions
// ═══════════════════════════════════════════════════════════════

use ashfall_server::world::objects::{Actor, Item};

#[test]
fn test_item_host_functions() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("spawn_work", &[]));
    assert!(engine.call_export_void("item_work", &[]), "item_work runs");

    let item_id = NetworkID::new(engine.call_export_i64("get_item_id", &[]).unwrap() as u64);
    assert_eq!(engine.call_export_i32("item_count", &[]), Some(1), "get_item_count reads state (Item::new count=1)");

    // Host-side: the item is linked into the actor's container and equipped
    let actor_id = NetworkID::new(engine.call_export_i64("get_actor_id", &[]).unwrap() as u64);
    let arc = state.registry.get(actor_id).expect("actor exists");
    let guard = arc.read();
    let actor = guard.as_any().downcast_ref::<Actor>().unwrap();
    assert!(actor.container.items.contains(&item_id), "item linked into actor container");
    drop(guard);

    let arc = state.registry.get(item_id).expect("item in registry");
    let guard = arc.read();
    let item = guard.as_any().downcast_ref::<Item>().unwrap();
    assert!(item.equipped, "equip_item marked item equipped");
    assert_eq!(item.container, actor_id, "item container set to actor");
    drop(guard);

    // remove_item destroys the item + unlinks it
    assert!(engine.call_export_void("drop_item", &[]));
    assert!(state.registry.get(item_id).is_none(), "remove_item destroyed item");
    let arc = state.registry.get(actor_id).unwrap();
    let guard = arc.read();
    let actor = guard.as_any().downcast_ref::<Actor>().unwrap();
    assert!(!actor.container.items.contains(&item_id), "item unlinked from container");
}

#[test]
fn test_combat_value_host_functions() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("spawn_work", &[]));
    // Set the actor's DR (0x29) and DT (0x2A) values, then read them back
    let actor_id = engine.call_export_i64("get_actor_id", &[]).unwrap();
    // set_actor_value via the module: reuse the existing $set_av import through
    // the exported spawn path is not exposed — call the host fn directly.
    let arc = state.registry.get(NetworkID::new(actor_id as u64)).unwrap();
    {
        let mut guard = arc.write();
        let actor = guard.as_any_mut().downcast_mut::<Actor>().unwrap();
        actor.set_value(0x29, 0.5, false);
        actor.set_value(0x2A, 4.0, false);
    }
    assert_eq!(engine.call_export_f32("get_dr", &[]), Some(0.5), "get_damage_resistance reads value");
    assert_eq!(engine.call_export_f32("get_dt", &[]), Some(4.0), "get_damage_threshold reads value");
}

#[test]
fn test_server_meta_host_functions() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("name_server", &[]));
    assert_eq!(*state.server_name.lock().unwrap(), "My Server", "set_server_name stored");

    assert!(engine.call_export_void("read_max_players", &[]));
    assert_eq!(engine.call_export_i32("get_cfg_max", &[]), Some(4), "get_config_int(max_players)");
}

#[test]
fn test_time_scale_host_function() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("speed_up_time", &[]));
    assert_eq!(state.game_time.get_scale(), 100.0, "set_time_scale stored");
}

#[test]
fn test_gui_widget_host_functions() {
    let state = new_state();
    let mut engine = boot_with(state.clone());
    assert!(engine.call_export_void("make_window", &[]), "make_window runs");

    let effects = engine.drain_effects();
    assert!(
        effects.iter().any(|e| matches!(
            e,
            ScriptEffect::BroadcastPacket(ashfall_core::protocol::Packet::WindowNew { label, .. })
                if label == "Test Window"
        )),
        "create_window emitted a WindowNew broadcast: {effects:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            ScriptEffect::BroadcastPacket(ashfall_core::protocol::Packet::UpdateWindowText { text, .. })
                if text == "Test Window"
        )),
        "set_window_text emitted an update"
    );
}
