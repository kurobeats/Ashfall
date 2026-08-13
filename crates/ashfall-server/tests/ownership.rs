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
    let packets = handle_actor_hit(&registry, &sess, &hit, true).expect("hit resolves");
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
    let packets = handle_actor_hit(&registry, &sess, &hit, true).expect("hit resolves");
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
    assert!(handle_actor_hit(&registry, &sess, &hit, true).is_none(), "spoofed attacker rejected");
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


// ═══════════════════════════════════════════════════════════════
// Lag compensation — combat range checks use the attacker's position
// ~1 RTT before processing, not its (ahead) current position
// ═══════════════════════════════════════════════════════════════

use ashfall_server::combat::resolver::CombatResolver;
use ashfall_server::world::position_history::{PositionHistory, LAG_COMP_REWIND};
use std::time::Duration as StdDuration;

fn player_nid(id: NetworkID, health: f32, pos: [f32; 3]) -> Player {
    let mut p = Player::new(id, 0x14, 0x07, 5);
    p.actor.set_value(0x14, health, false);
    p.actor.container.object.game_pos = pos;
    p.actor.container.object.net_pos = pos;
    p
}

#[test]
fn test_combat_lag_compensation_uses_old_position() {
    let registry = Arc::new(ObjectRegistry::new());
    let attacker = registry.allocate_id();
    let target = registry.allocate_id();
    // Attacker fired from 6000 units out (out of range) while moving toward
    // the target — by processing time it is at 4000 (in range).
    registry.insert(player_nid(attacker, 100.0, [4000.0, 0.0, 0.0]));
    registry.insert(player_nid(target, 100.0, [0.0, 0.0, 0.0]));

    let history = PositionHistory::new();
    // The attacker's position when it fired: 6000 units away (out of range)
    history.record(attacker, [6000.0, 0.0, 0.0]);
    std::thread::sleep(StdDuration::from_millis(LAG_COMP_REWIND.as_millis() as u64 + 20));

    let hit = Pkt::ActorHit {
        target,
        attacker,
        limb: 0,
        base_damage: 25.0,
        flags: 0,
        weapon_id: 0,
        projectile: 0,
    };

    // Without compensation: current position (4000) is in range → accepted.
    assert!(
        CombatResolver::resolve_hit(&registry, &hit).is_some(),
        "current-position check accepts (attacker now in range)"
    );
    // With compensation: the 6000-unit fire position is used → rejected.
    assert!(
        CombatResolver::resolve_hit_compensated(&registry, &hit, &history).is_none(),
        "lag-compensated check uses the attacker's fire-time position"
    );
}

// ═══════════════════════════════════════════════════════════════
// Unarmed punch — only the actor's own client may report it, and
// ItemNew is server-authoritative (clients cannot mint items)
// ═══════════════════════════════════════════════════════════════

use ashfall_server::handlers::combat::handle_actor_punch;

#[test]
fn test_own_punch_accepted() {
    let sess = session(4, Some(NetworkID::new(4)));
    let pkt = handle_actor_punch(&sess, NetworkID::new(4), true);
    match pkt {
        Some(ashfall_core::protocol::Packet::ActorPunch { id, power }) => {
            assert_eq!(id, NetworkID::new(4));
            assert!(power, "power flag relayed");
        }
        other => panic!("expected ActorPunch relay, got {other:?}"),
    }
}

#[test]
fn test_foreign_punch_rejected() {
    // A client may not report another actor's punch (anti-framing, same as
    // ActorHit).
    let sess = session(4, Some(NetworkID::new(4)));
    assert!(
        handle_actor_punch(&sess, NetworkID::new(99), false).is_none(),
        "foreign punch rejected"
    );
    assert!(
        handle_actor_punch(&sess, NetworkID::new(99), true).is_none(),
        "foreign power punch rejected"
    );
}

#[test]
fn test_item_new_rejected_from_client() {
    // Clients never legitimately send ItemNew (server-authoritative item
    // creation) — accepting it would let anyone mint items.
    let registry = Arc::new(ObjectRegistry::new());
    let sess = session(1, Some(NetworkID::new(1)));
    let pkt = item::handle_item_new(
        &registry,
        &sess,
        &Pkt::ItemNew {
            id: NetworkID::new(50),
            ref_id: 0x10,
            base_id: 0x999,
            container: NetworkID::new(1),
            count: 999,
            condition: 100.0,
            equipped: false,
            silent: false,
            stick: false,
            scale: 1.0,
        },
    );
    assert!(pkt.is_none(), "client ItemNew rejected");
    assert!(
        registry.get(NetworkID::new(50)).is_none(),
        "no item minted by client packet"
    );
}

