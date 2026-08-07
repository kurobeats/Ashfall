//! wasmtime engine — loads WASM modules, manages instances, timers,
//! and dispatches server events into script callbacks.

use crate::ai::factions::FactionMatrix;
use crate::quest::QuestManager;
use crate::script::host::HostFunctions;
use crate::script::state::{GameTime, GameTimeState, ScriptEffect, ScriptEffects};
use crate::script::timer::TimerManager;
use crate::world::globals::GlobalState;
use crate::world::registry::ObjectRegistry;
use crate::world::weather::WeatherState;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use wasmtime::*;

/// Default spawn cell when no script answers `on_player_request_game`.
pub const DEFAULT_SPAWN_CELL: u32 = 0x0001A26E; // Megaton exterior

/// Server state exposed to WASM host functions. Cloneable — every field is
/// shared state, so all instances observe the same world.
#[derive(Clone)]
pub struct ScriptState {
    pub registry: Arc<ObjectRegistry>,
    pub weather: WeatherState,
    pub globals: GlobalState,
    pub quests: QuestManager,
    pub factions: FactionMatrix,
    pub server_name: Arc<Mutex<String>>,
    pub server_map: Arc<Mutex<String>>,
    pub timers: Arc<Mutex<TimerManager>>,
    pub game_time: GameTimeState,
    pub effects: ScriptEffects,
    pub player_count: Arc<AtomicU32>,
    pub max_players: u32,
}

impl ScriptState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: Arc<ObjectRegistry>,
        weather: WeatherState,
        globals: GlobalState,
        quests: QuestManager,
        factions: FactionMatrix,
        server_name: String,
        server_map: String,
        max_players: u32,
    ) -> Self {
        ScriptState {
            registry,
            weather,
            globals,
            quests,
            factions,
            server_name: Arc::new(Mutex::new(server_name)),
            server_map: Arc::new(Mutex::new(server_map)),
            timers: Arc::new(Mutex::new(TimerManager::new())),
            game_time: GameTimeState::new(GameTime::default()),
            effects: ScriptEffects::default(),
            player_count: Arc::new(AtomicU32::new(0)),
            max_players,
        }
    }
}

/// WASM module instance wrapping a loaded script.
pub struct WasmInstance {
    instance: Instance,
    store: Store<ScriptState>,
}

impl WasmInstance {
    /// Scratch address for host→script string arguments. Modules are compiled
    /// with ≥64KB linear memory, so 1024 is safely inside; we still bounds-check.
    const STR_SCRATCH: usize = 1024;

    /// Write `s` into module memory at `offset`, growing memory if needed.
    fn write_str(&mut self, s: &str, offset: usize) -> bool {
        let mem = match self.instance.get_memory(&mut self.store, "memory") {
            Some(m) => m,
            None => return false,
        };
        let need = offset + s.len();
        let have = mem.data_size(&self.store);
        if need > have {
            let pages = (need - have + 65535) / 65536;
            if mem.grow(&mut self.store, pages as u64).is_err() {
                return false;
            }
        }
        mem.data_mut(&mut self.store)[offset..need].copy_from_slice(s.as_bytes());
        true
    }

    /// Invoke `on_client_authenticate(name_ptr, name_len, pwd_ptr, pwd_len) -> u32`.
    /// `None` when the module doesn't export the callback (→ permissive default).
    fn call_auth(&mut self, name: &str, password: &str) -> Option<u32> {
        if !self.write_str(name, Self::STR_SCRATCH) {
            return None;
        }
        let pwd_off = Self::STR_SCRATCH + name.len();
        if !self.write_str(password, pwd_off) {
            return None;
        }
        let f = self
            .instance
            .get_typed_func::<(i32, i32, i32, i32), i32>(&mut self.store, "on_client_authenticate")
            .ok()?;
        let result = f
            .call(
                &mut self.store,
                (
                    Self::STR_SCRATCH as i32,
                    name.len() as i32,
                    pwd_off as i32,
                    password.len() as i32,
                ),
            )
            .ok()?;
        Some(result as u32)
    }

    /// Invoke `on_player_chat(player_id: u64, msg_ptr, msg_len) -> u32`.
    fn call_chat(&mut self, player_id: u64, message: &str) -> Option<u32> {
        if !self.write_str(message, Self::STR_SCRATCH) {
            return None;
        }
        let f = self
            .instance
            .get_typed_func::<(i64, i32, i32), i32>(&mut self.store, "on_player_chat")
            .ok()?;
        let result = f
            .call(
                &mut self.store,
                (player_id as i64, Self::STR_SCRATCH as i32, message.len() as i32),
            )
            .ok()?;
        Some(result as u32)
    }

