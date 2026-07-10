# Ashfall — Rust Architecture

## 1. Crate Layout

```
ashfall/
├── Cargo.toml                          # workspace root
├── crates/
│   ├── ashfall-core/                   # shared types, protocol, constants
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # re-exports
│   │       ├── types.rs               # bitmask type hierarchy, ObjectKind enum
│   │       ├── constants.rs            # MAX_PLAYER_NAME, ports, channels, version
│   │       ├── math.rs                 # VaultVector, coords, IsValidCoordinate
│   │       ├── id.rs                   # NetworkID (newtype over u64)
│   │       └── protocol/              # packet definitions + serde
│   │           ├── mod.rs
│   │           ├── channel.rs          # Channel enum (System/Game/Chat)
│   │           ├── header.rs           # PacketHeader { id: u16, channel: u8 }
│   │           ├── system.rs           # Auth, Load, Start, End, Mod, Chat, etc.
│   │           ├── reference.rs        # Reference/Base create/update
│   │           ├── object.rs           # Object create/pos/angle/cell/name/lock
│   │           ├── item.rs             # Item create/count/condition/equip
│   │           ├── container.rs        # Container + ItemList
│   │           ├── actor.rs            # Actor values/race/anim/state/death
│   │           ├── player.rs           # Player controls/cell context/spawn
│   │           ├── window.rs           # Window + widget create/update
│   │           └── master.rs           # Master server announce/query/update
│   │
│   ├── ashfall-bridge/                 # Cross-compiled DLL for Proton/Wine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # DllMain entry point
│   │       ├── network.rs              # TCP server on 127.0.0.1:1771
│   │       ├── commands.rs             # Command dispatcher (opcodes)
│   │       └── hooks/mod.rs            # Gamebryo engine hook stubs
│   │
│   ├── ashfall-server/                 # Dedicated server binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                 # CLI + config load + startup
│   │       ├── config.rs               # ini-style config parsing
│   │       ├── dedicated.rs            # Main event loop, master announce
│   │       ├── network.rs              # UDP socket, session management
│   │       ├── session.rs              # Per-client Session { guid, addr, player_id, state }
│   │       ├── dispatch.rs             # Packet dispatch: match packet → handler
│   │       ├── handlers/               # Per-packet-type server logic
│   │       │   ├── mod.rs
│   │       │   ├── auth.rs             # Authenticate, reject
│   │       │   ├── game.rs             # Load/Start/End
│   │       │   ├── object.rs           # Create/update/remove objects
│   │       │   ├── actor.rs            # Actor state/value sync
│   │       │   ├── item.rs             # Item create/update/equip
│   │       │   ├── player.rs           # Player spawn, cell context, controls
│   │       │   ├── chat.rs             # Chat message handling
│   │       │   └── gui.rs              # Window event handling
│   │       ├── ai/                     # NPC AI system
│   │       │   ├── mod.rs
│   │       │   ├── packages.rs         # AI package state machine
│   │       │   └── factions.rs         # Faction hostility matrix
│   │       ├── combat/                 # Combat system
│   │       │   ├── mod.rs
│   │       │   └── resolver.rs         # Server-authoritative damage calculation
│   │       ├── quest/                  # Quest state manager
│   │       │   └── mod.rs              # Quest stage + dialogue flag storage
│   │       ├── physics/                # Physics validation
│   │       │   └── mod.rs              # Velocity/position/scale bounds checker
│   │       ├── world/                  # In-memory game state
│   │       │   ├── mod.rs
│   │       │   ├── registry.rs         # ObjectRegistry: NetworkID → Arc<dyn GameObject>
│   │       │   ├── objects.rs          # Object, Item, Container, Actor, Player structs
│   │       │   ├── inventory.rs         # ItemList logic (stack, equip, transfer)
│   │       │   ├── cell.rs             # Cell grid, cell context, visibility
│   │       │   ├── weather.rs          # Global weather state
│   │       │   └── globals.rs          # Global variables map
│   │       ├── db/                     # SQLite persistence
│   │       │   ├── mod.rs
│   │       │   ├── schema.rs           # CREATE TABLE statements
│   │       │   ├── record.rs           # Record: baseID → name/type/desc
│   │       │   ├── reference.rs        # Reference persistence
│   │       │   ├── exterior.rs         # Exterior cell data
│   │       │   ├── weapon.rs           # Weapon records
│   │       │   ├── race.rs             # Race records
│   │       │   ├── npc.rs              # NPC records
│   │       │   ├── container.rs        # Base container records
│   │       │   ├── item.rs             # Base item records
│   │       │   └── terminal.rs         # Terminal records
│   │       ├── script/                 # wasmtime scripting bridge
│   │       │   ├── mod.rs
│   │       │   ├── engine.rs           # WASM engine init, module loading
│   │       │   ├── host.rs             # Host functions exposed to WASM (51 APIs)
│   │       │   ├── callbacks.rs        # 35 callback stubs into WASM
│   │       │   └── timer.rs            # Script timer management
│   │       └── master.rs               # Master server registration + heartbeat
│   │       ├── anti_cheat.rs             # Position/item/damage/sequence validators
│   │
│   ├── ashfall-client/                 # Client binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                 # Platform entry, connect flow
│   │       ├── config.rs               # vaultmp.ini parsing
│   │       ├── game.rs                 # Client orchestrator (Game)
│   │       ├── network.rs              # UDP socket + reliability layer
│   │       ├── dispatch.rs             # Packet handler dispatch
│   │       ├── handlers/               # Client-side packet processing
│   │       │   ├── mod.rs
│   │       │   ├── game.rs             # Load, weather, global, deleted
│   │       │   ├── object.rs           # Create/update/remove objects
│   │       │   ├── item.rs             # Item state updates
│   │       │   ├── actor.rs            # Actor state/value updates
│   │       │   ├── player.rs           # Player spawn, context, controls
│   │       │   ├── chat.rs             # Incoming chat messages
│   │       │   └── gui.rs              # Window create/update handlers
│   │       ├── world/                  # Client-side object cache
│   │       │   ├── mod.rs
│   │       │   ├── registry.rs         # Light client-side object map
│   │       │   ├── cell.rs             # Cell context tracking
│   │       │   └── state.rs            # Derived render state
│   │       ├── ipc/                    # Bridge to game engine process
│   │       │   ├── mod.rs
│   │       │   ├── transport.rs         # TCP/Unix/Stub transport layer
│   │       │   ├── commands.rs         # Command encoding for game engine
│   │       └── ui/                     # egui-based GUI
│   │           ├── mod.rs
│   │           ├── app.rs              # egui app: server browser, connect, chat
│   │           ├── server_browser.rs   # Master server query, server list
│   │           └── widgets.rs          # Window, Button, Text, Edit, etc — server-authored GUI
│   │       └── chat.rs                  # Chat input/output overlay
│   │
│   ├── ashfall-master/                 # Master server binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── server_list.rs          # HashMap<addr, ServerEntry>
│   │       ├── announce.rs             # Handle ID_MASTER_ANNOUNCE
│   │       ├── query.rs                # Handle ID_MASTER_QUERY
│   │       └── cull.rs                 # Remove stale entries
│   │
│   └── ashfall-script/                 # Script SDK for WASM module authors
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs                  # bindgen helper macros, ID types
│
├── scripts/                            # Example WASM game mode scripts
│   └── freeroam/
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
│
├── data/                               # SQLite databases, config templates
│   └── fallout3.sqlite3
│
├── docs/
├── tests/
├── tools/
│   └── esm-reader/                     # ESM/Save reader (populate DB)
└── examples/
```

