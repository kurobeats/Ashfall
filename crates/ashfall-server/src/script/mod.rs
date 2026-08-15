//! WASM scripting engine — wasmtime integration, host functions, callbacks, timers.

pub mod callbacks;
pub mod engine;
pub mod host;
pub mod state;
pub mod timer;

pub use callbacks::CallbackDispatcher;
pub use engine::ScriptEngine;
pub use timer::TimerManager;
