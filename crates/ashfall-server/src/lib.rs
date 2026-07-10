//! Ashfall dedicated server — library root.
//!
//! All server modules live here. `main.rs` re-imports from this crate.

pub mod ai;
pub mod combat;
pub mod config;
pub mod db;
pub mod dedicated;
pub mod dispatch;
pub mod handlers;
pub mod network;
pub mod physics;
pub mod quest;
pub mod script;
pub mod session;
pub mod world;