    /// Invoke `on_player_request_game(player_id: u64) -> u32`.
    fn call_spawn_cell(&mut self, player_id: u64) -> Option<u32> {
        let f = self
            .instance
            .get_typed_func::<(i64,), i32>(&mut self.store, "on_player_request_game")
            .ok()?;
        let result = f.call(&mut self.store, (player_id as i64,)).ok()?;
        Some(result as u32)
    }

    /// Invoke a void callback with `(i64,)` args: on_spawn, on_player_disconnect.
    fn call_notify_void(&mut self, name: &str, a: u64) {
        let f = self
            .instance
            .get_typed_func::<(i64,), ()>(&mut self.store, name)
            .ok();
        if let Some(f) = f {
            let _ = f.call(&mut self.store, (a as i64,));
        }
    }

    /// Invoke `on_actor_death(actor: i64, killer: i64, limbs: i32, cause: i32)`.
    fn call_actor_death(&mut self, actor: u64, killer: u64, limbs: u16, cause: i8) {
        let f = self
            .instance
            .get_typed_func::<(i64, i64, i32, i32), ()>(&mut self.store, "on_actor_death")
            .ok();
        if let Some(f) = f {
            let _ = f.call(
                &mut self.store,
                (actor as i64, killer as i64, limbs as i32, cause as i32),
            );
        }
    }

    /// Invoke a timer callback by exported name, e.g. `tick_cb(id: i32)`.
    fn call_timer(&mut self, id: u32, callback: &str) {
        let f = self
            .instance
            .get_typed_func::<(i32,), ()>(&mut self.store, callback)
            .ok();
        if let Some(f) = f {
            let _ = f.call(&mut self.store, (id as i32,));
        }
    }
}

/// The scripting engine — loads WASM modules and dispatches callbacks.
pub struct ScriptEngine {
    engine: Engine,
    modules: Vec<(String, Module)>,
    instances: Vec<WasmInstance>,
    /// Shared timer manager — set after instantiate_all.
    pub timers: Option<Arc<Mutex<TimerManager>>>,
    /// Shared script effect queue — drained by the server each tick.
    pub effects: Option<ScriptEffects>,
    /// Shared player counter — maintained by the server each tick.
    pub player_count: Option<Arc<AtomicU32>>,
    /// Shared game clock — read by the server to detect script time changes.
    pub game_time: Option<GameTimeState>,
}

impl ScriptEngine {
    /// Create a new scripting engine.
    pub fn new() -> anyhow::Result<Self> {
        let mut config = Config::new();
        config.wasm_multi_memory(true);
        config.wasm_memory64(false);

        let engine = Engine::new(&config)?;

        Ok(ScriptEngine {
            engine,
            modules: Vec::new(),
            instances: Vec::new(),
            timers: None,
            effects: None,
            player_count: None,
            game_time: None,
        })
    }

    /// Load all .wasm modules from a directory.
    pub fn load_modules(&mut self, dir: &Path) -> anyhow::Result<()> {
        if !dir.exists() {
            tracing::info!("Script directory {:?} not found, skipping", dir);
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "wasm") {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let bytes = std::fs::read(&path)?;
                let module = Module::from_binary(&self.engine, &bytes)?;
                tracing::info!("Loaded WASM module: {} ({})", name, path.display());
                self.modules.push((name, module));
            }
        }

