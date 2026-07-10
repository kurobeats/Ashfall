//! Client-side chat handler.

pub fn handle_incoming_chat(message: &str) -> String {
    message.to_string()
}

pub fn build_chat_packet(message: String) -> ashfall_core::protocol::Packet {
    ashfall_core::protocol::Packet::GameChat { message }
}
