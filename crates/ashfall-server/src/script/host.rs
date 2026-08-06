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
use ashfall_core::protocol::Packet;
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

/// Mutable access to an Item by NetworkID.
fn with_item(
    caller: &Caller<'_, ScriptState>,
    id: u64,
    f: impl FnOnce(&mut crate::world::objects::Item),
) -> bool {
    let Some(arc) = caller.data().registry.get(NetworkID::new(id)) else {
        return false;
    };
    let mut guard = arc.write();
    match guard.as_any_mut().downcast_mut::<crate::world::objects::Item>() {
        Some(item) => {
            f(item);
            true
        }
        None => false,
    }
}

/// Mutable access to an Actor or Player (equip/values).
fn with_actor(
    caller: &Caller<'_, ScriptState>,
    id: u64,
    f: impl FnOnce(&mut Actor),
) -> bool {
    let Some(arc) = caller.data().registry.get(NetworkID::new(id)) else {
        return false;
    };
    let mut guard = arc.write();
    if let Some(a) = guard.as_any_mut().downcast_mut::<Actor>() {
        f(a);
        return true;
    }
    if let Some(p) = guard.as_any_mut().downcast_mut::<crate::world::objects::Player>() {
        f(&mut p.actor);
        return true;
    }
    false
}

/// Read an actor value (Actor or Player).
fn actor_value(caller: &Caller<'_, ScriptState>, id: u64, index: u8) -> f32 {
    let Some(arc) = caller.data().registry.get(NetworkID::new(id)) else {
        return 0.0;
    };
    let guard = arc.read();
    if let Some(a) = guard.as_any().downcast_ref::<Actor>() {
        return a.get_value(index);
    }
    if let Some(p) = guard.as_any().downcast_ref::<crate::world::objects::Player>() {
        return p.actor.get_value(index);
    }
    0.0
}

/// Add `item` to `container`'s item list, if it is a Container/Actor/Player.
fn link_item_to_container(caller: &Caller<'_, ScriptState>, item: u64, container: u64) {
    let Some(arc) = caller.data().registry.get(NetworkID::new(container)) else {
        return;
    };
    let mut guard = arc.write();
    let items = if let Some(c) = guard.as_any_mut().downcast_mut::<crate::world::objects::Container>() {
        &mut c.items
    } else if let Some(a) = guard.as_any_mut().downcast_mut::<Actor>() {
        &mut a.container.items
    } else if let Some(p) = guard.as_any_mut().downcast_mut::<crate::world::objects::Player>() {
        &mut p.actor.container.items
    } else {
        return;
    };
    if !items.contains(&NetworkID::new(item)) {
        items.push(NetworkID::new(item));
    }
    drop(guard);
    with_item(caller, item, |i| i.container = NetworkID::new(container));
}