#[test]
fn test_item_condition_out_of_range_rejected() {
    let registry = Arc::new(ObjectRegistry::new());
    let player_id = registry.allocate_id();
    registry.insert(Player::new(player_id, 0x14, 0x07, 5));
    let item_id = registry.allocate_id();
    registry.insert(Item::new(item_id, 0x10, 0x999, player_id));

    let sess = session(1, Some(player_id));
    // 150% condition (infinite-repair hack) must be rejected.
    assert!(
        item::handle_item_condition(&registry, &sess, item_id, 150.0, 100).is_none(),
        "condition > 100 rejected"
    );
    // Valid condition accepted and applied.
    assert!(item::handle_item_condition(&registry, &sess, item_id, 75.0, 100).is_some());
    let arc = registry.get(item_id).unwrap();
    let guard = arc.read();
    assert_eq!(guard.as_any().downcast_ref::<Item>().unwrap().condition, 75.0);
}

// ═══════════════════════════════════════════════════════════════
// STR port: simulation ownership transfer + differential state
// ═══════════════════════════════════════════════════════════════

use ashfall_core::protocol::Packet;

fn actor_new_packet(id: u64, ref_id: u32) -> Packet {
    Packet::ActorNew {
        id: NetworkID::new(id),
        ref_id,
        base_id: 0x1234,
        values: Default::default(),
        base_values: Default::default(),
        race: 0,
        age: 0,
        idle: 0,
        moving: 0,
        moving_xy: 0,
        weapon: 0,
        female: false,
        alerted: false,
        sneaking: false,
        dead: false,
        death_limbs: 0,
        death_cause: 0,
        scale: 1.0,
    }
}

#[test]
fn test_actor_new_grants_ownership_to_sender() {
    let registry = Arc::new(ObjectRegistry::new());
    let alice = session(1, Some(NetworkID::new(10)));

    let (response, broadcast) = actor::handle_actor_new(&registry, &alice, &actor_new_packet(100, 0x500));
    assert_eq!(response, Some(Packet::OwnershipGranted { id: NetworkID::new(100) }), "sender gets ownership");
    assert!(broadcast.is_some(), "others get the ActorNew to render");
    assert_eq!(registry.owner_of(NetworkID::new(100)), Some(NetworkID::new(10)));

    // Second client reporting the same actor (same ref_id, different id) → rejected.
    let bob = session(2, Some(NetworkID::new(20)));
    let (response2, _) = actor::handle_actor_new(&registry, &bob, &actor_new_packet(101, 0x500));
    assert!(response2.is_none(), "duplicate ref_id rejected");

    // Same actor id re-reported → rejected (already owned).
    let (response3, _) = actor::handle_actor_new(&registry, &bob, &actor_new_packet(100, 0x501));
    assert!(response3.is_none(), "already-owned actor rejected");
}

#[test]
fn test_actor_mutation_gated_by_ownership() {
    let registry = Arc::new(ObjectRegistry::new());
    let alice = session(1, Some(NetworkID::new(10)));
    let bob = session(2, Some(NetworkID::new(20)));
    actor::handle_actor_new(&registry, &alice, &actor_new_packet(100, 0x500));

    // Owner (alice) may mutate the NPC.
    assert!(actor::handle_actor_state(&registry, &alice, NetworkID::new(100), 1, 2, 3, 4, true, false, false).is_some());
    // Non-owner (bob) may not.
    assert!(actor::handle_actor_state(&registry, &bob, NetworkID::new(100), 9, 9, 9, 9, true, true, true).is_none());
    assert!(actor::handle_actor_value(&registry, &alice, NetworkID::new(100), false, 0x14, 50.0).is_some());
    assert!(actor::handle_actor_value(&registry, &bob, NetworkID::new(100), false, 0x14, 50.0).is_none());

    // Spell casts: owner relays, non-owner rejected (STR NotifySpellCast).
    assert!(actor::handle_spell_cast(&registry, &alice, NetworkID::new(100), 0x001234, 1, false, NetworkID::new(20)).is_some());
    assert!(actor::handle_spell_cast(&registry, &bob, NetworkID::new(100), 0x001234, 1, false, NetworkID::new(20)).is_none());
    // Own player may always cast.
    assert!(actor::handle_spell_cast(&registry, &bob, NetworkID::new(20), 0x005678, 2, true, NetworkID::new(10)).is_some());
}

#[test]
fn test_actor_state_delta_applies_present_fields_only() {
    let registry = Arc::new(ObjectRegistry::new());
    let alice = session(1, Some(NetworkID::new(10)));
    actor::handle_actor_new(&registry, &alice, &actor_new_packet(100, 0x500));

    // Delta with only `alerted` present.
    let delta = actor::handle_actor_state_delta(
        &registry, &alice, NetworkID::new(100),
        None, None, None, None, Some(true), None, None,
    );
    assert!(delta.is_some());
    assert_eq!(delta.unwrap(), Packet::ActorStateDelta {
        id: NetworkID::new(100), idle: None, moving: None, moving_xy: None, weapon: None,
        alerted: Some(true), sneaking: None, firing: None,
    });

    // Non-owner delta rejected.
    let bob = session(2, Some(NetworkID::new(20)));
    assert!(actor::handle_actor_state_delta(
        &registry, &bob, NetworkID::new(100),
        Some(5), None, None, None, None, None, None,
    ).is_none(), "non-owner delta rejected");
}