### Workspace Cargo.toml

```toml
[workspace]
members = [
    "crates/ashfall-core",
    "crates/ashfall-server",
    "crates/ashfall-client",
    "crates/ashfall-master",
    "crates/ashfall-script",
    "crates/ashfall-bridge",
]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
postcard = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
wasmtime = "22"
dashmap = "6"
slotmap = "1"
egui = "0.28"
eframe = "0.28"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1"
thiserror = "2"
bytes = "1"
uuid = { version = "1", features = ["v4"] }
parking_lot = "0.12"
```

---

## 2. Linux & Proton Compatibility

### 2.0 Process Layout

```
┌──────────────────────────────────────────────────┐
│ Linux Host                                       │
│                                                  │
│  ┌────────────┐   TCP loopback    ┌───────────┐ │
│  │ ashfall-    │◄═══════════════►│ Wine/     │ │
│  │ client      │  127.0.0.1:port  │  Proton   │ │
│  │ (native)    │                   │           │ │
│  └────────────┘                   │ ┌───────┐ │ │
│       │                           │ │bridge │ │ │
│       │ UDP                       │ │ .dll  │ │ │
│       ▼                           │ │(MingW)│ │ │
│  ┌────────────┐                   │ └──┬────┘ │ │
│  │ ashfall-    │                   │    │hook  │ │
│  │ server      │                   │ ┌──▼────┐ │ │
│  │ (native)    │                   │ │Fallout│ │ │
│  └────────────┘                   │ │3.exe  │ │ │
│                                   │ └───────┘ │ │
│  ┌────────────┐                   └───────────┘ │
│  │ ashfall-    │                                 │
│  │ master      │                                 │
│  │ (native)    │                                 │
│  └────────────┘                                 │
└──────────────────────────────────────────────────┘
```

### 2.1 Server & Master

Fully native Linux. No Wine/Proton needed. Uses `tokio::net::UdpSocket`, Unix signals for graceful shutdown, `rusqlite` (bundled SQLite). Systemd unit file provided.

### 2.2 Client

Native Linux binary (`ashfall-client`), egui GUI. Communicates with:
- **Server** via UDP (same as original).
- **Game engine** via TCP loopback → bridge DLL inside Proton.

### 2.3 Game Bridge (bridge.dll)

Cross-compiled Windows PE DLL (MinGW-w64 target `x86_64-pc-windows-gnu`). Injected into Fallout3.exe under Proton via Wine DLL override (`WINEDLLOVERRIDES="bridge=n,b"`). Responsibilities:
- Hook Gamebryo engine functions (VTable patching, same technique as original vaultmpdll).
- Expose TCP server on `127.0.0.1:1771` (loopback-only, no external exposure).
- Encode/decode pipe protocol (same opcodes: `PIPE_OP_COMMAND`, `PIPE_OP_RETURN`, etc.).
- Communicate with native `ashfall-client` over TCP.

### 2.4 IPC Transport

```rust
// Primary: TCP loopback (works everywhere including Proton/Wine)
// Fallback: Unix domain sockets (Linux native mode, no Proton)

pub enum IpcTransport {
    Tcp(TcpStream),        // 127.0.0.1:1771
    Unix(UnixStream),      // /tmp/ashfall-ipc.sock
}
```

TCP loopback is the default. Unix sockets only used when the bridge runs natively (future Linux-native game engine or standalone dev stub). Proton/Wine supports TCP loopback perfectly; Unix socket support in Wine is experimental.

### 2.5 Build Targets

| Binary | Target | Notes |
|--------|--------|-------|
| `ashfall-server` | `x86_64-unknown-linux-gnu` | Native, also `aarch64` |
| `ashfall-master` | `x86_64-unknown-linux-gnu` | Native |
| `ashfall-client` | `x86_64-unknown-linux-gnu` | Native |
| `bridge.dll` | `x86_64-pc-windows-gnu` | Cross-compiled via `cargo-xwin` or MinGW |

Bridge built with: `cargo build --target x86_64-pc-windows-gnu -p ashfall-bridge`

### 2.6 Proton Setup

```bash
# 1. Copy bridge.dll to game directory
cp target/x86_64-pc-windows-gnu/debug/bridge.dll \
   "$STEAM_LIBRARY/steamapps/common/Fallout 3/"

# 2. Launch with DLL override
WINEDLLOVERRIDES="bridge=n,b" \
  steam steam://rungameid/22370

# 3. Start Ashfall client (native)
cargo run -p ashfall-client
```

