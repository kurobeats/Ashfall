//! Bandwidth measurement — quantifies the wire cost of the sync model so
//! "bandwidth tuning" has real numbers to tune against (README What's Left).

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::transport::encode_varint_seq;
use ashfall_core::protocol::{Channel, Packet};

/// Wire size of one reliable frame (varint seq + postcard payload + header).
fn reliable_frame_size(pkt: &Packet) -> usize {
    let payload = postcard::to_stdvec(pkt).unwrap();
    let seq = 0u16;
    let seq_bytes = encode_varint_seq(seq);
    3 + seq_bytes.len() + payload.len() // [len u16][channel][varint][payload]
}

#[test]
fn test_position_sync_bandwidth_budget() {
    // 30 Hz position updates, unreliable channel (no seq/ack overhead)
    let pos = Packet::UpdatePos { id: NetworkID::new(42), pos: [100.0, 200.0, 30.5] };
    let payload = postcard::to_stdvec(&pos).unwrap();
    let frame = 3 + payload.len(); // [len u16][channel][payload]
    let per_sec = frame * 30;
    println!("UpdatePos frame: {frame} B, 30Hz => {per_sec} B/s per sender");

    // Each of N players receives the others' updates:
    for n in [2usize, 4, 8] {
        let rx = per_sec * (n - 1);
        println!("  {n} players: each receives ~{rx} B/s of position sync");
        assert!(rx < 20_000, "position sync under 20 KB/s per client");
    }
}

#[test]
fn test_world_state_handoff_size() {
    // Initial world state for a new player: weather + globals + a batch of
    // objects. Reliable frames include varint seq (1-3 B) + header.
    let weather = Packet::GameWeather { weather: 0x15E5E };
    let mut world: Vec<Packet> = vec![weather];
    for i in 0..50u64 {
        world.push(Packet::ObjectNew {
            id: NetworkID::new(i + 1),
            ref_id: 0x100 + i as u32,
            base_id: 0x200,
            name: format!("Object {i}").into(),
            game_pos: [i as f32, 0.0, 0.0],
            net_pos: [i as f32, 0.0, 0.0],
            angle: [0.0; 3],
            scale: 1.0,
            cell: 1,
            enabled: true,
            lock: 0,
            owner: 0,
        });
    }
    let total: usize = world.iter().map(reliable_frame_size).sum();
    println!("World handoff (1 weather + 50 objects): {total} B");
    // Comfortably under the default 65536-byte datagram budget, even before
    // splitting into batches.
    assert!(total < 65_536, "handoff fits one datagram");
}

#[test]
fn test_chat_and_event_sizes() {
    let chat = Packet::GameChat { message: "hello world".repeat(5).into() };
    let quest = Packet::QuestStage { quest_id: 0x6136D, stage: 100 };
    let hit = Packet::ActorHit {
        target: NetworkID::new(1),
        attacker: NetworkID::new(2),
        limb: 1,
        base_damage: 42.5,
        flags: 0,
        weapon_id: 0x1234,
        projectile: 0,
    };
    println!("GameChat: {} B, QuestStage: {} B, ActorHit: {} B",
             reliable_frame_size(&chat), reliable_frame_size(&quest), reliable_frame_size(&hit));
    assert!(reliable_frame_size(&hit) < 256);
}

#[test]
fn test_channel_classification() {
    use Channel::*;
    assert_eq!(Channel::from_packet(&Packet::UpdatePos { id: NetworkID::new(1), pos: [0.0; 3] }), Game);
    assert_eq!(Channel::from_packet(&Packet::GameChat { message: "x".into() }), Chat);
    assert_eq!(Channel::from_packet(&Packet::GameLoad), System);
}
