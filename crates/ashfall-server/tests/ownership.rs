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
use ashfall_server::world::objects::{Actor, Container, Item, Object, Player};
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

// ═══════════════════════════════════════════════════════════════
// Combat — players can deal and take damage; attacker spoof rejected
// ═══════════════════════════════════════════════════════════════

use ashfall_server::handlers::combat::handle_actor_hit;
use ashfall_core::protocol::Packet as Pkt;

fn player_at(id: u64, health: f32, pos: [f32; 3]) -> Player {
    let mut p = Player::new(NetworkID::new(id), 0x14, 0x07, 5);
    p.actor.set_value(0x14, health, false);
    p.actor.container.object.game_pos = pos;
    p.actor.container.object.net_pos = pos;
    p
}

#[test]
fn test_pvp_damage_resolves() {
    let registry = Arc::new(ObjectRegistry::new());
    let attacker = NetworkID::new(1);
    let target = NetworkID::new(2);
    registry.insert(player_at(1, 100.0, [0.0, 0.0, 0.0]));
    registry.insert(player_at(2, 100.0, [5.0, 0.0, 0.0]));

    let sess = session(1, Some(attacker));
    let hit = Pkt::ActorHit {
        target,
        attacker,
        limb: 0,
        base_damage: 25.0,
        flags: 0,
        weapon_id: 0,
        projectile: 0,
    };
    let packets = handle_actor_hit(&registry, &sess, &hit).expect("hit resolves");
    assert!(packets.iter().any(|p| matches!(p, Pkt::ActorDamaged { target: t, final_damage, .. } if *t == target && *final_damage > 0.0)));

    // Target health reduced by 25
    let arc = registry.get(target).unwrap();
    let guard = arc.read();
    let target_player = guard.as_any().downcast_ref::<Player>().unwrap();
    assert!((target_player.actor.get_value(0x14) - 75.0).abs() < 0.01);
}

#[test]
fn test_pvp_lethal_hit_kills() {
    let registry = Arc::new(ObjectRegistry::new());
    let attacker = NetworkID::new(1);
    let target = NetworkID::new(2);
    registry.insert(player_at(1, 100.0, [0.0, 0.0, 0.0]));
    registry.insert(player_at(2, 30.0, [5.0, 0.0, 0.0]));

    let sess = session(1, Some(attacker));
    let hit = Pkt::ActorHit {
        target,
        attacker,
        limb: 0,
        base_damage: 50.0,
        flags: 0,
        weapon_id: 0,
        projectile: 0,
    };
    let packets = handle_actor_hit(&registry, &sess, &hit).expect("hit resolves");
    assert!(packets.iter().any(|p| matches!(p, Pkt::ActorDeathExt { id, .. } if *id == target)));

    let arc = registry.get(target).unwrap();
    let guard = arc.read();
    let target_player = guard.as_any().downcast_ref::<Player>().unwrap();
    assert!(target_player.actor.dead, "lethal hit marks target dead");
}

#[test]
fn test_hit_attacker_spoof_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let target = NetworkID::new(2);
    registry.insert(player_at(1, 100.0, [0.0, 0.0, 0.0]));
    registry.insert(player_at(2, 100.0, [5.0, 0.0, 0.0]));

    // Session owns player 1, but the packet claims player 3 attacked — framing.
    let sess = session(1, Some(NetworkID::new(1)));
    let hit = Pkt::ActorHit {
        target,
        attacker: NetworkID::new(3),
        limb: 0,
        base_damage: 50.0,
        flags: 0,
        weapon_id: 0,
        projectile: 0,
    };
    assert!(handle_actor_hit(&registry, &sess, &hit).is_none(), "spoofed attacker rejected");
}

// ═══════════════════════════════════════════════════════════════
// Weather/global packets must update the authoritative server state
// (previously relayed raw — server state desynced from clients)
// ═══════════════════════════════════════════════════════════════

use ashfall_server::dispatch::Dispatcher;

