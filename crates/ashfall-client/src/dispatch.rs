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
            game.chat_messages.push(("Server".into(), message.clone()));
        }
        Packet::GameWeather { weather } => {
            game.weather = *weather;
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
        _ => {}
    }
}
