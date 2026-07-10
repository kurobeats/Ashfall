//! Master server — server browser registry.
//!
//! Receives MasterAnnounce heartbeats from dedicated servers.
//! Responds to MasterQuery from clients with current server list.

mod server_list;

use ashfall_core::protocol::Packet;
use server_list::ServerList;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::interval;

const HEADER_SIZE: usize = 3;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Ashfall master server v{}", ashfall_core::constants::MASTER_VERSION);

    let addr: SocketAddr = "0.0.0.0:1660".parse()?;
    let socket = UdpSocket::bind(addr).await?;
    tracing::info!("Master server listening on {}", addr);

    let mut servers = ServerList::new();
    let mut tick = interval(Duration::from_secs(60));
    let mut buf = vec![0u8; 65536];

    loop {
        tokio::select! {
            _ = tick.tick() => {
                servers.cull_stale();
            }
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, src)) => {
                        if let Some(packet) = decode_packet(&buf[..len]) {
                            handle_packet(&socket, &mut servers, src, packet).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Recv error: {e}");
                    }
                }
            }
        }
    }
}

/// Decode a packet from UDP wire format.
fn decode_packet(data: &[u8]) -> Option<Packet> {
    if data.len() < HEADER_SIZE {
        return None;
    }
    let _length = u16::from_le_bytes([data[0], data[1]]) as usize;
    let _channel = data[2];
    let payload = &data[HEADER_SIZE..];
    postcard::from_bytes(payload).ok()
}

/// Handle an incoming master protocol packet.
async fn handle_packet(
    socket: &UdpSocket,
    servers: &mut ServerList,
    src: SocketAddr,
    packet: Packet,
) {
    match packet {
        Packet::MasterAnnounce { name, map, players, max_players, mod_files, game_type, .. } => {
            servers.upsert(src, name, map, players, max_players, game_type, mod_files);
        }
        Packet::MasterUpdate { name, map, players, max_players } => {
            if max_players == 0 {
                servers.remove(src);
            } else {
                servers.upsert(src, name, map, players, max_players, String::new(), vec![]);
            }
        }
        Packet::MasterQuery => {
            let entries = servers.all();
            for entry in entries {
                let announce = Packet::MasterAnnounce {
                    name: entry.name.clone(),
                    map: entry.map.clone(),
                    players: entry.players,
                    max_players: entry.max_players,
                    rules: std::collections::HashMap::new(),
                    mod_files: entry.mod_files.clone(),
                    game_type: entry.game_type.clone(),
                };
                if let Ok(payload) = postcard::to_stdvec(&announce) {
                    let mut buf = Vec::with_capacity(3 + payload.len());
                    buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
                    buf.push(0u8);
                    buf.extend_from_slice(&payload);
                    let _ = socket.send_to(&buf, src).await;
                }
            }
        }
        _ => {}
    }
}
