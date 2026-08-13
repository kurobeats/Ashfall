//! IPC bridge to the game engine process.
//!
//! ponytail: unused in stub mode (canned responses). Kept for when the
//! game engine bridge (Proton/Wine) is exercised.
#![allow(dead_code)]
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

pub use commands::{CommandResult, Param, OP_KILL, OP_SET_ANGLE, OP_SET_ACTOR_VALUE, OP_SET_POS, OP_TRACK_ACTOR, OP_UNTRACK_ACTOR};
pub use transport::{IpcMode, IpcTransport};

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};

/// Opcodes for common pipe operations.
pub const PIPE_SYS_WAKEUP: u8 = 0x01;
pub const PIPE_OP_COMMAND: u8 = 0x02;
pub const PIPE_OP_RETURN: u8 = 0x03;
pub const PIPE_OP_RETURN_BIG: u8 = 0x04;
pub const PIPE_OP_RETURN_RAW: u8 = 0x05;
pub const PIPE_ERROR_CLOSE: u8 = 0x06;
pub const PIPE_OP_EVENT: u8 = 0x07;

/// Client side of the game engine bridge.
pub struct IpcClient {
    transport: IpcTransport,
    next_key: AtomicU32,
    /// RETURN payloads received, keyed lookup on demand.
    returns: VecDeque<Vec<u8>>,
    /// Bytes read but not yet assembled into a frame (TCP is a stream).
    read_buf: Vec<u8>,
    /// EVENT frames received while waiting for a command response.
    event_buf: Vec<ashfall_core::event::PipeFrame>,
}

impl IpcClient {
    /// Connect to the game bridge using the specified mode.
    pub async fn connect(mode: IpcMode) -> anyhow::Result<Self> {
        let transport = transport::connect(mode).await?;
        Ok(Self {
            transport,
            next_key: AtomicU32::new(1),
            returns: VecDeque::new(),
            read_buf: Vec::new(),
            event_buf: Vec::new(),
        })
    }

    /// Send a command to the game engine, await result. EVENT frames that
    /// interleave on the stream are buffered for [`Self::poll_events`].
    pub async fn execute(&mut self, opcode: u32, params: &[Param]) -> CommandResult {
        // Stub mode: no real game — every command fails fast instead of
        // waiting on a response that never comes.
        if self.transport.is_stub() {
            return CommandResult::Error("stub mode".into());
        }
        let key = self.next_key.fetch_add(1, Ordering::SeqCst);

        // Build request: [len][PIPE_OP_COMMAND][key:4B][opcode:4B][param_count:1B][params...]
        let mut payload = Vec::with_capacity(256);
        payload.extend_from_slice(&key.to_le_bytes());
        payload.extend_from_slice(&opcode.to_le_bytes());
        payload.push(params.len() as u8);
        for p in params {
            p.encode_into(&mut payload);
        }
        let request = ashfall_core::event::encode_frame(PIPE_OP_COMMAND, &payload);
        self.transport.send(&request).await;

        // Read until the response for our key arrives (events stashed aside).
        let mut response_buf = vec![0u8; 2048];
        loop {
            if let Some(resp) = self.take_return(key) {
                return CommandResult::decode(&resp);
            }
            let n = self.transport.recv(&mut response_buf).await;
            if n == 0 {
                return CommandResult::Error("bridge disconnected".into());
            }
            self.ingest(&response_buf[..n]);
        }
    }

    /// Drain any engine EVENT frames received so far. Reads whatever is
    /// buffered on the transport first (non-blocking) — events arrive from
    /// the bridge independently of command round-trips, so pollers must
    /// actually read the socket.
    pub fn poll_events(&mut self) -> Vec<ashfall_core::event::PipeFrame> {
        let mut buf = vec![0u8; 4096];
        let n = self.transport.try_read(&mut buf);
        if n > 0 {
            self.ingest(&buf[..n]);
        }
        std::mem::take(&mut self.event_buf)
    }

    /// Split freshly-read bytes into frames; RETURN frames for unknown keys
    /// are dropped (they belonged to an abandoned request).
    fn ingest(&mut self, data: &[u8]) {
        self.read_buf.extend_from_slice(data);
        let (frames, rest) = ashfall_core::event::split_frames(&self.read_buf);
        self.read_buf = rest.to_vec();
        for frame in frames {
            if frame.opcode == PIPE_OP_EVENT {
                self.event_buf.push(frame);
            } else if frame.opcode == PIPE_OP_RETURN {
                // route to pending by key — handled via take_return
                self.returns.push_back(frame.payload);
            }
        }
    }

    /// Find the RETURN payload for a key, if received.
    fn take_return(&mut self, key: u32) -> Option<Vec<u8>> {
        let pos = self.returns.iter().position(|p| {
            p.len() >= 4 && u32::from_le_bytes([p[0], p[1], p[2], p[3]]) == key
        })?;
        let payload = self.returns.remove(pos).unwrap();
        Some(payload[4..].to_vec())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Mock bridge: reads one framed command, replies with a RETURN frame,
    /// then pushes an EVENT frame (exercises execute + event buffering over
    /// real TCP with the length-prefixed framing).
    fn spawn_mock_bridge() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap();
            let (frames, _) = ashfall_core::event::split_frames(&buf[..n]);
            assert_eq!(frames.len(), 1, "one framed command");
            let payload = &frames[0].payload;
            let key = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            // RESPONSE: [len][RETURN][key][12 zero bytes] (GET_POS result)
            let resp = ashfall_core::event::encode_frame(
                PIPE_OP_RETURN,
                &key.to_le_bytes().iter().chain([0u8; 12].iter()).copied().collect::<Vec<_>>(),
            );
            // EVENT: player-state frame
            let ev = ashfall_core::event::encode_player_state_event(
                &ashfall_core::event::PlayerStateEvent {
                    ref_id: 0x14, pos: [1.0, 2.0, 3.0], angle: [0.0; 3],
                    idle: 0, moving: 1, moving_xy: 0, weapon: 0,
                    alerted: false, sneaking: false, health: 100.0,
                },
            );
            let mut out = resp;
            out.extend_from_slice(&ev);
            stream.write_all(&out).unwrap();
        });
        port
    }

    #[tokio::test]
    async fn test_execute_with_interleaved_event() {
        let port = spawn_mock_bridge();
        let mut ipc = IpcClient::connect(IpcMode::Proton { port }).await.unwrap();

        let result = ipc.execute(commands::OP_GET_POS, &[Param::U32(0x14)]).await;
        // 12 zero bytes → Floats [0,0,0]
        assert!(matches!(&result, CommandResult::Floats(v) if v.len() == 3), "got {result:?}");

        // The interleaved event must be buffered, not lost.
        let events = ipc.poll_events();
        assert_eq!(events.len(), 1, "event buffered during execute");
        let (event_type, data) = ashfall_core::event::decode_event(&events[0].payload).unwrap();
        assert_eq!(event_type, ashfall_core::event::EVENT_PLAYER_STATE);
        let e = ashfall_core::event::decode_player_state(data).unwrap();
        assert_eq!(e.pos, [1.0, 2.0, 3.0]);
    }

    #[tokio::test]
    async fn test_stub_execute_fails_fast() {
        let mut ipc = IpcClient::connect(IpcMode::Stub).await.unwrap();
        let result = ipc.execute(commands::OP_GET_POS, &[Param::U32(0x14)]).await;
        assert!(matches!(result, CommandResult::Error(_)), "stub errors fast, no hang");
        assert!(ipc.poll_events().is_empty());
    }
}