#[test]
fn test_ownership_claim_and_disconnect_release() {
    let registry = Arc::new(ObjectRegistry::new());
    let alice = session(1, Some(NetworkID::new(10)));
    let bob = session(2, Some(NetworkID::new(20)));
    actor::handle_actor_new(&registry, &alice, &actor_new_packet(100, 0x500));

    // Claim while owned → rejected.
    assert!(actor::handle_ownership_claim(&registry, &bob, NetworkID::new(100)).is_none());

    // Alice leaves: her owned actors are released.
    let released = registry.release_player_owned(NetworkID::new(10));
    assert_eq!(released, vec![NetworkID::new(100)]);
    assert_eq!(registry.owner_of(NetworkID::new(100)), None);

    // Bob can now claim it.
    let grant = actor::handle_ownership_claim(&registry, &bob, NetworkID::new(100));
    assert_eq!(grant, Some(Packet::OwnershipGranted { id: NetworkID::new(100) }));
    assert_eq!(registry.owner_of(NetworkID::new(100)), Some(NetworkID::new(20)));

    // Claiming an unknown actor → rejected.
    assert!(actor::handle_ownership_claim(&registry, &bob, NetworkID::new(999)).is_none());
}

#[test]
fn test_owner_released_when_actor_removed() {
    let registry = Arc::new(ObjectRegistry::new());
    let alice = session(1, Some(NetworkID::new(10)));
    actor::handle_actor_new(&registry, &alice, &actor_new_packet(100, 0x500));
    assert!(registry.owner_of(NetworkID::new(100)).is_some());

    registry.remove(NetworkID::new(100));
    assert!(registry.owner_of(NetworkID::new(100)).is_none(), "remove clears ownership");
    assert!(registry.is_deleted(NetworkID::new(100)));
}

#[test]
fn test_session_string_table_wired() {
    let mut s = session(99, None);
    assert!(s.string_table.is_empty());
    let id = s.string_table.intern("Vault101");
    assert_eq!(s.string_table.lookup(id), Some("Vault101"));
}

#[test]
fn test_pvp_disabled_rejects_player_hits() {
    let registry = Arc::new(ObjectRegistry::new());
    let a = registry.allocate_id();
    registry.insert(Player::new(a, 0x14, 0x07, 5));
    let b = registry.allocate_id();
    registry.insert(Player::new(b, 0x14, 0x07, 5));
    // Give them positions within range
    {
        let arc = registry.get(a).unwrap();
        let mut g = arc.write();
        g.as_any_mut().downcast_mut::<Player>().unwrap().actor.container.object.net_pos = [0.0, 0.0, 0.0];
    }
    {
        let arc = registry.get(b).unwrap();
        let mut g = arc.write();
        g.as_any_mut().downcast_mut::<Player>().unwrap().actor.container.object.net_pos = [10.0, 0.0, 0.0];
    }

    let sess = session(1, Some(a));
    let hit = Pkt::ActorHit {
        target: b, attacker: a, limb: 0, base_damage: 50.0, flags: 0, weapon_id: 0, projectile: 0,
    };
    // PvP off → player-on-player hit rejected.
    assert!(handle_actor_hit(&registry, &sess, &hit, false).is_none(), "pvp off rejects player hit");
    // PvP on → resolves.
    assert!(handle_actor_hit(&registry, &sess, &hit, true).is_some(), "pvp on resolves player hit");
}

#[test]
fn test_actor_registered_in_owners_cell_and_streams_on_context() {
    use ashfall_server::handlers::player;
    let registry = Arc::new(ObjectRegistry::new());

    // Alice claims an NPC while in cell 5.
    let mut alice = session(1, Some(NetworkID::new(10)));
    alice.current_cell = 5;
    actor::handle_actor_new(&registry, &alice, &actor_new_packet(100, 0x500));
    assert_eq!(registry.get_by_cell(5), vec![NetworkID::new(100)], "actor in owner's cell");

    // Bob moves his context to include cell 5 → he receives the actor.
    let mut bob = session(2, Some(NetworkID::new(20)));
    bob.cell_context = [1, 1, 1, 1, 1, 1, 1, 1, 1]; // all cell 1, no overlap
    let cells = [5, 5, 5, 5, 5, 5, 5, 5, 5];
    let pkts = player::handle_update_context(&registry, &mut bob, NetworkID::new(20), cells, false);
    assert!(
        pkts.iter().any(|p| matches!(p, Packet::ActorNew { id, .. } if *id == NetworkID::new(100))),
        "entered cell streams the actor"
    );

    // Bob leaves cell 5 → the actor is removed.
    let cells2 = [1, 1, 1, 1, 1, 1, 1, 1, 1];
    let pkts2 = player::handle_update_context(&registry, &mut bob, NetworkID::new(20), cells2, false);
    assert!(
        pkts2.iter().any(|p| matches!(p, Packet::ObjectRemove { id, .. } if *id == NetworkID::new(100))),
        "left cell removes the actor"
    );
}
