//! Handler ownership + player-sync tests.
//!
//! Regression coverage for two systemic gaps fixed 2026-08-06:
//!  1. Position/velocity/angle/state handlers only downcast to `Object`, so
//!     player updates (Player wraps Actor) were never validated or stored —
//!     yet the packet still broadcast (anti-cheat bypass + stale authority).
//!  2. No ownership check: any client could mutate any object/actor/player.

use ashfall_core::id::NetworkID;
use ashfall_server::handlers::{actor, object, physics};
use ashfall_server::session::Session;
use ashfall_server::world::objects::{Actor, Object, Player};
use ashfall_server::world::registry::ObjectRegistry;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

fn session(guid: u64, player_id: Option<NetworkID>) -> Session {
    let mut s = Session::new(
        NetworkID::new(guid),
        SocketAddr::from(([127, 0, 0, 1], 2000 + guid as u16)),
        format!("tester-{guid}"),
    );
    s.player_id = player_id;
    // Give the anti-cheat a realistic dt so small moves aren't speed-hacks.
    std::thread::sleep(Duration::from_millis(150));
    s
}

fn pos_of(registry: &Arc<ObjectRegistry>, id: NetworkID) -> [f32; 3] {
    let arc = registry.get(id).expect("entity exists");
    let guard = arc.read();
    if let Some(o) = guard.as_any().downcast_ref::<Object>() {
        o.net_pos
    } else if let Some(a) = guard.as_any().downcast_ref::<Actor>() {
        a.container.object.net_pos
    } else if let Some(p) = guard.as_any().downcast_ref::<Player>() {
        p.actor.container.object.net_pos
    } else {
        panic!("not position-bearing");
    }
}

// ═══════════════════════════════════════════════════════════════
// Player position sync — must validate, store, and broadcast
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_player_pos_update_validated_and_stored() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = NetworkID::new(1);
    let player = Player::new(player_id, 0x14, 0x07, 5);
    registry.insert(player);

    let sess = session(1, Some(player_id));
    let pkt = object::handle_update_pos(&registry, &sess, player_id, [1.0, 0.0, 0.0]);
    assert!(pkt.is_some(), "own player position update accepted");
    assert_eq!(pos_of(&registry, player_id), [1.0, 0.0, 0.0], "authoritative position stored");
}

#[test]
fn test_player_velocity_stored() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = NetworkID::new(1);
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));

    let sess = session(1, Some(player_id));
    let pkt = physics::handle_update_velocity(&registry, &sess, player_id, [3.0, 0.0, 0.0], true);
    assert!(pkt.is_some());
    let arc = registry.get(player_id).unwrap();
    let guard = arc.read();
    let player = guard.as_any().downcast_ref::<Player>().unwrap();
    assert_eq!(player.actor.container.object.velocity, [3.0, 0.0, 0.0]);
}

#[test]
fn test_player_angle_stored() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = NetworkID::new(1);
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));

    let sess = session(1, Some(player_id));
    let pkt = object::handle_update_angle(&registry, &sess, player_id, [1.5, -2.0]);
    assert!(pkt.is_some());
    let arc = registry.get(player_id).unwrap();
    let guard = arc.read();
    let player = guard.as_any().downcast_ref::<Player>().unwrap();
    assert_eq!(player.actor.container.object.angle, [1.5, 0.0, -2.0]);
}

// ═══════════════════════════════════════════════════════════════
// Ownership — clients may only mutate their own player
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_non_owner_pos_update_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let victim = NetworkID::new(1);
    registry.insert(Player::new(victim, 0x14, 0x07, 5));

    // Attacker session owns a different player
    let attacker = session(99, Some(NetworkID::new(2)));
    let pkt = object::handle_update_pos(&registry, &attacker, victim, [100.0, 0.0, 0.0]);
    assert!(pkt.is_none(), "non-owner position update rejected");
    assert_eq!(pos_of(&registry, victim), [0.0, 0.0, 0.0], "victim position untouched");
}

#[test]
fn test_world_object_update_rejected() {
    // World objects are server-authoritative — no client may move them.
    let registry = Arc::new(ObjectRegistry::new());
    let obj_id = NetworkID::new(50);
    registry.insert(Object::new(obj_id, 0x100, 0x200, 5));

    let sess = session(1, Some(NetworkID::new(1)));
    let pkt = object::handle_update_pos(&registry, &sess, obj_id, [5.0, 0.0, 0.0]);
    assert!(pkt.is_none(), "client cannot move a world object");
}

#[test]
fn test_non_owner_actor_state_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let victim = NetworkID::new(1);
    registry.insert(Player::new(victim, 0x14, 0x07, 5));

    let attacker = session(99, Some(NetworkID::new(2)));
    let pkt = actor::handle_actor_state(&registry, &attacker, victim, 0, 1, 2, 3, true, false, false);
    assert!(pkt.is_none(), "non-owner actor state rejected");
    assert_eq!(pkt, None);
}

#[test]
fn test_owner_actor_value_accepted() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = NetworkID::new(1);
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));

    let sess = session(1, Some(player_id));
    let pkt = actor::handle_actor_value(&registry, &sess, player_id, false, 0x14, 80.0);
    assert!(pkt.is_some(), "own actor value accepted");
    let arc = registry.get(player_id).unwrap();
    let guard = arc.read();
    let player = guard.as_any().downcast_ref::<Player>().unwrap();
    assert_eq!(player.actor.values.get(&0x14), Some(&80.0));
}

// ═══════════════════════════════════════════════════════════════
// Anti-cheat still enforced on the player path
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_player_teleport_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = NetworkID::new(1);
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));

    let sess = session(1, Some(player_id));
    // 20000 units in <1s — a teleport
    let pkt = object::handle_update_pos(&registry, &sess, player_id, [20000.0, 0.0, 0.0]);
    assert!(pkt.is_none(), "teleport rejected");
    assert_eq!(pos_of(&registry, player_id), [0.0, 0.0, 0.0], "position not updated");
}

#[test]
fn test_player_invalid_pos_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = NetworkID::new(1);
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));

    let sess = session(1, Some(player_id));
    let pkt = object::handle_update_pos(&registry, &sess, player_id, [f32::NAN, 0.0, 0.0]);
    assert!(pkt.is_none(), "NaN position rejected");
}