/// Remove `item` from its container's list and the registry.
fn detach_and_destroy_item(caller: &Caller<'_, ScriptState>, item: u64) {
    let item_id = NetworkID::new(item);
    let container_id = {
        let Some(arc) = caller.data().registry.get(item_id) else {
            return;
        };
        let guard = arc.read();
        let Some(i) = guard.as_any().downcast_ref::<crate::world::objects::Item>() else {
            return;
        };
        i.container
    };
    if let Some(arc) = caller.data().registry.get(container_id) {
        let mut guard = arc.write();
        let items = if let Some(c) = guard.as_any_mut().downcast_mut::<crate::world::objects::Container>() {
            &mut c.items
        } else if let Some(a) = guard.as_any_mut().downcast_mut::<Actor>() {
            &mut a.container.items
        } else if let Some(p) = guard.as_any_mut().downcast_mut::<crate::world::objects::Player>() {
            &mut p.actor.container.items
        } else {
            return;
        };
        items.retain(|id| *id != item_id);
    }
    caller.data().registry.remove(item_id);
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

        // ── Item (real — create/link/destroy/count) ──
        linker.func_wrap("env", "create_item",
            |caller: Caller<'_, ScriptState>, ref_id: i32, base_id: i32, container: i64| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                let item = crate::world::objects::Item::new(id, ref_id as u32, base_id as u32, NetworkID::new(container as u64));
                state.registry.insert(item);
                link_item_to_container(&caller, id.as_u64(), container as u64);
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "add_item",
            |caller: Caller<'_, ScriptState>, item: i64, container: i64| {
                link_item_to_container(&caller, item as u64, container as u64);
            })?;
        linker.func_wrap("env", "remove_item",
            |caller: Caller<'_, ScriptState>, item: i64| {
                detach_and_destroy_item(&caller, item as u64);
            })?;
        linker.func_wrap("env", "equip_item",
            |caller: Caller<'_, ScriptState>, actor: i64, item: i64| {
                // Link the item onto the actor and mark it equipped.
                with_actor(&caller, actor as u64, |_| {});
                link_item_to_container(&caller, item as u64, actor as u64);
                with_item(&caller, item as u64, |i| i.equipped = true);
            })?;
        linker.func_wrap("env", "get_item_count",
            |caller: Caller<'_, ScriptState>, item: i64| -> i32 {
                let mut count = 0u32;
                with_item(&caller, item as u64, |i| count = i.count);
                count as i32
            })?;

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

        // ── GUI (window widgets — emit real packets via the effect queue) ──
        // Note: the native client does not render window packets yet — the
        // server side (packet generation + broadcast) is complete.
        linker.func_wrap("env", "create_window",
            |mut caller: Caller<'_, ScriptState>, parent: i64, label_ptr: i32, label_len: i32| -> i64 {
                let label = read_wasm_string(&mut caller, label_ptr, label_len);
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::WindowNew {
                    id, parent: NetworkID::new(parent as u64),
                    label, pos: [0.0; 4], size: [200.0, 100.0, 0.0, 0.0],
                    locked: false, visible: true, text: String::new(),
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "destroy_window",
            |caller: Caller<'_, ScriptState>, id: i64| {
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::WindowRemove { id: NetworkID::new(id as u64) }));
            })?;
        linker.func_wrap("env", "set_window_pos",
            |caller: Caller<'_, ScriptState>, id: i64, x: f32, y: f32, ox: f32, oy: f32| {
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::UpdateWindowPos { id: NetworkID::new(id as u64), pos: [x, y, ox, oy] }));
            })?;
        linker.func_wrap("env", "set_window_size",
            |caller: Caller<'_, ScriptState>, id: i64, w: f32, h: f32| {
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::UpdateWindowSize { id: NetworkID::new(id as u64), size: [w, h, 0.0, 0.0] }));
            })?;
        linker.func_wrap("env", "set_window_visible",
            |caller: Caller<'_, ScriptState>, id: i64, visible: i32| {
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::UpdateWindowVisible { id: NetworkID::new(id as u64), visible: visible != 0 }));
            })?;
        linker.func_wrap("env", "set_window_locked",
            |caller: Caller<'_, ScriptState>, id: i64, locked: i32| {
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::UpdateWindowLocked { id: NetworkID::new(id as u64), locked: locked != 0 }));
            })?;
        linker.func_wrap("env", "set_window_text",
            |mut caller: Caller<'_, ScriptState>, id: i64, ptr: i32, len: i32| {
                let text = read_wasm_string(&mut caller, ptr, len);
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::UpdateWindowText { id: NetworkID::new(id as u64), text }));
            })?;
        linker.func_wrap("env", "create_button",
            |mut caller: Caller<'_, ScriptState>, parent: i64, label_ptr: i32, label_len: i32| -> i64 {
                let label = read_wasm_string(&mut caller, label_ptr, label_len);
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::ButtonNew {
                    id, parent: NetworkID::new(parent as u64),
                    label, pos: [0.0; 4], size: [64.0, 24.0, 0.0, 0.0],
                    locked: false, visible: true, text: String::new(),
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "create_text",
            |mut caller: Caller<'_, ScriptState>, parent: i64, text_ptr: i32, text_len: i32| -> i64 {
                let text = read_wasm_string(&mut caller, text_ptr, text_len);
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::TextNew {
                    id, parent: NetworkID::new(parent as u64),
                    label: text.clone(), pos: [0.0; 4], size: [100.0, 20.0, 0.0, 0.0],
                    locked: false, visible: true, text,
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "create_edit",
            |caller: Caller<'_, ScriptState>, parent: i64, max_len: i32| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::EditNew {
                    id, parent: NetworkID::new(parent as u64),
                    label: String::new(), pos: [0.0; 4], size: [200.0, 24.0, 0.0, 0.0],
                    locked: false, visible: true, text: String::new(),
                    max_len: max_len.max(0) as u32, validation: String::new(),
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "create_checkbox",
            |mut caller: Caller<'_, ScriptState>, parent: i64, label_ptr: i32, label_len: i32| -> i64 {
                let label = read_wasm_string(&mut caller, label_ptr, label_len);
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::CheckboxNew {
                    id, parent: NetworkID::new(parent as u64),
                    label, pos: [0.0; 4], size: [100.0, 20.0, 0.0, 0.0],
                    locked: false, visible: true, text: String::new(), selected: false,
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "create_radiobutton",
            |caller: Caller<'_, ScriptState>, parent: i64, group: i32| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::RadioButtonNew {
                    id, parent: NetworkID::new(parent as u64),
                    label: String::new(), pos: [0.0; 4], size: [100.0, 20.0, 0.0, 0.0],
                    locked: false, visible: true, text: String::new(),
                    selected: false, group: group.max(0) as u32,
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "create_list",
            |caller: Caller<'_, ScriptState>, parent: i64, multiselect: i32| -> i64 {
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::ListNew {
                    id, parent: NetworkID::new(parent as u64),
                    label: String::new(), pos: [0.0; 4], size: [200.0, 200.0, 0.0, 0.0],
                    locked: false, visible: true, text: String::new(),
                    multiselect: multiselect != 0,
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "add_list_item",
            |mut caller: Caller<'_, ScriptState>, list_id: i64, text_ptr: i32, text_len: i32| -> i64 {
                let text = read_wasm_string(&mut caller, text_ptr, text_len);
                let state = caller.data();
                let id = state.registry.allocate_id();
                state.effects.push(ScriptEffect::BroadcastPacket(Packet::ListItemNew {
                    id, container: NetworkID::new(list_id as u64), text, selected: false,
                }));
                id.as_u64() as i64
            })?;
        linker.func_wrap("env", "remove_list_item",
            |caller: Caller<'_, ScriptState>, item_id: i64| {
                caller.data().effects.push(ScriptEffect::BroadcastPacket(Packet::ListItemRemove { id: NetworkID::new(item_id as u64) }));
            })?;

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
            |caller: Caller<'_, ScriptState>, scale: f32| {
                caller.data().game_time.set_scale(scale);
            })?;

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

        // ── Combat (real — actor value DR/DT) ──
        linker.func_wrap("env", "get_damage_resistance",
            |caller: Caller<'_, ScriptState>, actor: i64| -> f32 {
                actor_value(&caller, actor as u64, 0x29).clamp(0.0, 0.85)
            })?;
        linker.func_wrap("env", "get_damage_threshold",
            |caller: Caller<'_, ScriptState>, actor: i64| -> f32 {
                actor_value(&caller, actor as u64, 0x2A).max(0.0)
            })?;

        // ── Utility ──
        linker.func_wrap("env", "get_config_int",
            |mut caller: Caller<'_, ScriptState>, key_ptr: i32, key_len: i32| -> i32 {
                let key = read_wasm_string(&mut caller, key_ptr, key_len);
                match key.as_str() {
                    "max_players" => caller.data().max_players as i32,
                    _ => 0,
                }
            })?;
        linker.func_wrap("env", "set_server_name",
            |mut caller: Caller<'_, ScriptState>, ptr: i32, len: i32| {
                let name = read_wasm_string(&mut caller, ptr, len);
                if !name.is_empty() {
                    *caller.data().server_name.lock().unwrap() = name;
                }
            })?;

        Ok(())
    }
}
