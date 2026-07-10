//! Unified Packet enum — all game protocol messages.
//!
//! Each variant corresponds to a `PF_MAKE` packet in the original C++.
//! Extended for server-authoritative combat, physics, quests, and FNV support.

use crate::form_id::FormIDSync;
use crate::id::NetworkID;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod channel;
pub mod header;

pub use channel::Channel;
pub use header::PacketHeader;

/// Maximum safe payload size (postcard-encoded) to fit in a single UDP datagram.
pub const MAX_PACKET_SIZE: usize = 1200;

/// Maximum size for a cell snapshot to avoid splitting.
pub const MAX_CELL_SNAPSHOT_OBJECTS: usize = 256;

// ── limb indices (match Fallout body part data) ──
pub const LIMB_TORSO: u8 = 0;
pub const LIMB_HEAD: u8 = 1;
pub const LIMB_LEFT_ARM: u8 = 2;
pub const LIMB_RIGHT_ARM: u8 = 3;
pub const LIMB_LEFT_LEG: u8 = 4;
pub const LIMB_RIGHT_LEG: u8 = 5;

// ── death flags ──
pub const DEATH_FLAG_EXPLOSIVE: u8 = 0x01;
pub const DEATH_FLAG_ENERGY: u8 = 0x02;
pub const DEATH_FLAG_DISMEMBER: u8 = 0x04;
pub const DEATH_FLAG_HEADSHOT: u8 = 0x08;

// ── hit flags ──
pub const HIT_FLAG_CRITICAL: u8 = 0x01;
pub const HIT_FLAG_SNEAK: u8 = 0x02;
pub const HIT_FLAG_VATS: u8 = 0x04;

// ── AI package types ──
pub const AI_PACKAGE_NONE: u32 = 0;
pub const AI_PACKAGE_WANDER: u32 = 1;
pub const AI_PACKAGE_TRAVEL: u32 = 2;
pub const AI_PACKAGE_COMBAT: u32 = 3;
pub const AI_PACKAGE_GUARD: u32 = 4;
pub const AI_PACKAGE_SLEEP: u32 = 5;
pub const AI_PACKAGE_EAT: u32 = 6;
pub const AI_PACKAGE_FLEE: u32 = 7;
pub const AI_PACKAGE_USE_ITEM: u32 = 8;
pub const AI_PACKAGE_DIALOGUE: u32 = 9;

// ── cell snapshot flags ──
pub const CELL_FLAG_INITIALLY_DISABLED: u32 = 0x01;
pub const CELL_FLAG_DELETED: u32 = 0x02;
pub const CELL_FLAG_PERSISTENT: u32 = 0x04;
pub const CELL_FLAG_QUEST_ITEM: u32 = 0x08;

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
        scale: f32,
        cell: u32,
        enabled: bool,
        lock: u32,
        owner: u32,
    },
    VolatileNew { id: NetworkID, base_id: u32, pos: [f32; 3] },
    ObjectRemove { id: NetworkID, silent: bool },
    UpdatePos { id: NetworkID, pos: [f32; 3] },
    UpdateAngle { id: NetworkID, angle: [f32; 2] },
    UpdateScale { id: NetworkID, scale: f32 },
    UpdateCell { id: NetworkID, cell: u32, pos: [f32; 3] },
    UpdateName { id: NetworkID, name: String },
    UpdateLock { id: NetworkID, lock: u32 },
    UpdateOwner { id: NetworkID, owner: u32 },
    UpdateActivate { id: NetworkID, actor: NetworkID },
    UpdateSound { id: NetworkID, sound: u32 },

    // ===== Physics (unreliable, broadcast) =====
    /// Server → Clients: object velocity + grounded state.
    UpdateVelocity { id: NetworkID, vel: [f32; 3], on_ground: bool },

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
        scale: f32,
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
        scale: f32,
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

    // ===== Combat =====
    /// Bridge → Server: actor hit event. Server validates + calculates damage.
    ActorHit {
        target: NetworkID,
        attacker: NetworkID,
        limb: u8,
        /// Raw base damage before DR/DT (server applies DR/DT).
        base_damage: f32,
        flags: u8,
        weapon_id: u32,
        /// Projectile FormID if ranged.
        projectile: u32,
    },
    /// Server → Clients: final damage applied after DR/DT validation.
    ActorDamaged {
        target: NetworkID,
        attacker: NetworkID,
        limb: u8,
        final_damage: f32,
        flags: u8,
    },
    /// Server → Clients: extended death data.
    ActorDeathExt {
        id: NetworkID,
        killer: NetworkID,
        weapon_id: u32,
        limbs: u16,
        cause: i8,
        death_flags: u8,
    },
    /// Bridge → Server / Server → Clients: projectile spawned.
    ProjectileNew {
        id: NetworkID,
        base_id: u32,
        pos: [f32; 3],
        vel: [f32; 3],
        owner: NetworkID,
    },
    /// Server → Clients: projectile expired / impacted.
    ProjectileRemove { id: NetworkID, impact_pos: [f32; 3] },
    /// Bridge → Server / Server → Clients: explosion event.
    ExplosionNew {
        base_id: u32,
        pos: [f32; 3],
        radius: f32,
        owner: NetworkID,
    },

    // ===== NPC AI =====
    /// Server → Clients: NPC combat target changed.
    ActorCombatTarget { id: NetworkID, target: NetworkID },
    /// Server → Clients: NPC AI package changed.
    ActorAIPackage { id: NetworkID, package_id: u32, flags: u8 },
    /// Server → Clients: NPC faction data.
    ActorFaction { id: NetworkID, faction_id: u32, rank: i8 },

    // ===== Player =====
    PlayerNew {
        id: NetworkID,
        ref_id: u32,
        base_id: u32,
        controls: HashMap<u8, (u8, bool)>,
        scale: f32,
    },
    UpdateControl { id: NetworkID, control: u8, key: u8 },
    UpdateInterior { id: NetworkID, cell: String, spawn: bool },
    UpdateExterior { id: NetworkID, world: u32, x: i32, y: i32, spawn: bool },
    UpdateContext { id: NetworkID, cells: [u32; 9], spawn: bool },
    UpdateConsole { id: NetworkID, enabled: bool },

    // ===== World Objects =====
    /// Server → Clients: door open/close state.
    DoorState { id: NetworkID, open: bool, ref_id: u32 },
    /// Server → Clients: terminal locked/unlocked state.
    TerminalState { id: NetworkID, locked: bool, ref_id: u32 },

    // ===== Quest & Dialogue =====
    /// Server → Client: quest stage update.
    QuestStage { quest_id: u32, stage: u16 },
    /// Server ↔ Client: dialogue flag value.
    DialogueFlag { flag_id: u32, value: bool },
    /// Client → Server: player made a dialogue choice.
    DialogueChoice { flag_id: u32, choice: u32 },

    // ===== FO3 Globals =====
    /// Server → Client: karma change broadcast.
    KarmaUpdate { value: i32 },

    // ===== FNV Globals (optional — ignored by FO3) =====
    /// Server → Client: reputation with a faction.
    ReputationUpdate { faction: u32, value: i32 },
    /// Server → Client: hardcore stat updates.
    HardcoreStats { hunger: f32, thirst: f32, sleep: f32 },

    // ===== Cell Snapshot =====
    /// Server → Client on cell entry: full FormID-based dump.
    CellSnapshot { cell: u32, objects: Vec<FormIDSync> },

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
        /// Game type identifier: "fo3" or "fnv".
        game_type: String,
    },
    MasterUpdate {
        name: String,
        map: String,
        players: u32,
        max_players: u32,
    },
}
