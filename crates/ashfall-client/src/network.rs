//! Client UDP networking — connect, send, recv, poll.
//!
//! Matches the server's framing: reliable frames carry `0x80 | channel` and a
//! varint sequence number; unreliable frames are bare channel + postcard.
//! The client ACKs received reliable packets so the server can drop its send
//! buffer and measure RTT. Client-side retransmission is deferred (the
//! server-driven model works for MVP).

use ashfall_core::protocol::transport::{
    decode_varint_seq, encode_ctrl_ack, encode_varint_seq, CHANNEL_CTRL, CHANNEL_RELIABLE_FLAG,
};
use ashfall_core::protocol::{Channel, Packet};
use std::collections::VecDeque;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

const HEADER_SIZE: usize = 3; // [2B len][1B channel]

/// Client network manager.
pub struct ClientNetwork {
    socket: UdpSocket,
    server_addr: SocketAddr,
    send_seq: u16,
    /// Highest reliable sequence received (cumulative ACK target).
    recv_seq: Option<u16>,
    /// ACK frames queued to send.
    ack_queue: VecDeque<Vec<u8>>,
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
            recv_seq: None,
            ack_queue: VecDeque::new(),
        })
    }

    /// Send a packet. Auto-selects reliable/unreliable channel and flushes
    /// any queued ACK frames first.
    pub async fn send(&mut self, packet: &Packet) -> anyhow::Result<()> {
        self.flush_acks().await?;

        let channel = Channel::from_packet(packet);
        let payload = postcard::to_stdvec(packet)?;

        let mut buf = Vec::with_capacity(HEADER_SIZE + 3 + payload.len());
        let is_unreliable = Channel::is_unreliable(packet);

        if is_unreliable {
            buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            buf.push(channel as u8);
        } else {
            // Only reliable sends consume the reliable sequence space —
            // unreliable frames carry no seq, so bumping here would leave
            // holes the server's reassembly stalls on.
            let seq = self.send_seq;
            self.send_seq = self.send_seq.wrapping_add(1);
            let seq_bytes = encode_varint_seq(seq);
            buf.extend_from_slice(&((seq_bytes.len() + payload.len()) as u16).to_le_bytes());
            buf.push(CHANNEL_RELIABLE_FLAG | channel as u8);
            buf.extend_from_slice(&seq_bytes);
        }
        buf.extend_from_slice(&payload);

        self.socket.send(&buf).await?;
        Ok(())
    }

    /// Send any queued ACK control frames.
    async fn flush_acks(&mut self) -> anyhow::Result<()> {
        while let Some(frame) = self.ack_queue.pop_front() {
            self.socket.send(&frame).await?;
        }
        Ok(())
    }

    /// Poll for incoming packets. Returns available packets.
    pub async fn poll(&mut self) -> anyhow::Result<Vec<Packet>> {
        let mut buf = vec![0u8; 65536];
        let mut packets = Vec::new();
        // ponytail: single recv per poll. Batch in production.
        if let Some(p) = self.recv(&mut buf).await? {
            packets.push(p)
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

        let channel_byte = buf[2];
        let payload = &buf[HEADER_SIZE..HEADER_SIZE + length];

        // Control frames (server ACK/NACK): nothing for the client to do —
        // it has no send buffer yet. (NACK handling arrives with client-side
        // retransmission.)
        if channel_byte == CHANNEL_CTRL {
            return Ok(None);
        }

        let packet_data = if channel_byte & CHANNEL_RELIABLE_FLAG != 0 {
            // Reliable: parse varint seq, queue a cumulative ACK
            let (seq, consumed) = decode_varint_seq(payload).ok_or_else(|| {
                anyhow::anyhow!("malformed reliable frame from {}", self.server_addr)
            })?;
            self.recv_seq = Some(seq);
            self.ack_queue.push_back(encode_ctrl_ack(seq));
            &payload[consumed..]
        } else {
            payload
        };

        postcard::from_bytes(packet_data)
            .map(Some)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// The local UDP socket address (used by tests and diagnostics).
    #[allow(dead_code)]
    pub fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// The server address this client is connected to.
    #[allow(dead_code)]
    pub fn server_addr(&self) -> SocketAddr {
        self.server_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashfall_core::protocol::transport::{
        decode_ctrl_frame, decode_varint_seq, encode_varint_seq, CtrlFrame, CHANNEL_RELIABLE_FLAG,
    };

    fn chat(message: &str) -> Packet {
        Packet::GameChat {
            message: message.into(),
        }
    }

    /// Parse a wire frame: returns (channel_byte, reliable_seq, owned payload).
    fn parse_frame(data: &[u8]) -> (u8, Option<u16>, Vec<u8>) {
        let length = u16::from_le_bytes([data[0], data[1]]) as usize;
        let channel = data[2];
        let payload = &data[3..3 + length];
        if channel & CHANNEL_RELIABLE_FLAG != 0 {
            let (seq, consumed) = decode_varint_seq(payload).expect("varint seq");
            (channel, Some(seq), payload[consumed..].to_vec())
        } else {
            (channel, None, payload.to_vec())
        }
    }

    /// Build a server-style reliable frame (varint seq + postcard payload).
    fn encode_reliable_frame(seq: u16, payload: &[u8]) -> Vec<u8> {
        let seq_bytes = encode_varint_seq(seq);
        let mut buf = Vec::new();
        buf.extend_from_slice(&((seq_bytes.len() + payload.len()) as u16).to_le_bytes());
        buf.push(CHANNEL_RELIABLE_FLAG);
        buf.extend_from_slice(&seq_bytes);
        buf.extend_from_slice(payload);
        buf
    }

    #[test]
    fn test_client_reliable_send_framing() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Peer (simulated server) socket
            let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let peer_addr = peer.local_addr().unwrap();

            let mut client = ClientNetwork::connect(peer_addr).await.unwrap();
            client.send(&chat("ping")).await.unwrap();

            let mut buf = vec![0u8; 2048];
            let len = peer.recv(&mut buf).await.unwrap();
            let (channel, seq, payload) = parse_frame(&buf[..len]);

            assert_eq!(channel & CHANNEL_RELIABLE_FLAG, CHANNEL_RELIABLE_FLAG);
            assert_eq!(seq, Some(0));
            let decoded: Packet = postcard::from_bytes(&payload).unwrap();
            assert_eq!(decoded, chat("ping"));
        });
    }

    #[test]
    fn test_client_acks_received_reliable() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let peer_addr = peer.local_addr().unwrap();

            let mut client = ClientNetwork::connect(peer_addr).await.unwrap();

            // Peer sends a reliable frame (seq 7) to the client's local addr
            peer.connect(client.local_addr().unwrap()).await.unwrap();
            let payload = postcard::to_stdvec(&chat("server message")).unwrap();
            let frame = encode_reliable_frame(7, &payload);
            peer.send(&frame).await.unwrap();

            let mut buf = vec![0u8; 2048];
            let pkt = client.recv(&mut buf).await.unwrap();
            assert_eq!(pkt, Some(chat("server message")));

            // Client's next send flushes the queued ACK
            client.send(&chat("reply")).await.unwrap();

            // Peer sees the ACK first, then the reply
            let mut buf = vec![0u8; 2048];
            let _len = peer.recv(&mut buf).await.unwrap();
            assert_eq!(buf[2], CHANNEL_CTRL, "first frame is the ACK");
            let decoded =
                decode_ctrl_frame(&buf[3..3 + u16::from_le_bytes([buf[0], buf[1]]) as usize])
                    .unwrap();
            assert_eq!(decoded, CtrlFrame::Ack(7));

            let len = peer.recv(&mut buf).await.unwrap();
            let (_, seq, payload) = parse_frame(&buf[..len]);
            assert_eq!(seq, Some(0), "reply is the client's first reliable send");
            let decoded: Packet = postcard::from_bytes(&payload).unwrap();
            assert_eq!(decoded, chat("reply"));
        });
    }

    #[test]
    fn test_client_unreliable_send_framing() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let peer_addr = peer.local_addr().unwrap();

            let mut client = ClientNetwork::connect(peer_addr).await.unwrap();
            let pos = Packet::UpdatePos {
                id: 5.into(),
                pos: [1.0, 2.0, 3.0],
            };
            client.send(&pos).await.unwrap();

            let mut buf = vec![0u8; 2048];
            let len = peer.recv(&mut buf).await.unwrap();
            let (channel, seq, payload) = parse_frame(&buf[..len]);

            assert_eq!(channel & CHANNEL_RELIABLE_FLAG, 0, "no reliable flag");
            assert_eq!(seq, None, "no seq on unreliable frames");
            let decoded: Packet = postcard::from_bytes(&payload).unwrap();
            assert_eq!(decoded, pos);
        });
    }
}
