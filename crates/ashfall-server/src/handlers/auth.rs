//! Authentication handler — GameAuth → GameLoad flow.

use ashfall_core::constants::MAX_PLAYER_NAME;
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;
use ashfall_core::types::Reason;
use crate::session::Session;
use std::net::SocketAddr;

/// Handle a GameAuth packet.
/// Returns (session, response_packets) or None if rejected.
pub fn handle_auth(
    addr: SocketAddr,
    name: String,
    password: String,
    session_guid: NetworkID,
) -> (Option<Session>, Vec<Packet>) {
    // Validate name
    if name.is_empty() || name.len() > MAX_PLAYER_NAME {
        tracing::warn!("Auth rejected: invalid name from {addr}");
        return (None, vec![Packet::GameEnd { reason: Reason::Denied as u8 }]);
    }

    // ponytail: password validation deferred to Phase 5 (script callback)
    let _ = password;

    let session = Session::new(session_guid, addr, name);
    tracing::info!("Auth OK: {} from {addr}", session.player_name);

    let packets = vec![
        Packet::GameLoad,
    ];

    (Some(session), packets)
}

/// Handle a GameEnd packet (client-initiated disconnect).
pub fn handle_disconnect(session: &Session, reason: u8) -> Vec<Packet> {
    tracing::info!("Player {} disconnected: reason={reason}", session.player_name);
    vec![Packet::GameEnd { reason }]
}
