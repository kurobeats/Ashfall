//! Global variables — key-value map synced to all clients.
//!
//! Clone shares the underlying map (Arc) — server and scripts agree.

use dashmap::DashMap;
use std::sync::Arc;

/// Server-authoritative global variable state.
pub struct GlobalState {
    globals: Arc<DashMap<u32, i32>>,
}

impl Clone for GlobalState {
    fn clone(&self) -> Self {
        GlobalState {
            globals: self.globals.clone(),
        }
    }
}

impl GlobalState {
    pub fn new() -> Self {
        GlobalState {
            globals: Arc::new(DashMap::new()),
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
