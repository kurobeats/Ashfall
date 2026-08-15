//! IPC transport layer — TCP, Unix sockets, or stub.

use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UnixStream};

/// How to connect to the game engine bridge.
pub enum IpcMode {
    /// Connect to bridge.dll inside Proton/Wine via TCP loopback.
    Proton { port: u16 },
    /// Connect to native Linux engine stub via Unix socket.
    Native { path: PathBuf },
    /// Development mode — returns canned responses without a real game engine.
    Stub,
}

impl Default for IpcMode {
    fn default() -> Self {
        IpcMode::Proton { port: 1771 }
    }
}

/// Active transport connection.
pub enum IpcTransport {
    Tcp(TcpStream),
    Unix(UnixStream),
    Stub,
}

/// Connect using the specified mode.
pub async fn connect(mode: IpcMode) -> anyhow::Result<IpcTransport> {
    match mode {
        IpcMode::Proton { port } => {
            let addr = format!("127.0.0.1:{port}");
            let stream = TcpStream::connect(&addr).await?;
            stream.set_nodelay(true)?; // low latency for game commands
            tracing::info!("IPC connected to bridge via TCP {addr}");
            Ok(IpcTransport::Tcp(stream))
        }
        IpcMode::Native { path } => {
            let stream = UnixStream::connect(&path).await?;
            tracing::info!("IPC connected to bridge via Unix socket {path:?}");
            Ok(IpcTransport::Unix(stream))
        }
        IpcMode::Stub => {
            tracing::info!("IPC stub mode — no game engine connected");
            Ok(IpcTransport::Stub)
        }
    }
}

impl IpcTransport {
    /// Whether this transport is the canned-response stub (no real game).
    pub fn is_stub(&self) -> bool {
        matches!(self, IpcTransport::Stub)
    }

    /// Non-blocking read of whatever is buffered right now (0 when nothing).
    pub fn try_read(&self, buf: &mut [u8]) -> usize {
        match self {
            IpcTransport::Tcp(stream) => stream.try_read(buf).unwrap_or(0),
            IpcTransport::Unix(stream) => stream.try_read(buf).unwrap_or(0),
            IpcTransport::Stub => 0,
        }
    }

    /// Send raw bytes.
    pub async fn send(&mut self, data: &[u8]) {
        match self {
            IpcTransport::Tcp(ref mut stream) => {
                let _ = stream.write_all(data).await;
            }
            IpcTransport::Unix(ref mut stream) => {
                let _ = stream.write_all(data).await;
            }
            IpcTransport::Stub => {
                // no-op
            }
        }
    }

    /// Receive raw bytes. Returns number of bytes read.
    pub async fn recv(&mut self, buf: &mut [u8]) -> usize {
        match self {
            IpcTransport::Tcp(ref mut stream) => stream.read(buf).await.unwrap_or(0),
            IpcTransport::Unix(ref mut stream) => stream.read(buf).await.unwrap_or(0),
            IpcTransport::Stub => {
                // Return a fake framed wakeup so the frame parser stays happy.
                if buf.len() >= 3 {
                    let frame = ashfall_core::event::encode_frame(super::PIPE_SYS_WAKEUP, &[]);
                    buf[..frame.len()].copy_from_slice(&frame);
                    frame.len()
                } else {
                    0
                }
            }
        }
    }
}
