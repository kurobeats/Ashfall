//! Authentication handler — GameAuth → GameLoad flow.

use crate::session::Session;
use ashfall_core::constants::MAX_PLAYER_NAME;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::Reason;
use std::net::SocketAddr;

/// Handle a GameAuth packet.
/// Returns (session, response_packets) or None if rejected.
pub fn handle_auth(
    addr: SocketAddr,
    name: String,
    password: String,
    version: String,
    session_guid: NetworkID,
) -> (Option<Session>, Vec<Packet>) {
    // Validate name
    if name.is_empty() || name.len() > MAX_PLAYER_NAME {
        tracing::warn!("Auth rejected: invalid name from {addr}");
        return (
            None,
            vec![Packet::GameEnd {
                reason: Reason::Denied as u8,
            }],
        );
    }

    // Version check (STR AuthenticationRequest carries Version): a client on
    // a different protocol version desyncs — reject hard.
    if version != ashfall_core::constants::DEDICATED_VERSION {
        tracing::warn!(
            "Auth rejected: version mismatch from {addr} (client {version:?}, server {:?})",
            ashfall_core::constants::DEDICATED_VERSION
        );
        return (
            None,
            vec![Packet::GameEnd {
                reason: Reason::Denied as u8,
            }],
        );
    }

    // ponytail: password validation deferred to Phase 5 (script callback)
    let _ = password;

    let session = Session::new(session_guid, addr, name);
    tracing::info!("Auth OK: {} from {addr}", session.player_name);

    let packets = vec![Packet::GameLoad];

    (Some(session), packets)
}

/// Handle a GameEnd packet (client-initiated disconnect).
pub fn handle_disconnect(session: &Session, reason: u8) -> Vec<Packet> {
    tracing::info!(
        "Player {} disconnected: reason={reason}",
        session.player_name
    );
    vec![Packet::GameEnd { reason }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guid() -> NetworkID {
        NetworkID::new(1)
    }

    #[test]
    fn test_version_mismatch_rejected() {
        let (session, packets) = handle_auth(
            "127.0.0.1:9000".parse().unwrap(),
            "Wanderer".into(),
            String::new(),
            "0.9-old-broken-build".into(),
            guid(),
        );
        assert!(session.is_none(), "wrong version rejected");
        assert_eq!(packets.len(), 1);
        assert!(
            matches!(&packets[0], Packet::GameEnd { reason } if *reason == Reason::Denied as u8)
        );
    }

    #[test]
    fn test_matching_version_authenticates() {
        let (session, packets) = handle_auth(
            "127.0.0.1:9000".parse().unwrap(),
            "Wanderer".into(),
            String::new(),
            ashfall_core::constants::DEDICATED_VERSION.into(),
            guid(),
        );
        assert!(session.is_some());
        assert!(matches!(&packets[0], Packet::GameLoad));
    }
}
