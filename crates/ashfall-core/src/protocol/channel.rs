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
            // ── System channel ──
            GameStart | GameLoad | GameEnd { .. } | GameAuth { .. }
            | GameMod { .. } | GameMessage { .. } | GameWeather { .. }
            | GameGlobal { .. } | GameBase { .. } | GameDeleted { .. }
            // Quest + dialogue (reliable, ordered)
            | QuestStage { .. } | DialogueFlag { .. } | DialogueChoice { .. }
            // World globals (reliable)
            | KarmaUpdate { .. } | ReputationUpdate { .. } | HardcoreStats { .. }
            => Channel::System,

            // ── Chat channel ──
            GameChat { .. } => Channel::Chat,

            // ── Game channel (everything else) ──
            _ => Channel::Game,
        }
    }

    /// Whether a packet should use unreliable (UDP fire-and-forget) delivery.
    /// Position, velocity, and animation updates tolerate loss.
    pub fn is_unreliable(packet: &super::Packet) -> bool {
        use super::Packet::*;
        matches!(
            packet,
            UpdatePos { .. } | UpdateAngle { .. } | UpdateVelocity { .. }
            | ProjectileRemove { .. }
        )
    }
}
