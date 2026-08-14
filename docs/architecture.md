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
│   │       ├── string_cache.rs         # StringTable + CachedString (per-conn ids)
│   │       ├── crc32.rs                # IEEE CRC-32 for mod-file verification
│   │       ├── event.rs                # Pipe frame framing + bridge event types
│   │       │                           #   (player state, NPC spawn/remove/state)
│   │       └── protocol/              # packet definitions + serde
│   │           ├── mod.rs
│   │           ├── channel.rs          # Channel enum (System/Game/Chat)
│   │           ├── header.rs           # PacketHeader { id: u16, channel: u8 }
│   │           ├── transport.rs        # Wire framing: varint seqs, reliable
│   │           │                       #   flag bit, ACK/NACK control frames
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
│   │       ├── lib.rs                  # DllMain + NVSEPlugin_* exports
│   │       ├── plugin.rs               # PluginInfo + NVSEInterface + Query/Load
│   │       ├── network.rs              # TCP server on 127.0.0.1:1771, PIPE_OP_EVENT
│   │       ├── commands.rs             # Command dispatcher (36 opcodes)
│   │       ├── events.rs               # EventSink types + authoritative registry
│   │       │                           #   (hit, activate, equip, cell change,
│   │       │                           #   death, load game, magic effect)
│   │       ├── console.rs              # Console command interception framework
│   │       └── hooks/
│   │           ├── mod.rs              # Hook API + encode_event_frame shim
│   │           ├── memory.rs           # SafeWrite8/16/32/Buf, WriteRelJump/Call,
│   │           │                       #   write_rel_jump_padded, find_pattern,
│   │           │                       #   MemoryProtect RAII, Patch struct
│   │           ├── vtable.rs           # VTable entry lookup, raw field access,
│   │           │                       #   FormID resolution, concrete getters
│   │           │                       #   (pos, angle, actor state, cell, enabled,
│   │           │                       #   name, lock, parent cell, combat target)
│   │           ├── address.rs          # AutoPtr lazy address, prologue-signature
│   │           │                       #   build selection, thiscall call shims
│   │           ├── discovery.rs        # NPC seen-set diff (STR VisitForms) +
│   │           │                       #   actor collector + 10 Hz event flush
│   │           ├── detour.rs           # Trampoline-based function detour
│   │           ├── opcode.rs           # Direct-indexed GECK opcode interception
│   │           │                       #   (PlaceAtMe, AddItem, EquipItem, Kill, Lock)
│   │           ├── vaultmp.rs          # Classic FO3 multiplayer patch table
│   │           │                       #   (35 addrs) + 34 byte-exact detour
│   │           │                       #   recipes (respawn, AI pause, race match,
│   │           │                       #   fire/activate/PlaceAtMe interception)
│   │           │                       #   + actor-discovery detour + NPC flush
│   │           └── animation.rs        # Remote-actor PlayGroup state machine
│   │                                   #   (vaultmp net_SetActorState semantics)
│   │
│   ├── ashfall-bridge-proxy/           # dinput8.dll proxy — runs bridge init
│   │                                   #   in the game process, forwards DInput
│   │
│   ├── ashfall-server/                 # Dedicated server binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs                 # CLI + config load + startup + --import-esm
│   │       ├── config.rs               # ini-style config parsing
│   │       ├── dedicated.rs            # Main event loop, master announce
│   │       ├── network.rs              # UDP socket, reliability layer, rate limiter
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
│   │       │   ├── esm_import.rs       # Native TES4 parser: ESM/ESP → all tables
│   │       │   ├── record.rs           # Record: baseID → name/type/desc
│   │       │   ├── reference.rs        # Reference persistence
│   │       │   ├── exterior.rs         # Exterior cell data
│   │       │   ├── interior.rs         # Interior cell data
│   │       │   ├── weapon.rs           # Weapon records
│   │       │   ├── race.rs             # Race records
│   │       │   ├── npc.rs              # NPC records
│   │       │   ├── container.rs        # Base container records
│   │       │   ├── item.rs             # Base item records
│   │       │   ├── faction.rs          # Faction records
│   │       │   ├── quest.rs            # Quest stages + dialogue flags
│   │       │   ├── global.rs           # Global variables
│   │       │   └── terminal.rs         # Terminal records
│   │       ├── script/                 # wasmtime scripting bridge
│   │       │   ├── mod.rs
│   │       │   ├── engine.rs           # WASM engine init, module loading
│   │       │   ├── host.rs             # Host functions exposed to WASM (56 APIs)
│   │       │   ├── callbacks.rs        # 35 callback stubs (permissive defaults)
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
│   │       ├── network.rs              # UDP socket, varint framing, ACK queue
│   │       ├── dispatch.rs             # Packet handler dispatch
│   │       ├── sync.rs                 # Bridge ↔ server sync: events→packets,
│   │       │                           #   packets→engine commands (the coop loop)
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
│   │       │   ├── state.rs            # Render-behind InterpBuffer (67ms delay,
│   │       │   │                       #   velocity extrapolation, 500ms freeze)
│   │       │   └── view.rs             # Top-down world projection (pure math)
│   │       ├── ipc/                    # Bridge to game engine process
│   │       │   ├── mod.rs
│   │       │   ├── transport.rs         # TCP/Unix/Stub transport layer
│   │       │   ├── commands.rs         # Command encoding for game engine
│   │       │   │                       #   (incl. OP_PLAY_GROUP 0x0028)
│   │       └── ui/                     # egui-based GUI
│   │           ├── mod.rs
│   │           ├── app.rs              # egui app: server browser, connect, chat
│   │           ├── server_browser.rs   # Master server query, server list
│   │           ├── widgets.rs          # Window, Button, Text, Edit, etc — server-authored GUI
│   │           └── world_view.rs       # 2D canvas: interpolated objects as dots
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
├── data/                               # Runtime SQLite databases (gitignored)
├── docs/                               # Architecture, implementation plan, guides
└── .pi-subagents/                      # Agent artifacts
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

