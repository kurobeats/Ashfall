//! wasmtime engine — loads WASM modules, manages instances, timers.

use crate::ai::factions::FactionMatrix;
use crate::quest::QuestManager;
use crate::script::host::HostFunctions;
use crate::script::timer::TimerManager;
use crate::world::globals::GlobalState;
use crate::world::registry::ObjectRegistry;
use crate::world::weather::WeatherState;
use std::path::Path;
use std::sync::{Arc, Mutex};
use wasmtime::*;

/// Server state exposed to WASM host functions.
#[derive(Clone)]
pub struct ScriptState {
    pub registry: Arc<ObjectRegistry>,
    pub weather: WeatherState,
    pub globals: GlobalState,
    pub quests: QuestManager,
    pub factions: FactionMatrix,
    pub server_name: String,
    pub server_map: String,
    pub timers: Arc<Mutex<TimerManager>>,
}

impl ScriptState {
    pub fn new(
        registry: Arc<ObjectRegistry>,
        weather: WeatherState,
        globals: GlobalState,
        quests: QuestManager,
        factions: FactionMatrix,
        server_name: String,
        server_map: String,
    ) -> Self {
        ScriptState {
            registry,
            weather,
            globals,
            quests,
            factions,
            server_name,
            server_map,
            timers: Arc::new(Mutex::new(TimerManager::new())),
        }
    }
}

/// WASM module instance wrapping a loaded script.
pub struct WasmInstance {
    _instance: Instance,
    _store: Store<ScriptState>,
    _name: String,
}

/// The scripting engine — loads WASM modules and dispatches callbacks.
pub struct ScriptEngine {
    engine: Engine,
    modules: Vec<(String, Module)>,
    instances: Vec<WasmInstance>,
    /// Shared timer manager — set after instantiate_all.
    pub timers: Option<Arc<Mutex<TimerManager>>>,
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

    /// Instantiate all loaded modules against the given state.
    pub fn instantiate_all(&mut self, state: ScriptState) -> anyhow::Result<()> {
        self.timers = Some(state.timers.clone());

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

            self.instances.push(WasmInstance {
                _instance: instance,
                _store: store,
                _name: name.clone(),
            });
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

    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}
