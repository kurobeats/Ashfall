//! Console command interception framework.
//!
//! Hooks `ConsoleManager::ExecuteCommand` to intercept multiplayer commands.
//! Registered handlers get first chance at matching commands before the engine
//! processes them. Unmatched commands pass through to the engine.
//!
//! # Pipe opcode ranges (commands.rs)
//!
//! | Range | Tier | Purpose |
//! |-------|------|---------|
//! | 0x0001–0x0017 | Original 17 | vaultmp Interface/API basics (pos, angle, cell, actor state/value, control, activate, fire weapon, name, enabled, lock, move-to, sound, place-at-me, get-base) |
//! | 0x0018–0x001C | Tier 1 | Position + actor state sync (base actor value, dead, current health, is-moving, parent cell) |
//! | 0x001D–0x0022 | Tier 2 | Item / inventory sync (equip, unequip, add, remove, remove-all, ref count) |
//! | 0x0023–0x0026 | Tier 3 | Combat + death (kill, damage/restore/force actor value) |
//! | 0x0027–0x002A | Tier 4 | AI + world (combat target, play group, force weather, restrained) |
//! | 0xE000–0xE036 | VAULTFUNCTION | vaultmp custom opcodes, bypass engine FuncLookup |
//!
//! Engine events flow the other direction as `PIPE_OP_EVENT` frames
//! (see `events.rs` + `hooks::encode_event_frame`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Handler type: receives command arguments, returns true if command was consumed.
type ConsoleHandler = fn(args: &[&str]) -> bool;

/// Global command registry.
fn command_registry() -> &'static Mutex<HashMap<String, ConsoleHandler>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ConsoleHandler>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a console command handler.
pub fn register_command(name: &str, handler: ConsoleHandler) {
    command_registry()
        .lock()
        .unwrap()
        .insert(name.to_string(), handler);
}

/// Try to handle a console command. Returns true if consumed by a handler.
/// The raw command line is split on whitespace; first token is the command name.
pub fn try_handle(command_line: &str) -> bool {
    let parts: Vec<&str> = command_line.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }

    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];

    if let Some(handler) = command_registry().lock().unwrap().get(&cmd) {
        handler(args)
    } else {
        false
    }
}

/// Register default multiplayer console commands.
pub fn register_defaults() {
    register_command("kick", |_args| {
        // ponytail: encode as pipe command to native client
        false
    });

    register_command("players", |_| {
        // ponytail: encode as pipe command to native client
        false
    });

    register_command("ashfall_status", |_| {
        // ponytail: print bridge connection status
        false
    });
}
