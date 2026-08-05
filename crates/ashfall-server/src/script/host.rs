//! Host functions exposed to WASM scripts.
//!
//! Real implementations for world/quest/chat/clock/player-count state;
//! object CRUD remains ID-allocation stubs until object scripting lands.
//!
//! ABI note: `u64` ids cross the boundary as `i64`, strings as `(ptr, len)`
//! pairs into linear memory — see scripts/freeroam/src/lib.rs.

use crate::script::engine::ScriptState;
use crate::script::state::{GameTime, ScriptEffect};
use crate::world::objects::{Actor, Object};
use ashfall_core::id::NetworkID;
use std::time::{SystemTime, UNIX_EPOCH};
use wasmtime::*;

/// Read a string from WASM linear memory at (ptr, len).
fn read_wasm_string(caller: &mut Caller<'_, ScriptState>, ptr: i32, len: i32) -> String {
    if ptr < 0 || len <= 0 {
        return String::new();
    }
    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return String::new(),
    };
    let start = ptr as usize;
    let end = start + len as usize;
    let bytes = mem.data(&caller);
    if end > bytes.len() {
        return String::new();
    }
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

/// Milliseconds since UNIX_EPOCH — real wall-clock for `timestamp()`.
fn now_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_millis() as i64,
        Err(_) => 0,
    }
}

/// Read the authoritative position of an object or actor by NetworkID.
fn object_pos(caller: &Caller<'_, ScriptState>, id: i64) -> Option<[f32; 3]> {
    let registry = &caller.data().registry;
    let arc = registry.get(NetworkID::new(id as u64))?;
    let guard = arc.read();
    if let Some(o) = guard.as_any().downcast_ref::<Object>() {
        return Some(o.game_pos);
    }
    if let Some(a) = guard.as_any().downcast_ref::<Actor>() {
        return Some(a.container.object.game_pos);
    }
    None
}

