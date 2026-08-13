//! Client-side packet dispatch — apply to registry + UI events.

use ashfall_core::protocol::Packet;
use crate::game::Game;

/// Dispatch an incoming server packet on the client side.
pub fn dispatch(game: &mut Game, packet: &Packet) {
    match packet {
        Packet::GameLoad => tracing::info!("World state received, loading..."),
        Packet::GameStart => tracing::info!("Game started!"),
        Packet::GameEnd { reason } => {
            game.state = crate::game::ClientState::Disconnected;
            game.chat_messages.push(("System".into(), format!("Disconnected (reason: {reason})")));
        }
        Packet::GameChat { message } => {
            let message = message.resolve(&mut game.registry.string_table);
            game.chat_messages.push(("Server".into(), message));
        }
        Packet::GameWeather { weather } => {
            game.weather = *weather;
        }
        Packet::GameTime { year, month, day, hour, time_scale } => {
            tracing::debug!("Game time: {year}-{month:02}-{day:02} {hour:02}:00 (scale {time_scale})");
        }
        Packet::ServerSettings { pvp_enabled } => {
            game.pvp_enabled = *pvp_enabled;
        }
        Packet::SpellCast { id, spell, .. } => {
            tracing::debug!("Spell cast by {id}: form {spell:#x}");
        }
        Packet::KarmaUpdate { value } => {
            game.karma = *value;
        }
        Packet::ReputationUpdate { faction, value } => {
            game.reputation.insert(*faction, *value);
        }
        Packet::HardcoreStats { hunger, thirst, sleep } => {
            game.hardcore_hunger = *hunger;
            game.hardcore_thirst = *thirst;
            game.hardcore_sleep = *sleep;
        }
        Packet::PlayerNew { id, .. } => {
            if game.local_player_id.is_none() { game.local_player_id = Some(*id); }
        }
        // Ownership: registry tracks the sets; nothing extra to do here —
        // the bridge consults `Game::owns()` before sending actor updates.
        Packet::OwnershipGranted { .. } | Packet::OwnershipReleased { .. } => {}
        // Server-authored GUI packets → GuiState
        _ => {
            if game.gui.apply_packet(packet) {
                return;
            }
        }
    }
}