        Ok(())
    }

    /// Load a single module from bytes (used by tests and tooling).
    pub fn load_module_bytes(&mut self, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let module = Module::from_binary(&self.engine, bytes)?;
        self.modules.push((name.to_string(), module));
        Ok(())
    }

    /// Instantiate all loaded modules against the given state.
    pub fn instantiate_all(&mut self, state: ScriptState) -> anyhow::Result<()> {
        self.timers = Some(state.timers.clone());
        self.effects = Some(state.effects.clone());
        self.player_count = Some(state.player_count.clone());
        self.game_time = Some(state.game_time.clone());

        for (name, module) in &self.modules {
            let mut store = Store::new(&self.engine, state.clone());

            let mut linker = Linker::new(&self.engine);

            let host = HostFunctions;
            host.define_in_linker(&mut linker)?;

            let instance = linker.instantiate(&mut store, module)?;

            let on_init = instance
                .get_typed_func::<(), ()>(&mut store, "on_server_init")
                .or_else(|_| instance.get_typed_func::<(), ()>(&mut store, "OnServerInit"));
            if let Ok(func) = on_init {
                tracing::info!("Calling OnServerInit for module {}", name);
                if let Err(e) = func.call(&mut store, ()) {
                    tracing::warn!("OnServerInit error in {}: {e}", name);
                }
            }

            self.instances.push(WasmInstance { instance, store });
        }

        Ok(())
    }

    /// Tick timers. Called from main server loop.
    pub fn tick_timers(&self) -> Vec<(u32, String)> {
        match &self.timers {
            Some(tm) => tm.lock().unwrap().tick(),
            None => Vec::new(),
        }
    }

    /// Dispatch a fired timer to every instance exporting the callback name.
    pub fn dispatch_timer(&mut self, id: u32, callback: &str) {
        for inst in &mut self.instances {
            inst.call_timer(id, callback);
        }
    }

    /// Drain script-queued side effects (chat, kick).
    pub fn drain_effects(&self) -> Vec<ScriptEffect> {
        match &self.effects {
            Some(effects) => effects.drain(),
            None => Vec::new(),
        }
    }

    /// Authenticate a connecting player. Every module gets a vote; any `0`
    /// (deny) rejects the connection. No modules → allow.
    pub fn dispatch_auth(&mut self, name: &str, password: &str) -> bool {
        if self.instances.is_empty() {
            return true;
        }
        for inst in &mut self.instances {
            if let Some(result) = inst.call_auth(name, password) {
                if result == 0 {
                    return false;
                }
            }
        }
        // No denial from any module (or no module exported the callback).
        true
    }

    /// Ask scripts whether a chat message may be relayed. Any `0` blocks it.
    pub fn dispatch_chat(&mut self, player_id: u64, message: &str) -> bool {
        if self.instances.is_empty() {
            return true;
        }
        for inst in &mut self.instances {
            if let Some(result) = inst.call_chat(player_id, message) {
                if result == 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Ask scripts for a spawn cell. First module answering wins; default
    /// (Megaton exterior) when no module exports the callback.
    pub fn dispatch_spawn_cell(&mut self, player_id: u64) -> u32 {
        for inst in &mut self.instances {
            if let Some(cell) = inst.call_spawn_cell(player_id) {
                return cell;
            }
        }
        DEFAULT_SPAWN_CELL
    }

    /// Notify scripts a player spawned.
    pub fn notify_spawn(&mut self, player_id: u64) {
        for inst in &mut self.instances {
            inst.call_notify_void("on_spawn", player_id);
        }
    }

    /// Notify scripts a player disconnected.
    pub fn notify_disconnect(&mut self, player_id: u64, reason: u8) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, "on_player_disconnect")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (player_id as i64, reason as i32));
            }
        }
    }

    /// Notify scripts an actor died.
    pub fn notify_actor_death(&mut self, actor: u64, killer: u64, limbs: u16, cause: i8) {
        for inst in &mut self.instances {
            inst.call_actor_death(actor, killer, limbs, cause);
        }
    }

    /// Notify scripts a quest stage changed.
    pub fn notify_quest_stage(&mut self, quest_id: u32, stage: u16) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i32, i32), ()>(&mut inst.store, "on_quest_stage")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (quest_id as i32, stage as i32));
            }
        }
    }

    /// Ask scripts whether a combat hit may resolve. Any module returning 0
    /// blocks the hit (on_hit: target, attacker, limb, damage).
    pub fn dispatch_hit(&mut self, target: u64, attacker: u64, limb: u8, damage: f32) -> bool {
        if self.instances.is_empty() {
            return true;
        }
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i64, i32, f32), i32>(&mut inst.store, "on_hit")
                .ok();
            if let Some(f) = f {
                let r = f
                    .call(&mut inst.store, (target as i64, attacker as i64, limb as i32, damage))
                    .unwrap_or(1);
                if r == 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Notify scripts an item was equipped/unequipped (on_equip: actor, item, equipped).
    pub fn notify_equip(&mut self, actor: u64, item: u64, equipped: bool) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i64, i32), ()>(&mut inst.store, "on_equip")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (actor as i64, item as i64, equipped as i32));
            }
        }
    }

    /// Notify scripts an item's count changed (on_item_count_change: item, count).
    pub fn notify_item_count(&mut self, item: u64, count: u32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, "on_item_count_change")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (item as i64, count as i32));
            }
        }
    }

    /// Notify scripts an object was activated (on_activate: ref_id, actor).
    pub fn notify_activate(&mut self, ref_id: u32, actor: u64) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i32, i64), ()>(&mut inst.store, "on_activate")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (ref_id as i32, actor as i64));
            }
        }
    }

    /// Notify scripts an object changed cells (on_cell_change: object, cell).
    pub fn notify_cell_change(&mut self, object: u64, cell: u32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, "on_cell_change")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (object as i64, cell as i32));
            }
        }
    }

    /// Notify scripts an object was created (on_create: object).
    pub fn notify_create(&mut self, object: u64) {
        for inst in &mut self.instances {
            inst.call_notify_void("on_create", object);
        }
    }

    /// Notify scripts an object was destroyed (on_destroy: object).
    pub fn notify_destroy(&mut self, object: u64) {
        for inst in &mut self.instances {
            inst.call_notify_void("on_destroy", object);
        }
    }

    /// Notify scripts a GUI window was clicked (on_window_click: player, window).
    pub fn notify_window_click(&mut self, player: u64, window: u64) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i64), ()>(&mut inst.store, "on_window_click")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (player as i64, window as i64));
            }
        }
    }

    /// Notify scripts a GUI window returned (on_window_return: player, window).
    pub fn notify_window_return(&mut self, player: u64, window: u64) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i64), ()>(&mut inst.store, "on_window_return")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (player as i64, window as i64));
            }
        }
    }

    /// Notify scripts the game clock changed.
    pub fn notify_game_time(&mut self, time: GameTime) {
        let f = |inst: &mut WasmInstance| {
            let func = inst
                .instance
                .get_typed_func::<(i32, i32, i32, i32), ()>(&mut inst.store, "on_game_time_change")
                .ok();
            if let Some(func) = func {
                let _ = func.call(
                    &mut inst.store,
                    (time.year as i32, time.month as i32, time.day as i32, time.hour as i32),
                );
            }
        };
        for inst in &mut self.instances {
            f(inst);
        }
    }

    /// Current live player count (set by the server each tick).
    pub fn live_player_count(&self) -> u32 {
        self.player_count
            .as_ref()
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Call an exported void function with raw values (test/tooling helper).
    /// Returns true when any instance's export ran without error.
    pub fn call_export_void(&mut self, name: &str, args: &[Val]) -> bool {
        for inst in &mut self.instances {
            if let Some(f) = inst.instance.get_func(&mut inst.store, name) {
                return f.call(&mut inst.store, args, &mut []).is_ok();
            }
        }
        false
    }

    /// Call an exported `(params) -> i32` function (test/tooling helper).
    pub fn call_export_i32(&mut self, name: &str, args: &[Val]) -> Option<i32> {
        for inst in &mut self.instances {
            if let Some(f) = inst.instance.get_func(&mut inst.store, name) {
                let mut results = [Val::I32(0)];
                if f.call(&mut inst.store, args, &mut results).is_ok() {
                    return results[0].i32();
                }
            }
        }
        None
    }

    /// Call an exported `(params) -> i64` function (test/tooling helper).
    pub fn call_export_i64(&mut self, name: &str, args: &[Val]) -> Option<i64> {
        for inst in &mut self.instances {
            if let Some(f) = inst.instance.get_func(&mut inst.store, name) {
                let mut results = [Val::I64(0)];
                if f.call(&mut inst.store, args, &mut results).is_ok() {
                    return results[0].i64();
                }
            }
        }
        None
    }

    /// Call an exported `(params) -> f32` function (test/tooling helper).
    pub fn call_export_f32(&mut self, name: &str, args: &[Val]) -> Option<f32> {
        for inst in &mut self.instances {
            if let Some(f) = inst.instance.get_func(&mut inst.store, name) {
                let mut results = [Val::F32(0.0_f32.to_bits())];
                if f.call(&mut inst.store, args, &mut results).is_ok() {
                    return results[0].f32();
                }
            }
        }
        None
    }

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}

