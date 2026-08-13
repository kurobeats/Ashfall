//! End-to-end ownership-transfer flow over real UDP (STR port).
//!
//! Proves the full loop: ActorNew grants the sender ownership, the non-owner
//! cannot mutate, disconnect releases, a survivor reclaims. Also asserts the
//! join-time GameTime + ServerSettings packets arrive (STR CalendarService /
//! ServerSettings).

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::transport::CHANNEL_RELIABLE_FLAG;
use ashfall_core::protocol::Packet;
use ashfall_server::config::{DatabaseSection, ScriptSection, ServerConfig, ServerSection};
use ashfall_server::dedicated::DedicatedServer;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::net::UdpSocket;

static SEQ: AtomicU32 = AtomicU32::new(0);

fn nid(n: u64) -> NetworkID {
    NetworkID::new(n)
}

struct TestClient {
    sock: UdpSocket,
    seq: u16,
}

impl TestClient {
    async fn connect(port: u16) -> Self {
        let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sock.connect(SocketAddr::from(([127, 0, 0, 1], port))).await.unwrap();
        TestClient { sock, seq: 0 }
    }

    async fn send_reliable(&mut self, packet: &Packet) {
        let payload = postcard::to_stdvec(packet).unwrap();
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        let seq_bytes: Vec<u8> = if seq < 128 { vec![0x80 | seq as u8] } else { vec![0; 3] };
        let mut buf = Vec::new();
        buf.extend_from_slice(&((seq_bytes.len() + payload.len()) as u16).to_le_bytes());
        buf.push(CHANNEL_RELIABLE_FLAG | 0);
        buf.extend_from_slice(&seq_bytes);
        buf.extend_from_slice(&payload);
        self.sock.send(&buf).await.unwrap();
    }

    /// Next packet, or None after 400ms of silence.
    async fn recv_packet(&self) -> Option<Packet> {
        let mut buf = vec![0u8; 2048];
        let n = tokio::time::timeout(Duration::from_millis(400), self.sock.recv(&mut buf))
            .await.ok()?.ok()?;
        let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        let ch = buf[2];
        if ch == 0xFF {
            return None; // control frame — ignore
        }
        let mut skip = 3;
        if ch & CHANNEL_RELIABLE_FLAG != 0 {
            skip += if buf[3] & 0x80 != 0 { 1 } else { 3 };
        }
        if 3 + len > n {
            return None;
        }
        postcard::from_bytes(&buf[skip..3 + len]).ok()
    }

    /// Read until one full silence gap (all queued packets drained).
    async fn drain(&self) -> Vec<Packet> {
        let mut out = Vec::new();
        while let Some(pkt) = self.recv_packet().await {
            out.push(pkt);
        }
        out
    }
}

