//! Full coop-loop integration test: mock bridge → client A → server →
//! client B → mock bridge (commands applied to the "game").
//!
//! Proves the entire vanilla-coop pipeline without a real game: A's engine
//! event becomes server packets, the server relays to B, B's client turns
//! them into engine commands. This is the CI proxy for the Proton
//! integration test (a mock game stands in for the DLL).

use crate::config::ClientConfig;
use crate::game::Game;
use ashfall_core::event::{encode_player_state_event, PlayerStateEvent};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SEQ: AtomicU32 = AtomicU32::new(0);

fn free_port() -> u16 {
    let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

fn tcp_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    s.local_addr().unwrap().port()
}

fn test_config(port: u16, name: &str, ipc_port: u16) -> ClientConfig {
    ClientConfig {
        name: name.into(),
        server_addr: "127.0.0.1".into(),
        server_port: port,
        ipc_mode: "tcp".into(),
        ipc_port,
        ..Default::default()
    }
}

/// Spawn a mock game bridge: sends one player-state event on connect, then
/// answers every command with a canned RETURN. Captures the opcodes the
/// client sent (the "applied to the game" evidence). The listener is bound
/// synchronously (before the spawn) so connects never race the bind.
fn spawn_mock_bridge(port: u16, captured: Arc<Mutex<Vec<u32>>>) {
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .unwrap();
        let (mut stream, _) = listener.accept().await.unwrap();
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Player-state event for the local player (ref 0x14), sent on connect.
        let event = PlayerStateEvent {
            ref_id: 0x14,
            pos: [0.0, 0.0, 0.0], // zero move — anti-cheat-safe
            angle: [0.0, 0.0, 90.0],
            idle: 0,
            moving: 1,
            moving_xy: 0,
            weapon: 0x2A,
            alerted: false,
            sneaking: false,
            health: 88.0,
        };
        let _ = stream.write_all(&encode_player_state_event(&event)).await;

        // Answer commands; capture their opcodes.
        let mut buf = vec![0u8; 2048];
        let mut pending = Vec::new();
        loop {
            let n = match stream.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            pending.extend_from_slice(&buf[..n]);
            let (frames, rest) = ashfall_core::event::split_frames(&pending);
            pending = rest.to_vec();
            for f in frames {
                // Command frame payload: [key:4][opcode:4][count:1][params]
                if f.payload.len() >= 9 {
                    let key = u32::from_le_bytes([
                        f.payload[0],
                        f.payload[1],
                        f.payload[2],
                        f.payload[3],
                    ]);
                    let opcode = u32::from_le_bytes([
                        f.payload[4],
                        f.payload[5],
                        f.payload[6],
                        f.payload[7],
                    ]);
                    captured.lock().unwrap().push(opcode);
                    // Canned response: [RETURN][key][12 zero bytes] → Floats.
                    let mut resp = Vec::with_capacity(4 + 12);
                    resp.extend_from_slice(&key.to_le_bytes());
                    resp.extend_from_slice(&[0u8; 12]);
                    let frame =
                        ashfall_core::event::encode_frame(crate::ipc::PIPE_OP_RETURN, &resp);
                    let _ = stream.write_all(&frame).await;
                }
            }
        }
    });
}

/// Drive a client's packet loop until `f` is satisfied or timeout.
async fn drive_until<F>(game: &mut Game, mut f: F)
where
    F: FnMut(&Game) -> bool,
{
    for _ in 0..200 {
        if f(game) {
            return;
        }
        let poll = tokio::time::timeout(Duration::from_millis(50), game.poll());
        if let Ok(Ok(packets)) = poll.await {
            for pkt in packets {
                game.handle_packet(pkt);
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("drive_until timed out");
}

/// The whole client-side sequence (server runs concurrently via select!).
async fn run_clients(
    server_port: u16,
    bridge_a_port: u16,
    bridge_b_port: u16,
    captured_b: Arc<Mutex<Vec<u32>>>,
) {
    // Client A connects + authenticates; gets its player id.
    let mut a = Game::new(test_config(server_port, "alice", bridge_a_port));
    a.connect(SocketAddr::from(([127, 0, 0, 1], server_port)))
        .await
        .unwrap();
    a.authenticate().await.unwrap();
    drive_until(&mut a, |g| g.local_player_id.is_some()).await;

    // Client B connects + authenticates (so it exists before A broadcasts).
    let mut b = Game::new(test_config(server_port, "bob", bridge_b_port));
    b.connect(SocketAddr::from(([127, 0, 0, 1], server_port)))
        .await
        .unwrap();
    b.authenticate().await.unwrap();

    // A's bridge event → server packets (UpdatePos/Angle/State/Value) →
    // server relays to the now-ingame B.
    a.poll_bridge().await.unwrap();
    drive_until(&mut b, |g| g.local_player_id.is_some()).await;

    // B applies A's state to the (mock) game: wait for the commands.
    drive_until(&mut b, |g| {
        g.pending_commands
            .iter()
            .any(|(op, _)| *op == crate::ipc::OP_SET_POS)
            && g.pending_commands
                .iter()
                .any(|(op, _)| *op == crate::ipc::OP_SET_ACTOR_VALUE)
    })
    .await;
    b.flush_commands().await.unwrap();

    // The mock bridge B must have seen the commands.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let ops = captured_b.lock().unwrap().clone();
    assert!(
        ops.contains(&crate::ipc::OP_SET_POS),
        "B applied A's position to its game (ops: {ops:?})"
    );
    assert!(
        ops.contains(&crate::ipc::OP_SET_ACTOR_VALUE),
        "B applied A's health to its game"
    );
}

async fn boot_server(port: u16) -> ashfall_server::dedicated::DedicatedServer {
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "ashfall_loop_scripts_{}_{}",
        std::process::id(),
        seq
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let db = std::env::temp_dir().join(format!(
        "ashfall_loop_db_{}_{}.sqlite3",
        std::process::id(),
        seq
    ));
    let config = ashfall_server::config::ServerConfig {
        server: ashfall_server::config::ServerSection {
            host: "127.0.0.1".into(),
            port,
            connections: 8,
            announce: "127.0.0.1".into(),
            master_port: free_port(),
            game_type: "fo3".into(),
            pvp_enabled: true,
            mods: Vec::new(),
        },
        scripts: ashfall_server::config::ScriptSection { path: dir },
        database: ashfall_server::config::DatabaseSection { path: db },
        ..Default::default()
    };
    let server = ashfall_server::dedicated::DedicatedServer::new(config)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    server
}

#[tokio::test]
async fn test_full_coop_loop_bridge_to_bridge() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();
    let server_port = free_port();
    let server = boot_server(server_port).await;

    // Two mock bridges: A's events drive the loop, B's commands get captured.
    // Reserve ports by binding + dropping a std listener, then let the
    // tokio bridge tasks bind them; the sleep warms the tasks up so the
    // clients never connect before the bridges listen.
    let bridge_a_port = tcp_port();
    let bridge_b_port = tcp_port();
    let captured_b = Arc::new(Mutex::new(Vec::new()));
    spawn_mock_bridge(bridge_a_port, Arc::new(Mutex::new(Vec::new())));
    spawn_mock_bridge(bridge_b_port, captured_b.clone());
    tokio::time::sleep(Duration::from_millis(100)).await;

    tokio::select! {
        _ = server.run() => panic!("server exited unexpectedly"),
        _ = run_clients(server_port, bridge_a_port, bridge_b_port, captured_b) => {}
    }
}