### 2.3 Game Bridge (bridge.dll + dinput8 proxy)

Cross-compiled Windows PE DLLs (MinGW-w64 target `i686-pc-windows-gnu` — FO3/FNV
are 32-bit). The game is injected two ways:

- **dinput8 proxy (recommended, verified under Proton)** — `ashfall-bridge-proxy`
  ships as `dinput8.dll` in the game dir; the game imports dinput8 so a native
  copy wins over wine's builtin. Its `DllMain` runs the bridge init, and
  `DirectInput8Create` forwards to wine's builtin dinput8.
- **FOSE/NVSE plugin** — `ashfall-bridge` exports the FOSEPlugin_/NVSEPlugin_
  ABI for xSE loaders.

Responsibilities:
- Hook Gamebryo engine functions (VTable patching, same technique as original vaultmpdll).
- Apply multiplayer behavior patches (`hooks::vaultmp`): respawn disable, AI pause,
  race matching, fire/activate/PlaceAtMe interception — byte-exact recipes from
  vaultmp, valid for the classic FO3 build (which the GOG exe matches).
- Drive remote-actor animations (`hooks::animation`): actor-state → PlayGroup opcodes.
- **Address library** (`hooks::address`, STR TiltedReverse port): `AutoPtr`
  (lazy resolve-once cached address), `select_candidate` (per-build address
  selection by prologue signature — GOG vs Steam), and `call_thiscall_0..3`
  (x86 thiscall at an explicit address, edx preserved). `fo3_lookup_addr`
  uses it to pick the right `LookupFormByID` per build.
- Expose TCP server on `127.0.0.1:1771` (loopback-only, no external exposure).
- Encode/decode pipe protocol (length-prefixed frames: `PIPE_OP_COMMAND`,
  `PIPE_OP_RETURN`, `PIPE_OP_EVENT`, etc. — see `ashfall_core::event`).
- Queue + push engine event frames to the native client (event queue drained
  in the connection loop; `report_player_state()` samples the local player).
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
| `ashfall-bridge.dll` | `i686-pc-windows-gnu` | 32-bit — FO3/FNV are i386 PEs |
| `ashfall_bridge_proxy.dll` | `i686-pc-windows-gnu` | ships as `dinput8.dll` |

Bridge built with: `cargo build --release --target i686-pc-windows-gnu -p ashfall-bridge -p ashfall-bridge-proxy`

### 2.6 Proton Setup

