//! Real-UDP integration tests for the reliability layer.
//!
//! Exercises the full wire path with actual sockets on loopback: framing
//! (reliable flag + varint seq), ACK processing, send-window throttling,
//! and RTO retransmission.

use ashfall_core::protocol::transport::{
    decode_ctrl_frame, decode_varint_seq, encode_ctrl_ack, CtrlFrame, CHANNEL_RELIABLE_FLAG,
};
use ashfall_core::protocol::Packet;
use ashfall_server::network::NetworkManager;
use std::time::Duration;
use tokio::net::UdpSocket;

/// Parse a wire frame: returns (channel_byte, reliable_seq, payload).
fn parse_frame(data: &[u8]) -> (u8, Option<u16>, &[u8]) {
    let length = u16::from_le_bytes([data[0], data[1]]) as usize;
    assert!(
        data.len() >= 3 + length,
        "truncated frame: {} < {}",
        data.len(),
        3 + length
    );
    let channel = data[2];
    let payload = &data[3..3 + length];
    if channel & CHANNEL_RELIABLE_FLAG != 0 {
        let (seq, consumed) = decode_varint_seq(payload).expect("varint seq");
        (channel, Some(seq), &payload[consumed..])
    } else {
        (channel, None, payload)
    }
}

fn chat(message: &str) -> Packet {
    Packet::GameChat {
        message: message.into(),
    }
}

#[tokio::test]
async fn test_reliable_roundtrip_over_udp() {
    let mut server = NetworkManager::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.connect(server_addr).await.unwrap();
    let client_addr = client.local_addr().unwrap();
    server.register_session(client_addr);

    // Server → client reliable send
    server
        .send_reliable(client_addr, &chat("hello world"))
        .await
        .unwrap();

    let mut buf = vec![0u8; 2048];
    let len = client.recv(&mut buf).await.unwrap();
    let (channel, seq, payload) = parse_frame(&buf[..len]);

    assert_eq!(
        channel & CHANNEL_RELIABLE_FLAG,
        CHANNEL_RELIABLE_FLAG,
        "reliable flag set"
    );
    assert_eq!(seq, Some(0), "first seq");
    let decoded: Packet = postcard::from_bytes(payload).unwrap();
    assert_eq!(decoded, chat("hello world"));
}

#[tokio::test]
async fn test_unreliable_roundtrip_over_udp() {
    let mut server = NetworkManager::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.connect(server_addr).await.unwrap();
    let client_addr = client.local_addr().unwrap();
    server.register_session(client_addr);

    // UpdatePos is unreliable per Channel::is_unreliable
    let pos = Packet::UpdatePos {
        id: 42.into(),
        pos: [1.0, 2.0, 3.0],
    };
    server.send_unreliable(client_addr, &pos).await.unwrap();

    let mut buf = vec![0u8; 2048];
    let len = client.recv(&mut buf).await.unwrap();
    let (channel, seq, payload) = parse_frame(&buf[..len]);

    assert_eq!(
        channel & CHANNEL_RELIABLE_FLAG,
        0,
        "no reliable flag on unreliable frame"
    );
    assert_eq!(seq, None, "no sequence number");
    let decoded: Packet = postcard::from_bytes(payload).unwrap();
    assert_eq!(decoded, pos);
}

#[tokio::test]
async fn test_ack_drains_send_window() {
    let mut server = NetworkManager::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.connect(server_addr).await.unwrap();
    let client_addr = client.local_addr().unwrap();
    server.register_session(client_addr);

    // Fill the window (MAX_INFLIGHT = 32)
    for i in 0..32 {
        server
            .send_reliable(client_addr, &chat(&format!("packet {i}")))
            .await
            .unwrap();
    }
    // 33rd send is throttled
    let err = server.send_reliable(client_addr, &chat("overflow")).await;
    assert!(err.is_err(), "send window must throttle the 33rd packet");

    // Client receives everything and ACKs the last seq — drains the buffer
    let mut buf = vec![0u8; 2048];
    for i in 0..32 {
        let len = client.recv(&mut buf).await.unwrap();
        let (_, seq, _) = parse_frame(&buf[..len]);
        assert_eq!(seq, Some(i));
    }

    let ack = encode_ctrl_ack(31);
    let mut raw = vec![0u8; 2048];
    raw[..ack.len()].copy_from_slice(&ack);
    assert!(
        server.try_recv(client_addr, &raw[..ack.len()]).is_none(),
        "ACK yields no packet"
    );

    // Window is open again
    server
        .send_reliable(client_addr, &chat("after ack"))
        .await
        .unwrap();
    let len = client.recv(&mut buf).await.unwrap();
    let (_, seq, payload) = parse_frame(&buf[..len]);
    assert_eq!(seq, Some(32), "next seq after ACK-drained window");
    let decoded: Packet = postcard::from_bytes(payload).unwrap();
    assert_eq!(decoded, chat("after ack"));
}

