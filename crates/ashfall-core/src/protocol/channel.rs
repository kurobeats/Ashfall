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
            UpdatePos { .. } | UpdateAngle { .. } | UpdateVelocity { .. } | ProjectileRemove { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Packet;

    #[test]
    fn packet_channel_mapping() {
        assert_eq!(Channel::from_packet(&Packet::GameStart), Channel::System);
        assert_eq!(
            Channel::from_packet(&Packet::GameChat {
                message: "hi".into()
            }),
            Channel::Chat
        );
        assert_eq!(
            Channel::from_packet(&Packet::UpdatePos {
                id: crate::id::NetworkID::new(1),
                pos: [0.0; 3]
            }),
            Channel::Game
        );
        assert_eq!(
            Channel::from_packet(&Packet::QuestStage {
                quest_id: 1,
                stage: 2
            }),
            Channel::System
        );
    }

    #[test]
    fn unreliable_delivery_set() {
        let pos = Packet::UpdatePos {
            id: crate::id::NetworkID::new(1),
            pos: [0.0; 3],
        };
        let ang = Packet::UpdateAngle {
            id: crate::id::NetworkID::new(1),
            angle: [0.0; 2],
        };
        let vel = Packet::UpdateVelocity {
            id: crate::id::NetworkID::new(1),
            vel: [0.0; 3],
            on_ground: true,
        };
        assert!(Channel::is_unreliable(&pos));
        assert!(Channel::is_unreliable(&ang));
        assert!(Channel::is_unreliable(&vel));
        assert!(!Channel::is_unreliable(&Packet::GameStart));
        assert!(!Channel::is_unreliable(&Packet::GameChat {
            message: "hi".into()
        }));
    }
}
