//! Unified Packet enum — all game protocol messages.
//!
//! Each variant corresponds to a `PF_MAKE` packet in the original C++.

use crate::id::NetworkID;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod channel;
pub mod header;

pub use channel::Channel;
pub use header::PacketHeader;

/// Maximum safe payload size (postcard-encoded) to fit in a single UDP datagram.
pub const MAX_PACKET_SIZE: usize = 1200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Packet {
    // ===== System (channel 0) =====
    /// Client → Server: authenticate with name + password.
    GameAuth { name: String, password: String },
    /// Server → Client: game world state ready to load.
    GameLoad,
    /// Server → Client: start game loop.
    GameStart,
    /// Server ↔ Client: end session with reason.
    GameEnd { reason: u8 },
    /// Server ↔ Client: mod file verification (filename, CRC32).
    GameMod { filename: String, crc: u32 },
    /// Server ↔ Client: UI notification message.
    GameMessage { message: String, emoticon: u8 },
    /// Server ↔ Client: chat message broadcast.
    GameChat { message: String },
    /// Server → Client: weather change.
    GameWeather { weather: u32 },
    /// Server → Client: global variable update.
    GameGlobal { global: u32, value: i32 },
    /// Server → Client: player base race ID.
    GameBase { player_base: u32 },
    /// Server → Client: set of deleted static object refs.
    GameDeleted { deleted: HashMap<u32, Vec<u32>> },

    // ===== Reference =====
    ReferenceNew { id: NetworkID, ref_id: u32, base_id: u32 },

    // ===== Object (channel 1) =====
    ObjectNew {
        id: NetworkID,
        ref_id: u32,
        base_id: u32,
        name: String,
        game_pos: [f32; 3],
        net_pos: [f32; 3],
        angle: [f32; 3],
        cell: u32,
        enabled: bool,
        lock: u32,
        owner: u32,
    },
    VolatileNew { id: NetworkID, base_id: u32, pos: [f32; 3] },
    ObjectRemove { id: NetworkID, silent: bool },
    UpdatePos { id: NetworkID, pos: [f32; 3] },
    UpdateAngle { id: NetworkID, angle: [f32; 2] },
    UpdateCell { id: NetworkID, cell: u32, pos: [f32; 3] },
    UpdateName { id: NetworkID, name: String },
    UpdateLock { id: NetworkID, lock: u32 },
    UpdateOwner { id: NetworkID, owner: u32 },
    UpdateActivate { id: NetworkID, actor: NetworkID },
    UpdateSound { id: NetworkID, sound: u32 },

    // ===== Item =====
    ItemNew {
        id: NetworkID,
        ref_id: u32,
        base_id: u32,
        container: NetworkID,
        count: u32,
        condition: f32,
        equipped: bool,
        silent: bool,
        stick: bool,
    },
    UpdateItemCount { id: NetworkID, count: u32, silent: bool },
    UpdateItemCondition { id: NetworkID, condition: f32, health: u32 },
    UpdateItemEquipped { id: NetworkID, equipped: bool, silent: bool, stick: bool },

    // ===== Container =====
    ContainerNew { id: NetworkID, ref_id: u32, base_id: u32 },
    ItemListNew { id: NetworkID, items: Vec<NetworkID> },

    // ===== Actor =====
    ActorNew {
        id: NetworkID,
        ref_id: u32,
        base_id: u32,
        values: HashMap<u8, f32>,
        base_values: HashMap<u8, f32>,
        race: u32,
        age: i32,
        idle: u32,
        moving: u8,
        moving_xy: u8,
        weapon: u8,
        female: bool,
        alerted: bool,
        sneaking: bool,
        dead: bool,
        death_limbs: u16,
        death_cause: i8,
    },
    UpdateActorState {
        id: NetworkID,
        idle: u32,
        moving: u8,
        moving_xy: u8,
        weapon: u8,
        alerted: bool,
        sneaking: bool,
        firing: bool,
    },
    UpdateActorRace { id: NetworkID, race: u32, age: i32, delta_age: i32 },
    UpdateActorSex { id: NetworkID, female: bool },
    UpdateActorDead { id: NetworkID, dead: bool, limbs: u16, cause: i8 },
    UpdateActorValue { id: NetworkID, base: bool, index: u8, value: f32 },
    UpdateFireWeapon { id: NetworkID, weapon: u32 },
    UpdateActorIdle { id: NetworkID, idle: u32, name: String },

    // ===== Player =====
    PlayerNew {
        id: NetworkID,
        ref_id: u32,
        base_id: u32,
        controls: HashMap<u8, (u8, bool)>,
    },
    UpdateControl { id: NetworkID, control: u8, key: u8 },
    UpdateInterior { id: NetworkID, cell: String, spawn: bool },
    UpdateExterior { id: NetworkID, world: u32, x: i32, y: i32, spawn: bool },
    UpdateContext { id: NetworkID, cells: [u32; 9], spawn: bool },
    UpdateConsole { id: NetworkID, enabled: bool },

    // ===== Window / GUI =====
    WindowNew {
        id: NetworkID,
        parent: NetworkID,
        label: String,
        pos: [f32; 4],
        size: [f32; 4],
        locked: bool,
        visible: bool,
        text: String,
    },
    WindowRemove { id: NetworkID },
    EditNew {
        id: NetworkID,
        parent: NetworkID,
        label: String,
        pos: [f32; 4],
        size: [f32; 4],
        locked: bool,
        visible: bool,
        text: String,
        max_len: u32,
        validation: String,
    },
    CheckboxNew {
        id: NetworkID,
        parent: NetworkID,
        label: String,
        pos: [f32; 4],
        size: [f32; 4],
        locked: bool,
        visible: bool,
        text: String,
        selected: bool,
    },
    RadioButtonNew {
        id: NetworkID,
        parent: NetworkID,
        label: String,
        pos: [f32; 4],
        size: [f32; 4],
        locked: bool,
        visible: bool,
        text: String,
        selected: bool,
        group: u32,
    },
    ListNew {
        id: NetworkID,
        parent: NetworkID,
        label: String,
        pos: [f32; 4],
        size: [f32; 4],
        locked: bool,
        visible: bool,
        text: String,
        multiselect: bool,
    },
    ListItemNew { id: NetworkID, container: NetworkID, text: String, selected: bool },
    ListItemRemove { id: NetworkID },
    UpdateWindowPos { id: NetworkID, pos: [f32; 4] },
    UpdateWindowSize { id: NetworkID, size: [f32; 4] },
    UpdateWindowVisible { id: NetworkID, visible: bool },
    UpdateWindowLocked { id: NetworkID, locked: bool },
    UpdateWindowText { id: NetworkID, text: String },
    UpdateEditMaxLen { id: NetworkID, max_len: u32 },
    UpdateEditValidation { id: NetworkID, validation: String },
    UpdateCheckboxSelected { id: NetworkID, selected: bool },
    UpdateRadioButtonSelected { id: NetworkID, previous: NetworkID, selected: bool },
    UpdateRadioButtonGroup { id: NetworkID, group: u32 },
    UpdateListMultiSelect { id: NetworkID, multiselect: bool },
    UpdateListItemSelected { id: NetworkID, selected: bool },
    UpdateListItemText { id: NetworkID, text: String },
    UpdateWindowMode { enabled: bool },
    UpdateWindowClick { id: NetworkID },
    UpdateWindowReturn { id: NetworkID },

    // ===== Master server =====
    MasterQuery,
    MasterAnnounce {
        name: String,
        map: String,
        players: u32,
        max_players: u32,
        rules: HashMap<String, String>,
        mod_files: Vec<String>,
    },
    MasterUpdate {
        name: String,
        map: String,
        players: u32,
        max_players: u32,
    },
}
