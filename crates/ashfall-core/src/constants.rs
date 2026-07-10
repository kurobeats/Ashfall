//! Constants ported from vaultmp-extended.

/// Game version string.
pub const DEDICATED_VERSION: &str = "0.1a snapshot \"Gary 2.10\"";
pub const MASTER_VERSION: &str = "0.1a snapshot \"Gary 2.10\"";
pub const CLIENT_VERSION: &str = "0.1a snapshot \"Gary 2.10\"";

/// CRC32 checksums for binary/data validation.
pub const FALLOUT3_EN_VER17: u32 = 0x00E59528;
pub const FOSE_VER0122: u32 = 0x0004E1B5;
pub const VAULTMP_DLL: u32 = 0x000368FD;
pub const VAULTMP_F3: u32 = 0x1C877592;
pub const XLIVE_PATCH: u32 = 0x0000D57E;

/// Size limits.
pub const MAX_PLAYER_NAME: usize = 16;
pub const MAX_PASSWORD_SIZE: usize = 16;
pub const MAX_MASTER_SERVER: usize = 32;
pub const MAX_MOD_FILE: usize = 64;
pub const MAX_CELL_NAME: usize = 36;
pub const MAX_MESSAGE_LENGTH: usize = 64;
pub const MAX_CHAT_LENGTH: usize = 128;

/// Pipe IPC limits.
pub const PIPE_LENGTH: usize = 2048;

/// Packet channels (must match RakNet channel semantics).
pub const CHANNEL_SYSTEM: u8 = 0;
pub const CHANNEL_GAME: u8 = 1;
pub const CHANNEL_CHAT: u8 = 2;

/// Default ports.
pub const RAKNET_STANDARD_PORT: u16 = 1770;
pub const RAKNET_MASTER_STANDARD_PORT: u16 = 1660;
pub const RAKNET_FILE_SERVER: u16 = 1550;

/// Max connections per server.
pub const RAKNET_STANDARD_CONNECTIONS: usize = 4;
