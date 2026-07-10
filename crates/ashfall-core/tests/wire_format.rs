//! Wire-format round-trip tests for all Packet variants.
//!
//! Ensures every packet: serialize→deserialize→assert_eq,
//! and that no packet exceeds MAX_PACKET_SIZE (1200 bytes).

use ashfall_core::constants;
use ashfall_core::form_id::{FormID, FormIDSync};
use ashfall_core::id::NetworkID;
use ashfall_core::protocol::*;
use std::collections::HashMap;

fn roundtrip(packet: &Packet) {
    let bytes = postcard::to_stdvec(packet).expect("serialize");
    assert!(
        bytes.len() <= MAX_PACKET_SIZE,
        "packet size {} exceeds MAX_PACKET_SIZE: {:?}",
        bytes.len(),
        packet
    );
    let decoded: Packet = postcard::from_bytes(&bytes).expect("deserialize");
    assert_eq!(*packet, decoded, "round-trip mismatch");
}

fn nid(id: u64) -> NetworkID {
    NetworkID::new(id)
}

fn fid(id: u32) -> FormID {
    FormID::new(id)
}

// ═══════════════════════════════════════════════════════════════
// System packets
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_game_auth() {
    roundtrip(&Packet::GameAuth { name: "VaultDweller".into(), password: "101".into() });
}

#[test]
fn test_game_simple() {
    roundtrip(&Packet::GameLoad);
    roundtrip(&Packet::GameStart);
}

#[test]
fn test_game_end() {
    roundtrip(&Packet::GameEnd { reason: 4 }); // Quit
}

#[test]
fn test_game_mod() {
    roundtrip(&Packet::GameMod { filename: "vaultmp.esp".into(), crc: 0x1C877592 });
}

#[test]
fn test_game_message() {
    roundtrip(&Packet::GameMessage { message: "Hello".into(), emoticon: 1 });
}

#[test]
fn test_game_chat() {
    roundtrip(&Packet::GameChat { message: "Hi everyone!".into() });
}

#[test]
fn test_game_weather() {
    roundtrip(&Packet::GameWeather { weather: 0x00015E5E }); // Fallout3Clear
}

#[test]
fn test_game_global() {
    roundtrip(&Packet::GameGlobal { global: 0x123456, value: 42 });
}

#[test]
fn test_game_base() {
    roundtrip(&Packet::GameBase { player_base: 0x00000007 });
}

#[test]
fn test_game_deleted() {
    let mut deleted = HashMap::new();
    deleted.insert(0x1234, vec![0x5678, 0x9ABC]);
    roundtrip(&Packet::GameDeleted { deleted });
}

// ═══════════════════════════════════════════════════════════════
// Reference
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_reference_new() {
    roundtrip(&Packet::ReferenceNew { id: nid(1), ref_id: 0x1234, base_id: 0x5678 });
}

// ═══════════════════════════════════════════════════════════════
// Object (with scale)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_object_new() {
    roundtrip(&Packet::ObjectNew {
        id: nid(10),
        ref_id: 0x000A,
        base_id: 0x000B,
        name: "TestObject".into(),
        game_pos: [1.0, 2.0, 3.0],
        net_pos: [1.0, 2.0, 3.0],
        angle: [0.0, 0.0, 90.0],
        scale: 1.0,
        cell: 5,
        enabled: true,
        lock: 0,
        owner: 0,
    });
}

#[test]
fn test_object_scale_non_default() {
    roundtrip(&Packet::ObjectNew {
        id: nid(11),
        ref_id: 0x000C,
        base_id: 0x000D,
        name: "BigDoor".into(),
        game_pos: [0.0; 3],
        net_pos: [0.0; 3],
        angle: [0.0; 3],
        scale: 2.5,
        cell: 3,
        enabled: true,
        lock: 50,
        owner: 0,
    });
}

