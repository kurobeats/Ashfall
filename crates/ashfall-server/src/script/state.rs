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
    time_scale: Arc<Mutex<f32>>,
}

impl GameTimeState {
    pub fn new(time: GameTime) -> Self {
        Self {
            time: Arc::new(Mutex::new(time)),
            // ponytail: Fallout default time scale 30 (game seconds per real second)
            time_scale: Arc::new(Mutex::new(30.0)),
        }
    }

    pub fn get(&self) -> GameTime {
        *self.time.lock()
    }

    pub fn set(&self, time: GameTime) {
        *self.time.lock() = time;
    }

    pub fn get_scale(&self) -> f32 {
        *self.time_scale.lock()
    }

    pub fn set_scale(&self, scale: f32) {
        *self.time_scale.lock() = scale.clamp(0.0, 1000.0);
    }
}

/// Side effect a WASM script queues; the server drains the queue each tick.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptEffect {
    /// Send a chat message to one player (by NetworkID).
    PrivateChat { player_id: u64, message: String },
    /// Send a chat message to every ingame player.
    BroadcastChat { message: String },
    /// Disconnect a player (by NetworkID).
    Kick { player_id: u64 },
    /// Relay an arbitrary packet to every ingame player (GUI widgets, etc.).
    BroadcastPacket(ashfall_core::protocol::Packet),
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
