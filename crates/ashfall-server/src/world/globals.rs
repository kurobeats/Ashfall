//! Global variables — key-value map synced to all clients.

use dashmap::DashMap;

/// Server-authoritative global variable state.
pub struct GlobalState {
    globals: DashMap<u32, i32>,
}

impl Clone for GlobalState {
    fn clone(&self) -> Self {
        let cloned = DashMap::new();
        for entry in &self.globals {
            cloned.insert(*entry.key(), *entry.value());
        }
        GlobalState { globals: cloned }
    }
}

impl GlobalState {
    pub fn new() -> Self {
        GlobalState {
            globals: DashMap::new(),
        }
    }

    pub fn get(&self, id: u32) -> Option<i32> {
        self.globals.get(&id).map(|v| *v.value())
    }

    pub fn set(&self, id: u32, value: i32) {
        self.globals.insert(id, value);
    }

    pub fn all(&self) -> Vec<(u32, i32)> {
        self.globals.iter().map(|e| (*e.key(), *e.value())).collect()
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}