```bash
# 1. Copy the dinput8 proxy into the game directory (native copy wins over
#    wine's builtin dinput8; DllMain runs the bridge init)
cp target/i686-pc-windows-gnu/release/ashfall_bridge_proxy.dll \
   "$GAME_DIR/dinput8.dll"

# 2. Launch the game (Steam build: through Steam — SteamStub DRM; GOG build:
#    DRM-free, run directly — and it matches the classic address table)

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

The `Packet` enum in `crates/ashfall-core/src/protocol/mod.rs` is the single
source of truth (140+ variants, flat serde enum, postcard-encoded). This doc
lists the categories and the post-2026-08 additions; field-level detail lives
in the source.

Categories:
- **System**: `GameStart/Load/End/Auth/Mod/Message/Chat/Weather/Global/Base/Deleted`,
  quest + dialogue flags, karma, reputation, hardcore stats. `GameAuth` now
  carries the client **version** (mismatch → rejected); `GameModList` carries
  the client's full load order for the mod policy.
- **Object / Item / Container / Actor / Player**: create + field-level update
  packets. String fields (`ObjectNew.name`, `UpdateName`, `UpdateActorIdle.name`,
  `GameChat.message`, `GameMessage.message`, `UpdateInterior.cell`) are
  `CachedString` — the server interns them per-connection (see §8.3).
- **Combat / physics / projectiles**: `ActorHit` → server resolver →
  `ActorDamaged`/`ActorDeathExt`; `SpellCast` (owner-gated relay, STR
  `NotifySpellCast` shape); `UpdateVelocity` on the unreliable channel.
- **Ownership (STR OwnershipTransfer)**: `OwnershipClaim` (client → server),
  `OwnershipGranted` / `OwnershipReleased` (server → client). Who simulates
  which NPC; the owner's state updates relay, everyone else renders.
- **Actor differential state (STR Differential.h)**: `ActorStateDelta` — all
  fields optional; only changed fields present, receiver merges into its last
  known state.
- **World**: `GameTime` (authoritative clock, join-send + change broadcast),
  `ServerSettings` (pvp rule on join), `CellSnapshot`, window/GUI widgets.
- **Master server**: `MasterQuery/Announce/Update`.

### 3.2 Wire Format

Data frames (shared helpers in `ashfall_core::protocol::transport`):

```
| 2 bytes | 1 byte       | N bytes                     |
|---------|--------------|-----------------------------|
| length  | channel      | payload                     |
|         | reliable:    | 0x80 | Channel, [varint seq][postcard]
|         | unreliable:  | Channel,       [postcard]
|         | control:     | 0xFF,          [ACK/NACK frame]
```

Max packet size: 1200 bytes (safe UDP MTU). The reliable-flag bit on the
channel byte makes framing unambiguous even for single-byte postcard packets
(no payload-length heuristics). Control frames (cumulative ACK, NACK) carry
varint sequence numbers.

`postcard` encodes directly to/from `&[u8]`. The `Packet` enum is a flat serde
enum; postcard's varint encoding keeps it compact. No hand-rolled BitStream
needed.

### 3.3 Reliability Layer

```rust
// crates/ashfall-server/src/network.rs
// and crates/ashfall-client/src/network.rs

/// Reliable channel — ACK-based, ordered delivery, per-channel priority queues.
/// Send queues are indexed by `Channel as usize` (System=0, Game=1, Chat=2)
/// so retransmission drains System first, then Game, then Chat.
struct ReliableChannel {
    send_seq: u16,
    recv_seq: u16,
    send_queues: [VecDeque<SendEntry>; 3],   // priority-ordered outbound
    recv_buffer: BTreeMap<u16, Vec<u8>>,      // reassembly
    ready_queue: VecDeque<Vec<u8>>,           // in-order delivery
    srtt: Duration,                           // smoothed RTT (Jacobson/Karels)
    rttvar: Duration,
    rto: Duration,                            // srtt + 4*rttvar, clamped 100ms-3s
}

/// Unreliable channel: fire-and-forget for position updates (no seqs).
struct UnreliableChannel;
```

Wire framing (shared helpers in `ashfall_core::protocol::transport`):

```text
[2B length][channel: u8][payload]
  reliable:  channel = 0x80 | Channel, payload = [varint seq][postcard]
  unreliable: channel = Channel,        payload = [postcard]
  control:   channel = 0xFF,            payload = [ACK/NACK frame]
