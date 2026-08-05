//! Script-facing server state shared with WASM instances: the game clock,
//! and the drainable effect queue (chat, kick) scripts use to reach clients.

use parking_lot::Mutex;
use std::sync::Arc;

/// Game clock (server-authoritative, settable by scripts via `set_game_time`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
}

impl Default for GameTime {
    fn default() -> Self {
        GameTime { year: 2277, month: 8, day: 17, hour: 9 }
    }
}

/// Shared, cloneable game-time state. Every WASM instance sees the same clock.
#[derive(Clone, Default)]
pub struct GameTimeState {
    time: Arc<Mutex<GameTime>>,
}

impl GameTimeState {
    pub fn new(time: GameTime) -> Self {
        Self { time: Arc::new(Mutex::new(time)) }
    }

    pub fn get(&self) -> GameTime {
        *self.time.lock()
    }

    pub fn set(&self, time: GameTime) {
        *self.time.lock() = time;
    }
}

/// Side effect a WASM script queues; the server drains the queue each tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptEffect {
    /// Send a chat message to one player (by NetworkID).
    PrivateChat { player_id: u64, message: String },
    /// Send a chat message to every ingame player.
    BroadcastChat { message: String },
    /// Disconnect a player (by NetworkID).
    Kick { player_id: u64 },
}

/// Drainable effect queue shared between WASM instances and the server loop.
#[derive(Clone, Default)]
pub struct ScriptEffects {
    queue: Arc<Mutex<Vec<ScriptEffect>>>,
}

impl ScriptEffects {
    pub fn push(&self, effect: ScriptEffect) {
        self.queue.lock().push(effect);
    }

    /// Take all queued effects.
    pub fn drain(&self) -> Vec<ScriptEffect> {
        std::mem::take(&mut *self.queue.lock())
    }
}
