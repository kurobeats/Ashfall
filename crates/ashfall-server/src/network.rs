//! UDP networking — socket bind, send/recv, reliability layer.
//!
//! Replaces RakNet: 3 ordered reliable channels (System, Game, Chat)
//! + 1 unordered unreliable channel for position/physics updates.
//!
//! Reliability: ACK-based cumulative acknowledgment with Jacobson/Karels
//! RTT estimation, exponential-backoff retransmission, a bounded send window,
//! NACK fast-retransmit on sequence gaps, and a per-address token-bucket
//! rate limiter. Frame layout and control frames live in
//! `ashfall_core::protocol::transport`.

use ashfall_core::protocol::transport::{
    decode_ctrl_frame, decode_varint_seq, encode_ctrl_ack, encode_ctrl_nack, encode_varint_seq,
    CtrlFrame, CHANNEL_CTRL, CHANNEL_RELIABLE_FLAG,
};
use ashfall_core::protocol::{Channel, Packet};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

/// Wire format: [2B len][1B channel][payload]
const HEADER_SIZE: usize = 3;

/// Maximum in-flight (unacknowledged) reliable packets per session.
const MAX_INFLIGHT: usize = 32;

/// Reassembly window: packets more than this many seqs ahead are dropped.
const RECV_WINDOW: u16 = 32;

/// RTO floor / ceiling (Jacobson/Karels, clamped).
const RTO_MIN: Duration = Duration::from_millis(100);
const RTO_MAX: Duration = Duration::from_millis(3000);

/// Exponential backoff cap (2^6 = 64× base RTO).
const MAX_BACKOFF: u32 = 6;

/// One buffered outbound reliable packet.
struct SendEntry {
    seq: u16,
    channel: Channel,
    sent_at: Instant,
    data: Vec<u8>,
    retransmits: u32,
}

/// Reliable channel — ACK-based, ordered delivery, per-channel priority queues.
///
/// Send queues are indexed by `Channel as usize` (System=0, Game=1, Chat=2)
/// so retransmission drains System traffic first, then Game, then Chat.
struct ReliableChannel {
    send_seq: u16,
    recv_seq: u16,
    send_queues: [VecDeque<SendEntry>; 3],
    recv_buffer: BTreeMap<u16, Vec<u8>>,
    /// In-order packets awaiting delivery (reassembled runs, drained by `take_ready`).
    ready_queue: VecDeque<Vec<u8>>,
    ack_pending: bool,
    nack_pending: bool,
    pending_nacks: Vec<u16>,
    /// Smoothed RTT (Jacobson/Karels).
    srtt: Duration,
    /// RTT variance.
    rttvar: Duration,
    /// Retransmission timeout (srtt + 4*rttvar, clamped).
    rto: Duration,
}

impl ReliableChannel {
    fn new() -> Self {
        ReliableChannel {
            send_seq: 0,
            recv_seq: 0,
            send_queues: [VecDeque::new(), VecDeque::new(), VecDeque::new()],
            recv_buffer: BTreeMap::new(),
            ready_queue: VecDeque::new(),
            ack_pending: false,
            nack_pending: false,
            pending_nacks: Vec::new(),
            srtt: Duration::from_millis(100),
            rttvar: Duration::from_millis(50),
            rto: Duration::from_millis(300),
        }
    }

