//! WASM callback context + dispatcher — holds pending callback outcomes
//! and provides permissive default callback stubs.

use ashfall_core::id::NetworkID;
use std::collections::HashMap;
use std::sync::Mutex;

/// Context for script callbacks. Scripts modify this; dispatch reads it.
pub struct CallbackContext {
    /// Pending auth results: player_name → allowed
    pub auth_results: Mutex<HashMap<String, bool>>,
    /// Pending chat results: player_id → allowed
    pub chat_results: Mutex<HashMap<u64, bool>>,
}

impl CallbackContext {
    pub fn new() -> Self {
        CallbackContext {
            auth_results: Mutex::new(HashMap::new()),
            chat_results: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_auth_result(&self, name: &str, allowed: bool) {
        self.auth_results.lock().unwrap().insert(name.to_string(), allowed);
    }

    pub fn take_auth_result(&self, name: &str) -> Option<bool> {
        self.auth_results.lock().unwrap().remove(name)
    }

    pub fn set_chat_result(&self, player_id: u64, allowed: bool) {
        self.chat_results.lock().unwrap().insert(player_id, allowed);
    }

    pub fn take_chat_result(&self, player_id: u64) -> Option<bool> {
        self.chat_results.lock().unwrap().remove(&player_id)
    }
}

impl Default for CallbackContext {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
// Callback stubs — permissive defaults for all 35 callbacks.
// Called when no WASM script defines the callback.
// ═══════════════════════════════════════════════════════════════

/// Permissive default callback implementation.
/// All callbacks that return bool default to `true` (allow).
pub struct CallbackDispatcher;

impl CallbackDispatcher {
    // ── Lifecycle ──
    pub fn on_server_init() {}
    pub fn on_server_exit(_shutdown: bool) {}

    // ── Auth ──
    pub fn on_client_authenticate(_name: &str, _password: &str) -> bool { true }

    // ── Player ──
    pub fn on_player_disconnect(_player_id: NetworkID, _reason: u8) {}
    pub fn on_player_request_game(_player_id: NetworkID) -> u32 { 0x0001A26E }
    pub fn on_spawn(_player_id: NetworkID) {}
    pub fn on_player_chat(_player_id: NetworkID, _message: &str) -> bool { true }

    // ── Object ──
    pub fn on_create(_object_id: NetworkID) {}
    pub fn on_destroy(_object_id: NetworkID) {}
    pub fn on_activate(_ref_id: u32, _actor_id: NetworkID) {}
    pub fn on_cell_change(_object_id: NetworkID, _cell: u32) {}
    pub fn on_lock_change(_object_id: NetworkID, _actor_id: NetworkID, _lock: u32) {}

    // ── Item ──
    pub fn on_item_count_change(_item_id: NetworkID, _count: u32) {}
    pub fn on_item_condition_change(_item_id: NetworkID, _condition: f32) {}
    pub fn on_item_equipped_change(_item_id: NetworkID, _equipped: bool) {}

    // ── Actor ──
    pub fn on_actor_value_change(_actor_id: NetworkID, _index: u8, _value: f32) {}
    pub fn on_actor_base_value_change(_actor_id: NetworkID, _index: u8, _value: f32) {}
    pub fn on_actor_alert(_actor_id: NetworkID, _alerted: bool) {}
    pub fn on_actor_sneak(_actor_id: NetworkID, _sneaking: bool) {}
    pub fn on_actor_death(_actor_id: NetworkID, _killer_id: NetworkID, _limbs: u16, _cause: i8) {}
    pub fn on_actor_punch(_actor_id: NetworkID, _power: bool) {}
    pub fn on_actor_fire_weapon(_actor_id: NetworkID, _weapon: u32) {}

    // ── Combat ──
    pub fn on_hit(_target_id: NetworkID, _attacker_id: NetworkID, _limb: u8, _damage: f32) -> bool { true }
    pub fn on_equip(_actor_id: NetworkID, _item_id: NetworkID, _equipped: bool) {}

    // ── GUI ──
    pub fn on_window_mode(_player_id: NetworkID, _enabled: bool) {}
    pub fn on_window_click(_player_id: NetworkID, _window_id: NetworkID) {}
    pub fn on_window_return(_player_id: NetworkID, _window_id: NetworkID) {}
    pub fn on_window_text_change(_player_id: NetworkID, _window_id: NetworkID, _text: &str) {}
    pub fn on_checkbox_select(_player_id: NetworkID, _checkbox_id: NetworkID, _selected: bool) {}
    pub fn on_radio_button_select(_player_id: NetworkID, _radio_id: NetworkID, _prev_id: NetworkID) {}
    pub fn on_list_item_select(_player_id: NetworkID, _item_id: NetworkID, _selected: bool) {}

    // ── Quest ──
    pub fn on_quest_stage(_quest_id: u32, _stage: u16) {}
    pub fn on_dialogue_choice(_player_id: NetworkID, _flag_id: u32, _choice: u32) {}

    // ── Time ──
    pub fn on_game_time_change(_year: u32, _month: u32, _day: u32, _hour: u32) {}
}