---

## 3. Type System & Object Hierarchy

### 2.1 Object Kind Bitmask

```rust
// crates/ashfall-core/src/types.rs

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Reference   = 0x0000_0001,
    Object      = 0x0000_0002,
    ItemList    = 0x0000_0004,
    Item        = 0x0000_0008,
    Container   = 0x0000_0010,
    Actor       = 0x0000_0020,
    Player      = 0x0000_0040,
    Window      = 0x0000_0080,
    Button      = 0x0000_0100,
    Text        = 0x0000_0200,
    Edit        = 0x0000_0400,
    Checkbox    = 0x0000_0800,
    RadioButton = 0x0000_1000,
    ListItem    = 0x0000_2000,
    List        = 0x0000_4000,
}

// Composite masks
pub const ALL_REFERENCES:  u32 = 0x0000_007F;
pub const ALL_OBJECTS:     u32 = 0x0000_007E;
pub const ALL_ITEMLISTS:   u32 = 0x0000_0074;
pub const ALL_CONTAINERS:  u32 = 0x0000_0070;
pub const ALL_ACTORS:      u32 = 0x0000_0060;
pub const ALL_WINDOWS:     u32 = 0x0000_7F80;
```

### 2.2 GameObject Trait

```rust
// crates/ashfall-core/src/types.rs

use crate::id::NetworkID;
use crate::protocol;
use std::any::Any;

/// Core trait for all game objects.
pub trait GameObject: Any + Send + Sync {
    fn id(&self) -> NetworkID;
    fn kind(&self) -> ObjectKind;
    fn kind_mask(&self) -> u32;       // bitmask for subtype checks
    fn is_kind(&self, kind: ObjectKind) -> bool {
        self.kind_mask() & (kind as u32) != 0
    }
    fn as_any(&self) -> &dyn Any;

    // Serialization
    fn to_packet(&self) -> protocol::ObjectPacket;
}

// Convenience downcast helpers
impl dyn GameObject {
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}
```

### 2.3 Concrete Types (server-side)

```rust
// crates/ashfall-server/src/world/objects.rs

use ashfall_core::types::*;
use ashfall_core::id::NetworkID;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ReferenceData {
    pub ref_id: u32,
    pub base_id: u32,
}

#[derive(Debug, Clone)]
pub struct ObjectData {
    pub name: String,
    pub pos: [f32; 3],
    pub angle: [f32; 3],
    pub cell: u32,
    pub enabled: bool,
    pub lock_level: u32,
    pub owner: u32,
}

#[derive(Debug, Clone)]
pub struct ItemData {
    pub container: NetworkID,
    pub count: u32,
    pub condition: f32,
    pub equipped: bool,
    pub silent: bool,
    pub stick: bool,
}

#[derive(Debug, Clone)]
pub struct ActorData {
    pub values: HashMap<u8, f32>,
    pub base_values: HashMap<u8, f32>,
    pub race: u32,
    pub age: i32,
    pub idle_anim: u32,
    pub moving_anim: u8,
    pub moving_xy: u8,
    pub weapon_anim: u8,
    pub female: bool,
    pub alerted: bool,
    pub sneaking: bool,
    pub dead: bool,
    pub death_limbs: u16,
    pub death_cause: i8,
}

#[derive(Debug, Clone)]
pub struct PlayerData {
    pub controls: HashMap<u8, (u8, bool)>,   // control_idx → (key, enabled)
    pub respawn_time: u32,
    pub spawn_cell: u32,
    pub cell_context: [u32; 9],
    pub console_enabled: bool,
    pub attached_windows: Vec<NetworkID>,
}
```

### 2.4 Object Registry (replaces GameFactory)

```rust
// crates/ashfall-server/src/world/registry.rs

use ashfall_core::id::NetworkID;
use ashfall_core::types::{ObjectKind, GameObject};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;

/// Central object registry. DashMap for concurrent read, Arc for shared ownership.
pub struct ObjectRegistry {
    objects: DashMap<NetworkID, Arc<RwLock<dyn GameObject>>>,
    type_counts: DashMap<ObjectKind, u32>,
    deleted: DashMap<NetworkID, ()>,           // tombstone set
    ref_to_id: DashMap<u32, NetworkID>,        // refID → NetworkID
    cell_refs: DashMap<u32, Vec<NetworkID>>,   // cell → objects in cell
}

impl ObjectRegistry {
    pub fn insert<T: GameObject + 'static>(&self, obj: T) -> NetworkID {
        let id = obj.id();
        let kind = obj.kind();
        self.objects.insert(id, Arc::new(RwLock::new(obj)));
        self.type_counts.entry(kind).and_modify(|c| *c += 1).or_insert(1);
        id
    }

    pub fn get<T: 'static>(&self, id: NetworkID) -> Option<Arc<RwLock<T>>> {
        self.objects.get(&id).and_then(|arc| {
            let guard = arc.value().read();
            let obj: &dyn GameObject = &*guard;
            // Safety: we trust kind checks; only downcast if kind matches
            obj.as_any().downcast_ref::<T>()?;
            // ponytail: return typed ref; caller uses read lock
            Some(Arc::new(unsafe {
                std::mem::transmute::<Arc<RwLock<dyn GameObject>>, Arc<RwLock<T>>>(arc.value().clone())
            }))
        })
    }

    pub fn remove(&self, id: NetworkID) -> bool {
        if let Some((_, arc)) = self.objects.remove(&id) {
            let guard = arc.read();
            let kind = guard.kind();
            self.type_counts.entry(kind).and_modify(|c| *c -= 1);
            self.deleted.insert(id, ());
            true
        } else {
            false
        }
    }

    pub fn is_deleted(&self, id: NetworkID) -> bool {
        self.deleted.contains_key(&id)
    }

    pub fn get_by_cell(&self, cell: u32) -> Vec<NetworkID> {
        self.cell_refs.get(&cell)
            .map(|r| r.value().clone())
            .unwrap_or_default()
    }

    pub fn get_by_kind(&self, kind: ObjectKind, mask: u32) -> Vec<NetworkID> {
        self.objects.iter()
            .filter(|entry| entry.value().read().kind_mask() & mask != 0)
            .map(|entry| *entry.key())
            .collect()
    }
}
```

