//! Transport-level wire helpers shared by server and client.
//!
//! Frame layout (all little-endian):
//! ```text
//! [len: u16][channel: u8][payload...]
//! ```
//! - Reliable data frames set the reliable flag on the channel byte
//!   (`0x80 | Channel`) and prefix the payload with a varint sequence number.
//! - Unreliable data frames use the bare channel byte and no sequence number.
//! - Control frames use `CHANNEL_CTRL` (0xFF) and carry ACK/NACK payloads.
//!
//! The reliable flag (instead of the old `payload.len() >= 2` heuristic)
//! makes framing unambiguous even for single-byte postcard packets.

/// Flag set on the channel byte to mark a reliable (sequenced) frame.
pub const CHANNEL_RELIABLE_FLAG: u8 = 0x80;

/// Channel byte for transport control frames (ACK/NACK) — never a data channel.
pub const CHANNEL_CTRL: u8 = 0xFF;

/// Control frame subtypes.
pub const CTRL_ACK: u8 = 0x01;
pub const CTRL_NACK: u8 = 0x02;

/// A decoded transport control frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtrlFrame {
    /// Cumulative acknowledgment: everything up to and including `seq`.
    Ack(u16),
    /// Negative acknowledgment: these sequence numbers were missed.
    Nack(Vec<u16>),
}

// ── VarInt sequence numbers ──

/// Encode a u16 sequence number: single byte (`0x80 | seq`) when < 128,
/// otherwise a `0x00` marker + 2 LE bytes. Saves 1 byte ~50% of the time.
pub fn encode_varint_seq(seq: u16) -> Vec<u8> {
    if seq < 128 {
        vec![0x80 | seq as u8]
    } else {
        let mut out = Vec::with_capacity(3);
        out.push(0x00);
        out.extend_from_slice(&seq.to_le_bytes());
        out
    }
}

/// Decode a varint sequence number from the front of `data`.
/// Returns `(seq, bytes_consumed)` or `None` on truncation.
pub fn decode_varint_seq(data: &[u8]) -> Option<(u16, usize)> {
    match data.first() {
        Some(&0x00) => {
            if data.len() < 3 {
                return None;
            }
            Some((u16::from_le_bytes([data[1], data[2]]), 3))
        }
        Some(&b) if b & 0x80 != 0 => Some(((b & 0x7F) as u16, 1)),
        _ => None,
    }
}

// ── Control frames (full wire frames, length-prefixed) ──

/// Encode a cumulative ACK frame: `[len][CHANNEL_CTRL][CTRL_ACK][varint seq]`.
pub fn encode_ctrl_ack(ack_seq: u16) -> Vec<u8> {
    let seq = encode_varint_seq(ack_seq);
    let mut frame = Vec::with_capacity(3 + seq.len());
    frame.extend_from_slice(&((1 + seq.len()) as u16).to_le_bytes());
    frame.push(CHANNEL_CTRL);
    frame.push(CTRL_ACK);
    frame.extend_from_slice(&seq);
    frame
}

/// Encode a NACK frame: `[len][CHANNEL_CTRL][CTRL_NACK][count: u8][varint seq...]`.
pub fn encode_ctrl_nack(missing: &[u16]) -> Vec<u8> {
    let mut frame = vec![
        0u8,
        0u8,
        CHANNEL_CTRL,
        CTRL_NACK,
        missing.len().min(255) as u8,
    ];
    for seq in missing.iter().take(255) {
        frame.extend_from_slice(&encode_varint_seq(*seq));
    }
    let len = (frame.len() - 3) as u16; // length counts payload after channel byte
    frame[0..2].copy_from_slice(&len.to_le_bytes());
    frame
}