```

Three ordered reliable channels (System, Game, Chat) + one unordered unreliable
channel for position/animation updates — RakNet semantics without RakNet.

Reliability properties:
- **Cumulative ACK** with real RTT sampling (retransmit timer adapts per link).
- **NACK fast retransmit**: sequence gaps trigger immediate resend.
- **Exponential backoff** on retransmit (2^n × RTO), reset on ACK.
- **Send window** (MAX_INFLIGHT = 32) throttles unacknowledged in-flight packets.
- **Rate limiter**: per-address token bucket (200 pkt/s, burst 100) drops floods.
- **Varint sequence numbers**: 1 byte for seqs < 128, else 3.

Verified by real-UDP loopback tests (`tests/reliability.rs`,
`tests/loss_simulation.rs`): 50/50 packets delivered exactly once, in order,
under 25% randomized loss.

---

## 4. Server Architecture

### 4.1 Server State

```rust
// crates/ashfall-server/src/dedicated.rs (simplified)

pub struct DedicatedServer {
    config: ServerConfig,
    dispatcher: Dispatcher,          // registry, weather, globals, quests,
                                     // factions, pos_history, pvp_enabled,
                                     // expected_mods
    sessions: DashMap<SocketAddr, Session>,  // per-session string_table
    network: NetworkManager,         // UDP + reliability
    script_engine: ScriptEngine,     // wasmtime + shared GameTimeState
    master_announcer: MasterAnnouncer,
    // tick sync: last_weather, last_quest_stages, last_game_time, time_accum
}
```

Config keys (`server.ini`, ini or TOML): `host`, `port`, `connections`,
`announce`, `master_port`, `game_type` (fo3/fnv), `pvp_enabled` (enforced in
combat), `mod` (repeatable: `mod = "Fallout3.esm:C092218B"` — the expected
client load order, off when empty), `tick_rate`, `time_scale`, `scripts_path`,
`db_path`. `ashfall-server --list-mod-crc <dir>` prints ready-to-paste `mod =`
lines (IEEE CRC-32 of the raw file bytes, base master first).
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
// crates/ashfall-server/src/script/engine.rs (Phase 5 Part B — real execution)

pub struct ScriptEngine {
    engine: Engine,
    modules: Vec<(String, Module)>,
    instances: Vec<WasmInstance>,        // one (instance, store) per module
    timers: Option<Arc<Mutex<TimerManager>>>,
    effects: Option<ScriptEffects>,      // drainable chat/kick queue
    player_count: Option<Arc<AtomicU32>>, // maintained by the server tick
    game_time: Option<GameTimeState>,    // shared clock, notify on change
}

// Server events dispatched INTO wasm (engine methods):
//   dispatch_auth(name, pwd) -> bool      — any module vote 0 denies
//   dispatch_chat(player_id, msg) -> bool — blocks messages
//   dispatch_spawn_cell(player_id) -> u32 — script-chosen spawn cell
//   notify_spawn / notify_disconnect / notify_actor_death
//   notify_quest_stage / notify_game_time
//   dispatch_timer(id, callback_name)     — routed to exported fn by name
// Remaining callbacks (on_hit, on_equip, on_activate, GUI, ...) still fall
// back to permissive defaults in callbacks.rs.
```

**Host functions** (56) exposed to WASM — ALL real (2026-08-06, +4 on 08-07): `set_game_weather`,
`get_game_weather`, `set_game_time`, `get_quest_stage`, `set_quest_stage`,
`get_dialogue_flag`, `set_dialogue_flag`, `chat_message`, `ui_message`, `kick`,
`create_timer`, `kill_timer`, `get_current_players`, `get_max_players`,
`timestamp`, `host_log`, `debug_log`, `create_object`, `destroy_object`,
`get_pos_x/y/z`, `set_pos`, `create_actor`, `get_actor_value`,
`set_actor_value`, `kill_actor`, `create_item`, `add_item`, `remove_item`,
`equip_item`, `get_item_count`, `get_damage_resistance`, `get_damage_threshold`,
`set_server_name`, `get_config_int`, `set_time_scale`, and the GUI widget set
(`create_window`/`create_button`/`create_text`/`create_edit`/`create_checkbox`/
`create_radiobutton`/`create_list`/`add_list_item`/`remove_list_item`/
`destroy_window`/`set_window_*`) — widget calls emit real packets via the
`ScriptEffect::BroadcastPacket` effect queue. ABI: `u64` ids cross as `i64`,
strings as `(ptr, len)` into linear memory (see scripts/freeroam/src/lib.rs).

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

### 8.3 Bandwidth (done — STR Differential + StringCache)

`ponytail:` originally deferred until bandwidth was proven a problem; the
SkyrimTogetherReborn port made it cheap enough to ship now.