#[test]
fn test_update_pos() {
    roundtrip(&Packet::UpdatePos { id: nid(10), pos: [100.0, 200.0, 50.0] });
}

#[test]
fn test_update_angle() {
    roundtrip(&Packet::UpdateAngle { id: nid(10), angle: [0.0, 180.0] });
}

#[test]
fn test_update_scale() {
    roundtrip(&Packet::UpdateScale { id: nid(10), scale: 1.5 });
}

#[test]
fn test_update_cell() {
    roundtrip(&Packet::UpdateCell { id: nid(10), cell: 42, pos: [0.0; 3] });
}

// ═══════════════════════════════════════════════════════════════
// Physics
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_update_velocity() {
    roundtrip(&Packet::UpdateVelocity { id: nid(10), vel: [50.0, 0.0, 0.0], on_ground: true });
}

#[test]
fn test_update_velocity_falling() {
    roundtrip(&Packet::UpdateVelocity { id: nid(10), vel: [0.0, 0.0, -200.0], on_ground: false });
}

// ═══════════════════════════════════════════════════════════════
// Item (with scale)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_item_new() {
    roundtrip(&Packet::ItemNew {
        id: nid(20),
        ref_id: 0x100,
        base_id: 0x200,
        container: nid(10),
        count: 1,
        condition: 1.0,
        equipped: false,
        silent: false,
        stick: false,
        scale: 1.0,
    });
}

#[test]
fn test_update_item_count() {
    roundtrip(&Packet::UpdateItemCount { id: nid(20), count: 5, silent: false });
}

#[test]
fn test_update_item_condition() {
    roundtrip(&Packet::UpdateItemCondition { id: nid(20), condition: 0.75, health: 100 });
}

#[test]
fn test_update_item_equipped() {
    roundtrip(&Packet::UpdateItemEquipped {
        id: nid(20), equipped: true, silent: false, stick: false,
    });
}

// ═══════════════════════════════════════════════════════════════
// Actor (with scale)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_actor_new() {
    let mut values = HashMap::new();
    values.insert(0x14, 100.0); // Health
    values.insert(0x15, 50.0);  // Action points
    let mut base_values = HashMap::new();
    base_values.insert(0x14, 100.0);
    roundtrip(&Packet::ActorNew {
        id: nid(30),
        ref_id: 0x300,
        base_id: 0x400,
        values,
        base_values,
        race: 0x19, // Caucasian
        age: 25,
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
    });
}

#[test]
fn test_update_actor_state() {
    roundtrip(&Packet::UpdateActorState {
        id: nid(30),
        idle: 1,
        moving: 2,
        moving_xy: 0,
        weapon: 3,
        alerted: true,
        sneaking: false,
        firing: false,
    });
}

#[test]
fn test_update_actor_value() {
    roundtrip(&Packet::UpdateActorValue { id: nid(30), base: false, index: 0x14, value: 75.0 });
}

#[test]
fn test_fire_weapon() {
    roundtrip(&Packet::UpdateFireWeapon { id: nid(30), weapon: 0x1234 });
}

// ═══════════════════════════════════════════════════════════════
// Combat
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_actor_hit() {
    roundtrip(&Packet::ActorHit {
        target: nid(30),
        attacker: nid(40),
        limb: LIMB_TORSO,
        base_damage: 25.0,
        flags: HIT_FLAG_CRITICAL,
        weapon_id: 0x1234,
        projectile: 0,
    });
}

#[test]
fn test_actor_hit_headshot() {
    roundtrip(&Packet::ActorHit {
        target: nid(30),
        attacker: nid(40),
        limb: LIMB_HEAD,
        base_damage: 80.0,
        flags: HIT_FLAG_CRITICAL | HIT_FLAG_SNEAK,
        weapon_id: 0x5678,
        projectile: 0xABCD,
    });
}

#[test]
fn test_actor_damaged() {
    roundtrip(&Packet::ActorDamaged {
        target: nid(30),
        attacker: nid(40),
        limb: LIMB_TORSO,
        final_damage: 18.5,
        flags: 0,
    });
}

