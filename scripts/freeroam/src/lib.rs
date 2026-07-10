//! Ashfall freeroam game mode — example WASM script.
//!
//! Minimal implementation: default spawn at Megaton exterior,
//! chat echo, and unrestricted player authentication.
//!
//! Built with: `cargo build --target wasm32-unknown-unknown --release`
//! Copy `target/wasm32-unknown-unknown/release/ashfall_freeroam.wasm`
//! to the server's `scripts/` directory.

use ashfall_script::*;

// ═══════════════════════════════════════════════════════════════
// Host function imports (provided by ashfall-server wasmtime engine)
// ═══════════════════════════════════════════════════════════════

extern "C" {
    // ── Server lifecycle ──
    fn host_log(level: u32, ptr: *const u8, len: u32);

    // ── Player lifecycle ──
    fn host_chat_message(player_id: u64, ptr: *const u8, len: u32);

    // ── World ──
    fn host_set_game_weather(weather: u32);
    fn host_set_game_time(year: u32, month: u32, day: u32, hour: u32);
}

// ═══════════════════════════════════════════════════════════════
// Exported callbacks (called by ashfall-server)
// ═══════════════════════════════════════════════════════════════

/// Called when server starts.
#[no_mangle]
pub extern "C" fn on_server_init() {
    let msg = b"Freeroam game mode loaded";
    unsafe { host_log(3, msg.as_ptr(), msg.len() as u32) }; // 3 = info

    // Set default weather (clear)
    unsafe { host_set_game_weather(0x00015E5E) };

    // Set game time to morning
    unsafe { host_set_game_time(2277, 8, 17, 9) };
}

/// Called when server shuts down.
#[no_mangle]
pub extern "C" fn on_server_exit(shutdown: bool) {
    let _ = shutdown;
}

/// Authenticate a connecting player. Return 1 = allow, 0 = deny.
#[no_mangle]
pub extern "C" fn on_client_authenticate(
    name_ptr: *const u8,
    name_len: u32,
    _pwd_ptr: *const u8,
    _pwd_len: u32,
) -> u32 {
    // Allow any name (no password check in freeroam)
    if name_len == 0 || name_len > 16 {
        return 0; // reject empty or too-long names
    }
    1 // allow
}

/// Player disconnected.
#[no_mangle]
pub extern "C" fn on_player_disconnect(player_id: u64, reason: u32) {
    let _ = (player_id, reason);
}

/// Choose spawn cell for a new player.
/// Returns cell ID — Megaton exterior (0x0001A26E).
#[no_mangle]
pub extern "C" fn on_player_request_game(_player_id: u64) -> u32 {
    // ponytail: Megaton exterior cell
    // In a full implementation, this would query spawn points.
    0x0001A26E
}

/// Player spawned into the world.
#[no_mangle]
pub extern "C" fn on_spawn(player_id: u64) {
    // Welcome message
    let msg = b"Welcome to the Wasteland!";
    unsafe { host_chat_message(player_id, msg.as_ptr(), msg.len() as u32) };
}

/// Player sent a chat message. Return 1 = relay, 0 = block.
#[no_mangle]
pub extern "C" fn on_player_chat(
    player_id: u64,
    message_ptr: *const u8,
    message_len: u32,
) -> u32 {
    let _ = (player_id, message_ptr, message_len);
    1 // always allow
}

/// Object created.
#[no_mangle]
pub extern "C" fn on_create(object_id: u64) {
    let _ = object_id;
}

/// Object destroyed.
#[no_mangle]
pub extern "C" fn on_destroy(object_id: u64) {
    let _ = object_id;
}

/// Object activated (door, container, NPC).
#[no_mangle]
pub extern "C" fn on_activate(ref_id: u32, actor_id: u64) {
    let _ = (ref_id, actor_id);
}

/// Object changed cells.
#[no_mangle]
pub extern "C" fn on_cell_change(object_id: u64, cell: u32) {
    let _ = (object_id, cell);
}

/// Item count changed.
#[no_mangle]
pub extern "C" fn on_item_count_change(item_id: u64, count: u32) {
    let _ = (item_id, count);
}

/// Actor died.
#[no_mangle]
pub extern "C" fn on_actor_death(
    actor_id: u64,
    killer_id: u64,
    limbs: u32,
    cause: u32,
) {
    let _ = (actor_id, killer_id, limbs, cause);
}

/// Actor value changed.
#[no_mangle]
pub extern "C" fn on_actor_value_change(actor_id: u64, index: u32, value: f32) {
    let _ = (actor_id, index, value);
}

/// Hit event (combat).
#[no_mangle]
pub extern "C" fn on_hit(target_id: u64, attacker_id: u64, limb: u32, damage: f32) -> u32 {
    let _ = (target_id, attacker_id, limb, damage);
    1 // allow all hits
}

/// Item equipped/unequipped.
#[no_mangle]
pub extern "C" fn on_equip(actor_id: u64, item_id: u64, equipped: u32) {
    let _ = (actor_id, item_id, equipped);
}

/// Game time changed.
#[no_mangle]
pub extern "C" fn on_game_time_change(year: u32, month: u32, day: u32, hour: u32) {
    let _ = (year, month, day, hour);
}

// ── The following callbacks exist but are unused in freeroam ──
// on_lock_change, on_item_condition_change, on_item_equipped_change,
// on_actor_base_value_change, on_actor_alert, on_actor_sneak,
// on_actor_punch, on_actor_fire_weapon, on_window_mode, on_window_click,
// on_window_return, on_window_text_change, on_checkbox_select,
// on_radio_button_select, on_list_item_select, on_quest_stage,
// on_dialogue_choice
//
// ponytail: add as needed for specific game modes.
