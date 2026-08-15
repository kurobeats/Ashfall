//! Command opcodes and parameter types for the game engine IPC protocol.

/// Command opcodes (subset — full set matches original vaultmp).
pub const OP_GET_POS: u32 = 0x0001;
pub const OP_SET_POS: u32 = 0x0002;
pub const OP_GET_ANGLE: u32 = 0x0003;
pub const OP_SET_ANGLE: u32 = 0x0004;
pub const OP_GET_CELL: u32 = 0x0005;
pub const OP_SET_CELL: u32 = 0x0006;
pub const OP_GET_ACTOR_STATE: u32 = 0x0007;
pub const OP_GET_ACTOR_VALUE: u32 = 0x0008;
pub const OP_SET_ACTOR_VALUE: u32 = 0x0009;
pub const OP_GET_CONTROL: u32 = 0x000A;
pub const OP_SET_CONTROL: u32 = 0x000B;
pub const OP_GET_ACTIVATE: u32 = 0x000C;
pub const OP_FIRE_WEAPON: u32 = 0x000D;
pub const OP_GET_NAME: u32 = 0x000E;
pub const OP_SET_NAME: u32 = 0x000F;
pub const OP_GET_LOCK: u32 = 0x0012;
pub const OP_SET_LOCK: u32 = 0x0013;
pub const OP_SET_ENABLED: u32 = 0x0011;
pub const OP_MOVE_TO: u32 = 0x0014;
pub const OP_PLAY_SOUND: u32 = 0x0015;
pub const OP_SET_SCALE: u32 = 0x002B;
pub const OP_PLAY_GROUP: u32 = 0x0028;
pub const OP_KILL: u32 = 0x0023;
pub const OP_TRACK_ACTOR: u32 = 0x00F6;
pub const OP_UNTRACK_ACTOR: u32 = 0x00F5;

/// A parameter to a game engine command.
#[derive(Debug, Clone)]
pub enum Param {
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    Str(String),
    /// Single byte (actor-value index, limb, cause).
    U8(u8),
}

impl Param {
    /// Encode this parameter into a byte buffer (pipe protocol).
    pub fn encode_into(&self, buf: &mut Vec<u8>) {
        match self {
            Param::U32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Param::I32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Param::F32(v) => buf.extend_from_slice(&v.to_le_bytes()),
            Param::U8(v) => buf.push(*v),
            Param::Bool(v) => buf.push(if *v { 1 } else { 0 }),
            Param::Str(s) => {
                let bytes = s.as_bytes();
                buf.push(bytes.len() as u8);
                buf.extend_from_slice(bytes);
            }
        }
    }
}

/// Result of a game engine command.
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// One or more float values (position, angle, actor value).
    Floats(Vec<f32>),
    /// Integer result.
    Int(i32),
    /// String result.
    Text(String),
    /// Actor state tuple.
    ActorState {
        idle: u32,
        moving: u8,
        weapon: u8,
        flags: u8,
        alerted: bool,
        sneaking: bool,
    },
    /// Operation succeeded (no return value).
    Success,
    /// Error message.
    Error(String),
}

impl CommandResult {
    /// Decode a command result from the raw result bytes (the RETURN frame
    /// payload minus its 4-byte key).
    pub fn decode(data: &[u8]) -> Self {
        if data.is_empty() {
            return CommandResult::Success;
        }

        // ponytail: simple decode — first byte is type tag, rest is data.
        // Full implementation in PR99.
        if data.len() >= 12 {
            // Heuristic: if payload is multiples of 4, decode as floats
            let count = data.len() / 4;
            let mut floats = Vec::with_capacity(count);
            for i in 0..count.min(3) {
                let start = i * 4;
                floats.push(f32::from_le_bytes([
                    data[start],
                    data[start + 1],
                    data[start + 2],
                    data[start + 3],
                ]));
            }
            CommandResult::Floats(floats)
        } else {
            CommandResult::Success
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcodes_are_distinct() {
        let ops = [
            OP_GET_POS, OP_SET_POS, OP_GET_ANGLE, OP_SET_ANGLE,
            OP_GET_CELL, OP_SET_CELL, OP_GET_ACTOR_STATE, OP_GET_ACTOR_VALUE,
            OP_SET_ACTOR_VALUE, OP_GET_CONTROL, OP_SET_CONTROL, OP_GET_ACTIVATE,
            OP_FIRE_WEAPON, OP_GET_NAME, OP_SET_NAME, OP_GET_LOCK, OP_SET_LOCK,
            OP_SET_ENABLED, OP_MOVE_TO, OP_PLAY_SOUND, OP_SET_SCALE, OP_PLAY_GROUP,
            OP_KILL, OP_TRACK_ACTOR, OP_UNTRACK_ACTOR,
        ];
        let mut sorted = ops.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ops.len(), "opcodes must be unique");
    }

    #[test]
    fn param_encode_wire_layout() {
        let mut buf = Vec::new();
        Param::U32(0x01020304).encode_into(&mut buf);
        assert_eq!(buf, [0x04, 0x03, 0x02, 0x01]); // LE
        buf.clear();
        Param::F32(1.0).encode_into(&mut buf);
        assert_eq!(buf, 1.0f32.to_le_bytes());
        buf.clear();
        Param::U8(0x14).encode_into(&mut buf);
        assert_eq!(buf, [0x14]);
        buf.clear();
        Param::Bool(true).encode_into(&mut buf);
        assert_eq!(buf, [1]);
        buf.clear();
        Param::Str("hi".into()).encode_into(&mut buf);
        assert_eq!(buf, [2, b'h', b'i']); // len-prefixed
    }

    #[test]
    fn result_decode_empty_is_success() {
        assert!(matches!(CommandResult::decode(&[]), CommandResult::Success));
    }

    #[test]
    fn result_decode_12_bytes_heuristic_floats() {
        let mut buf = Vec::new();
        for f in [1.5f32, -2.0f32, 3.25f32] {
            buf.extend_from_slice(&f.to_le_bytes());
        }
        match CommandResult::decode(&buf) {
            CommandResult::Floats(floats) => {
                assert_eq!(floats.len(), 3);
                assert!((floats[0] - 1.5).abs() < 1e-6);
                assert!((floats[1] + 2.0).abs() < 1e-6);
                assert!((floats[2] - 3.25).abs() < 1e-6);
            }
            other => panic!("expected floats, got {other:?}"),
        }
    }
}