- **`ActorStateDelta`** (STR `Differential.h` pattern): actor state changes are
  batched into one packet with optional fields — only changed fields present,
  the receiver merges into its last-known state. One packet per state burst
  (entering combat flips weapon+alerted+sneaking together) instead of N.
- **StringCache** (`ashfall_core::string_cache`): names, cell names, and chat
  text repeat constantly (every cell entry re-sends the same object names).
  The server is the sole id assigner: each connection keeps a `StringTable`;
  the first sight of a string goes out as `Inline { id, value }`, repeats as a
  2-byte `Id`. The binding happens per-recipient in the server send path
  (`Packet::finalize_strings` in `dedicated.rs send()`), so object names in
  cell handoffs compress after first sight.

### 8.4 Interpolation

Client interpolates between last two known positions for remote objects. Linear lerp over tick interval. No extrapolation — if update missed, hold last position.

> Status (2026-08-06): interpolation is wired — `interpolated_pos()` blends
> between the last two received updates (100ms window) and the connected panel
> lists remote objects with interpolated positions.

```rust
fn interpolate_position(last: [f32; 3], current: [f32; 3], t: f32) -> [f32; 3] {
    [last[0] + (current[0] - last[0]) * t,
     last[1] + (current[1] - last[1]) * t,
     last[2] + (current[2] - last[2]) * t]
}
```

### 8.5 Ownership Transfer (STR OwnershipTransferEvent pattern)

Who simulates which NPC. The server is the authority: `ActorNew` grants the
sender simulation ownership (first reporter wins; duplicate `ref_id` reports
are rejected), tracked in a registry `owners` map (actor id → owning player
id). The owner may mutate the actor via client packets; everyone else renders
it. `OwnershipClaim` lets a client take over an *unowned* actor; on disconnect
the server releases every owned actor and broadcasts `OwnershipReleased` so
survivors can reclaim. NPCs register in their owner's cell, so `UpdateContext`
enter/leave streaming carries them between players.

### 8.6 Authoritative Game Clock + Server Rules

- **GameTime** (STR `CalendarService`): the server owns the clock — `GameTime`
  (year/month/day/hour) advances each tick at `time_scale × real time` (hour-
  granular with a fractional accumulator, 30-day-month rollover; scripts can
  override via `set_game_time`). Sent on join + broadcast on change; clients
  display it.
- **ServerSettings**: `pvp_enabled` from config is broadcast on join and
  **enforced** in the combat resolver (player-on-player hits rejected when
  off).

### 8.7 Bridge Event Pipeline (the coop loop)

`ashfall_core::event` defines a length-prefixed pipe frame
(`[len][opcode][payload]`) so command responses and engine events share the
bridge's TCP stream unambiguously. The bridge DLL queues event frames
(`EVENT_PLAYER_STATE`, `EVENT_NPC_SPAWN`) and flushes them in its connection
loop; `OP_REPORT_PLAYER_STATE` (0x00F7) samples the local player via the
vtable getters. The client buffers interleaved events during command
round-trips and `sync.rs` maps:

- events → packets: own-player state → `UpdatePos/UpdateAngle/ActorStateDelta`
  + health; NPC spawn → `ActorNew` + `OwnershipClaim` (ref-derived entity ids,
  high bit set, never colliding with server ids)
- packets → commands: remote `UpdatePos/UpdateAngle` → `OP_SET_POS/OP_SET_ANGLE`
  applied to the local game (remote entities addressed by their `ref_id`)

Wired into the client poll loop; stub IPC mode fails fast instead of hanging.
The remaining game-side triggers: the per-frame player-state hook (frame-
function RE) and live verification of the detours. The actor-discovery
detour covers both builds — GOG 0x6FAE90 and Steam 0x7F9B70 (re-derived
2026-08-13 from the flat dump via the `[reg+0xFC]` state-check fingerprint;
`ai_predicate_site()` picks by prologue signature). See
`docs/steam-re.md` for both.

---

## 9. Database Schema (rusqlite)

Direct port of the same schema, populated two ways:

1. **Startup** — `Database::startup_load()` loads records, weapons, NPCs,
   items, containers, quest stages, dialogue flags, and factions into memory.