#[test]
fn test_actor_death_ext() {
    roundtrip(&Packet::ActorDeathExt {
        id: nid(30),
        killer: nid(40),
        weapon_id: 0x1234,
        limbs: 0x1F,
        cause: 1,
        death_flags: DEATH_FLAG_HEADSHOT,
    });
}

#[test]
fn test_projectile_new() {
    roundtrip(&Packet::ProjectileNew {
        id: nid(50),
        base_id: 0x9999,
        pos: [100.0, 200.0, 50.0],
        vel: [500.0, 0.0, 10.0],
        owner: nid(40),
    });
}

#[test]
fn test_projectile_remove() {
    roundtrip(&Packet::ProjectileRemove { id: nid(50), impact_pos: [150.0, 200.0, 50.0] });
}

#[test]
fn test_explosion_new() {
    roundtrip(&Packet::ExplosionNew {
        base_id: 0xAAAA,
        pos: [100.0, 200.0, 0.0],
        radius: 256.0,
        owner: nid(40),
    });
}

// ═══════════════════════════════════════════════════════════════
// NPC AI
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_actor_combat_target() {
    roundtrip(&Packet::ActorCombatTarget { id: nid(60), target: nid(40) });
}

#[test]
fn test_actor_ai_package() {
    roundtrip(&Packet::ActorAIPackage { id: nid(60), package_id: AI_PACKAGE_COMBAT, flags: 0x01 });
}

#[test]
fn test_actor_faction() {
    roundtrip(&Packet::ActorFaction { id: nid(60), faction_id: 0x1234, rank: -1 });
}

// ═══════════════════════════════════════════════════════════════
// Player (with scale)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_player_new() {
    let mut controls = HashMap::new();
    controls.insert(0, (0x11, true)); // W key, enabled
    roundtrip(&Packet::PlayerNew {
        id: nid(40),
        ref_id: 0x14,
        base_id: 0x07,
        controls,
        scale: 1.0,
    });
}

#[test]
fn test_update_control() {
    roundtrip(&Packet::UpdateControl { id: nid(40), control: 0, key: 0x11 });
}

#[test]
fn test_update_interior() {
    roundtrip(&Packet::UpdateInterior { id: nid(40), cell: "Vault101Start".into(), spawn: true });
}

#[test]
fn test_update_exterior() {
    roundtrip(&Packet::UpdateExterior {
        id: nid(40), world: 1, x: -5, y: 3, spawn: true,
    });
}

#[test]
fn test_update_context() {
    let cells = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    roundtrip(&Packet::UpdateContext { id: nid(40), cells, spawn: false });
}

// ═══════════════════════════════════════════════════════════════
// World Objects
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_door_state() {
    roundtrip(&Packet::DoorState { id: nid(70), open: true, ref_id: 0xABCD });
}

#[test]
fn test_terminal_state() {
    roundtrip(&Packet::TerminalState { id: nid(71), locked: true, ref_id: 0x1234 });
}

// ═══════════════════════════════════════════════════════════════
// Quest & Dialogue
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_quest_stage() {
    roundtrip(&Packet::QuestStage { quest_id: 0x1234, stage: 10 });
}

#[test]
fn test_dialogue_flag() {
    roundtrip(&Packet::DialogueFlag { flag_id: 0x5678, value: true });
}

#[test]
fn test_dialogue_choice() {
    roundtrip(&Packet::DialogueChoice { flag_id: 0x5678, choice: 2 });
}

// ═══════════════════════════════════════════════════════════════
// FO3 Globals
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_karma_update() {
    roundtrip(&Packet::KarmaUpdate { value: 500 });
}

#[test]
fn test_karma_negative() {
    roundtrip(&Packet::KarmaUpdate { value: -300 });
}

