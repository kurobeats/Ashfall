//! Script timer management — CreateTimer, KillTimer, tick dispatch.

use std::collections::HashMap;

/// Timer entry.
#[derive(Debug)]
pub struct TimerEntry {
    pub id: u32,
    pub interval_ms: u64,
    pub callback_name: String,
    pub next_fire: std::time::Instant,
    pub repeating: bool,
}

/// Timer manager for WASM scripts.
pub struct TimerManager {
    timers: HashMap<u32, TimerEntry>,
    pub next_id: u32,
}

impl TimerManager {
    pub fn new() -> Self {
        TimerManager {
            timers: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new timer. Returns timer ID.
    pub fn create_timer(&mut self, interval_ms: u64, callback: String, repeating: bool) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.timers.insert(id, TimerEntry {
            id,
            interval_ms,
            callback_name: callback,
            next_fire: std::time::Instant::now() + std::time::Duration::from_millis(interval_ms),
            repeating,
        });
        id
    }

    /// Shortcut: create a repeating timer with a name.
    pub fn create(&mut self, name: String, interval_ms: u64) -> u32 {
        self.create_timer(interval_ms, name, true)
    }

    /// Shortcut: kill timer by ID.
    pub fn kill(&mut self, id: u32) -> bool {
        self.kill_timer(id)
    }

    /// Kill a timer by ID.
    pub fn kill_timer(&mut self, id: u32) -> bool {
        self.timers.remove(&id).is_some()
    }

    /// Tick all timers — returns (id, callback_name) for each ready timer.
    pub fn tick(&mut self) -> Vec<(u32, String)> {
        let now = std::time::Instant::now();
        let mut ready = Vec::new();

        let mut to_remove = Vec::new();
        for timer in self.timers.values_mut() {
            if now >= timer.next_fire {
                ready.push((timer.id, timer.callback_name.clone()));
                if timer.repeating {
                    timer.next_fire = now + std::time::Duration::from_millis(timer.interval_ms);
                } else {
                    to_remove.push(timer.id);
                }
            }
        }

        for id in to_remove {
            self.timers.remove(&id);
        }

        ready
    }
}

impl Default for TimerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_tick() {
        let mut tm = TimerManager::new();
        tm.create_timer(1, "cb".into(), false);
        std::thread::sleep(std::time::Duration::from_millis(5));
        let ready = tm.tick();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].1, "cb");
    }

    #[test]
    fn test_kill_timer() {
        let mut tm = TimerManager::new();
        let id = tm.create_timer(10000, "slow".into(), false);
        assert!(tm.kill_timer(id));
        assert!(!tm.kill_timer(id)); // already gone
    }

    #[test]
    fn test_create_and_kill_shortcuts() {
        let mut tm = TimerManager::new();
        let id = tm.create("repeat".into(), 100);
        assert!(tm.kill(id));
    }
}