---

## 3. Packet Protocol

### 3.1 Packet Enum

```rust
// crates/ashfall-core/src/protocol/mod.rs

use serde::{Serialize, Deserialize};
use crate::id::NetworkID;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Channel {
    System = 0,
    Game   = 1,
    Chat   = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Packet {
    // === System ===
    GameStart,
    GameLoad,
    GameEnd { reason: u8 },
    GameAuth { name: String, password: String },
    GameMod { filename: String, crc: u32 },
    GameMessage { message: String, emoticon: u8 },
    GameChat { message: String },
    GameWeather { weather: u32 },
    GameGlobal { global: u32, value: i32 },
    GameBase { player_base: u32 },
    GameDeleted { deleted: HashMap<u32, Vec<u32>> },

    // === Object ===
    ObjectNew {
        id: NetworkID,
        ref_id: u32,
        base_id: u32,
        name: String,
        pos: [f32; 3],
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

    // === Item ===
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

    // === Container ===
    ContainerNew { id: NetworkID, ref_id: u32, base_id: u32 },
    ItemListNew { id: NetworkID, items: Vec<NetworkID> },

    // === Actor ===
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

    // === Player ===
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

    // === Window / GUI ===
    WindowNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String },
    WindowRemove { id: NetworkID },
    ButtonNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String },
    TextNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String },
    EditNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String, max_len: u32, validation: String },
    CheckboxNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String, selected: bool },
    RadioButtonNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String, selected: bool, group: u32 },
    ListNew { id: NetworkID, parent: NetworkID, label: String, pos: [f32; 4], size: [f32; 4], locked: bool, visible: bool, text: String, multiselect: bool },
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

    // === Master server ===
    MasterQuery,
    MasterAnnounce { name: String, map: String, players: u32, max_players: u32, rules: HashMap<String, String>, mod_files: Vec<String> },
    MasterUpdate { name: String, map: String, players: u32, max_players: u32 },
}
```

### 3.2 Wire Format

```
| 2 bytes | 1 byte  | N bytes            |
|---------|---------|--------------------|
| length  | channel | postcard(Packet)   |
```

Max packet size: 1200 bytes (safe UDP MTU). Fragmented packets use a reliability layer.

`postcard` encodes directly to/from `&[u8]`. The `Packet` enum is a flat serde enum; postcard's varint encoding keeps it compact. No hand-rolled BitStream needed.

### 3.3 Reliability Layer

```rust
// crates/ashfall-server/src/network.rs
// and crates/ashfall-client/src/network.rs

/// Lightweight reliable-over-UDP layer per session.
/// Ordered per channel. ACK-based, with sequence numbers.
pub struct ReliableChannel {
    send_seq: u16,
    recv_seq: u16,
    send_buffer: VecDeque<(u16, Instant, Vec<u8>)>,   // (seq, sent_at, data)
    recv_buffer: BTreeMap<u16, Vec<u8>>,                // reassembly
    rtt: Duration,
    rto: Duration,
}

/// Unreliable channel: fire-and-forget for position updates.
pub struct UnreliableChannel {
    send_seq: u16,
}
```

Three ordered reliable channels (System, Game, Chat) + one unordered unreliable channel for position/animation updates. This matches RakNet's channel semantics without RakNet's complexity.

---

## 4. Server Architecture

### 4.1 Server State

```rust
// crates/ashfall-server/src/dedicated.rs

pub struct DedicatedServer {
    config: ServerConfig,
    registry: Arc<ObjectRegistry>,
    sessions: DashMap<NetworkID, Arc<Session>>,
    db: Database,
    script_engine: ScriptEngine,
    master_announcer: MasterAnnouncer,
    weather: RwLock<u32>,
    globals: DashMap<u32, i32>,
    game_time: RwLock<GameTime>,
    time_scale: RwLock<f32>,
}
```

### 4.2 Main Loop

```rust
// Each tick (~33ms, 30Hz):
// 1. Poll UDP socket for incoming packets (tokio::select! with tick interval)
// 2. For each packet: deserialize → dispatch to handler function
// 3. Dispatch queued outgoing packets from handler side effects
// 4. Tick script timers
// 5. Master server heartbeat (every 60s)
// 6. Cull stale sessions (inactive >30s)

async fn server_loop(server: Arc<DedicatedServer>) {
    let mut tick = tokio::time::interval(Duration::from_millis(33));
    let socket = UdpSocket::bind(server.config.addr).await.unwrap();
    let mut buf = vec![0u8; 65536];

    loop {
        tokio::select! {
            _ = tick.tick() => {
                server.script_engine.tick_timers();
                server.master_announcer.heartbeat();
                server.cull_sessions();
            }
            result = socket.recv_from(&mut buf) => {
                let (len, addr) = result.unwrap();
                let packet: Packet = postcard::from_bytes(&buf[..len]).unwrap();
                server.dispatch(addr, packet).await;
            }
        }
    }
}
```

### 4.3 Connection Lifecycle

```rust
// states:
//   Connecting → Authenticating → Loading → InGame → Disconnecting

async fn handle_auth(server: &DedicatedServer, addr: SocketAddr, auth: GameAuth) {
    // 1. Validate version (already checked at connect)
    // 2. Call script callback OnClientAuthenticate(name, password)
    let ok = server.script_engine.call_auth(&auth.name, &auth.password);
    if !ok {
        send(addr, GameEnd { reason: REASON_DENIED });
        return;
    }
    // 3. Create session
    let session = Session::new(addr, auth.name);
    server.sessions.insert(session.guid, Arc::new(session));
    // 4. Send GameLoad (empty → signals client to load game)
    send(addr, GameLoad);
    // 5. Send global state: weather, globals, game_time, player_base, deleted
    send(addr, GameWeather { weather: *server.weather.read() });
    for entry in server.globals.iter() {
        send(addr, GameGlobal { global: *entry.key(), value: *entry.value() });
    }
    // 6. Send all existing players to new client
    for (pid, ps) in server.sessions.iter() {
        // send PlayerNew for each
    }
    // 7. Create player object → registry → script callback OnPlayerRequestGame → spawn cell
    let cell = server.script_engine.call_request_game(player_id);
    let player = Player::new(player_id, base_id, cell, controls);
    server.registry.insert(player);
    // 8. Send actor/object state for player's cell context
    server.send_cell_context(player_id, cell);
    // 9. Client enters InGame state, steady sync begins
}
```

