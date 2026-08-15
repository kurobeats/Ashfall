//! Client-side packet dispatch — apply to registry + UI events.

use crate::game::Game;
use ashfall_core::protocol::Packet;

/// Dispatch an incoming server packet on the client side.
pub fn dispatch(game: &mut Game, packet: &Packet) {
    match packet {
        Packet::GameLoad => tracing::info!("World state received, loading..."),
        Packet::GameStart => tracing::info!("Game started!"),
        Packet::GameEnd { reason } => {
            game.state = crate::game::ClientState::Disconnected;
            game.chat_messages
                .push(("System".into(), format!("Disconnected (reason: {reason})")));
        }
        Packet::GameChat { message } => {
            let message = message.resolve(&mut game.registry.string_table);
            game.chat_messages.push(("Server".into(), message));
        }
        Packet::GameWeather { weather } => {
            game.weather = *weather;
        }
        Packet::GameTime {
            year,
            month,
            day,
            hour,
            time_scale,
        } => {
            game.game_time = Some(crate::game::GameClock {
                year: *year,
                month: *month,
                day: *day,
                hour: *hour,
                time_scale: *time_scale,
            });
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
        Packet::HardcoreStats {
            hunger,
            thirst,
            sleep,
        } => {
            game.hardcore_hunger = *hunger;
            game.hardcore_thirst = *thirst;
            game.hardcore_sleep = *sleep;
        }
        Packet::PlayerNew { id, .. } => {
            if game.local_player_id.is_none() {
                game.local_player_id = Some(*id);
            }
        }
        // Ownership: registry tracks the sets; nothing extra to do here —
        // the bridge consults `Game::owns()` before sending actor updates.
        Packet::OwnershipGranted { .. } | Packet::OwnershipReleased { .. } => {}
        // Server-authored GUI packets → GuiState
        _ => {
            game.gui.apply_packet(packet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClientConfig;
    use crate::game::Game;

    fn game() -> Game {
        Game::new(ClientConfig::default())
    }

    #[test]
    fn test_game_time_stored_from_packet() {
        let mut g = game();
        dispatch(
            &mut g,
            &Packet::GameTime {
                year: 2277,
                month: 8,
                day: 17,
                hour: 9,
                time_scale: 30.0,
            },
        );
        let t = g.game_time.expect("clock stored");
        assert_eq!((t.year, t.month, t.day, t.hour), (2277, 8, 17, 9));
        assert_eq!(t.time_scale, 30.0);
    }

    #[test]
    fn test_server_settings_stored() {
        let mut g = game();
        assert!(!g.pvp_enabled);
        dispatch(&mut g, &Packet::ServerSettings { pvp_enabled: true });
        assert!(g.pvp_enabled);
    }
}