async fn boot() -> (DedicatedServer, u16) {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    // Empty script dir — plain server, no auth gates.
    let dir = std::env::temp_dir().join(format!("ashfall_owner_scripts_{}_{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).unwrap();

    let db = std::env::temp_dir().join(format!("ashfall_owner_db_{}_{}.sqlite3", std::process::id(), seq));
    let port = {
        let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        s.local_addr().unwrap().port()
    };
    let config = ServerConfig {
        server: ServerSection {
            host: "127.0.0.1".into(),
            port,
            connections: 4,
            announce: "127.0.0.1".into(),
            master_port: port + 1,
            game_type: "fo3".into(),
            pvp_enabled: true,
            mods: Vec::new(),
        },
        scripts: ScriptSection { path: dir },
        database: DatabaseSection { path: db },
        ..Default::default()
    };
    let server = DedicatedServer::new(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    (server, port)
}

async fn run_with<F, O>(server: DedicatedServer, client: F) -> O
where
    F: std::future::Future<Output = O>,
{
    tokio::select! {
        _ = server.run() => panic!("server exited unexpectedly"),
        out = client => out,
    }
}

fn actor_new_packet(id: u64, ref_id: u32) -> Packet {
    Packet::ActorNew {
        id: nid(id),
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

async fn authenticate(sock: &mut TestClient, name: &str) {
    sock.send_reliable(&Packet::GameAuth {
        name: name.into(),
        password: String::new(),
        version: ashfall_core::constants::DEDICATED_VERSION.into(),
    })
    .await;
}

#[tokio::test]
async fn test_ownership_transfer_roundtrip() {
    let (server, port) = boot().await;

    let client = async {
        let mut alice = TestClient::connect(port).await;
        let mut bob = TestClient::connect(port).await;

        authenticate(&mut alice, "alice").await;
        authenticate(&mut bob, "bob").await;

        // Join handshake: GameLoad + world state + settings + clock.
        let alice_join = alice.drain().await;
        assert!(alice_join.iter().any(|p| matches!(p, Packet::GameLoad)), "GameLoad");
        assert!(
            alice_join.iter().any(|p| matches!(p, Packet::ServerSettings { pvp_enabled: true })),
            "ServerSettings broadcast (pvp=true from config)"
        );
        assert!(
            alice_join.iter().any(|p| matches!(p, Packet::GameTime { .. })),
            "GameTime clock sent on join"
        );
        let bob_join = bob.drain().await;
        assert!(bob_join.iter().any(|p| matches!(p, Packet::GameLoad)));

        // Alice reports an NPC → she owns it; bob gets the render broadcast.
        alice.send_reliable(&actor_new_packet(100, 0x500)).await;
        let alice_grant = alice.drain().await;
        assert!(
            alice_grant.iter().any(|p| matches!(p, Packet::OwnershipGranted { id } if *id == nid(100))),
            "sender granted ownership"
        );
        let bob_saw_actor = bob.drain().await;
        assert!(
            bob_saw_actor.iter().any(|p| matches!(p, Packet::ActorNew { id, .. } if *id == nid(100))),
            "bob renders the actor"
        );

        // Bob (non-owner) tries to mutate the NPC → silently rejected.
        bob.send_reliable(&Packet::UpdateActorState {
            id: nid(100), idle: 9, moving: 9, moving_xy: 9, weapon: 9,
            alerted: true, sneaking: true, firing: false,
        }).await;
        assert!(
            bob.drain().await.is_empty(),
            "non-owner state update not relayed to anyone"
        );

        // Alice disconnects → her actors are released → bob is told.
        alice.send_reliable(&Packet::GameEnd { reason: 0 }).await;
        let bob_release = bob.drain().await;
        assert!(
            bob_release.iter().any(|p| matches!(p, Packet::OwnershipReleased { id } if *id == nid(100))),
            "release broadcast after owner disconnect"
        );

        // Bob reclaims.
        bob.send_reliable(&Packet::OwnershipClaim { id: nid(100) }).await;
        let bob_claim = bob.drain().await;
        assert!(
            bob_claim.iter().any(|p| matches!(p, Packet::OwnershipGranted { id } if *id == nid(100))),
            "survivor reclaims ownership"
        );
    };

    run_with(server, client).await;
}

// Silence unused-import warning for PathBuf kept for boot symmetry.
#[allow(dead_code)]
fn _unused(_: PathBuf) {}

async fn boot_with_mods(expected: Vec<String>) -> (DedicatedServer, u16) {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ashfall_mods_scripts_{}_{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).unwrap();
    let db = std::env::temp_dir().join(format!("ashfall_mods_db_{}_{}.sqlite3", std::process::id(), seq));
    let port = {
        let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        s.local_addr().unwrap().port()
    };
    let config = ServerConfig {
        server: ServerSection {
            host: "127.0.0.1".into(),
            port,
            connections: 4,
            announce: "127.0.0.1".into(),
            master_port: port + 1,
            game_type: "fo3".into(),
            pvp_enabled: false,
            mods: expected,
        },
        scripts: ScriptSection { path: dir },
        database: DatabaseSection { path: db },
        ..Default::default()
    };
    let server = DedicatedServer::new(config).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    (server, port)
}

#[tokio::test]
async fn test_mod_policy_rejects_mismatch() {
    let (server, port) = boot_with_mods(vec!["Fallout3.esm:C092218B".into()]).await;

    let client = async {
        // Wrong load order → rejected with GameEnd.
        let mut sock = TestClient::connect(port).await;
        authenticate(&mut sock, "Wanderer").await;
        sock.send_reliable(&Packet::GameModList {
            mods: vec![("Oblivion.esm".into(), 0xC092218B)],
        }).await;
        let mut saw_end = false;
        for _ in 0..8 {
            if let Some(Packet::GameEnd { .. }) = sock.recv_packet().await {
                saw_end = true;
                break;
            }
        }
        assert!(saw_end, "mod mismatch → GameEnd + disconnect");

        // Matching load order → accepted (GameLoad arrives).
        let mut sock2 = TestClient::connect(port).await;
        authenticate(&mut sock2, "Wanderer").await;
        sock2.send_reliable(&Packet::GameModList {
            mods: vec![("Fallout3.esm".into(), 0xC092218B)],
        }).await;
        let mut saw_load = false;
        for _ in 0..12 {
            if let Some(pkt) = sock2.recv_packet().await {
                if matches!(pkt, Packet::GameLoad) {
                    saw_load = true;
                    break;
                }
            }
        }
        assert!(saw_load, "matching load order accepted → GameLoad");
    };

    run_with(server, client).await;
}