### 4.4 Packet Dispatch Pattern

```rust
// crates/ashfall-server/src/dispatch.rs

pub async fn dispatch(server: &DedicatedServer, addr: SocketAddr, packet: Packet) {
    let session = server.lookup_session(addr);

    match packet {
        Packet::GameAuth { .. } => handle_auth(server, addr, packet).await,
        Packet::UpdatePos { id, pos } => handle_pos(server, session, id, pos).await,
        Packet::UpdateAngle { id, angle } => handle_angle(server, session, id, angle).await,
        Packet::UpdateActorState { id, idle, moving, .. } => {
            // Server validates, updates authoritative state, broadcasts to cell
            handle_actor_state(server, session, id, packet).await;
        }
        Packet::GameChat { message } => {
            // Call OnPlayerChat callback, broadcast if allowed
            handle_chat(server, session, message).await;
        }
        // ... etc
        _ => {}
    }
}
```

### 4.5 Script Engine (wasmtime)

```rust
// crates/ashfall-server/src/script/engine.rs

use wasmtime::*;

pub struct ScriptEngine {
    engine: Engine,
    store: Store<ScriptState>,
    instances: Vec<Instance>,           // One per loaded script module
    host_functions: HostFunctions,
    timers: Vec<Timer>,
}

/// 35 script callbacks (31 original + OnHit, OnEquip, OnQuestStage, OnDialogueChoice)
impl ScriptEngine {
    pub fn call_on_create(&mut self, object_id: NetworkID) { /* invoke all instances */ }
    pub fn call_on_destroy(&mut self, object_id: NetworkID) { /* ... */ }
    pub fn call_on_spawn(&mut self, player_id: NetworkID) { /* ... */ }
    pub fn call_on_activate(&mut self, ref_id: NetworkID, actor_id: NetworkID) { /* ... */ }
    pub fn call_on_cell_change(&mut self, object_id: NetworkID, cell: u32) { /* ... */ }
    pub fn call_on_lock_change(&mut self, object_id: NetworkID, actor_id: NetworkID, lock: u32) { /* ... */ }
    pub fn call_on_item_count_change(&mut self, item_id: NetworkID, count: u32) { /* ... */ }
    pub fn call_on_item_condition_change(&mut self, item_id: NetworkID, condition: f32) { /* ... */ }
    pub fn call_on_item_equipped_change(&mut self, item_id: NetworkID, equipped: bool) { /* ... */ }
    pub fn call_on_actor_value_change(&mut self, actor_id: NetworkID, index: u8, value: f32) { /* ... */ }
    pub fn call_on_actor_base_value_change(&mut self, actor_id: NetworkID, index: u8, value: f32) { /* ... */ }
    pub fn call_on_actor_alert(&mut self, actor_id: NetworkID, alerted: bool) { /* ... */ }
    pub fn call_on_actor_sneak(&mut self, actor_id: NetworkID, sneaking: bool) { /* ... */ }
    pub fn call_on_actor_death(&mut self, actor_id: NetworkID, killer_id: NetworkID, limbs: u16, cause: i8) { /* ... */ }
    pub fn call_on_actor_punch(&mut self, actor_id: NetworkID, power: bool) { /* ... */ }
    pub fn call_on_actor_fire_weapon(&mut self, actor_id: NetworkID, weapon: u32) { /* ... */ }
    pub fn call_on_player_disconnect(&mut self, player_id: NetworkID, reason: u8) { /* ... */ }
    pub fn call_on_player_request_game(&mut self, player_id: NetworkID) -> u32 { /* ... */ }
    pub fn call_on_player_chat(&mut self, player_id: NetworkID, message: &str) -> bool { /* ... */ }
    pub fn call_on_window_mode(&mut self, player_id: NetworkID, enabled: bool) { /* ... */ }
    pub fn call_on_window_click(&mut self, player_id: NetworkID, window_id: NetworkID) { /* ... */ }
    pub fn call_on_window_return(&mut self, player_id: NetworkID, window_id: NetworkID) { /* ... */ }
    pub fn call_on_window_text_change(&mut self, player_id: NetworkID, window_id: NetworkID, text: &str) { /* ... */ }
    pub fn call_on_checkbox_select(&mut self, player_id: NetworkID, checkbox_id: NetworkID, selected: bool) { /* ... */ }
    pub fn call_on_radio_button_select(&mut self, player_id: NetworkID, radio_id: NetworkID, prev_id: NetworkID) { /* ... */ }
    pub fn call_on_list_item_select(&mut self, player_id: NetworkID, item_id: NetworkID, selected: bool) { /* ... */ }
    pub fn call_on_client_authenticate(&mut self, name: &str, pwd: &str) -> bool { /* ... */ }
    pub fn call_on_game_time_change(&mut self, year: u32, month: u32, day: u32, hour: u32) { /* ... */ }
    pub fn call_on_server_init(&mut self) { /* ... */ }
    pub fn call_on_server_exit(&mut self, shutdown: bool) { /* ... */ }
}
```

**Host functions** (51) exposed to WASM: `CreateObject`, `DestroyObject`, `GetPos`, `SetPos`, `GetCell`, `SetCell`, `AddItem`, `RemoveItem`, `EquipItem`, `CreateWindow`, `SetWindowText`, `ChatMessage`, `Kick`, `SetGameWeather`, `SetGameTime`, `CreateTimer`, `KillTimer`, etc.

WASM modules use `ashfall-script` SDK crate which provides typed wrappers around host imports.

