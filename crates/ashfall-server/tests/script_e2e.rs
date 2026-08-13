//! End-to-end script integration test — real DedicatedServer + raw UDP client
//! + a WASM game mode loaded from disk.
//!
//! Proves the whole loop: script `on_client_authenticate` gate on the wire,
//! script-set weather reaching the client, and `on_spawn` → `chat_message`
//! effect delivered as a GameChat packet.
//!
//! Note: DedicatedServer is not `Send` (parking_lot guards across awaits), so
//! the server future runs on the test's current thread via `tokio::select!`.

use ashfall_core::id::NetworkID;
use ashfall_core::protocol::transport::CHANNEL_RELIABLE_FLAG;
use ashfall_core::protocol::Packet;
use ashfall_server::config::{DatabaseSection, ScriptSection, ServerConfig, ServerSection};
use ashfall_server::dedicated::DedicatedServer;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::net::UdpSocket;

/// The test game mode: denies "bob", sets weather 0x12345 at boot,
/// spawn-chats "Hello from script!".
const E2E_MODE: &str = r#"
(module
  (import "env" "set_game_weather" (func $set_weather (param i32)))
  (import "env" "chat_message" (func $chat (param i64 i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 4096) "Hello from script!")

  (func (export "on_server_init")
    (call $set_weather (i32.const 0x00012345)))

  (func (export "on_client_authenticate")
    (param $nptr i32) (param $nlen i32) (param $pptr i32) (param $plen i32) (result i32)
    (if (i32.and (i32.eq (local.get $nlen) (i32.const 3))
                 (i32.eq (i32.load8_u (local.get $nptr)) (i32.const 98)))
      (then (return (i32.const 0))))
    (i32.const 1))

  (func (export "on_player_request_game") (param $pid i64) (result i32)
    (i32.const 0x0000CAFE))

  (func (export "on_spawn") (param $pid i64)
    (call $chat (local.get $pid) (i32.const 4096) (i32.const 18)))

  ;; on_hit: block hits dealing more than 100 damage
  (func (export "on_hit") (param $t i64) (param $a i64) (param $limb i32) (param $dmg f32) (result i32)
    (if (f32.gt (local.get $dmg) (f32.const 100))
      (then (return (i32.const 0))))
    (i32.const 1))
)
"#;

static TEST_SEQ: AtomicU32 = AtomicU32::new(0);

/// Free UDP port for the test server.
fn free_port() -> u16 {
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral");
    sock.local_addr().expect("local addr").port()
}

/// Build a per-test ServerConfig (unique scripts dir + db, random port).
async fn test_config() -> ServerConfig {
    let seq = TEST_SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ashfall_e2e_scripts_{}_{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).expect("create scripts dir");
    let wasm = wat::parse_str(E2E_MODE).expect("valid WAT");
    std::fs::write(dir.join("e2e_mode.wasm"), &wasm).expect("write wasm");

    let db_path = std::env::temp_dir().join(format!("ashfall_e2e_{}_{}.sqlite3", std::process::id(), seq));
    ServerConfig {
        server: ServerSection {
            host: "127.0.0.1".into(),
            port: free_port(),
            connections: 4,
            announce: "127.0.0.1".into(),
            master_port: free_port(),
            game_type: "fo3".into(),
            pvp_enabled: false,
        },
        scripts: ScriptSection { path: dir },
        database: DatabaseSection { path: db_path },
        ..Default::default()
    }
}

/// Encode a reliable packet frame with the given seq (mirrors the client).
fn encode_reliable(packet: &Packet, seq: u16) -> Vec<u8> {
    let payload = postcard::to_stdvec(packet).expect("postcard");
    let seq_bytes: Vec<u8> = if seq < 128 {
        vec![0x80 | seq as u8]
    } else {
        let mut v = vec![0x00];
        v.extend_from_slice(&seq.to_le_bytes());
        v
    };
    let mut buf = Vec::with_capacity(3 + seq_bytes.len() + payload.len());
    buf.extend_from_slice(&((seq_bytes.len() + payload.len()) as u16).to_le_bytes());
    buf.push(CHANNEL_RELIABLE_FLAG | 0); // System channel
    buf.extend_from_slice(&seq_bytes);
    buf.extend_from_slice(&payload);
    buf
}

/// Raw UDP client that tracks its reliable sequence counter.
struct TestClient {
    sock: UdpSocket,
    seq: u16,
}

impl TestClient {
    async fn connect(port: u16) -> Self {
        let sock = UdpSocket::bind("127.0.0.1:0").await.expect("client bind");
        sock.connect(SocketAddr::from(([127, 0, 0, 1], port)))
            .await
            .expect("client connect");
        TestClient { sock, seq: 0 }
    }

    async fn send_reliable(&mut self, packet: &Packet) {
        let seq = self.seq;
        self.seq = self.seq.wrapping_add(1);
        self.sock
            .send(&encode_reliable(packet, seq))
            .await
            .expect("send reliable");
    }

    async fn recv_packet(&self) -> Option<Packet> {
        recv_packet(&self.sock).await
    }
}

/// Read one packet from the wire, skipping ACK/control frames.
async fn recv_packet(sock: &UdpSocket) -> Option<Packet> {
    let mut buf = vec![0u8; 2048];
    let n = tokio::time::timeout(Duration::from_millis(400), sock.recv(&mut buf))
        .await
        .ok()?
        .ok()?;
    let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    let channel = buf[2];
    if channel == 0xFF {
        return None; // control frame (ACK/NACK) — skip
    }
    let mut skip = 3; // len(2) + channel(1)
    if channel & CHANNEL_RELIABLE_FLAG != 0 {
        skip += if buf[3] & 0x80 != 0 { 1 } else { 3 }; // varint seq
    }
    if 3 + len > n {
        return None;
    }
    postcard::from_bytes(&buf[skip..3 + len]).ok()
}

/// Run the server future on this thread alongside the client future.
/// Returns the client future's output when it finishes first.
async fn run_with_server<F, O>(server: DedicatedServer, client: F) -> O
where
    F: std::future::Future<Output = O>,
{
    tokio::select! {
        _ = server.run() => panic!("server exited unexpectedly"),
        out = client => out,
    }
}

#[tokio::test]
async fn test_script_auth_gate_on_wire() {
    let config = test_config().await;
    let port = config.server.port;
    let server = DedicatedServer::new(config).await.expect("server boots");
    tokio::time::sleep(Duration::from_millis(200)).await; // let scripts instantiate

    let client = async {
        let mut sock = TestClient::connect(port).await;
        sock.send_reliable(&Packet::GameAuth {
            name: "bob".into(),
            password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        })
        .await;

        let mut saw_end = false;
        for _ in 0..8 {
            if let Some(pkt) = sock.recv_packet().await {
                if let Packet::GameEnd { reason } = pkt {
                    saw_end = true;
                    assert_eq!(reason, ashfall_core::types::Reason::Denied as u8, "denied by script");
                    break;
                }
            }
        }
        assert!(saw_end, "script must reject 'bob' with GameEnd");
    };

    run_with_server(server, client).await;
}

#[tokio::test]
async fn test_two_client_chat_relay() {
    let config = test_config().await;
    let port = config.server.port;
    let server = DedicatedServer::new(config).await.expect("server boots");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = async {
        // Both players connect and reach InGame
        let mut alice = TestClient::connect(port).await;
        alice
            .send_reliable(&Packet::GameAuth {
                name: "alice".into(),
                password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        })
        .await;

        let mut bob = TestClient::connect(port).await;
        bob.send_reliable(&Packet::GameAuth {
            name: "carol".into(),
            password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        })
        .await;

        let mut alice_ready = false;
        for _ in 0..12 {
            if let Some(pkt) = alice.recv_packet().await {
                if matches!(pkt, Packet::GameLoad) {
                    alice_ready = true;
                    break;
                }
            }
        }
        assert!(alice_ready, "alice reached GameLoad");

        let mut bob_ready = false;
        for _ in 0..12 {
            if let Some(pkt) = bob.recv_packet().await {
                if matches!(pkt, Packet::GameLoad) {
                    bob_ready = true;
                    break;
                }
            }
        }
        assert!(bob_ready, "carol reached GameLoad");

        bob.send_reliable(&Packet::GameChat {
            message: "hello alice".into(),
        })
        .await;

        let mut saw_relay = false;
        for _ in 0..16 {
            if let Some(pkt) = alice.recv_packet().await {
                // Stale spawn-welcome / PlayerNew packets may still be in
                // alice's buffer — keep scanning until bob's relay arrives.
                if let Packet::GameChat { message } = pkt {
                    use ashfall_core::string_cache::StringTable;
                    if message.resolve(&mut StringTable::new()) == "hello alice" {
                        saw_relay = true;
                        break;
                    }
                }
            }
        }
        assert!(saw_relay, "alice received bob's chat");
    };

    run_with_server(server, client).await;
}

#[tokio::test]
async fn test_script_world_and_spawn_effects_on_wire() {
    let config = test_config().await;
    let port = config.server.port;
    let server = DedicatedServer::new(config).await.expect("server boots");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = async {
        let mut sock = TestClient::connect(port).await;
        sock.send_reliable(&Packet::GameAuth {
            name: "alice".into(),
            password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        })
        .await;

        let mut saw_load = false;
        let mut saw_weather = false;
        let mut saw_welcome = false;
        for _ in 0..24 {
            if let Some(pkt) = sock.recv_packet().await {
                match pkt {
                    Packet::GameLoad => saw_load = true,
                    Packet::GameWeather { weather } => {
                        saw_weather = weather == 0x00012345;
                    }
                    Packet::GameChat { message } => {
                        use ashfall_core::string_cache::StringTable;
                        saw_welcome = message.resolve(&mut StringTable::new()) == "Hello from script!";
                    }
                    _ => {}
                }
            }
        }

        assert!(saw_load, "alice gets GameLoad");
        assert!(saw_weather, "script-set weather (0x12345) reached the client");
        assert!(saw_welcome, "on_spawn chat effect delivered to the client");
    };

    run_with_server(server, client).await;
}

#[tokio::test]
async fn test_script_hit_gate_on_wire() {
    let config = test_config().await;
    let port = config.server.port;
    let server = DedicatedServer::new(config).await.expect("server boots");
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = async {
        let mut sock = TestClient::connect(port).await;
        sock.send_reliable(&Packet::GameAuth {
            name: "alice".into(),
            password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        })
        .await;

        // Reach InGame (GameLoad), then send a hit with damage > 100 —
        // the script's on_hit must block it (no ActorDamaged echoes back).
        let mut ready = false;
        for _ in 0..12 {
            if let Some(pkt) = sock.recv_packet().await {
                if matches!(pkt, Packet::GameLoad) {
                    ready = true;
                    break;
                }
            }
        }
        assert!(ready, "alice reached GameLoad");

        // The player's own id comes from PlayerNew
        let mut my_id = 0u64;
        for _ in 0..8 {
            if let Some(Packet::PlayerNew { id, .. }) = sock.recv_packet().await {
                my_id = id.as_u64();
                break;
            }
        }
        assert_ne!(my_id, 0, "got PlayerNew with our id");

        sock.send_reliable(&Packet::ActorHit {
            target: NetworkID::new(my_id),
            attacker: NetworkID::new(my_id),
            limb: 0,
            base_damage: 500.0, // > 100 → script blocks
            flags: 0,
            weapon_id: 0,
            projectile: 0,
        })
        .await;

        // No ActorDamaged must come back (blocked); the channel stays usable.
        let mut saw_damaged = false;
        for _ in 0..8 {
            if let Some(pkt) = sock.recv_packet().await {
                if matches!(pkt, Packet::ActorDamaged { .. }) {
                    saw_damaged = true;
                    break;
                }
            }
        }
        assert!(!saw_damaged, "script on_hit blocked the >100 damage hit");
    };

    run_with_server(server, client).await;
}
