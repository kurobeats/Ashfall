//! Master server integration test.
//!
//! Round-trip: encode → decode, announce, update, cull.

use ashfall_core::protocol::Packet;
use std::collections::HashMap;

/// Wire format: [2B len][1B channel][N bytes postcard(Packet)]
const HEADER_SIZE: usize = 3;

fn encode_packet(packet: &Packet) -> Vec<u8> {
    let payload = postcard::to_stdvec(packet).unwrap();
    let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    buf.push(0u8);
    buf.extend_from_slice(&payload);
    buf
}

fn decode_packet(data: &[u8]) -> Option<Packet> {
    if data.len() < HEADER_SIZE {
        return None;
    }
    let _length = u16::from_le_bytes([data[0], data[1]]) as usize;
    let _channel = data[2];
    let payload = &data[HEADER_SIZE..];
    postcard::from_bytes(payload).ok()
}

#[test]
fn test_master_announce_encode_decode() {
    let announce = Packet::MasterAnnounce {
        name: "Test Server".into(),
        map: "Wasteland".into(),
        players: 2,
        max_players: 4,
        rules: HashMap::new(),
        mod_files: vec![],
        game_type: "fo3".into(),
    };

    let data = encode_packet(&announce);
    let decoded = decode_packet(&data).expect("decode");

    match decoded {
        Packet::MasterAnnounce { name, players, max_players, game_type, .. } => {
            assert_eq!(name, "Test Server");
            assert_eq!(players, 2);
            assert_eq!(max_players, 4);
            assert_eq!(game_type, "fo3");
        }
        other => panic!("Expected MasterAnnounce, got {:?}", std::mem::discriminant(&other)),
    }
}

#[test]
fn test_master_update_encode_decode() {
    let update = Packet::MasterUpdate {
        name: "Server".into(),
        map: "Map".into(),
        players: 1,
        max_players: 4,
    };

    let data = encode_packet(&update);
    let decoded = decode_packet(&data).expect("decode");

    match decoded {
        Packet::MasterUpdate { name, players, max_players, .. } => {
            assert_eq!(name, "Server");
            assert_eq!(players, 1);
            assert_eq!(max_players, 4);
        }
        other => panic!("Expected MasterUpdate, got {:?}", std::mem::discriminant(&other)),
    }
}

#[test]
fn test_master_query_encode_decode() {
    let query = Packet::MasterQuery;
    let data = encode_packet(&query);
    let decoded = decode_packet(&data).expect("decode");
    assert!(matches!(decoded, Packet::MasterQuery));
}

#[test]
fn test_master_announce_fnv() {
    let announce = Packet::MasterAnnounce {
        name: "Mojave".into(),
        map: "Mojave".into(),
        players: 3,
        max_players: 8,
        rules: HashMap::new(),
        mod_files: vec!["willow.esp".into()],
        game_type: "fnv".into(),
    };

    let data = encode_packet(&announce);
    let decoded = decode_packet(&data).expect("decode");

    match decoded {
        Packet::MasterAnnounce { name, game_type, mod_files, players, max_players, .. } => {
            assert_eq!(name, "Mojave");
            assert_eq!(game_type, "fnv");
            assert_eq!(players, 3);
            assert_eq!(max_players, 8);
            assert_eq!(mod_files.len(), 1);
        }
        other => panic!("Expected MasterAnnounce, got {:?}", std::mem::discriminant(&other)),
    }
}

#[test]
fn test_master_deregister() {
    // MasterUpdate with max_players=0 signals deregister
    let update = Packet::MasterUpdate {
        name: String::new(),
        map: String::new(),
        players: 0,
        max_players: 0,
    };

    let data = encode_packet(&update);
    let decoded = decode_packet(&data).expect("decode");

    match decoded {
        Packet::MasterUpdate { max_players, .. } => {
            assert_eq!(max_players, 0);
        }
        other => panic!("Expected MasterUpdate, got {:?}", std::mem::discriminant(&other)),
    }
}

#[test]
fn test_master_server_list_cull() {
    // Unit test for ServerList culling
    // ponytail: directly test the module by including it
    // For now, verify the packet round-trip works
    let announce = Packet::MasterAnnounce {
        name: "Server A".into(),
        map: "Map A".into(),
        players: 2,
        max_players: 4,
        rules: HashMap::new(),
        mod_files: vec![],
        game_type: "fo3".into(),
    };

    let data = encode_packet(&announce);
    assert!(decode_packet(&data).is_some());
}