// ═══════════════════════════════════════════════════════════════
// FNV Globals
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_reputation_update() {
    roundtrip(&Packet::ReputationUpdate { faction: 0x1234, value: 50 });
}

#[test]
fn test_reputation_vilified() {
    roundtrip(&Packet::ReputationUpdate { faction: 0x5678, value: -100 });
}

#[test]
fn test_hardcore_stats() {
    roundtrip(&Packet::HardcoreStats { hunger: 250.0, thirst: 300.0, sleep: 150.0 });
}

#[test]
fn test_hardcore_stats_zero() {
    roundtrip(&Packet::HardcoreStats { hunger: 0.0, thirst: 0.0, sleep: 0.0 });
}

// ═══════════════════════════════════════════════════════════════
// Cell Snapshot
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cell_snapshot() {
    let objects = vec![
        FormIDSync::new(fid(0x1234), [100.0, 200.0, 0.0], [0.0; 3], 1.0, 0),
        FormIDSync::new(fid(0x5678), [150.0, 250.0, 0.0], [0.0; 3], 0.5, CELL_FLAG_INITIALLY_DISABLED),
    ];
    roundtrip(&Packet::CellSnapshot { cell: 42, objects });
}

#[test]
fn test_cell_snapshot_empty() {
    roundtrip(&Packet::CellSnapshot { cell: 42, objects: vec![] });
}

#[test]
fn test_cell_snapshot_large() {
    // ponytail: batch of 35 objects — max that fits within 1200 bytes
    // Larger cells need multi-packet splitting (deferred to Phase 9).
    let objects: Vec<_> = (0..35)
        .map(|i| FormIDSync::new(fid(0x1000 + i as u32), [i as f32 * 100.0, 0.0, 0.0], [0.0; 3], 1.0, 0))
        .collect();
    roundtrip(&Packet::CellSnapshot { cell: 42, objects });
}

// ═══════════════════════════════════════════════════════════════
// Master Server
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_master_query() {
    roundtrip(&Packet::MasterQuery);
}

#[test]
fn test_master_announce_fo3() {
    let mut rules = HashMap::new();
    rules.insert("gamemode".into(), "freeroam".into());
    roundtrip(&Packet::MasterAnnounce {
        name: "Test Server".into(),
        map: "Wasteland".into(),
        players: 2,
        max_players: 4,
        rules,
        mod_files: vec!["vaultmp.esp".into()],
        game_type: "fo3".into(),
    });
}

#[test]
fn test_master_announce_fnv() {
    roundtrip(&Packet::MasterAnnounce {
        name: "Mojave".into(),
        map: "WastelandNV".into(),
        players: 3,
        max_players: 8,
        rules: HashMap::new(),
        mod_files: vec![],
        game_type: "fnv".into(),
    });
}

#[test]
fn test_master_update() {
    roundtrip(&Packet::MasterUpdate { name: "Test".into(), map: "Map".into(), players: 1, max_players: 4 });
}

// ═══════════════════════════════════════════════════════════════
// Window / GUI (unchanged, just verify they still work)
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_window_new() {
    roundtrip(&Packet::WindowNew {
        id: nid(100),
        parent: nid(0),
        label: "MainMenu".into(),
        pos: [0.0; 4],
        size: [800.0, 600.0, 0.0, 0.0],
        locked: false,
        visible: true,
        text: "Welcome".into(),
    });
}