/// Set the authoritative position of an object or actor by NetworkID.
fn set_object_pos(caller: &Caller<'_, ScriptState>, id: i64, pos: [f32; 3]) {
    let registry = &caller.data().registry;
    let Some(arc) = registry.get(NetworkID::new(id as u64)) else { return };
    let mut guard = arc.write();
    if let Some(o) = guard.as_any_mut().downcast_mut::<Object>() {
        o.game_pos = pos;
        o.net_pos = pos;
        return;
    }
    if let Some(a) = guard.as_any_mut().downcast_mut::<Actor>() {
        a.container.object.game_pos = pos;
        a.container.object.net_pos = pos;
    }
}

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
            |caller: Caller<'_, ScriptState>| -> i32 { caller.data().max_players as i32 })?;
        linker.func_wrap("env", "get_current_players",
            |caller: Caller<'_, ScriptState>| -> i32 {
                use std::sync::atomic::Ordering;
                caller.data().player_count.load(Ordering::Relaxed) as i32
            })?;
        linker.func_wrap("env", "timestamp", || -> i64 { now_ms() })?;

        // ── Logging ──
        linker.func_wrap("env", "host_log",
            |mut caller: Caller<'_, ScriptState>, level: i32, ptr: i32, len: i32| {
                let msg = read_wasm_string(&mut caller, ptr, len);
                match level {
                    0 => tracing::error!("[script] {msg}"),
                    1 => tracing::warn!("[script] {msg}"),
                    2 => tracing::debug!("[script] {msg}"),
                    _ => tracing::info!("[script] {msg}"),
                }
            })?;
        linker.func_wrap("env", "debug_log",
            |mut caller: Caller<'_, ScriptState>, ptr: i32, len: i32| {
                let msg = read_wasm_string(&mut caller, ptr, len);
                tracing::debug!("[script] {msg}");
            })?;

        // ── Object CRUD (real — spawn/destroy/position) ──
        linker.func_wrap("env", "create_object",
            |caller: Caller<'_, ScriptState>, ref_id: i32, base_id: i32, cell: i32| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                let obj = crate::world::objects::Object::new(id, ref_id as u32, base_id as u32, cell as u32);
                state.registry.insert(obj);
                state.registry.add_to_cell(cell as u32, id);
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "destroy_object",
            |caller: Caller<'_, ScriptState>, id: i64| {
                caller.data().registry.remove(NetworkID::new(id as u64));
            })?;
        linker.func_wrap("env", "get_pos_x",
            |caller: Caller<'_, ScriptState>, id: i64| -> f32 {
                object_pos(&caller, id).map(|p| p[0]).unwrap_or(0.0)
            })?;
        linker.func_wrap("env", "get_pos_y",
            |caller: Caller<'_, ScriptState>, id: i64| -> f32 {
                object_pos(&caller, id).map(|p| p[1]).unwrap_or(0.0)
            })?;
        linker.func_wrap("env", "get_pos_z",
            |caller: Caller<'_, ScriptState>, id: i64| -> f32 {
                object_pos(&caller, id).map(|p| p[2]).unwrap_or(0.0)
            })?;
        linker.func_wrap("env", "set_pos",
            |caller: Caller<'_, ScriptState>, id: i64, x: f32, y: f32, z: f32| {
                set_object_pos(&caller, id, [x, y, z]);
            })?;

        // ── Actor (real — spawn/values/kill) ──
        linker.func_wrap("env", "create_actor",
            |caller: Caller<'_, ScriptState>, ref_id: i32, base_id: i32, cell: i32| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                let actor = crate::world::objects::Actor::new(id, ref_id as u32, base_id as u32, cell as u32);
                state.registry.insert(actor);
                state.registry.add_to_cell(cell as u32, id);
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "get_actor_value",
            |caller: Caller<'_, ScriptState>, id: i64, index: i32| -> f32 {
                let state = caller.data();
                let Some(arc) = state.registry.get(NetworkID::new(id as u64)) else { return 0.0 };
                let guard = arc.read();
                guard.as_any().downcast_ref::<crate::world::objects::Actor>()
                    .and_then(|a| a.values.get(&(index as u8)))
                    .copied()
                    .unwrap_or(0.0)
            })?;
        linker.func_wrap("env", "set_actor_value",
            |caller: Caller<'_, ScriptState>, id: i64, index: i32, value: f32| {
                let state = caller.data();
                let Some(arc) = state.registry.get(NetworkID::new(id as u64)) else { return };
                let mut guard = arc.write();
                if let Some(actor) = guard.as_any_mut().downcast_mut::<crate::world::objects::Actor>() {
                    actor.set_value(index as u8, value, false);
                }
            })?;
        linker.func_wrap("env", "kill_actor",
            |caller: Caller<'_, ScriptState>, id: i64| {
                let state = caller.data();
                let Some(arc) = state.registry.get(NetworkID::new(id as u64)) else { return };
                let mut guard = arc.write();
                if let Some(actor) = guard.as_any_mut().downcast_mut::<crate::world::objects::Actor>() {
                    actor.dead = true;
                }
            })?;

        // ── Item (create real; stack ops stubbed) ──
        linker.func_wrap("env", "create_item",
            |caller: Caller<'_, ScriptState>, ref_id: i32, base_id: i32, container: i64| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                let item = crate::world::objects::Item::new(id, ref_id as u32, base_id as u32, NetworkID::new(container as u64));
                state.registry.insert(item);
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "add_item",
            |_: Caller<'_, ScriptState>, _item: i64, _container: i64| {})?;
        linker.func_wrap("env", "remove_item",
            |_: Caller<'_, ScriptState>, _item: i64| {})?;
        linker.func_wrap("env", "equip_item",
            |_: Caller<'_, ScriptState>, _actor: i64, _item: i64| {})?;
        linker.func_wrap("env", "get_item_count",
            |_: Caller<'_, ScriptState>, _item: i64| -> i32 { 0 })?;

        // ── Chat / UI / kick (real — queued as script effects) ──
        linker.func_wrap("env", "chat_message",
            |mut caller: Caller<'_, ScriptState>, player_id: i64, ptr: i32, len: i32| {
                let message = read_wasm_string(&mut caller, ptr, len);
                if message.is_empty() {
                    return;
                }
                caller.data().effects.push(ScriptEffect::PrivateChat {
                    player_id: player_id as u64,
                    message,
                });
            })?;
        linker.func_wrap("env", "ui_message",
            |mut caller: Caller<'_, ScriptState>, player_id: i64, ptr: i32, len: i32| {
                let message = read_wasm_string(&mut caller, ptr, len);
                if message.is_empty() {
                    return;
                }
                // ponytail: UI channel not implemented yet — surface as chat.
                caller.data().effects.push(ScriptEffect::PrivateChat {
                    player_id: player_id as u64,
                    message,
                });
            })?;
        linker.func_wrap("env", "kick",
            |caller: Caller<'_, ScriptState>, player_id: i64| {
                caller.data().effects.push(ScriptEffect::Kick {
                    player_id: player_id as u64,
                });
            })?;

        // ── GUI (window widgets — stubs) ──
        linker.func_wrap("env", "create_window",
            |caller: Caller<'_, ScriptState>, _parent: i64, _label_ptr: i32, _label_len: i32| -> i64 {
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
            |caller: Caller<'_, ScriptState>, _parent: i64, _label_ptr: i32, _label_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_text",
            |caller: Caller<'_, ScriptState>, _parent: i64, _text_ptr: i32, _text_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_edit",
            |caller: Caller<'_, ScriptState>, _parent: i64, _max_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_checkbox",
            |caller: Caller<'_, ScriptState>, _parent: i64, _label_ptr: i32, _label_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_radiobutton",
            |caller: Caller<'_, ScriptState>, _parent: i64, _group: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "create_list",
            |caller: Caller<'_, ScriptState>, _parent: i64, _multiselect: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "add_list_item",
            |caller: Caller<'_, ScriptState>, _list_id: i64, _text_ptr: i32, _text_len: i32| -> i64 {
                caller.data().registry.allocate_id().as_u64() as i64
            })?;
        linker.func_wrap("env", "remove_list_item",
            |_: Caller<'_, ScriptState>, _item_id: i64| {})?;

        // ── World state (real) ──
        linker.func_wrap("env", "set_game_weather",
            |caller: Caller<'_, ScriptState>, weather: i32| {
                caller.data().weather.set(weather as u32);
            })?;
        linker.func_wrap("env", "get_game_weather",
            |caller: Caller<'_, ScriptState>| -> i32 { caller.data().weather.get() as i32 })?;
        linker.func_wrap("env", "set_game_time",
            |caller: Caller<'_, ScriptState>, year: i32, month: i32, day: i32, hour: i32| {
                caller.data().game_time.set(GameTime {
                    year: year.max(0) as u32,
                    month: month.max(0) as u32,
                    day: day.max(0) as u32,
                    hour: hour.max(0) as u32,
                });
            })?;
        linker.func_wrap("env", "set_time_scale",
            |_: Caller<'_, ScriptState>, _scale: f32| {})?;

        // ── Timers (real; callback name read from module memory) ──
        linker.func_wrap("env", "create_timer",
            |mut caller: Caller<'_, ScriptState>, interval_ms: i32, cb_ptr: i32, cb_len: i32, repeat: i32| -> i32 {
                let cb = read_wasm_string(&mut caller, cb_ptr, cb_len);
                let cb = if cb.is_empty() { "script_timer".to_string() } else { cb };
                let state = caller.data();
                let mut tm = state.timers.lock().unwrap();
                let id = tm.create_timer(interval_ms.max(1) as u64, cb, repeat != 0);
                id as i32
            })?;
        linker.func_wrap("env", "kill_timer",
            |caller: Caller<'_, ScriptState>, id: i32| {
                caller.data().timers.lock().unwrap().kill_timer(id as u32);
            })?;

        // ── Quest (real) ──
        linker.func_wrap("env", "get_quest_stage",
            |caller: Caller<'_, ScriptState>, quest_id: i32| -> i32 {
                caller.data().quests.get_stage(quest_id as u32) as i32
            })?;
        linker.func_wrap("env", "set_quest_stage",
            |caller: Caller<'_, ScriptState>, quest_id: i32, stage: i32| {
                caller.data().quests.set_stage(quest_id as u32, stage.max(0) as u16);
            })?;
        linker.func_wrap("env", "get_dialogue_flag",
            |caller: Caller<'_, ScriptState>, flag_id: i32| -> i32 {
                caller.data().quests.get_flag(flag_id as u32) as i32
            })?;
        linker.func_wrap("env", "set_dialogue_flag",
            |caller: Caller<'_, ScriptState>, flag_id: i32, value: i32| {
                caller.data().quests.set_flag(flag_id as u32, value != 0);
            })?;

        // ── Combat (stubs) ──
        linker.func_wrap("env", "get_damage_resistance",
            |_: Caller<'_, ScriptState>, _actor_hi: i32, _actor_lo: i32| -> f32 { 0.0 })?;
        linker.func_wrap("env", "get_damage_threshold",
            |_: Caller<'_, ScriptState>, _actor_hi: i32, _actor_lo: i32| -> f32 { 0.0 })?;

        // ── Utility ──
        linker.func_wrap("env", "get_config_int",
            |_: Caller<'_, ScriptState>, _key_ptr: i32, _key_len: i32| -> i32 { 0 })?;

        Ok(())
    }
}
