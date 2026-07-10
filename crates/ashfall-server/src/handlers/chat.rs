//! Chat handler — message broadcast.

use ashfall_core::constants::MAX_CHAT_LENGTH;
use ashfall_core::protocol::Packet;

/// Handle GameChat — validate and relay.
pub fn handle_chat(message: String) -> Option<Packet> {
    if message.is_empty() || message.len() > MAX_CHAT_LENGTH {
        return None;
    }
    Some(Packet::GameChat { message })
}