// ═══════════════════════════════════════════════════════════════
// Channel routing
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_channel_routing() {
    // System channel
    assert_eq!(Channel::from_packet(&Packet::GameStart), Channel::System);
    assert_eq!(Channel::from_packet(&Packet::QuestStage { quest_id: 1, stage: 1 }), Channel::System);
    assert_eq!(Channel::from_packet(&Packet::DialogueFlag { flag_id: 1, value: true }), Channel::System);
    assert_eq!(Channel::from_packet(&Packet::DialogueChoice { flag_id: 1, choice: 1 }), Channel::System);
    assert_eq!(Channel::from_packet(&Packet::KarmaUpdate { value: 0 }), Channel::System);
    assert_eq!(Channel::from_packet(&Packet::ReputationUpdate { faction: 1, value: 0 }), Channel::System);
    assert_eq!(Channel::from_packet(&Packet::HardcoreStats { hunger: 0.0, thirst: 0.0, sleep: 0.0 }), Channel::System);

    // Chat channel
    assert_eq!(Channel::from_packet(&Packet::GameChat { message: "".into() }), Channel::Chat);

    // Game channel (all position/combat/ai packets)
    assert_eq!(Channel::from_packet(&Packet::UpdatePos { id: nid(1), pos: [0.0; 3] }), Channel::Game);
    assert_eq!(Channel::from_packet(&Packet::ActorHit {
        target: nid(1), attacker: nid(2), limb: 0, base_damage: 10.0, flags: 0, weapon_id: 0, projectile: 0,
    }), Channel::Game);
}

#[test]
fn test_unreliable_routing() {
    assert!(Channel::is_unreliable(&Packet::UpdatePos { id: nid(1), pos: [0.0; 3] }));
    assert!(Channel::is_unreliable(&Packet::UpdateAngle { id: nid(1), angle: [0.0; 2] }));
    assert!(Channel::is_unreliable(&Packet::UpdateVelocity { id: nid(1), vel: [0.0; 3], on_ground: true }));
    assert!(Channel::is_unreliable(&Packet::ProjectileRemove { id: nid(1), impact_pos: [0.0; 3] }));

    assert!(!Channel::is_unreliable(&Packet::GameChat { message: "".into() }));
    assert!(!Channel::is_unreliable(&Packet::ActorDamaged {
        target: nid(1), attacker: nid(2), limb: 0, final_damage: 10.0, flags: 0,
    }));
}

// ═══════════════════════════════════════════════════════════════
// Size limits
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_max_name_size() {
    let name = "A".repeat(constants::MAX_PLAYER_NAME);
    roundtrip(&Packet::GameAuth { name, password: "".into() });
}

#[test]
fn test_max_chat_length() {
    let message = "B".repeat(constants::MAX_CHAT_LENGTH);
    roundtrip(&Packet::GameChat { message });
}

// ═══════════════════════════════════════════════════════════════
// FormID
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_form_id_roundtrip() {
    let f = FormID::new(0x01001234);
    let bytes = postcard::to_stdvec(&f).unwrap();
    let decoded: FormID = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(f, decoded);
}

#[test]
fn test_form_id_mod_index() {
    assert_eq!(FormID::new(0x01001234).mod_index(), 1);
    assert_eq!(FormID::new(0x00001234).mod_index(), 0);
    assert_eq!(FormID::new(0xFF001234).mod_index(), 255);
}

#[test]
fn test_form_id_object_id() {
    assert_eq!(FormID::new(0x01001234).object_id(), 0x001234);
    assert_eq!(FormID::new(0x02FFFFFF).object_id(), 0x00FFFFFF);
}

#[test]
fn test_form_id_null() {
    assert!(FormID::NULL.is_null());
    assert!(!FormID::new(1).is_null());
}

#[test]
fn test_form_id_sync_roundtrip() {
    let sync = FormIDSync::new(fid(0x1234), [100.0, 200.0, 0.0], [0.0; 3], 1.0, CELL_FLAG_PERSISTENT);
    let bytes = postcard::to_stdvec(&sync).unwrap();
    let decoded: FormIDSync = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(sync, decoded);
}

// ═══════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_anti_cheat_constants() {
    assert!(constants::MAX_SPEED > 0.0);
    assert!(constants::MAX_TELEPORT_DISTANCE > 0.0);
    assert_eq!(constants::MAX_ITEM_STACK, 65535);
    assert!(constants::MIN_SCALE > 0.0);
    assert!(constants::MAX_SCALE > constants::MIN_SCALE);
}