#[tokio::test]
async fn test_retransmission_on_rto_timeout() {
    let mut server = NetworkManager::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.connect(server_addr).await.unwrap();
    let client_addr = client.local_addr().unwrap();
    server.register_session(client_addr);

    // Send but never ACK (simulated loss of the ACK or the frame)
    server
        .send_reliable(client_addr, &chat("lost in transit"))
        .await
        .unwrap();

    // Client receives the first copy...
    let mut buf = vec![0u8; 2048];
    let len = client.recv(&mut buf).await.unwrap();
    let (_, seq1, _) = parse_frame(&buf[..len]);
    assert_eq!(seq1, Some(0));

    // ...but never ACKs. After RTO (default 300ms) the server must resend.
    tokio::time::sleep(Duration::from_millis(400)).await;
    server.tick().await;

    // Retransmission arrives with the SAME seq
    let len = client.recv(&mut buf).await.unwrap();
    let (_, seq2, payload) = parse_frame(&buf[..len]);
    assert_eq!(seq2, Some(0), "retransmitted with same seq");
    let decoded: Packet = postcard::from_bytes(payload).unwrap();
    assert_eq!(decoded, chat("lost in transit"));
}

#[tokio::test]
async fn test_out_of_order_nack_recovers() {
    let mut server = NetworkManager::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.connect(server_addr).await.unwrap();
    let client_addr = client.local_addr().unwrap();
    server.register_session(client_addr);

    // Server sends two reliable packets; the client "drops" the first
    // (simulated loss) so it receives seq 1 before seq 0.
    server
        .send_reliable(client_addr, &chat("first"))
        .await
        .unwrap();
    server
        .send_reliable(client_addr, &chat("second"))
        .await
        .unwrap();

    let mut buf = vec![0u8; 2048];
    // Client receives both frames (owned copies — buf is reused below)
    let mut frames = Vec::new();
    for _ in 0..2 {
        let len = client.recv(&mut buf).await.unwrap();
        let (_, seq, payload) = parse_frame(&buf[..len]);
        frames.push((seq, payload.to_vec()));
    }
    // Simulate loss: only process the second frame (seq 1) via try_recv
    let second = frames.iter().find(|(seq, _)| *seq == Some(1)).unwrap();
    let mut raw = vec![0u8; 2048];
    let data = encode_reliable_frame(second.0.unwrap(), &second.1);
    raw[..data.len()].copy_from_slice(&data);

    // try_recv buffers it out-of-order (None) and records a NACK
    let pkt = server.try_recv(client_addr, &raw[..data.len()]);
    assert!(pkt.is_none(), "out-of-order packet buffered, not delivered");

    // The pending NACK/ACK control frames flush on tick; client receives a
    // control frame (NACK listing seq 0) — verify it decodes as Nack.
    server.tick().await;
    let len = client.recv(&mut buf).await.unwrap();
    let ctrl = &buf[..len];
    assert_eq!(ctrl[2], ashfall_core::protocol::transport::CHANNEL_CTRL);
    let decoded = decode_ctrl_frame(&ctrl[3..]).unwrap();
    match decoded {
        CtrlFrame::Nack(missing) => assert!(missing.contains(&0), "NACK must list missing seq 0"),
        other => panic!("expected Nack frame, got {other:?}"),
    }
}

/// Build a reliable frame with a varint seq + postcard payload (as the server would).
fn encode_reliable_frame(seq: u16, payload: &[u8]) -> Vec<u8> {
    use ashfall_core::protocol::transport::encode_varint_seq;
    let seq_bytes = encode_varint_seq(seq);
    let mut buf = Vec::new();
    buf.extend_from_slice(&((seq_bytes.len() + payload.len()) as u16).to_le_bytes());
    buf.push(CHANNEL_RELIABLE_FLAG);
    buf.extend_from_slice(&seq_bytes);
    buf.extend_from_slice(payload);
    buf
}

/// Regression: a fresh client's first reliable frame (GameAuth) must
/// bootstrap the reliable channel. Before the fix, try_recv dropped it
/// because the session wasn't registered yet — so no client could ever
/// authenticate after the reliability layer landed.
#[tokio::test]
async fn test_first_contact_bootstraps_reliable_channel() {
    let mut server = NetworkManager::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let server_addr = server.socket().local_addr().unwrap();

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    client.connect(server_addr).await.unwrap();
    let client_addr = client.local_addr().unwrap();
    // NOTE: no server.register_session(client_addr) — this is first contact.

    let auth = Packet::GameAuth {
        name: "Wanderer".into(),
        password: String::new(),
        version: ashfall_core::constants::DEDICATED_VERSION.into(),
    };
    let payload = postcard::to_stdvec(&auth).unwrap();
    client
        .send(&encode_reliable_frame(0, &payload))
        .await
        .unwrap();

    let mut buf = vec![0u8; 2048];
    let (len, _) = server.recv_raw(&mut buf).await.unwrap();
    let pkt = server.try_recv(client_addr, &buf[..len]);
    assert!(
        pkt.is_some(),
        "first-contact reliable frame must be delivered"
    );
    assert_eq!(pkt.unwrap(), auth);

    // The server can now reply reliably (channel was bootstrapped)
    server
        .send_reliable(client_addr, &chat("welcome"))
        .await
        .unwrap();
    let n = client.recv(&mut buf).await.unwrap();
    let (channel, _, payload) = parse_frame(&buf[..n]);
    assert_eq!(channel & CHANNEL_RELIABLE_FLAG, CHANNEL_RELIABLE_FLAG);
    assert_eq!(
        postcard::from_bytes::<Packet>(payload).unwrap(),
        chat("welcome")
    );
}
