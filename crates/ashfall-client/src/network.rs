//! Client UDP networking — connect, send, recv, poll.

use ashfall_core::protocol::{Channel, Packet};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

const HEADER_SIZE: usize = 3; // [2B len][1B channel]

/// Client network manager.
pub struct ClientNetwork {
    socket: UdpSocket,
    server_addr: SocketAddr,
    send_seq: u16,
}

impl ClientNetwork {
    pub async fn connect(server_addr: SocketAddr) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(server_addr).await?;
        tracing::info!("Connected to server {server_addr}");
        Ok(ClientNetwork {
            socket,
            server_addr,
            send_seq: 0,
        })
    }

    /// Send a packet. Auto-selects reliable/unreliable channel.
    pub async fn send(&mut self, packet: &Packet) -> anyhow::Result<()> {
        let channel = Channel::from_packet(packet);
        let payload = postcard::to_stdvec(packet)?;
        let is_unreliable = Channel::is_unreliable(packet);

        let mut buf = Vec::with_capacity(HEADER_SIZE + 2 + payload.len());
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);

        if is_unreliable {
            buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        } else {
            buf.extend_from_slice(&((payload.len() + 2) as u16).to_le_bytes());
        }
        buf.push(channel as u8);
        if !is_unreliable {
            buf.extend_from_slice(&seq.to_le_bytes());
        }
        buf.extend_from_slice(&payload);

        self.socket.send(&buf).await?;
        Ok(())
    }

    /// Poll for incoming packets. Returns available packets.
    pub async fn poll(&mut self) -> anyhow::Result<Vec<Packet>> {
        let mut buf = vec![0u8; 65536];
        let mut packets = Vec::new();
        // ponytail: single recv per poll. Batch in production.
        match self.recv(&mut buf).await? {
            Some(p) => packets.push(p),
            None => {}
        }
        Ok(packets)
    }

    /// Receive a single deserialized packet.
    pub async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<Option<Packet>> {
        let len = self.socket.recv(buf).await?;
        if len < HEADER_SIZE {
            return Ok(None);
        }

        let length = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        if len < HEADER_SIZE + length {
            return Ok(None);
        }

        let payload = &buf[HEADER_SIZE..HEADER_SIZE + length];

        let packet_data = if payload.len() >= 2 {
            &payload[2..]
        } else {
            payload
        };

        postcard::from_bytes(packet_data).map(Some).map_err(|e| anyhow::anyhow!("{e}"))
    }

    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }
}