---

## 5. Cell & Visibility Management

### 5.1 Cell Context (9-Cell Grid)

```rust
// crates/ashfall-server/src/world/cell.rs

/// 9-cell grid: center + 8 neighbors.
pub struct CellContext {
    cells: [u32; 9],       // index 4 = current cell
    last_change: Instant,
}

impl CellContext {
    pub fn new(center: u32, world: &CellWorld) -> Self {
        let neighbors = world.neighbors(center);
        let mut cells = [0u32; 9];
        cells[4] = center;
        for (i, n) in neighbors.iter().enumerate() {
            if i < 4 { cells[i] = *n; }
            else { cells[i + 1] = *n; }
        }
        CellContext { cells, last_change: Instant::now() }
    }

    pub fn is_in_context(&self, cell: u32) -> bool {
        self.cells.contains(&cell)
    }

    pub fn update_center(&mut self, center: u32, world: &CellWorld) {
        if self.cells[4] == center { return; }
        let neighbors = world.neighbors(center);
        self.cells[4] = center;
        for (i, n) in neighbors.iter().enumerate() {
            let idx = if i < 4 { i } else { i + 1 };
            self.cells[idx] = *n;
        }
        self.last_change = Instant::now();
    }
}
```

### 5.2 Visibility Update Flow

When a player's cell context changes:
1. Compute `enter_cells = new_context - old_context`
2. Compute `leave_cells = old_context - new_context`
3. For each enter cell: send `ObjectNew` for all objects in that cell
4. For each leave cell: send `ObjectRemove` for objects exclusive to that cell
5. Send `UpdateContext` to player

Server maintains `cell_refs: DashMap<u32, Vec<NetworkID>>` for O(1) cell→objects lookup.

### 5.3 Position Broadcast

```rust
// crates/ashfall-server/src/handlers/object.rs

async fn handle_pos(server: &DedicatedServer, session: &Session, id: NetworkID, pos: [f32; 3]) {
    // 1. Validate coordinates
    if !is_valid_pos(pos) { return; }
    // 2. Update authoritative state
    if let Some(obj) = server.registry.get::<ObjectData>(id) {
        obj.write().pos = pos;
    }
    // 3. Fanout to all players whose cell context contains this object's cell
    let cell = obj.read().cell;
    for (pid, ps) in server.sessions.iter() {
        let ctx = ps.cell_context.read();
        if ctx.is_in_context(cell) && ps.guid != session.guid {
            send_to(ps.addr, UpdatePos { id, pos });
        }
    }
}
```

---

## 6. Client Architecture

### 6.1 Client State

```rust
// crates/ashfall-client/src/game.rs

pub struct Game {
    config: ClientConfig,
    socket: UdpSocket,
    server_addr: SocketAddr,
    server_guid: NetworkID,
    state: GameState,           // Connecting, Authenticating, Loading, InGame, Disconnecting
    local_player: Option<NetworkID>,
    registry: ClientRegistry,   // Lightweight object cache
    cell_context: CellContext,
    ipc: IpcClient,             // Bridge to game engine process
    ui: GuiState,
}
```

### 6.2 Client Loop

```rust
async fn client_loop(mut game: Game) {
    let mut tick = tokio::time::interval(Duration::from_millis(33));
    let mut buf = vec![0u8; 65536];

    loop {
        tokio::select! {
            _ = tick.tick() => {
                // Poll game engine for position/angle/state
                game.poll_engine_state().await;
                // Send queued updates to server
                game.flush_outgoing().await;
            }
            result = game.socket.recv_from(&mut buf) => {
                let (len, _addr) = result.unwrap();
                let packet: Packet = postcard::from_bytes(&buf[..len]).unwrap();
                game.handle_packet(packet).await;
            }
        }
    }
}
```

### 7.3 IPC to Game Engine

```rust
// crates/ashfall-client/src/ipc/mod.rs

/// IPC transport abstraction — TCP for Proton, Unix for native.
pub enum IpcTransport {
    Tcp(TcpStream),        // 127.0.0.1:1771 (bridge.dll in Proton)
    Unix(UnixStream),      // /tmp/ashfall-ipc.sock (native engine stub)
    Stub,                  // Dev mode: returns canned responses
}

/// Client side of the game engine bridge.
pub struct IpcClient {
    transport: IpcTransport,
    pending: HashMap<u32, oneshot::Sender<CommandResult>>,
}

impl IpcClient {
    /// Connect to the game bridge. Tries TCP first, falls back to stub.
    pub async fn connect(mode: IpcMode) -> anyhow::Result<Self> {
        match mode {
            IpcMode::Proton { port } => {
                let stream = TcpStream::connect(("127.0.0.1", port)).await?;
                Ok(Self { transport: IpcTransport::Tcp(stream), pending: HashMap::new() })
            }
            IpcMode::Native { path } => {
                let stream = UnixStream::connect(path).await?;
                Ok(Self { transport: IpcTransport::Unix(stream), pending: HashMap::new() })
            }
            IpcMode::Stub => Ok(Self { transport: IpcTransport::Stub, pending: HashMap::new() }),
        }
    }

    /// Send a command to the game engine, await result.
    /// Wire format: [opcode:4B][key:4B][param_count:1B][params...]
    /// Response:     [key:4B][result_count:1B][results...]
    pub async fn execute(&mut self, opcode: u32, params: &[Param]) -> CommandResult {
        match &mut self.transport {
            IpcTransport::Tcp(stream) => send_over_tcp(stream, opcode, params).await,
            IpcTransport::Unix(stream) => send_over_unix(stream, opcode, params).await,
            IpcTransport::Stub => canned_response(opcode),
        }
    }

    pub async fn get_pos(&mut self, ref_id: u32) -> [f32; 3] { /* ... */ }
    pub async fn get_angle(&mut self, ref_id: u32) -> [f32; 3] { /* ... */ }
    pub async fn get_actor_state(&mut self, ref_id: u32) -> ActorState { /* ... */ }
}

pub enum IpcMode {
    Proton { port: u16 },     // 127.0.0.1:1771
    Native { path: PathBuf }, // /tmp/ashfall-ipc.sock
    Stub,                     // Dev/stub mode
}
```

