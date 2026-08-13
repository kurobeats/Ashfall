//! Chat handler — message broadcast.

use ashfall_core::constants::MAX_CHAT_LENGTH;
use ashfall_core::protocol::Packet;
use ashfall_core::string_cache::CachedString;

/// Handle GameChat — validate and relay. The string-cache binding happens
/// per-recipient in the server send path (`Packet::finalize_strings`).
pub fn handle_chat(message: CachedString) -> Option<Packet> {
    // Only Plain is a legit client form; Id/Inline have no meaning inbound
    // (the server is the sole id assigner).
    let CachedString::Plain(s) = message else {
        return None;
    };
    if s.is_empty() || s.len() > MAX_CHAT_LENGTH {
        return None;
    }
    Some(Packet::GameChat { message: s.into() })
}
