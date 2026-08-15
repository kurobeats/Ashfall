//! Client UI module — server-authoritative GUI, server browser, chat.
//!
//! ponytail: superseded by the inline `AppState` implementation in main.rs
//! (stub-mode client). Kept as the documented GUI architecture for when the
//! game-engine IPC bridge lands.
#![allow(dead_code)]

pub mod app;
pub mod chat;
pub mod server_browser;
pub mod widgets;
pub mod world_view;