impl Default for ScriptEngine {
    fn default() -> Self {
        ScriptEngine::new().expect("wasmtime engine init")
    }
}

// ═══════════════════════════════════════════════════════════════
// Remaining callback notifications (GUI selects, actor sub-events,
// dialogue, lock) — freeroam-declared, wired 2026-08-06.
// ═══════════════════════════════════════════════════════════════

impl ScriptEngine {
    /// on_window_text_change(player, window, text_ptr, text_len)
    pub fn notify_window_text(&mut self, player: u64, window: u64, text: &str) {
        for inst in &mut self.instances {
            if !inst.write_str(text, WasmInstance::STR_SCRATCH) {
                continue;
            }
            let f = inst
                .instance
                .get_typed_func::<(i64, i64, i32, i32), ()>(&mut inst.store, "on_window_text_change")
                .ok();
            if let Some(f) = f {
                let _ = f.call(
                    &mut inst.store,
                    (
                        player as i64,
                        window as i64,
                        WasmInstance::STR_SCRATCH as i32,
                        text.len() as i32,
                    ),
                );
            }
        }
    }

    /// on_checkbox_select(player, checkbox, selected) — bool is i32 in wasm.
    pub fn notify_checkbox(&mut self, player: u64, checkbox: u64, selected: bool) {
        self.notify_2i64_1i32("on_checkbox_select", player, checkbox, selected as u32);
    }

