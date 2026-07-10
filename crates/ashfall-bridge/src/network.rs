//! TCP server inside the Wine/Proton process.
//!
//! Listens on loopback only (127.0.0.1:1771). Accepts a single connection
//! from the native Linux ashfall-client. Decodes pipe-protocol commands
//! and dispatches to the command handler.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::RUNNING;
use crate::commands;

/// Pipe protocol opcodes (match original vaultmp.hpp).
pub const PIPE_SYS_WAKEUP: u8    = 0x01;
pub const PIPE_OP_COMMAND: u8    = 0x02;
pub const PIPE_OP_RETURN: u8     = 0x03;
pub const PIPE_OP_RETURN_BIG: u8 = 0x04; // reserved for large responses
pub const PIPE_OP_RETURN_RAW: u8 = 0x05; // reserved for raw binary
pub const PIPE_ERROR_CLOSE: u8   = 0x06;

/// Encode a pipe command: [PIPE_OP_COMMAND][key:4B LE][func:4B LE][param_count:1B][params...]
pub fn encode_pipe_command(key: u32, func: u32, params: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 4 + 4 + 1 + params.len());
    buf.push(PIPE_OP_COMMAND);
    buf.extend_from_slice(&key.to_le_bytes());
    buf.extend_from_slice(&func.to_le_bytes());
    buf.push(params.len() as u8);
    buf.extend_from_slice(params);
    buf
}

/// Encode a pipe return: [PIPE_OP_RETURN][key:4B LE][result...]
pub fn encode_pipe_return(key: u32, result: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 4 + result.len());
    buf.push(PIPE_OP_RETURN);
    buf.extend_from_slice(&key.to_le_bytes());
    buf.extend_from_slice(result);
    buf
}

/// Decode pipe return: returns (opcode, key, result_bytes) or None if malformed.
pub fn decode_pipe_return(data: &[u8]) -> Option<(u8, u32, Vec<u8>)> {
    if data.len() < 5 { return None; }
    let opcode = data[0];
    let key = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
    let result = data[5..].to_vec();
    Some((opcode, key, result))
}

const PIPE_LENGTH: usize = 2048;

/// Run the TCP server loop. Blocks until shutdown signaled.
pub fn run_server(addr: &str) {
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            // ponytail: log to file; no console in DLL context
            let _ = e;
            return;
        }
    };

    // Accept one connection (single client)
    for stream in listener.incoming() {
        if !RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(_) => continue,
        }
    }
}

/// Handle a single client connection.
fn handle_client(mut stream: TcpStream) {
    let mut buf = [0u8; PIPE_LENGTH];

    while RUNNING.load(std::sync::atomic::Ordering::SeqCst) {
        match stream.read(&mut buf) {
            Ok(0) => break, // EOF, client disconnected
            Ok(n) => {
                let response = dispatch(&buf[..n]);
                if !response.is_empty() {
                    let _ = stream.write_all(&response);
                }
            }
            Err(_) => break,
        }
    }
}

/// Parse and dispatch a pipe-protocol command.
fn dispatch(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return vec![];
    }

    let opcode = data[0];
    let payload = &data[1..];

    match opcode {
        PIPE_SYS_WAKEUP => {
            // Keep-alive / heartbeat, respond with same
            vec![PIPE_SYS_WAKEUP]
        }
        PIPE_OP_COMMAND => {
            // [opcode:1B][key:4B][func:4B][param_count:1B][params...]
            if payload.len() < 9 {
                return vec![PIPE_ERROR_CLOSE];
            }
            let key = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
            let func = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            let _param_count = payload[8] as usize;
            let params = &payload[9..];

            let result = commands::execute(func, params);

            // Encode response: [PIPE_OP_RETURN][key:4B][result...]
            let mut response = Vec::with_capacity(1 + 4 + result.len());
            response.push(PIPE_OP_RETURN);
            response.extend_from_slice(&key.to_le_bytes());
            response.extend_from_slice(&result);
            response
        }
        PIPE_ERROR_CLOSE => {
            vec![PIPE_ERROR_CLOSE]
        }
        _ => {
            // Unknown opcode
            vec![PIPE_ERROR_CLOSE]
        }
    }
}
