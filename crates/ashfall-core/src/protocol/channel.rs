//! Packet channel enumeration — matches RakNet channel semantics.

/// Communication channel for packet ordering guarantees.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    /// Authentication, state setup, disconnect.
    System = 0,
    /// Object/actor/item/position sync.
    Game = 1,
    /// Chat messages.
    Chat = 2,
}

impl Channel {
    /// Map a Packet variant to its channel.
    pub fn from_packet(packet: &super::Packet) -> Self {
        use super::Packet::*;
        match packet {
            // System channel
            GameStart | GameLoad | GameEnd { .. } | GameAuth { .. }
            | GameMod { .. } | GameMessage { .. } | GameWeather { .. }
            | GameGlobal { .. } | GameBase { .. } | GameDeleted { .. } => Channel::System,

            // Chat channel
            GameChat { .. } => Channel::Chat,

            // Everything else → Game channel
            _ => Channel::Game,
        }
    }
}
