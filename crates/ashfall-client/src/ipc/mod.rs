//! IPC bridge to the game engine process.
//!
//! Transports:
//! - **TCP loopback** (default) — `127.0.0.1:1771` to bridge.dll in Proton/Wine.
//! - **Unix domain socket** — `/tmp/ashfall-ipc.sock` for native Linux engine stub.
//! - **Stub** — returns canned responses for development without game running.
//!
//! Wire format (matches original vaultmp pipe protocol):
//! ```text
//! Request:  [opcode:1B][key:4B][func:4B][param_count:1B][params...]
//! Response: [opcode:1B][key:4B][result...]
//! ```

mod commands;
mod transport;

pub use commands::{CommandResult, Param};
pub use transport::{IpcMode, IpcTransport};

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::oneshot;

/// Opcodes for common pipe operations.
pub const PIPE_SYS_WAKEUP: u8 = 0x01;
pub const PIPE_OP_COMMAND: u8 = 0x02;
pub const PIPE_OP_RETURN: u8 = 0x03;
pub const PIPE_OP_RETURN_BIG: u8 = 0x04;
pub const PIPE_OP_RETURN_RAW: u8 = 0x05;
pub const PIPE_ERROR_CLOSE: u8 = 0x06;

/// Client side of the game engine bridge.
pub struct IpcClient {
    transport: IpcTransport,
    next_key: AtomicU32,
    pending: HashMap<u32, oneshot::Sender<CommandResult>>,
}

impl IpcClient {
    /// Connect to the game bridge using the specified mode.
    pub async fn connect(mode: IpcMode) -> anyhow::Result<Self> {
        let transport = transport::connect(mode).await?;
        Ok(Self {
            transport,
            next_key: AtomicU32::new(1),
            pending: HashMap::new(),
        })
    }

    /// Send a command to the game engine, await result.
    pub async fn execute(&mut self, opcode: u32, params: &[Param]) -> CommandResult {
        let key = self.next_key.fetch_add(1, Ordering::SeqCst);

        // Build request: [PIPE_OP_COMMAND][key:4B][opcode:4B][param_count:1B][params...]
        let mut request = Vec::with_capacity(256);
        request.push(PIPE_OP_COMMAND);
        request.extend_from_slice(&key.to_le_bytes());
        request.extend_from_slice(&opcode.to_le_bytes());
        request.push(params.len() as u8);
        for p in params {
            p.encode_into(&mut request);
        }

        self.transport.send(&request).await;

        // In a real implementation: read response from transport receive task,
        // match by key. ponytail: for now, synchronous send+recv.
        let mut response_buf = vec![0u8; 2048];
        let n = self.transport.recv(&mut response_buf).await;

        if n < 5 {
            return CommandResult::Error("short response".into());
        }

        CommandResult::decode(&response_buf[..n])
    }

    // ── Convenience methods ──

    pub async fn get_pos(&mut self, ref_id: u32) -> anyhow::Result<[f32; 3]> {
        let result = self.execute(commands::OP_GET_POS, &[Param::U32(ref_id)]).await;
        match result {
            CommandResult::Floats(v) if v.len() >= 3 => Ok([v[0], v[1], v[2]]),
            CommandResult::Error(e) => Err(anyhow::anyhow!("get_pos: {e}")),
            _ => Err(anyhow::anyhow!("get_pos: unexpected result")),
        }
    }

    pub async fn get_angle(&mut self, ref_id: u32) -> anyhow::Result<[f32; 3]> {
        let result = self.execute(commands::OP_GET_ANGLE, &[Param::U32(ref_id)]).await;
        match result {
            CommandResult::Floats(v) if v.len() >= 3 => Ok([v[0], v[1], v[2]]),
            CommandResult::Error(e) => Err(anyhow::anyhow!("get_angle: {e}")),
            _ => Err(anyhow::anyhow!("get_angle: unexpected result")),
        }
    }

    pub async fn get_actor_state(
        &mut self,
        ref_id: u32,
    ) -> anyhow::Result<(u32, u8, u8, u8, bool, bool)> {
        let result = self.execute(commands::OP_GET_ACTOR_STATE, &[Param::U32(ref_id)]).await;
        match result {
            CommandResult::ActorState { idle, moving, weapon, flags, alerted, sneaking } => {
                Ok((idle, moving, weapon, flags, alerted, sneaking))
            }
            CommandResult::Error(e) => Err(anyhow::anyhow!("get_actor_state: {e}")),
            _ => Err(anyhow::anyhow!("get_actor_state: unexpected result")),
        }
    }
}
