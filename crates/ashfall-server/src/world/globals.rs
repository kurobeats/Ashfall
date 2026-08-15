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
        self.globals
            .iter()
            .map(|e| (*e.key(), *e.value()))
            .collect()
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globals_default_missing() {
        let g = GlobalState::new();
        assert_eq!(g.get(1), None);
    }

    #[test]
    fn globals_set_get_snapshot() {
        let g = GlobalState::new();
        g.set(1, 100);
        g.set(2, -5);
        assert_eq!(g.get(1), Some(100));
        assert_eq!(g.get(2), Some(-5));
        let mut all = g.all();
        all.sort();
        assert_eq!(all, vec![(1, 100), (2, -5)]);
    }

    #[test]
    fn globals_overwrite() {
        let g = GlobalState::new();
        g.set(1, 1);
        g.set(1, 2);
        assert_eq!(g.get(1), Some(2));
    }

    #[test]
    fn globals_clone_shares() {
        let g = GlobalState::new();
        let g2 = g.clone();
        g.set(9, 42);
        assert_eq!(g2.get(9), Some(42));
    }
}