    fn next_seq(&mut self) -> u16 {
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);
        seq
    }

    /// Send-window guard: refuse to buffer more than `MAX_INFLIGHT` packets.
    fn can_send(&self) -> bool {
        self.send_queues.iter().map(|q| q.len()).sum::<usize>() < MAX_INFLIGHT
    }

    fn enqueue(&mut self, channel: Channel, seq: u16, data: Vec<u8>) {
        let entry = SendEntry { seq, channel, sent_at: Instant::now(), data, retransmits: 0 };
        self.send_queues[channel as usize].push_back(entry);
    }

    /// Build a reliable wire frame for one buffered entry (resend path).
    fn resend_frame(entry: &SendEntry) -> Vec<u8> {
        let seq_bytes = encode_varint_seq(entry.seq);
        let mut buf = Vec::with_capacity(HEADER_SIZE + seq_bytes.len() + entry.data.len());
        buf.extend_from_slice(&((seq_bytes.len() + entry.data.len()) as u16).to_le_bytes());
        buf.push(CHANNEL_RELIABLE_FLAG | entry.channel as u8);
        buf.extend_from_slice(&seq_bytes);
        buf.extend_from_slice(&entry.data);
        buf
    }

    /// Retransmission timeout for an entry (exponential backoff on retransmits).
    fn timeout_for(&self, entry: &SendEntry) -> Duration {
        self.rto.saturating_mul(2u32.saturating_pow(entry.retransmits.min(MAX_BACKOFF)))
    }

    /// Collect packets whose RTO has elapsed. Updates `sent_at` and the
    /// per-entry retransmit counter (exponential backoff).
    /// Returns wire frames to resend.
    fn retransmit_expired(&mut self, now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let base_rto = self.rto;
        // Drain System first (weight 4), then Game (2), then Chat (1)
        for queue in &mut self.send_queues {
            for entry in queue.iter_mut() {
                let timeout =
                    base_rto.saturating_mul(2u32.saturating_pow(entry.retransmits.min(MAX_BACKOFF)));
                if now.duration_since(entry.sent_at) >= timeout {
                    entry.sent_at = now;
                    entry.retransmits += 1;
                    out.push(Self::resend_frame(entry));
                }
            }
        }
        out
    }

    /// Immediately retransmit specific sequence numbers (NACK fast retransmit).
    /// Returns wire frames to resend.
    fn nack_retransmit(&mut self, missing: &[u16], now: Instant) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for queue in &mut self.send_queues {
            for entry in queue.iter_mut() {
                if missing.contains(&entry.seq) {
                    entry.sent_at = now;
                    entry.retransmits += 1;
                    out.push(Self::resend_frame(entry));
                }
            }
        }
        out
    }

    /// Process a cumulative ACK: sample RTT from the acked entry and drop
    /// everything at or before `ack_seq` (wrapping half-range comparison).
    fn ack_recv(&mut self, ack_seq: u16) {
        let sample = self
            .send_queues
            .iter()
            .flatten()
            .find(|e| e.seq == ack_seq)
            .map(|e| Instant::now().duration_since(e.sent_at));
        if let Some(sample) = sample {
            self.update_rtt(sample);
        }
        for queue in &mut self.send_queues {
            queue.retain(|e| {
                // Keep only seqs strictly after ack_seq in the 16-bit space
                e.seq != ack_seq && e.seq.wrapping_sub(ack_seq) < 0x8000
            });
        }
    }

    /// Process received packet: deliver in-order runs via the ready queue,
    /// buffer out-of-order packets within the window and NACK the gaps.
    fn recv(&mut self, seq: u16, data: Vec<u8>) {
        if seq == self.recv_seq {
            self.recv_seq = self.recv_seq.wrapping_add(1);
            self.ack_pending = true;
            self.ready_queue.push_back(data);
            // Drain any now-contiguous buffered packets (order preserved)
            while let Some(d) = self.recv_buffer.remove(&self.recv_seq) {
                self.recv_seq = self.recv_seq.wrapping_add(1);
                self.ready_queue.push_back(d);
            }
        } else if seq.wrapping_sub(self.recv_seq) < RECV_WINDOW {
            // Out of order but within window: buffer, NACK the gaps
            self.recv_buffer.insert(seq, data);
            let gap = seq.wrapping_sub(self.recv_seq);
            for i in 0..gap {
                let missing = self.recv_seq.wrapping_add(i);
                if !self.recv_buffer.contains_key(&missing) && !self.pending_nacks.contains(&missing)
                {
                    self.pending_nacks.push(missing);
                }
            }
            self.nack_pending = true;
        }
        // Too far ahead — dropped
    }

    /// Pop the next in-order reassembled packet, if any.
    fn take_ready(&mut self) -> Option<Vec<u8>> {
        self.ready_queue.pop_front()
    }

    /// Take pending ACK/NACK control frames to send on the wire.
    fn take_control_frames(&mut self) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        if self.ack_pending {
            let ack_seq = self.recv_seq.wrapping_sub(1);
            frames.push(encode_ctrl_ack(ack_seq));
            self.ack_pending = false;
        }
        if self.nack_pending && !self.pending_nacks.is_empty() {
            frames.push(encode_ctrl_nack(&self.pending_nacks));
            self.pending_nacks.clear();
            self.nack_pending = false;
        }
        frames
    }

    /// Jacobson/Karels RTT estimation:
    /// ```text
    /// srtt   = srtt   + 0.125 * (sample - srtt)
    /// rttvar = rttvar + 0.25  * (|sample - srtt| - rttvar)
    /// rto    = srtt + 4*rttvar, clamped to [RTO_MIN, RTO_MAX]
    /// ```
    fn update_rtt(&mut self, sample: Duration) {
        let sample_ms = sample.as_millis() as i64;
        let srtt_ms = self.srtt.as_millis() as i64;
        let rttvar_ms = self.rttvar.as_millis() as i64;

        let new_srtt = srtt_ms + (sample_ms - srtt_ms) / 8;
        let diff = (sample_ms - srtt_ms).abs();
        let new_rttvar = rttvar_ms + (diff - rttvar_ms) / 4;

        let rto_ms = new_srtt + 4 * new_rttvar;
        self.srtt = Duration::from_millis(new_srtt.max(0) as u64);
        self.rttvar = Duration::from_millis(new_rttvar.max(0) as u64);
        self.rto = Duration::from_millis(rto_ms.max(0) as u64)
            .clamp(RTO_MIN, RTO_MAX);
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

/// Token-bucket rate limiter (per address).
pub struct RateLimiter {
    tokens: f64,
    last_refill: Instant,
    max_tokens: f64,
    rate: f64,
}

impl RateLimiter {
    /// `rate` tokens/sec, `burst` max tokens.
    pub fn new(rate: f64, burst: f64) -> Self {
        RateLimiter { tokens: burst, last_refill: Instant::now(), max_tokens: burst, rate }
    }

    /// Consume one token. Returns false (drop) when the bucket is empty.
    pub fn check_rate(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate).min(self.max_tokens);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Server network manager — single UDP socket, per-session channels.
pub struct NetworkManager {
    socket: Arc<UdpSocket>,
    reliable: HashMap<SocketAddr, ReliableChannel>,
    unreliable: HashMap<SocketAddr, UnreliableChannel>,
    rate_limiters: HashMap<SocketAddr, RateLimiter>,
    /// Frames queued for immediate resend (NACK fast retransmit), drained in tick.
    resend_queue: VecDeque<(SocketAddr, Vec<u8>)>,
}

impl NetworkManager {
    pub async fn bind(addr: SocketAddr) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        tracing::info!("Server listening on {}", addr);
        Ok(NetworkManager {
            socket: Arc::new(socket),
            reliable: HashMap::new(),
            unreliable: HashMap::new(),
            rate_limiters: HashMap::new(),
            resend_queue: VecDeque::new(),
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
        self.rate_limiters.remove(&addr);
    }

    /// Token-bucket gate for an address (default 200 pkt/s, burst 100).
    /// Call on every raw datagram before processing; drop silently when false.
    pub fn check_rate(&mut self, addr: SocketAddr) -> bool {
        let limiter = self
            .rate_limiters
            .entry(addr)
            .or_insert_with(|| RateLimiter::new(200.0, 100.0));
        limiter.check_rate()
    }

    /// Send a packet reliably (ordered, system/game/chat channels).
    pub async fn send_reliable(&mut self, addr: SocketAddr, packet: &Packet) -> anyhow::Result<()> {
        let channel = Channel::from_packet(packet);
        let payload = postcard::to_stdvec(packet)?;

        let ch = match self.reliable.get_mut(&addr) {
            Some(ch) => ch,
            None => return Err(anyhow::anyhow!("Session not registered for {addr}")),
        };

        // Send-window throttle
        if !ch.can_send() {
            return Err(anyhow::anyhow!(
                "Send window full for {addr} ({} in flight)",
                MAX_INFLIGHT
            ));
        }

        let seq = ch.next_seq();
        let seq_bytes = encode_varint_seq(seq);
        ch.enqueue(channel, seq, payload.clone());

        let mut buf = Vec::with_capacity(HEADER_SIZE + seq_bytes.len() + payload.len());
        buf.extend_from_slice(&((seq_bytes.len() + payload.len()) as u16).to_le_bytes());
        buf.push(CHANNEL_RELIABLE_FLAG | channel as u8);
        buf.extend_from_slice(&seq_bytes);
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
    /// Control frames (ACK/NACK) are processed in place; data frames are
    /// reassembled through the reliability layer.
    /// Returns Some(Packet) if an ordered byte stream is ready.
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

        // Control frames carry no packet
        if channel_byte == CHANNEL_CTRL {
            self.handle_ctrl_frame(addr, payload);
            return None;
        }

        // Unreliable: bare channel byte, no sequence number
        if channel_byte & CHANNEL_RELIABLE_FLAG == 0 {
            return postcard::from_bytes(payload).ok();
        }

        // Reliable: parse varint seq, reassemble
        let channel = channel_byte & !CHANNEL_RELIABLE_FLAG;
        if channel > 2 {
            return None;
        }
        let ch = self.reliable.get_mut(&addr)?;
        let (seq, consumed) = decode_varint_seq(payload)?;
        let packet_data = &payload[consumed..];
        ch.recv(seq, packet_data.to_vec());
        // Deliver in order (reassembled run + this packet, FIFO)
        ch.take_ready().and_then(|data| postcard::from_bytes(&data).ok())
    }

    /// Process an ACK/NACK control frame, queueing NACK retransmits.
    fn handle_ctrl_frame(&mut self, addr: SocketAddr, payload: &[u8]) {
        let Some(frame) = decode_ctrl_frame(payload) else { return };
        let ch = match self.reliable.get_mut(&addr) {
            Some(ch) => ch,
            None => return,
        };
        match frame {
            CtrlFrame::Ack(ack_seq) => ch.ack_recv(ack_seq),
            CtrlFrame::Nack(missing) => {
                for buf in ch.nack_retransmit(&missing, Instant::now()) {
                    self.resend_queue.push_back((addr, buf));
                }
            }
        }
    }

    /// Per-tick maintenance: flush ACK/NACK control frames, retransmit expired
    /// packets, and drain the NACK fast-retransmit queue.
    pub async fn tick(&mut self) {
        let now = Instant::now();
        let mut outbound: VecDeque<(SocketAddr, Vec<u8>)> = std::mem::take(&mut self.resend_queue);

        // Flush pending ACK/NACKs and expired retransmits for every session
        let addrs: Vec<SocketAddr> = self.reliable.keys().copied().collect();
        for addr in addrs {
            let ch = match self.reliable.get_mut(&addr) {
                Some(ch) => ch,
                None => continue,
            };
            let expired = ch.retransmit_expired(now);
            for buf in expired {
                outbound.push_back((addr, buf));
            }
            for frame in ch.take_control_frames() {
                outbound.push_back((addr, frame));
            }
        }

        for (addr, buf) in outbound {
            if let Err(e) = self.socket.send_to(&buf, addr).await {
                tracing::warn!("Failed to send to {addr}: {e}");
            }
        }
    }

    /// Get the raw socket for async operations.
    pub fn socket(&self) -> Arc<UdpSocket> {
        self.socket.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel_for(channel: Channel) -> ReliableChannel {
        let mut ch = ReliableChannel::new();
        let seq = ch.next_seq();
        ch.enqueue(channel, seq, b"hello".to_vec());
        ch
    }

    // ── Send window ──
    #[test]
    fn test_send_window_can_send() {
        let mut ch = ReliableChannel::new();
        for i in 0..MAX_INFLIGHT {
            let seq = ch.next_seq();
            ch.enqueue(Channel::Game, seq, vec![i as u8]);
        }
        assert!(!ch.can_send(), "window full at MAX_INFLIGHT");

        // Acknowledging frees the window
        ch.ack_recv(0);
        assert!(ch.can_send(), "window reopens after ACK");
    }

    // ── ACK / RTT ──

    #[test]
    fn test_ack_drops_through_seq() {
        let mut ch = ReliableChannel::new();
        for _ in 0..5 {
            let seq = ch.next_seq();
            ch.enqueue(Channel::Game, seq, vec![0xAA]);
        }
        assert_eq!(ch.send_queues[Channel::Game as usize].len(), 5);

        ch.ack_recv(2); // ack everything ≤ 2
        let remaining: Vec<u16> = ch
            .send_queues
            .iter()
            .flat_map(|q| q.iter().map(|e| e.seq))
            .collect();
        assert_eq!(remaining, vec![3, 4]);

        ch.ack_recv(4);
        assert_eq!(
            ch.send_queues.iter().map(|q| q.len()).sum::<usize>(),
            0,
            "all acked entries dropped"
        );
    }

    #[test]
    fn test_ack_updates_rtt() {
        let mut ch = ReliableChannel::new();
        let seq = ch.next_seq();
        ch.enqueue(Channel::System, seq, vec![1]);

        // Fake an old sent_at so RTT is measurable
        let sent = Instant::now() - Duration::from_millis(120);
        ch.send_queues[Channel::System as usize][0].sent_at = sent;

        ch.ack_recv(seq);
        assert!(
            ch.srtt.as_millis() >= 100,
            "srtt should converge toward the 120ms sample, got {}ms",
            ch.srtt.as_millis()
        );
        assert!(ch.rto >= RTO_MIN && ch.rto <= RTO_MAX, "rto clamped");
    }

    #[test]
    fn test_ack_wraparound() {
        let mut ch = ReliableChannel::new();
        // Prime seq so the next seq wraps past 0xFFFF
        ch.send_seq = 0xFFFE;
        let a = ch.next_seq(); // 0xFFFE
        let b = ch.next_seq(); // 0xFFFF
        let c = ch.next_seq(); // 0x0000
        ch.enqueue(Channel::Game, a, vec![1]);
        ch.enqueue(Channel::Game, b, vec![2]);
        ch.enqueue(Channel::Game, c, vec![3]);

        ch.ack_recv(b);
        let remaining: Vec<u16> = ch
            .send_queues
            .iter()
            .flat_map(|q| q.iter().map(|e| e.seq))
            .collect();
        assert_eq!(remaining, vec![0], "only the wrapped seq 0 survives");
    }

    // ── Retransmission ──

    /// Parse `(seq, payload)` out of a reliable wire frame.
    fn parse_frame(buf: &[u8]) -> (u16, &[u8]) {
        let length = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        assert!(buf.len() >= HEADER_SIZE + length);
        let payload = &buf[HEADER_SIZE..HEADER_SIZE + length];
        let (seq, consumed) = decode_varint_seq(payload).expect("varint seq");
        (seq, &payload[consumed..])
    }

    #[test]
    fn test_retransmit_expired() {
        let mut ch = ReliableChannel::new();
        let seq = ch.next_seq();
        ch.enqueue(Channel::Game, seq, vec![0x42]);

        // Nothing expired yet
        assert!(ch.retransmit_expired(Instant::now()).is_empty());

        // Simulate a long-sent packet past RTO
        ch.send_queues[Channel::Game as usize][0].sent_at = Instant::now() - Duration::from_secs(5);
        let resends = ch.retransmit_expired(Instant::now());
        assert_eq!(resends.len(), 1);
        let (rseq, rdata) = parse_frame(&resends[0]);
        assert_eq!(rseq, seq);
        assert_eq!(rdata, &[0x42]);

        // Retransmit counter bumped → backoff doubled, not immediately due again
        assert_eq!(ch.send_queues[Channel::Game as usize][0].retransmits, 1);
        assert!(ch.retransmit_expired(Instant::now()).is_empty());
    }

    #[test]
    fn test_retransmit_priority_order() {
        let mut ch = ReliableChannel::new();
        // Enqueue Chat first, then System — drain must prefer System
        let chat_seq = ch.next_seq();
        ch.enqueue(Channel::Chat, chat_seq, vec![1]);
        let sys_seq = ch.next_seq();
        ch.enqueue(Channel::System, sys_seq, vec![2]);

        for queue in &mut ch.send_queues {
            for e in queue.iter_mut() {
                e.sent_at = Instant::now() - Duration::from_secs(5);
            }
        }

        let resends = ch.retransmit_expired(Instant::now());
        assert_eq!(parse_frame(&resends[0]).0, sys_seq, "System drains before Chat");
        assert_eq!(parse_frame(&resends[1]).0, chat_seq);
    }

    #[test]
    fn test_nack_fast_retransmit() {
        let mut ch = ReliableChannel::new();
        let a = ch.next_seq();
        let b = ch.next_seq();
        ch.enqueue(Channel::Game, a, vec![0x01]);
        ch.enqueue(Channel::Game, b, vec![0x02]);

        let resends = ch.nack_retransmit(&[a], Instant::now());
        assert_eq!(resends.len(), 1);
        assert_eq!(parse_frame(&resends[0]).0, a);

        let resends = ch.nack_retransmit(&[a, b], Instant::now());
        assert_eq!(resends.len(), 2);
    }

    // ── Receive / reassembly ──

    #[test]
    fn test_recv_in_order() {
        let mut ch = ReliableChannel::new();
        ch.recv(0, vec![1]);
        ch.recv(1, vec![2]);
        ch.recv(2, vec![3]);
        assert_eq!(ch.take_ready(), Some(vec![1]));
        assert_eq!(ch.take_ready(), Some(vec![2]));
        assert_eq!(ch.take_ready(), Some(vec![3]));
        assert_eq!(ch.take_ready(), None);
    }

    #[test]
    fn test_recv_out_of_order_buffers_and_nacks() {
        let mut ch = ReliableChannel::new();
        // Receive seq 5 before 3, 4
        ch.recv(5, vec![0x55]);
        assert_eq!(ch.take_ready(), None, "out of order → not ready yet");
        assert!(ch.pending_nacks.contains(&3));
        assert!(ch.pending_nacks.contains(&4));
        assert!(!ch.pending_nacks.contains(&5));

        // Filling the gap drains the buffer in order
        ch.recv(3, vec![0x33]);
        assert_eq!(ch.take_ready(), None, "0-2 still missing");
        ch.recv(0, vec![0x00]);
        assert_eq!(ch.take_ready(), Some(vec![0x00]));
        ch.recv(1, vec![0x11]);
        assert_eq!(ch.take_ready(), Some(vec![0x11]));
        ch.recv(2, vec![0x22]);
        assert_eq!(ch.take_ready(), Some(vec![0x22]));
        assert_eq!(ch.take_ready(), Some(vec![0x33]), "buffered 3 drains");
        ch.recv(4, vec![0x44]);
        assert_eq!(ch.take_ready(), Some(vec![0x44]));
        ch.recv(5, vec![0x55]);
        assert_eq!(ch.take_ready(), Some(vec![0x55]));
        assert!(ch.recv_buffer.is_empty());
        assert_eq!(ch.take_ready(), None);
    }

    #[test]
    fn test_recv_buffered_run_delivered_in_order() {
        // Regression: draining contiguous buffered packets must not discard data
        let mut ch = ReliableChannel::new();
        ch.recv(4, vec![4]);
        ch.recv(5, vec![5]);
        ch.recv(0, vec![0]); // delivers 0; 1,2,3 still missing so 4,5 stay buffered
        assert_eq!(ch.take_ready(), Some(vec![0]));
        ch.recv(1, vec![1]);
        assert_eq!(ch.take_ready(), Some(vec![1]));
        ch.recv(2, vec![2]);
        assert_eq!(ch.take_ready(), Some(vec![2]));
        ch.recv(3, vec![3]); // now 4,5 become contiguous and drain in order
        assert_eq!(ch.take_ready(), Some(vec![3]));
        assert_eq!(ch.take_ready(), Some(vec![4]));
        assert_eq!(ch.take_ready(), Some(vec![5]));
        assert!(ch.recv_buffer.is_empty());
    }

    #[test]
    fn test_recv_too_far_ahead_dropped() {
        let mut ch = ReliableChannel::new();
        // 100 > RECV_WINDOW ahead of recv_seq 0 → dropped
        ch.recv(100, vec![1]);
        assert!(ch.recv_buffer.is_empty());
        assert_eq!(ch.take_ready(), None);
    }

    #[test]
    fn test_control_frames_pending() {
        let mut ch = ReliableChannel::new();
        assert!(ch.take_control_frames().is_empty(), "nothing pending initially");

        ch.recv(0, vec![1]);
        let frames = ch.take_control_frames();
        assert_eq!(frames.len(), 1);
        let decoded = decode_ctrl_frame(&frames[0][3..]).unwrap();
        assert_eq!(decoded, CtrlFrame::Ack(0));
        assert!(ch.take_control_frames().is_empty(), "acked frames cleared");

        // Out-of-order triggers NACK
        ch.recv(4, vec![2]);
        let frames = ch.take_control_frames();
        let nack = frames.iter().find(|f| f[3] == ashfall_core::protocol::transport::CTRL_NACK).expect("nack frame");
        let decoded = decode_ctrl_frame(&nack[3..]).unwrap();
        if let CtrlFrame::Nack(missing) = decoded {
            assert!(missing.contains(&1));
        } else {
            panic!("expected Nack");
        }
    }

    // ── Rate limiter ──

    #[test]
    fn test_rate_limiter_burst_then_throttle() {
        let mut rl = RateLimiter::new(200.0, 100.0);
        // Burst of 100 allowed instantly
        for i in 0..100 {
            assert!(rl.check_rate(), "burst token {i} should pass");
        }
        // 101st is refused (no refill time has passed)
        assert!(!rl.check_rate());
    }

    #[test]
    fn test_rate_limiter_refills() {
        let mut rl = RateLimiter::new(100.0, 10.0);
        for _ in 0..10 {
            assert!(rl.check_rate());
        }
        assert!(!rl.check_rate());

        // Simulate elapsed time: refill tokens
        rl.last_refill = Instant::now() - Duration::from_millis(50);
        assert!(rl.check_rate(), "refilled token should pass");
    }

    #[test]
    fn test_rate_limiter_caps_burst() {
        let mut rl = RateLimiter::new(1.0, 5.0);
        // Pretend a long idle period — bucket must not exceed max_tokens
        rl.last_refill = Instant::now() - Duration::from_secs(3600);
        for _ in 0..5 {
            assert!(rl.check_rate());
        }
        assert!(!rl.check_rate(), "burst capped at max_tokens");
    }
}
