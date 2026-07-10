//! Console command interception framework.
//!
//! Hooks `ConsoleManager::ExecuteCommand` to intercept multiplayer commands.
//! Registered handlers get first chance at matching commands before the engine
//! processes them. Unmatched commands pass through to the engine.

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
