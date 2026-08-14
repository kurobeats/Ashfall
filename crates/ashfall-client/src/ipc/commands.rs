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
pub const OP_SET_SCALE: u32 = 0x0015;
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
