//! GUI handler — server-authoritative window/widget events.
//!
//! ponytail: stubs — full GUI implementation in Phase 6.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::Packet;

/// Handle WindowNew — create GUI window.
pub fn handle_window_new(packet: &Packet) -> Option<Packet> {
    Some(packet.clone())
}

/// Handle WindowRemove.
pub fn handle_window_remove(id: NetworkID) -> Packet {
    Packet::WindowRemove { id }
}

/// Handle UpdateWindowMode — toggle full GUI mode.
pub fn handle_window_mode(enabled: bool) -> Packet {
    Packet::UpdateWindowMode { enabled }
}

/// Handle window click event.
pub fn handle_window_click(id: NetworkID) -> Packet {
    Packet::UpdateWindowClick { id }
}

/// Handle window return event (edit enter).
pub fn handle_window_return(id: NetworkID) -> Packet {
    Packet::UpdateWindowReturn { id }
}