    /// on_radio_button_select(player, radio, prev)
    pub fn notify_radio(&mut self, player: u64, radio: u64, prev: u64) {
        self.notify_3i64("on_radio_button_select", player, radio, prev);
    }

    /// on_list_item_select(player, item, selected)
    pub fn notify_list_item(&mut self, player: u64, item: u64, selected: bool) {
        self.notify_2i64_1i32("on_list_item_select", player, item, selected as u32);
    }

    /// Notify scripts an actor swung unarmed (on_actor_punch: actor, power).
    pub fn notify_punch(&mut self, actor: u64, power: bool) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, "on_actor_punch")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (actor as i64, power as i32));
            }
        }
    }

    /// Notify scripts an actor fired a weapon (on_actor_fire_weapon: actor, weapon).
    pub fn notify_fire_weapon(&mut self, actor: u64, weapon: u32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, "on_actor_fire_weapon")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (actor as i64, weapon as i32));
            }
        }
    }

    /// on_item_condition_change(item, condition)
    pub fn notify_item_condition(&mut self, item: u64, condition: f32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, f32), ()>(&mut inst.store, "on_item_condition_change")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (item as i64, condition));
            }
        }
    }

    /// on_item_equipped_change(item, equipped)
    pub fn notify_item_equipped_change(&mut self, item: u64, equipped: bool) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, "on_item_equipped_change")
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (item as i64, equipped as i32));
            }
        }
    }

    /// on_actor_alert(actor, alerted)
    pub fn notify_actor_alert(&mut self, actor: u64, alerted: bool) {
        self.notify_i64_1i32("on_actor_alert", actor, alerted as u32);
    }

    /// on_actor_sneak(actor, sneaking)
    pub fn notify_actor_sneak(&mut self, actor: u64, sneaking: bool) {
        self.notify_i64_1i32("on_actor_sneak", actor, sneaking as u32);
    }

    /// on_actor_value_change / on_actor_base_value_change(actor, index, value)
    pub fn notify_actor_value(&mut self, actor: u64, index: u8, value: f32, base: bool) {
        let name = if base { "on_actor_base_value_change" } else { "on_actor_value_change" };
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32, f32), ()>(&mut inst.store, name)
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (actor as i64, index as i32, value));
            }
        }
    }

    /// on_window_mode(player, enabled)
    pub fn notify_window_mode(&mut self, player: u64, enabled: bool) {
        self.notify_i64_1i32("on_window_mode", player, enabled as u32);
    }

    /// on_dialogue_choice(player, flag_id, choice)
    pub fn notify_dialogue_choice(&mut self, player: u64, flag_id: u32, choice: u32) {
        self.notify_i64_2i32("on_dialogue_choice", player, flag_id, choice);
    }

    /// on_lock_change(object, actor, lock)
    pub fn notify_lock_change(&mut self, object: u64, actor: u64, lock: u32) {
        self.notify_2i64_1i32("on_lock_change", object, actor, lock);
    }

    /// Shared: (i64, i32) -> () callback (bool/u32 args).
    fn notify_i64_1i32(&mut self, name: &str, a: u64, b: u32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32), ()>(&mut inst.store, name)
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (a as i64, b as i32));
            }
        }
    }

    /// Shared: (i64, i32, i32) -> () callback.
    fn notify_i64_2i32(&mut self, name: &str, a: u64, b: u32, c: u32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i32, i32), ()>(&mut inst.store, name)
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (a as i64, b as i32, c as i32));
            }
        }
    }

    /// Shared: (i64, i64, i32) -> () callback (bool/u32 third arg).
    fn notify_2i64_1i32(&mut self, name: &str, a: u64, b: u64, c: u32) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i64, i32), ()>(&mut inst.store, name)
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (a as i64, b as i64, c as i32));
            }
        }
    }

    /// Shared: (i64, i64, i64) -> () callback.
    fn notify_3i64(&mut self, name: &str, a: u64, b: u64, c: u64) {
        for inst in &mut self.instances {
            let f = inst
                .instance
                .get_typed_func::<(i64, i64, i64), ()>(&mut inst.store, name)
                .ok();
            if let Some(f) = f {
                let _ = f.call(&mut inst.store, (a as i64, b as i64, c as i64));
            }
        }
    }
}
