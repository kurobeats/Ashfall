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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_message_relayed() {
        let msg = CachedString::Plain("hello world".into());
        match handle_chat(msg) {
            Some(Packet::GameChat { message }) => assert_eq!(message, "hello world".into()),
            other => panic!("expected relay, got {other:?}"),
        }
    }

    #[test]
    fn id_form_rejected() {
        // the server is the sole id assigner — Id/Inline forms are not
        // legitimate inbound
        assert!(handle_chat(CachedString::Id(5)).is_none());
    }

    #[test]
    fn empty_rejected() {
        assert!(handle_chat(CachedString::Plain(String::new())).is_none());
    }

    #[test]
    fn overlong_rejected() {
        let long = "x".repeat(MAX_CHAT_LENGTH as usize + 1);
        assert!(handle_chat(CachedString::Plain(long)).is_none());
        // at the limit is fine
        let at_limit = "x".repeat(MAX_CHAT_LENGTH as usize);
        assert!(handle_chat(CachedString::Plain(at_limit)).is_some());
    }
}
