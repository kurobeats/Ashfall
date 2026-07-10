//! Client configuration (vaultmp.ini style).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_master")]
    pub master: String,
    #[serde(default = "default_server")]
    pub server_addr: String,
    #[serde(default = "default_server_port")]
    pub server_port: u16,
    #[serde(default = "default_init_time")]
    pub init_time: u32,
    #[serde(default = "default_ipc_mode")]
    pub ipc_mode: String,
    #[serde(default = "default_ipc_port")]
    pub ipc_port: u16,
}

fn default_name() -> String { "Wanderer".into() }
fn default_master() -> String { "127.0.0.1:1660".into() }
fn default_server() -> String { "127.0.0.1".into() }
fn default_server_port() -> u16 { 1770 }
fn default_init_time() -> u32 { 9000 }
fn default_ipc_mode() -> String { "stub".into() }
fn default_ipc_port() -> u16 { 1771 }

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            name: default_name(),
            master: default_master(),
            server_addr: default_server(),
            server_port: default_server_port(),
            init_time: default_init_time(),
            ipc_mode: default_ipc_mode(),
            ipc_port: default_ipc_port(),
        }
    }
}