2. **ESM import tool** — `ashfall-server --import-esm <file> --import-game fo3|fnv
   --import-db <path> [--import-index N]` parses plugin files with the native
   TES4 parser (`db/esm_import.rs`) and fills all 17 tables in one SQLite
   transaction. `--import-index` assigns the load-order byte for DLC esms
   (their placeholder formIDs all collide at 0x01 otherwise). Runs at
   tool-time, not startup. Both games fully imported + verified against the
   vaultmp dump corpus (`scripts/verify-esm-dumps.py`).

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

> The living phase-by-phase record is [docs/impl-plan.md](./impl-plan.md) —
> phases 1–10 done + post-phase-10 ingestion and STR-reuse work, 483 tests.
> The plan below is the original design sketch, kept for history.

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
2. Host functions (56) — world/quest/chat/clock/player-count/object-actor CRUD real (Part B)
3. Callback dispatch (auth, chat, spawn cell, spawn, death, quest stage, time)
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

### Phase 10: Proton Bridge ✅ DONE (post-MVP RE work ongoing)
1. Gamebryo VTable hooks (reverse engineering dependent)
2. Full command dispatcher (36 opcodes)
3. NVSE/FOSE plugin registration
4. Event sinks (OnHit, OnActivate, OnEquip, OnCellChange, OnDeath)
5. Console command hooks
6. Proton integration test
7. CI cross-compile workflow

**Implemented** (impl-plan.md Phase 10, 96 tests): 36 pipe opcodes, memory/
VTable/detour/opcode hooks, 11 default GECK opcode interceptors (15 verified
with two tools), real VTable getters, FOSE/NVSE ABI fixed, i686 cross-build +
wine round-trip. **Post-MVP RE (2026-08-07/08, live on the game host):** Steam
post-2023 build re-derived (LookupFormByID 0x711EF0, cdecl + thiscall
convention fixes), field reads live-verified under Proton. **2026-08-14:** the
FalloutAnniversaryPatcher vcdiff (downgrade delta) provided a verified
classic↔Steam byte map (63,616 runs) — 4 more vaultmp behavior-patch sites
re-derived EXACT (ai_fix1, get_activate_jmp, delegator stub spot,
play_group_fix) + av_fix/fire_weapon/get_activate_ret by static analysis;
Steam FNV verified = GOG at runtime (fnv_14 table applies unchanged). The 8
vaultmp hooks are implemented and the activate/fire/cell/enabled/move/
scale/lock/sound relays are complete (field-based, Steam-safe); get_scale/
set_scale + is_dead are field reads; get_actor_state's alerted/sneaking call
the classic engine getters (0x6F6C70/0x6F58B0, byte-guarded). Steam PC
vtable base 0xF938FC mapped (GET_LOCKED slot GOG +0xA0 → Steam +0xFC).
**gh crawl (2026-08-14):** Project Crossroads (active VaultMP-lineage
revival) ships the Anniversary-Patcher catalog — the full 31-patch vaultmp
site table + 8 engine entry points, byte-verified — independently
confirming our classic table is the exact vaultmp lineage; added SET_POS
0x6F2050 / QUEUE_UI_MESSAGE 0x61B850 to fo3_17. Remaining = fire-fix/
match_race/place_at_me/ai_fix2-4/play_idle_fix + the Steam AV/anim vtable
slots (GetActorValue/State/is_moving) live-probe (hooks::vaultmp recipes)
— see docs/steam-re.md.

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
2. **wasmtime host functions**: 56 API functions require careful FFI design. Mitigation: code-gen from a specification.
3. **ObjectRegistry contention**: DashMap reads are lock-free, but write contention around cell changes could be an issue. Mitigation: batch cell changes.
4. **IPC game engine bridge**: Depends on bridge.dll running inside Proton. Mitigation: stub mode for development — client runs standalone without game engine. TCP loopback tested and works in Proton 9+.
5. **Packet ordering**: postcard + UDP means no built-in ordering. Mitigation: reliability layer handles sequence numbers and reordering.
6. **Proton bridge injection**: Wine DLL override only loads DLLs something imports — nothing imports `bridge`, so `WINEDLLOVERRIDES="bridge=n,b"` fails. Mitigation (verified): dinput8 proxy — the game imports dinput8, a native copy in the game dir wins over wine's builtin; `DllMain` runs bridge init and `DirectInput8Create` forwards to the real one. VTable hooking works the same inside Wine as native Windows.
7. **Cross-compilation**: bridge/proxy built for `i686-pc-windows-gnu` (FO3/FNV are 32-bit) requires MinGW-w64 toolchain. Mitigation: CI provides prebuilt DLL; local dev uses stub mode.