### 6.4 Client-Side Object Cache

```rust
// crates/ashfall-client/src/world/registry.rs

/// Client-side: no Arc<RwLock>, just owned data.
/// Updated by server packets, read by render/UI.
pub struct ClientRegistry {
    objects: HashMap<NetworkID, ClientObject>,
    cell_objects: HashMap<u32, Vec<NetworkID>>,
}

pub enum ClientObject {
    Object {
        ref_id: u32, base_id: u32,
        name: String,
        pos: [f32; 3], net_pos: [f32; 3],
        angle: [f32; 3], cell: u32,
        enabled: bool, lock_level: u32, owner: u32,
    },
    Item {
        ref_id: u32, base_id: u32,
        container: NetworkID, count: u32,
        condition: f32, equipped: bool,
    },
    Actor {
        ref_id: u32, base_id: u32,
        values: HashMap<u8, f32>,
        race: u32, age: i32,
        idle_anim: u32, moving_anim: u8, weapon_anim: u8,
        alerted: bool, sneaking: bool, dead: bool,
    },
    Player {
        ref_id: u32, base_id: u32,
        controls: HashMap<u8, (u8, bool)>,
    },
}

impl ClientRegistry {
    pub fn apply_packet(&mut self, packet: &Packet) {
        match packet {
            Packet::ObjectNew { id, .. } => { self.objects.insert(*id, ClientObject::from(packet)); }
            Packet::UpdatePos { id, pos } => {
                if let Some(ClientObject::Object { pos: p, .. }) = self.objects.get_mut(id) {
                    *p = *pos;
                }
            }
            // ... etc
            _ => {}
        }
    }
}
```

---

## 7. GUI Architecture (egui)

Server-authoritative GUI: server creates windows/buttons via scripts, sends them as packets, client renders them.

```rust
// crates/ashfall-client/src/ui/widgets.rs

/// Server-authored GUI state
pub struct GuiState {
    windows: HashMap<NetworkID, GuiWindow>,
    mode: bool,     // window mode enabled
}

pub struct GuiWindow {
    id: NetworkID,
    parent: Option<NetworkID>,
    label: String,
    pos: [f32; 4],
    size: [f32; 4],
    locked: bool,
    visible: bool,
    text: String,
    kind: GuiWidgetKind,
}

pub enum GuiWidgetKind {
    Window,
    Button,
    Text,
    Edit { max_len: u32, validation: String },
    Checkbox { selected: bool },
    RadioButton { selected: bool, group: u32 },
    List { multiselect: bool, items: Vec<NetworkID> },
    ListItem { selected: bool, container: NetworkID },
}
```

Server browser: standalone egui window that queries master server and shows server list.

---

## 8. Sync Model

### 8.1 Server-Authoritative