/// Decode a control frame from its payload (bytes after the channel byte).
pub fn decode_ctrl_frame(payload: &[u8]) -> Option<CtrlFrame> {
    match payload.first() {
        Some(&CTRL_ACK) => {
            let (seq, _) = decode_varint_seq(&payload[1..])?;
            Some(CtrlFrame::Ack(seq))
        }
        Some(&CTRL_NACK) => {
            let count = *payload.get(1)? as usize;
            let mut seqs = Vec::with_capacity(count);
            let mut offset = 2;
            for _ in 0..count {
                let (seq, consumed) = decode_varint_seq(&payload[offset..])?;
                seqs.push(seq);
                offset += consumed;
            }
            Some(CtrlFrame::Nack(seqs))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint_roundtrip_boundaries() {
        for seq in [0u16, 1, 127, 128, 129, 255, 256, 0x7FFF, 0xFFFF] {
            let enc = encode_varint_seq(seq);
            let (dec, consumed) = decode_varint_seq(&enc).expect("decodable");
            assert_eq!(dec, seq);
            assert_eq!(consumed, enc.len());
        }
    }

    #[test]
    fn test_varint_size() {
        assert_eq!(encode_varint_seq(0).len(), 1);
        assert_eq!(encode_varint_seq(127).len(), 1);
        assert_eq!(encode_varint_seq(128).len(), 3);
        assert_eq!(encode_varint_seq(0xFFFF).len(), 3);
        // High bit set marks the short form
        assert_eq!(encode_varint_seq(0)[0], 0x80);
        assert_eq!(encode_varint_seq(127)[0], 0xFF);
        assert_eq!(encode_varint_seq(128)[0], 0x00);
    }

    #[test]
    fn test_varint_truncated() {
        assert!(decode_varint_seq(&[]).is_none());
        assert!(decode_varint_seq(&[0x00]).is_none());
        assert!(decode_varint_seq(&[0x00, 0x01]).is_none());
        assert!(decode_varint_seq(&[0x7F]).is_none()); // no high bit, not a marker
    }

    #[test]
    fn test_ctrl_ack_roundtrip() {
        for seq in [0u16, 42, 127, 128, 0xFFFF] {
            let frame = encode_ctrl_ack(seq);
            assert_eq!(frame[2], CHANNEL_CTRL);
            assert_eq!(frame[3], CTRL_ACK);
            let len = u16::from_le_bytes([frame[0], frame[1]]) as usize;
            assert_eq!(len, frame.len() - 3, "length counts payload after channel");
            let decoded = decode_ctrl_frame(&frame[3..]).unwrap();
            assert_eq!(decoded, CtrlFrame::Ack(seq));
        }
    }

    #[test]
    fn test_ctrl_nack_roundtrip() {
        let missing = vec![1u16, 130, 7, 0xFFFF];
        let frame = encode_ctrl_nack(&missing);
        assert_eq!(frame[3], CTRL_NACK);
        assert_eq!(frame[4], missing.len() as u8);
        let decoded = decode_ctrl_frame(&frame[3..]).unwrap();
        assert_eq!(decoded, CtrlFrame::Nack(missing));
    }

    #[test]
    fn test_ctrl_nack_empty_and_truncated() {
        let frame = encode_ctrl_nack(&[]);
        assert_eq!(
            decode_ctrl_frame(&frame[3..]).unwrap(),
            CtrlFrame::Nack(vec![])
        );

        assert!(decode_ctrl_frame(&[]).is_none());
        assert!(decode_ctrl_frame(&[0x99]).is_none()); // unknown subtype
                                                       // NACK with count but missing body
        assert!(decode_ctrl_frame(&[CTRL_NACK, 1]).is_none());
    }

    #[test]
    fn test_ctrl_nack_caps_at_255() {
        let many: Vec<u16> = (0..300).collect();
        let frame = encode_ctrl_nack(&many);
        assert_eq!(frame[4], 255);
        if let CtrlFrame::Nack(seqs) = decode_ctrl_frame(&frame[3..]).unwrap() {
            assert_eq!(seqs.len(), 255);
        } else {
            panic!("expected Nack");
        }
    }
}
