//! End-to-end test with the REAL compiled freeroam WASM game mode
//! (scripts/freeroam, built with `cargo build --target wasm32-unknown-unknown`).
//!
//! Proves the whole stack with a Rust-compiled module (not a hand-written WAT):
//!   on_server_init sets weather/time, on_client_authenticate gates names,
//!   on_spawn private-chats the player.

use ashfall_core::protocol::transport::CHANNEL_RELIABLE_FLAG;
use ashfall_core::protocol::Packet;
use ashfall_server::config::{DatabaseSection, ScriptSection, ServerConfig, ServerSection};
use ashfall_server::dedicated::DedicatedServer;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::net::UdpSocket;

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Path to the built freeroam module. Skips the test when not built yet.
fn freeroam_wasm() -> Option<std::path::PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scripts/freeroam/target/wasm32-unknown-unknown/release/ashfall_freeroam.wasm");
    p.exists().then_some(p)
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
        buf.push(CHANNEL_RELIABLE_FLAG);
        buf.extend_from_slice(&seq_bytes);
        buf.extend_from_slice(&payload);
        self.sock.send(&buf).await.unwrap();
    }

    async fn recv_packet(&self) -> Option<Packet> {
        let mut buf = vec![0u8; 2048];
        let n = tokio::time::timeout(Duration::from_millis(400), self.sock.recv(&mut buf))
            .await.ok()?.ok()?;
        let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        let ch = buf[2];
        if ch == 0xFF {
            return None;
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
}

async fn boot() -> (DedicatedServer, u16) {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let wasm = freeroam_wasm().expect("freeroam.wasm built — run: cd scripts/freeroam && cargo build --release --target wasm32-unknown-unknown");
    let dir = std::env::temp_dir().join(format!("ashfall_freeroam_scripts_{}_{}", std::process::id(), seq));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(&wasm, dir.join("freeroam.wasm")).unwrap();

    let db = std::env::temp_dir().join(format!("ashfall_freeroam_db_{}_{}.sqlite3", std::process::id(), seq));
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

#[tokio::test]
async fn test_real_freeroam_module_end_to_end() {
    let (server, port) = boot().await;

    let client = async {
        let mut sock = TestClient::connect(port).await;

        // Reject an empty name (freeroam auth gate)
        sock.send_reliable(&Packet::GameAuth { name: String::new(), password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        }).await;
        let mut saw_end = false;
        for _ in 0..8 {
            if let Some(Packet::GameEnd { .. }) = sock.recv_packet().await {
                saw_end = true;
                break;
            }
        }
        assert!(saw_end, "empty name rejected by freeroam auth");

        // Accept a normal name; script-set weather + spawn welcome must arrive
        sock.send_reliable(&Packet::GameAuth { name: "Wanderer".into(), password: String::new(),
            version: ashfall_core::constants::DEDICATED_VERSION.into(),
        }).await;
        let mut saw_load = false;
        let mut saw_weather = false;
        let mut saw_welcome = false;
        for _ in 0..24 {
            if let Some(pkt) = sock.recv_packet().await {
                match pkt {
                    Packet::GameLoad => saw_load = true,
                    Packet::GameWeather { weather } => {
                        saw_weather = weather == 0x00015E5E; // freeroam on_server_init
                    }
                    Packet::GameChat { message } => {
                        use ashfall_core::string_cache::StringTable;
                        saw_welcome = message.resolve(&mut StringTable::new()) == "Welcome to the Wasteland!";
                    }
                    _ => {}
                }
            }
        }
        assert!(saw_load, "auth accepted -> GameLoad");
        assert!(saw_weather, "on_server_init weather (0x15E5E) reached the client");
        assert!(saw_welcome, "on_spawn welcome chat delivered");
    };

    run_with(server, client).await;
}
