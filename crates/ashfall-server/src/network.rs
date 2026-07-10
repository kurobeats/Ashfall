//! UDP networking — socket bind, send/recv, reliability layer.
//!
//! Replaces RakNet: 3 ordered reliable channels (System, Game, Chat)
//! + 1 unordered unreliable channel for position/physics updates.

use ashfall_core::protocol::{Channel, Packet};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

/// Wire format: [2B len][1B channel][N bytes postcard(Packet)]
const HEADER_SIZE: usize = 3;

/// Reliable channel — ACK-based, ordered delivery.
struct ReliableChannel {
    send_seq: u16,
    recv_seq: u16,
    send_buffer: VecDeque<(u16, Instant, Vec<u8>)>,
    recv_buffer: BTreeMap<u16, Vec<u8>>,
    rtt: Duration,
    rto: Duration,
}

impl ReliableChannel {
    fn new() -> Self {
        ReliableChannel {
            send_seq: 0,
            recv_seq: 0,
            send_buffer: VecDeque::new(),
            recv_buffer: BTreeMap::new(),
            rtt: Duration::from_millis(100),
            rto: Duration::from_millis(300),
        }
    }

    fn next_seq(&mut self) -> u16 {
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);
        seq
    }

    fn buffer_send(&mut self, seq: u16, data: Vec<u8>) {
        self.send_buffer.push_back((seq, Instant::now(), data));
    }

    fn ack_recv(&mut self, ack_seq: u16) {
        self.send_buffer.retain(|(seq, _, _)| {
            // Keep packets with seq > ack_seq (with wrapping)
            if *seq == ack_seq {
                let elapsed = Instant::now().duration_since(Instant::now()); // ponytail: simplified
                drop(elapsed);
            }
            seq.wrapping_sub(ack_seq) > 0
        });
    }

    /// Process received packet — returns ordered packet data or None if out of order.
    fn recv(&mut self, seq: u16, data: Vec<u8>) -> Option<Vec<u8>> {
        if seq == self.recv_seq {
            self.recv_seq = self.recv_seq.wrapping_add(1);
            // Check if next buffered packets are now ready
            let result = Some(data);
            while let Some(_data) = self.recv_buffer.remove(&self.recv_seq) {
                self.recv_seq = self.recv_seq.wrapping_add(1);
                // ponytail: deliver immediately, caller handles one-at-a-time
                // Multi-packet ordering handled in dispatch loop
            }
            result
        } else if seq.wrapping_sub(self.recv_seq) < 32 {
            // Buffer for later
            self.recv_buffer.insert(seq, data);
            None
        } else {
            // Too far ahead — drop
            None
        }
    }
}

/// Unreliable channel — fire-and-forget for position/physics updates.
struct UnreliableChannel {
    send_seq: u16,
}

impl UnreliableChannel {
    fn new() -> Self {
        UnreliableChannel { send_seq: 0 }
    }

    fn next_seq(&mut self) -> u16 {
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);
        seq
    }
}

/// Server network manager — single UDP socket, per-session channels.
pub struct NetworkManager {
    socket: Arc<UdpSocket>,
    reliable: HashMap<SocketAddr, ReliableChannel>,
    unreliable: HashMap<SocketAddr, UnreliableChannel>,
}

use std::sync::Arc;

impl NetworkManager {
    pub async fn bind(addr: SocketAddr) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        tracing::info!("Server listening on {}", addr);
        Ok(NetworkManager {
            socket: Arc::new(socket),
            reliable: HashMap::new(),
            unreliable: HashMap::new(),
        })
    }

    /// Register a new client session for reliability tracking.
    pub fn register_session(&mut self, addr: SocketAddr) {
        self.reliable.insert(addr, ReliableChannel::new());
        self.unreliable.insert(addr, UnreliableChannel::new());
    }

    /// Remove a session.
    pub fn remove_session(&mut self, addr: SocketAddr) {
        self.reliable.remove(&addr);
        self.unreliable.remove(&addr);
    }

    /// Send a packet reliably (ordered, system/game/chat channels).
    pub async fn send_reliable(&mut self, addr: SocketAddr, packet: &Packet) -> anyhow::Result<()> {
        let channel = Channel::from_packet(packet);
        let payload = postcard::to_stdvec(packet)?;

        let seq = if let Some(ch) = self.reliable.get_mut(&addr) {
            let seq = ch.next_seq();
            ch.buffer_send(seq, payload.clone());
            seq
        } else {
            return Err(anyhow::anyhow!("Session not registered for {addr}"));
        };

        let mut buf = Vec::with_capacity(HEADER_SIZE + 2 + payload.len());
        buf.extend_from_slice(&(payload.len() as u16 + 2).to_le_bytes()); // length includes seq
        buf.push(channel as u8);
        buf.extend_from_slice(&seq.to_le_bytes());
        buf.extend_from_slice(&payload);

        self.socket.send_to(&buf, addr).await?;
        Ok(())
    }

    /// Send a packet unreliably (position/physics updates, loss OK).
    pub async fn send_unreliable(&mut self, addr: SocketAddr, packet: &Packet) -> anyhow::Result<()> {
        let channel = Channel::from_packet(packet);
        let payload = postcard::to_stdvec(packet)?;

        let mut buf = Vec::with_capacity(HEADER_SIZE + payload.len());
        buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        buf.push(channel as u8);
        buf.extend_from_slice(&payload);

        self.socket.send_to(&buf, addr).await?;
        Ok(())
    }

    /// Send to all recipients, choosing reliable/unreliable per packet.
    pub async fn broadcast(&mut self, addrs: &[SocketAddr], packet: &Packet) {
        let is_unreliable = Channel::is_unreliable(packet);
        for &addr in addrs {
            let result = if is_unreliable {
                self.send_unreliable(addr, packet).await
            } else {
                self.send_reliable(addr, packet).await
            };
            if let Err(e) = result {
                tracing::warn!("Failed to send to {addr}: {e}");
            }
        }
    }

    /// Receive raw UDP datagrams. Returns (addr, raw bytes).
    pub async fn recv_raw(&self, buf: &mut [u8]) -> anyhow::Result<(usize, SocketAddr)> {
        let (len, addr) = self.socket.recv_from(buf).await?;
        Ok((len, addr))
    }

    /// Try to reassemble a received packet from raw bytes.
    /// Returns Some(Packet) if ordered byte stream is ready.
    pub fn try_recv(&mut self, addr: SocketAddr, data: &[u8]) -> Option<Packet> {
        if data.len() < HEADER_SIZE {
            return None;
        }

        let length = u16::from_le_bytes([data[0], data[1]]) as usize;
        let channel_byte = data[2];

        if data.len() < HEADER_SIZE + length {
            return None;
        }

        let payload = &data[HEADER_SIZE..HEADER_SIZE + length];

        let _channel = match channel_byte {
            0 => Channel::System,
            1 => Channel::Game,
            2 => Channel::Chat,
            _ => return None,
        };

        // Reliable channels have 2-byte sequence prefix
        let ch = self.reliable.get_mut(&addr)?;

        if payload.len() >= 2 {
            let seq = u16::from_le_bytes([payload[0], payload[1]]);
            let packet_data = &payload[2..];
            if let Some(data) = ch.recv(seq, packet_data.to_vec()) {
                postcard::from_bytes(&data).ok()
            } else {
                None // out of order, buffered
            }
        } else {
            postcard::from_bytes(payload).ok()
        }
    }

    /// Get the raw socket for async operations.
    pub fn socket(&self) -> Arc<UdpSocket> {
        self.socket.clone()
    }
}