Server owns truth. Client sends input (position, angles, controls), server validates and broadcasts. No client-side prediction for first version — acceptable for a mod of this nature (the original doesn't have it either).

### 8.2 Tick Rate

- Server tick: 30Hz (33ms)
- Client send rate: 30Hz (sync with server tick)
- Position/angle: unreliable channel (drop OK, next update covers it)
- Chat/events: reliable ordered channel
- Actor state changes: reliable ordered channel

### 8.3 Delta Compression (Future)

ponytail: skip initial; simple full state per update. Add delta compression when bandwidth problem proven.

### 8.4 Interpolation

Client interpolates between last two known positions for remote objects. Linear lerp over tick interval. No extrapolation — if update missed, hold last position.

```rust
fn interpolate_position(last: [f32; 3], current: [f32; 3], t: f32) -> [f32; 3] {
    [last[0] + (current[0] - last[0]) * t,
     last[1] + (current[1] - last[1]) * t,
     last[2] + (current[2] - last[2]) * t]
}
```

---

## 9. Database Schema (rusqlite)

Direct port of the same schema:

```sql
CREATE TABLE IF NOT EXISTS records (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    description TEXT,
    type INTEGER
);

CREATE TABLE IF NOT EXISTS references (
    refID INTEGER PRIMARY KEY,
    baseID INTEGER,
    cellID INTEGER,
    objectID INTEGER
);

CREATE TABLE IF NOT EXISTS exteriors (
    worldID INTEGER,
    x INTEGER,
    y INTEGER,
    PRIMARY KEY (worldID, x, y)
);

CREATE TABLE IF NOT EXISTS weapons (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    -- ... weapon-specific fields
);

CREATE TABLE IF NOT EXISTS races (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    -- ... race-specific fields
);

CREATE TABLE IF NOT EXISTS npcs (
    baseID INTEGER PRIMARY KEY,
    name TEXT,
    -- ... NPC-specific fields
);

CREATE TABLE IF NOT EXISTS base_containers (
    baseID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS base_items (
    baseID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS terminals (
    baseID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS interiors (
    cellID INTEGER PRIMARY KEY,
    name TEXT
);

CREATE TABLE IF NOT EXISTS ac_references (
    refID INTEGER PRIMARY KEY,
    baseID INTEGER,
    cellID INTEGER
);

-- Phase 4 expansion tables (quests, FNV, factions)

CREATE TABLE IF NOT EXISTS quest_stages (
    quest_id INTEGER,
    stage INTEGER,
    PRIMARY KEY (quest_id)
);

CREATE TABLE IF NOT EXISTS dialogue_flags (
    flag_id INTEGER PRIMARY KEY,
    value INTEGER
);

CREATE TABLE IF NOT EXISTS karma (
    value INTEGER
);

CREATE TABLE IF NOT EXISTS reputation (
    faction_id INTEGER,
    value INTEGER,
    PRIMARY KEY (faction_id)
);

CREATE TABLE IF NOT EXISTS hardcore_stats (
    hunger REAL,
    thirst REAL,
    sleep REAL
);

CREATE TABLE IF NOT EXISTS factions (
    faction_id INTEGER PRIMARY KEY,
    name TEXT,
    hostility INTEGER
);
```

Database layer uses typed query structs:

```rust
// crates/ashfall-server/src/db/mod.rs

pub struct Database {
    conn: rusqlite::Connection,
}

impl Database {
    pub fn open(path: &Path) -> anyhow::Result<Self> { /* ... */ }

    pub fn get_record(&self, base_id: u32) -> Option<Record> { /* ... */ }
    pub fn get_records_by_type(&self, kind: u32) -> Vec<Record> { /* ... */ }
    pub fn insert_record(&self, record: &Record) { /* ... */ }
    // ... per-table CRUD methods
}
```

---

## 10. Master Server

```rust
// crates/ashfall-master/src/main.rs

#[derive(Debug, Clone)]
pub struct ServerEntry {
    pub name: String,
    pub map: String,
    pub players: u32,
    pub max_players: u32,
    pub rules: HashMap<String, String>,
    pub mod_files: Vec<String>,
    pub addr: SocketAddr,
    pub last_seen: Instant,
}

pub struct MasterServer {
    servers: HashMap<SocketAddr, ServerEntry>,
    socket: UdpSocket,
}

// Loop:
// - On MasterAnnounce: insert/update entry
// - On MasterQuery: serialize all entries, send back
// - Every 60s: remove entries with last_seen > 120s
```

---

## 11. Implementation Phases

### Phase 1: Core Protocol
1. `ashfall-core` crate: types, constants, ID, math, Packet enum with serde
2. Wire format validation: round-trip test for every packet variant
3. `Cargo.toml` workspace setup

### Phase 2: Server Foundation
1. UDP socket + session management
2. Packet dispatch loop
3. Object registry (in-memory)
4. Object/Item/Container/Actor/Player structs
5. Connection flow: connect → auth → load → ingame

### Phase 3: World Sync
1. Cell system + cell context
2. Position/angle sync
3. Actor state sync
4. Item/inventory sync
5. Weather + globals

### Phase 4: Persistence
1. Database schema + rusqlite setup
2. Load records/npcs/weapons on startup
3. Persist reference data

### Phase 5: Scripting
1. wasmtime engine setup
2. Host function stubs (51)
3. Callback dispatch (35 callbacks)
4. Example freeroam script

### Phase 6: GUI (Server-Authoritative)
1. Window/button/text/edit/etc packet handlers
2. egui rendering of server-authored GUI
3. GUI event dispatch (click, text change) back to server

### Phase 7: Client
1. UDP socket + connection flow
2. Client-side object cache
3. IPC stub to game engine (Unix socket)
4. egui server browser
5. Chat UI

### Phase 8: Master Server
1. UDP announce/query handling
2. Server list culling
3. Client master query integration in server browser

### Phase 9: Security + Testing
1. Anti-cheat validators (position, velocity, item count, damage, sequence, FormID)
2. Movement tests (Vault 101, Megaton, Freeside, Strip)
3. Combat tests (raiders, mutants, NCR, Legion)
4. Quest tests (Wasteland Survival Guide, They Went That-A-Way, Ring-a-Ding-Ding)
5. Cell transition tests (metros, Strip gates)
6. Stress tests (10–20 players in Megaton/Freeside)

### Phase 10: Proton Bridge ⚠️ DEFERRED
1. Gamebryo VTable hooks (reverse engineering dependent)
2. Full command dispatcher (~120 opcodes)
3. NVSE/FOSE plugin registration
4. Event sinks (OnHit, OnActivate, OnEquip, OnCellChange, OnDeath)
5. Console command hooks
6. Proton integration test
7. CI cross-compile workflow

**Phase 10 deferred.** TCP server + command stubs + hook stubs exist. Real VTable patches require Gamebryo reverse engineering — post-MVP.

---

## 12. Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Async runtime | tokio | Industry standard, full-featured, UDP support via `UdpSocket` |
| Networking | Raw UDP + custom reliability | Simpler than QUIC, matches RakNet model, 3 ordered channels + 1 unordered |
| Serialization | postcard | Compact binary, serde-compatible, no_std, no schema needed |
| Scripting | wasmtime + WASM | Sandboxed, portable, supports many languages compiling to WASM |
| Object registry | `DashMap<NetworkID, Arc<RwLock<dyn GameObject>>>` | Concurrent reads, type-safe downcast, matches GameFactory semantics |
| Database | rusqlite | Direct port of SQLite3 schema, bundled mode |
| GUI | egui | Immediate mode, cross-platform, good for server browser + overlay |
| IPC | TCP loopback (primary), Unix domain sockets (fallback) | TCP works in Proton/Wine and natively; Unix sockets for Linux-native mode only |
| ECS | Not used | Domain is naturally hierarchical (Object→Item→Container→Actor→Player); ECS would fight the model |
| Cell grid | Hash-based 9-cell context | Same as original, O(1) lookups |

---

## 14. Risk Areas

1. **UDP reliability layer**: Custom ACK/reassembly is non-trivial. Mitigation: start simple, test with packet loss simulation.
2. **wasmtime host functions**: 51 API functions require careful FFI design. Mitigation: code-gen from a specification.
3. **ObjectRegistry contention**: DashMap reads are lock-free, but write contention around cell changes could be an issue. Mitigation: batch cell changes.
4. **IPC game engine bridge**: Depends on bridge.dll running inside Proton. Mitigation: stub mode for development — client runs standalone without game engine. TCP loopback tested and works in Proton 9+.
5. **Packet ordering**: postcard + UDP means no built-in ordering. Mitigation: reliability layer handles sequence numbers and reordering.
6. **Proton bridge.dll injection**: Wine DLL override mechanism differs from Windows `CreateRemoteThread` injection. Mitigation: use `WINEDLLOVERRIDES` env var for loading; VTable hooking works the same inside Wine as native Windows.
7. **Cross-compilation**: bridge.dll built for `x86_64-pc-windows-gnu` requires MinGW-w64 toolchain. Mitigation: CI provides prebuilt DLL; local dev uses stub mode.