#[test]
fn test_client_weather_updates_authoritative_state() {
    let dispatcher = Dispatcher::new();
    let mut sess = Session::new(
        NetworkID::new(7),
        SocketAddr::from(([127, 0, 0, 1], 3007)),
        "weather-tester".into(),
    );
    sess.player_id = Some(NetworkID::new(7));

    let result = dispatcher.dispatch(&mut sess, Pkt::GameWeather { weather: 0x00012345 });
    assert!(!result.broadcasts.is_empty(), "weather change relayed");
    assert_eq!(dispatcher.weather.get(), 0x00012345, "authoritative weather updated");
}

#[test]
fn test_client_global_updates_authoritative_state() {
    let dispatcher = Dispatcher::new();
    let mut sess = Session::new(
        NetworkID::new(8),
        SocketAddr::from(([127, 0, 0, 1], 3008)),
        "global-tester".into(),
    );
    sess.player_id = Some(NetworkID::new(8));

    let result = dispatcher.dispatch(&mut sess, Pkt::GameGlobal { global: 0x100, value: 42 });
    assert!(!result.broadcasts.is_empty(), "global change relayed");
    assert_eq!(dispatcher.globals.get(0x100), Some(42), "authoritative global updated");
}

// ═══════════════════════════════════════════════════════════════
// Item ownership — only the owning player may mutate inventory state
// ═══════════════════════════════════════════════════════════════

use ashfall_server::handlers::item;

#[test]
fn test_own_item_count_accepted() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = registry.allocate_id();
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));
    let item_id = registry.allocate_id();
    registry.insert(Item::new(item_id, 0x10, 0x999, player_id));

    let sess = session(1, Some(player_id));
    let pkt = item::handle_item_count(&registry, &sess, item_id, 5, false);
    assert!(pkt.is_some(), "own item count accepted");
    let arc = registry.get(item_id).unwrap();
    let guard = arc.read();
    assert_eq!(guard.as_any().downcast_ref::<Item>().unwrap().count, 5);
}

#[test]
fn test_foreign_item_count_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_a = registry.allocate_id();
    let player_b = registry.allocate_id();
    registry.insert(Player::new(player_a, 0x14, 0x07, 5));
    registry.insert(Player::new(player_b, 0x14, 0x07, 5));
    // Item owned by player B (container = B)
    let item_id = registry.allocate_id();
    registry.insert(Item::new(item_id, 0x10, 0x999, player_b));

    // A tries to inflate B's item
    let sess_a = session(1, Some(player_a));
    let pkt = item::handle_item_count(&registry, &sess_a, item_id, 9999, false);
    assert!(pkt.is_none(), "foreign item count rejected");
    let arc = registry.get(item_id).unwrap();
    let guard = arc.read();
    assert_eq!(guard.as_any().downcast_ref::<Item>().unwrap().count, 1, "count untouched");
}

#[test]
fn test_world_container_item_rejected() {
    // Items in world containers (not owned by any player) are server-managed.
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = registry.allocate_id();
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));
    let cont_id = registry.allocate_id();
    registry.insert(Container::new(cont_id, 0x100, 0x200, 0));
    let item_id = registry.allocate_id();
    registry.insert(Item::new(item_id, 0x10, 0x999, cont_id));

    let sess = session(1, Some(player_id));
    let pkt = item::handle_item_count(&registry, &sess, item_id, 5, false);
    assert!(pkt.is_none(), "world-container item is server-managed");
}

#[test]
fn test_own_equip_accepted() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = registry.allocate_id();
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));
    let item_id = registry.allocate_id();
    registry.insert(Item::new(item_id, 0x10, 0x999, player_id));

    let sess = session(1, Some(player_id));
    let pkt = item::handle_item_equipped(&registry, &sess, item_id, true, false, false);
    assert!(pkt.is_some(), "own equip accepted");
    let arc = registry.get(item_id).unwrap();
    let guard = arc.read();
    assert!(guard.as_any().downcast_ref::<Item>().unwrap().equipped);
}
