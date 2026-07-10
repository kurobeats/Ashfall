//! Host functions exposed to WASM scripts.
//!
//! ponytail: minimal stubs that compile. Real implementations
//! in Phase 5 Part B (detailed host function codegen).

use crate::script::engine::ScriptState;
use crate::script::timer::TimerManager;
use std::sync::{Arc, Mutex};
use wasmtime::*;

/// Registry of host functions callable from WASM.
pub struct HostFunctions;

impl HostFunctions {
    /// Define all host functions in the wasmtime Linker.
    pub fn define_in_linker(
        &self,
        linker: &mut Linker<ScriptState>,
    ) -> anyhow::Result<()> {
        // ── Server management ──
        linker.func_wrap("env", "set_server_name",
            |_: Caller<'_, ScriptState>, _ptr: i32, _len: i32| {})?;
        linker.func_wrap("env", "get_max_players",
            |_: Caller<'_, ScriptState>| -> i32 { 4 })?;
        linker.func_wrap("env", "get_current_players",
            |_: Caller<'_, ScriptState>| -> i32 { 0 })?;
        linker.func_wrap("env", "timestamp",
            || -> i64 { 0 })?;

        // ── Object CRUD ──
        linker.func_wrap("env", "create_object",
            |mut caller: Caller<'_, ScriptState>, _ref_id: i32, _base_id: i32, _cell: i32| -> i64 {
                let state = caller.data();
                state.registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "destroy_object",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32| {})?;
        linker.func_wrap("env", "get_pos_x",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32| -> f32 { 0.0 })?;
        linker.func_wrap("env", "get_pos_y",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32| -> f32 { 0.0 })?;
        linker.func_wrap("env", "get_pos_z",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32| -> f32 { 0.0 })?;
        linker.func_wrap("env", "set_pos",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32, _x: f32, _y: f32, _z: f32| {})?;

        // ── Actor ──
        linker.func_wrap("env", "create_actor",
            |mut caller: Caller<'_, ScriptState>, _ref_id: i32, _base_id: i32, _cell: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "get_actor_value",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32, _index: i32| -> f32 { 0.0 })?;
        linker.func_wrap("env", "set_actor_value",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32, _index: i32, _value: f32| {})?;
        linker.func_wrap("env", "kill_actor",
            |_: Caller<'_, ScriptState>, _id_hi: i32, _id_lo: i32| {})?;

        // ── Item ──
        linker.func_wrap("env", "create_item",
            |mut caller: Caller<'_, ScriptState>, _ref_id: i32, _base_id: i32, _cont_hi: i32, _cont_lo: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "add_item",
            |_: Caller<'_, ScriptState>, _item_hi: i32, _item_lo: i32, _cont_hi: i32, _cont_lo: i32| {})?;
        linker.func_wrap("env", "remove_item",
            |_: Caller<'_, ScriptState>, _item_hi: i32, _item_lo: i32| {})?;
        linker.func_wrap("env", "equip_item",
            |_: Caller<'_, ScriptState>, _actor_hi: i32, _actor_lo: i32, _item_hi: i32, _item_lo: i32| {})?;
        linker.func_wrap("env", "get_item_count",
            |_: Caller<'_, ScriptState>, _item_hi: i32, _item_lo: i32| -> i32 { 0 })?;

        // ── Chat / UI ──
        linker.func_wrap("env", "chat_message",
            |_: Caller<'_, ScriptState>, _ptr: i32, _len: i32| {})?;
        linker.func_wrap("env", "ui_message",
            |_: Caller<'_, ScriptState>, _player_hi: i32, _player_lo: i32, _ptr: i32, _len: i32| {})?;
        linker.func_wrap("env", "kick",
            |_: Caller<'_, ScriptState>, _player_hi: i32, _player_lo: i32| {})?;

        // ── GUI (window widgets) ──
        linker.func_wrap("env", "create_window",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _label_ptr: i32, _label_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "destroy_window",
            |_: Caller<'_, ScriptState>, _id: i64| {})?;
        linker.func_wrap("env", "set_window_pos",
            |_: Caller<'_, ScriptState>, _id: i64, _x: f32, _y: f32, _ox: f32, _oy: f32| {})?;
        linker.func_wrap("env", "set_window_size",
            |_: Caller<'_, ScriptState>, _id: i64, _w: f32, _h: f32| {})?;
        linker.func_wrap("env", "set_window_visible",
            |_: Caller<'_, ScriptState>, _id: i64, _visible: i32| {})?;
        linker.func_wrap("env", "set_window_locked",
            |_: Caller<'_, ScriptState>, _id: i64, _locked: i32| {})?;
        linker.func_wrap("env", "set_window_text",
            |_: Caller<'_, ScriptState>, _id: i64, _ptr: i32, _len: i32| {})?;
        linker.func_wrap("env", "create_button",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _label_ptr: i32, _label_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_text",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _text_ptr: i32, _text_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_edit",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _max_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_checkbox",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _label_ptr: i32, _label_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_radiobutton",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _group: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_list",
            |mut caller: Caller<'_, ScriptState>, _parent: i64, _multiselect: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "add_list_item",
            |mut caller: Caller<'_, ScriptState>, _list_id: i64, _text_ptr: i32, _text_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "remove_list_item",
            |_: Caller<'_, ScriptState>, _item_id: i64| {})?;

        // ── World state ──
        linker.func_wrap("env", "set_game_weather",
            |_: Caller<'_, ScriptState>, _weather: i32| {})?;
        linker.func_wrap("env", "get_game_weather",
            |_: Caller<'_, ScriptState>| -> i32 { 0 })?;
        linker.func_wrap("env", "set_game_time",
            |_: Caller<'_, ScriptState>, _year: i32, _month: i32, _day: i32, _hour: i32| {})?;
        linker.func_wrap("env", "set_time_scale",
            |_: Caller<'_, ScriptState>, _scale: f32| {})?;

        // ── Timers ──
        linker.func_wrap("env", "create_timer",
            |mut caller: Caller<'_, ScriptState>, interval_ms: i32, _cb_ptr: i32, _cb_len: i32, _repeat: i32| -> i32 {
                let state = caller.data();
                let mut tm = state.timers.lock().unwrap();
                let id = tm.create_timer(interval_ms as u64, "script_timer".to_string(), true);
                id as i32
            })?;
        linker.func_wrap("env", "kill_timer",
            |mut caller: Caller<'_, ScriptState>, id: i32| {
                caller.data().timers.lock().unwrap().kill_timer(id as u32);
            })?;

        // ── Quest ──
        linker.func_wrap("env", "get_quest_stage",
            |_: Caller<'_, ScriptState>, _quest_id: i32| -> i32 { 0 })?;
        linker.func_wrap("env", "set_quest_stage",
            |_: Caller<'_, ScriptState>, _quest_id: i32, _stage: i32| {})?;
        linker.func_wrap("env", "get_dialogue_flag",
            |_: Caller<'_, ScriptState>, _flag_id: i32| -> i32 { 0 })?;
        linker.func_wrap("env", "set_dialogue_flag",
            |_: Caller<'_, ScriptState>, _flag_id: i32, _value: i32| {})?;

        // ── Combat ──
        linker.func_wrap("env", "get_damage_resistance",
            |_: Caller<'_, ScriptState>, _actor_hi: i32, _actor_lo: i32| -> f32 { 0.0 })?;
        linker.func_wrap("env", "get_damage_threshold",
            |_: Caller<'_, ScriptState>, _actor_hi: i32, _actor_lo: i32| -> f32 { 0.0 })?;

        // ── Utility ──
        linker.func_wrap("env", "debug_log",
            |_: Caller<'_, ScriptState>, _ptr: i32, _len: i32| {})?;
        linker.func_wrap("env", "get_config_int",
            |_: Caller<'_, ScriptState>, _key_ptr: i32, _key_len: i32| -> i32 { 0 })?;

        Ok(())
    }
}
