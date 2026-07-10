//! Wire-format packet header.

use serde::{Deserialize, Serialize};

/// Packet header sent before the payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketHeader {
    /// Total payload length (excluding this header).
    pub length: u16,
    /// Channel for ordering guarantees.
    pub channel: u8,
}

impl PacketHeader {
    pub const SIZE: usize = 3; // 2 bytes length + 1 byte channel
}
